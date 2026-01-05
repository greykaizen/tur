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
    num_connections: number;
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
    num_connections: number;
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
                    num_connections: dl.num_connections,
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

        // Progress update event - backend sends pre-smoothed speed
        listen<ProgressEvent>('download_progress', (event) => {
            const p = event.payload;
            setDownloads(prev => {
                const next = new Map(prev);
                const dl = next.get(p.id);
                if (dl) {
                    // Speed is already EWMA-smoothed by backend
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

    // Load history from database (for History page and app restart)
    const loadHistory = useCallback(async () => {
        try {
            setError(null);
            const history = await invoke<Array<{
                id: string;
                url: string;
                filename: string;
                size: number | null;
                bytes_received: number;
                destination: string;
                accept_ranges: boolean;
                status: string | null;
            }>>('get_download_history');

            // Add history items to downloads map
            setDownloads(prev => {
                const next = new Map(prev);
                for (const item of history) {
                    // Don't overwrite active downloads
                    if (!next.has(item.id)) {
                        next.set(item.id, {
                            id: item.id,
                            url: item.url,
                            filename: item.filename,
                            size: item.size,
                            downloaded: item.bytes_received,
                            speed: 0,
                            progress: item.size ? (item.bytes_received / item.size) * 100 : 0,
                            status: (item.status as DownloadInfo['status']) || 'queued',
                            destination: item.destination,
                            resume_supported: item.accept_ranges,
                            num_connections: item.accept_ranges ? 8 : 1, // Default, actual value not stored in history
                        });
                    }
                }
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
        loadHistory,
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

// ETA throttling state - prevents jittery time-left display
const etaState = new Map<string, { lastEta: number; lastUpdate: number }>();
const ETA_UPDATE_INTERVAL_MS = 1000; // Update ETA at most once per second
const ETA_MAX_INCREASE_FACTOR = 2.0; // Cap upward jumps to 2x previous ETA

// Format time remaining - IDM style with throttling
// id parameter allows per-download ETA throttling
export function formatTimeLeft(downloaded: number, total: number, speed: number, id?: string): string {
    if (speed === 0 || total === 0) return '--';

    const remaining = total - downloaded;
    let totalSeconds = Math.ceil(remaining / speed);

    // Apply throttling if we have an ID
    if (id) {
        const now = Date.now();
        let state = etaState.get(id);

        // If completed, clear state
        if (downloaded >= total) {
            etaState.delete(id);
        } else {
            if (!state) {
                // First time
                state = { lastEta: totalSeconds, lastUpdate: now };
                etaState.set(id, state);
            } else if (now - state.lastUpdate >= ETA_UPDATE_INTERVAL_MS) {
                // Enough time passed for update
                let newEta = totalSeconds;
                // Cap upward jumps
                if (state.lastEta > 0 && totalSeconds > state.lastEta * ETA_MAX_INCREASE_FACTOR) {
                    newEta = Math.ceil(state.lastEta * ETA_MAX_INCREASE_FACTOR);
                }
                state.lastEta = newEta;
                state.lastUpdate = now;
                etaState.set(id, state);
                totalSeconds = newEta;
            } else {
                // Use cached ETA
                totalSeconds = state.lastEta;
            }
        }
    }

    if (totalSeconds < 0) return '--';

    const hours = Math.floor(totalSeconds / 3600);
    const mins = Math.floor((totalSeconds % 3600) / 60);
    const secs = totalSeconds % 60;

    if (hours > 0) {
        // "1 hr 5 min" or "2 hrs 30 min"
        return `${hours} hr${hours > 1 ? 's' : ''} ${mins} min`;
    } else if (mins > 0) {
        // "5 min 35 sec" or "12 min 0 sec"
        return `${mins} min ${secs} sec`;
    } else {
        // "45 sec"
        return `${secs} sec`;
    }
}

export function clearEtaState(id: string) {
    etaState.delete(id);
}
