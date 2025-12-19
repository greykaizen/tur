/**
 * useDownloads hook - manages downloads via Tauri backend
 */
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// Download state from backend
export interface DownloadInfo {
    id: string;
    url: string;
    filename: string;
    size: number | null;
    downloaded: number;
    speed: number;
    progress: number;
    status: 'queued' | 'downloading' | 'paused' | 'completed' | 'failed';
    destination: string;
    resume_supported: boolean;
    segments?: { start: number; end: number }[];
    error?: string;
}

// Progress update from backend event
interface ProgressEvent {
    id: string;
    downloaded: number;
    total: number;
    speed: number;
    progress: number;
}

// Queue event from backend
interface QueueEvent {
    id: string;
    url: string;
    filename: string;
    size: number | null;
    destination: string;
    resume_supported: boolean;
    status: string;
}

export function useDownloads() {
    const [downloads, setDownloads] = useState<Map<string, DownloadInfo>>(new Map());
    const [error, setError] = useState<string | null>(null);

    // Listen to backend events
    useEffect(() => {
        const unlistenFns: UnlistenFn[] = [];

        // Queue new download event
        listen<QueueEvent>('queue_download', (event) => {
            const dl = event.payload;
            setDownloads(prev => {
                const next = new Map(prev);
                next.set(dl.id, {
                    id: dl.id,
                    url: dl.url,
                    filename: dl.filename,
                    size: dl.size,
                    downloaded: 0,
                    speed: 0,
                    progress: 0,
                    status: 'queued',
                    destination: dl.destination,
                    resume_supported: dl.resume_supported,
                });
                return next;
            });
        }).then(fn => unlistenFns.push(fn));

        // Download started event
        listen<{ id: string }>('download_started', (event) => {
            setDownloads(prev => {
                const next = new Map(prev);
                const dl = next.get(event.payload.id);
                if (dl) {
                    next.set(dl.id, { ...dl, status: 'downloading' });
                }
                return next;
            });
        }).then(fn => unlistenFns.push(fn));

        // Progress update event
        listen<ProgressEvent>('download_progress', (event) => {
            const p = event.payload;
            setDownloads(prev => {
                const next = new Map(prev);
                const dl = next.get(p.id);
                if (dl) {
                    next.set(dl.id, {
                        ...dl,
                        downloaded: p.downloaded,
                        speed: p.speed,
                        progress: p.progress,
                        status: 'downloading',
                    });
                }
                return next;
            });
        }).then(fn => unlistenFns.push(fn));

        // Download complete event
        listen<{ id: string }>('download_complete', (event) => {
            setDownloads(prev => {
                const next = new Map(prev);
                const dl = next.get(event.payload.id);
                if (dl) {
                    next.set(dl.id, { ...dl, status: 'completed', progress: 100 });
                }
                return next;
            });
        }).then(fn => unlistenFns.push(fn));

        // Download failed event
        listen<{ id: string; error: string }>('download_failed', (event) => {
            setDownloads(prev => {
                const next = new Map(prev);
                const dl = next.get(event.payload.id);
                if (dl) {
                    next.set(dl.id, { ...dl, status: 'failed', error: event.payload.error });
                }
                return next;
            });
        }).then(fn => unlistenFns.push(fn));

        return () => {
            unlistenFns.forEach(fn => fn());
        };
    }, []);

    // Start new downloads
    const startDownloads = useCallback(async (urls: string[]) => {
        try {
            setError(null);
            await invoke('handle_download_request', {
                request: {
                    type: 'New',
                    data: urls,
                },
            });
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        }
    }, []);

    // Resume downloads
    const resumeDownloads = useCallback(async (ids: string[]) => {
        try {
            setError(null);
            await invoke('handle_download_request', {
                request: {
                    type: 'Resume',
                    data: ids,
                },
            });
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        }
    }, []);

    // Pause a download
    const pauseDownload = useCallback(async (id: string) => {
        try {
            await invoke('pause_download', { id });
            setDownloads(prev => {
                const next = new Map(prev);
                const dl = next.get(id);
                if (dl) {
                    next.set(id, { ...dl, status: 'paused' });
                }
                return next;
            });
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        }
    }, []);

    // Cancel a download
    const cancelDownload = useCallback(async (id: string) => {
        try {
            await invoke('cancel_download', { id });
            setDownloads(prev => {
                const next = new Map(prev);
                next.delete(id);
                return next;
            });
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        }
    }, []);

    // Get downloads as array
    const downloadList = Array.from(downloads.values());

    // Get active (non-completed) downloads
    const activeDownloads = downloadList.filter(d => d.status !== 'completed' && d.status !== 'failed');

    return {
        downloads: downloadList,
        activeDownloads,
        error,
        startDownloads,
        resumeDownloads,
        pauseDownload,
        cancelDownload,
    };
}

// Format bytes to human readable
export function formatSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
}

// Format speed to human readable
export function formatSpeed(bytesPerSec: number): string {
    return `${formatSize(bytesPerSec)}/s`;
}

// Format time remaining
export function formatTimeLeft(downloaded: number, total: number, speed: number): string {
    if (speed === 0 || total === 0) return '--:--';
    const remaining = total - downloaded;
    const seconds = Math.ceil(remaining / speed);
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    if (mins > 60) {
        const hours = Math.floor(mins / 60);
        return `${hours}h ${mins % 60}m`;
    }
    return `${mins}:${secs.toString().padStart(2, '0')}`;
}
