import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MoreHorizontal, Copy } from 'lucide-react';

const CATEGORIES = ['Videos', 'Music', 'Documents', 'Software', 'Compressed', 'Other'];

// Format bytes to human readable size
function formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

export default function DownloadPage() {
    const params = new URLSearchParams(window.location.search);
    const url = params.get('url') || '';
    const formatId = params.get('format') || '';
    const audioId = params.get('audio') || '';
    const titleParam = params.get('title') || '';
    const filesizeParam = params.get('filesize') || '';
    const extParam = params.get('ext') || '';
    const videoStreamUrl = params.get('videoStreamUrl') || '';
    const audioStreamUrl = params.get('audioStreamUrl') || '';

    // Debug: log received params
    console.log('[Download] Received params:', { url, formatId, audioId, titleParam, filesizeParam, extParam, videoStreamUrl, audioStreamUrl });

    const [savePath, setSavePath] = useState('');
    const [filename, setFilename] = useState('download');
    const [filesize, setFilesize] = useState<number | null>(null);
    const [fileExt, setFileExt] = useState('');
    const [category, setCategory] = useState('Videos');
    const [rememberPath, setRememberPath] = useState(false);
    const [categoryPath, setCategoryPath] = useState('');
    const [description, setDescription] = useState('');
    const [isLoading, setIsLoading] = useState(false);


    useEffect(() => {
        // Use title from params if available, otherwise parse from URL
        if (titleParam) {
            const name = extParam ? `${titleParam}.${extParam}` : titleParam;
            setFilename(name);
        } else {
            try {
                const parsed = new URL(url);
                const parts = parsed.pathname.split('/');
                const fname = decodeURIComponent(parts[parts.length - 1] || 'download');
                setFilename(fname);
            } catch {
                setFilename('download');
            }
        }

        // Set filesize if provided
        if (filesizeParam) {
            const size = parseInt(filesizeParam, 10);
            if (!isNaN(size)) setFilesize(size);
        }

        // Set extension
        if (extParam) setFileExt(extParam);

        invoke<string>('get_default_download_path')
            .then(path => { setSavePath(path); setCategoryPath(path); })
            .catch(() => setSavePath('~/Downloads'));
    }, [url, titleParam, filesizeParam, extParam]);



    const handleBrowse = async () => {
        const selected = await open({ directory: true, defaultPath: savePath });
        if (selected) setSavePath(selected as string);
    };

    const handleClose = async () => {
        const label = getCurrentWindow().label;
        try {
            await invoke('close_download_window', { label });
        } catch (e) {
            console.error('[Download] Close failed:', e);
        }
    };

    const handleCopy = () => navigator.clipboard.writeText(url);

    const handleStart = async () => {
        if (!url) return;
        setIsLoading(true);
        try {
            // Use actual stream URL if available (from yt-dlp format extraction)
            // Otherwise fall back to page URL for direct downloads
            const downloadUrl = videoStreamUrl || url;

            console.log('[Download] Starting download with URL:', downloadUrl, 'filename:', filename);

            // Pass URL with optional filename (null = extract from headers)
            await invoke('handle_download_request', {
                request: {
                    type: 'New',
                    data: [{
                        url: downloadUrl,
                        filename: (videoStreamUrl && filename) ? filename : null
                    }]
                }
            });

            await handleClose();
        } catch (e) {
            console.error('Download failed:', e);
            setIsLoading(false);
        }
    };

    const handlePreview = () => url && window.open(url, '_blank');

    return (
        <div className="h-screen bg-background flex flex-col p-3 gap-1.5">
            {/* Header - OUTSIDE layout */}
            {/* <div className="flex items-center justify-between px-2">
                <div className="flex items-center gap-2">
                    <img src="/icon.png" alt="tur" className="w-10 h-10" />
                    <span className="text-5xl tracking-tight" style={{ fontFamily: "'Margin', sans-serif" }}>tur</span>
                </div>
                <div className="relative" ref={moreRef}>
                    <button
                        onClick={() => setShowMore(!showMore)}
                        className="px-4 py-1.5 text-sm bg-card border border-border rounded-md hover:bg-muted/80 flex items-center gap-1.5"
                    >
                        More <ChevronDown className="w-3 h-3" />
                    </button>
                    {showMore && (
                        <div className="absolute right-0 top-full mt-1 bg-popover border border-border rounded-lg shadow-lg py-1 min-w-[160px] z-50">
                            <button className="w-full px-3 py-2 text-sm text-left hover:bg-muted flex items-center gap-2">
                                <Clock className="w-4 h-4" /> Schedule Download
                            </button>
                            <button className="w-full px-3 py-2 text-sm text-left hover:bg-muted flex items-center gap-2">
                                <ListPlus className="w-4 h-4" /> Queue Download
                            </button>
                        </div>
                    )}
                </div>
            </div> */}

            {/* Main layout */}
            <div className="flex-1 bg-card rounded-2xl border border-border flex flex-col overflow-hidden">
                {/* Row 1: Two columns */}
                <div className="flex-1 flex">
                    {/* Left Column: Form */}
                    <div className="flex-1 p-3 flex flex-col gap-2.5">
                        {/* URL */}
                        <div className="flex items-center gap-2">
                            <label className="w-20 text-xs text-muted-foreground shrink-0">URL</label>
                            <div className="flex-1 flex items-center px-2.5 py-1.5 bg-muted/30 rounded-lg border border-border">
                                <input readOnly value={url} className="flex-1 bg-transparent text-xs truncate outline-none" />
                                <button onClick={handleCopy} className="ml-2 p-1 hover:bg-muted rounded" title="Copy to clipboard">
                                    <Copy className="w-3.5 h-3.5 text-muted-foreground" />
                                </button>
                            </div>
                        </div>

                        {/* Category - fixed width, proper colors */}
                        <div className="flex items-center gap-2">
                            <label className="w-20 text-xs text-muted-foreground shrink-0">Category</label>
                            <select value={category} onChange={(e) => setCategory(e.target.value)}
                                className="w-32 px-2.5 py-1.5 text-xs bg-background text-foreground rounded-xl border border-border">
                                {CATEGORIES.map(c => <option key={c} value={c}>{c}</option>)}
                            </select>
                            <button className="w-8 h-8 flex items-center justify-center text-sm bg-muted hover:bg-muted/80 rounded-md border border-border">
                                +
                            </button>
                        </div>

                        {/* Save As */}
                        <div className="flex items-center gap-2">
                            <label className="w-20 text-xs text-muted-foreground shrink-0">Save as</label>
                            <input value={`${savePath}/${filename}`}
                                onChange={(e) => {
                                    const v = e.target.value, i = v.lastIndexOf('/');
                                    if (i > 0) { setSavePath(v.slice(0, i)); setFilename(v.slice(i + 1)); }
                                }}
                                className="flex-1 px-2.5 py-1.5 text-xs bg-muted/30 rounded-md border border-border" />
                            <button onClick={handleBrowse} className="w-8 h-8 flex items-center justify-center bg-muted hover:bg-muted/80 rounded-md border border-border">
                                <MoreHorizontal className="w-3.5 h-3.5" />
                            </button>
                        </div>

                        {/* Remember Path */}
                        <div className="flex flex-col gap-1.5 ml-22">
                            <label className="flex items-center gap-1.5 text-xs cursor-pointer">
                                <input type="checkbox" checked={rememberPath} onChange={(e) => setRememberPath(e.target.checked)} className="w-3.5 h-3.5 rounded" />
                                Remember this path for "{category}"
                            </label>
                            <input value={categoryPath} onChange={(e) => setCategoryPath(e.target.value)}
                                className="px-2.5 py-1.5 text-xs bg-muted/30 rounded-md border border-border" />
                        </div>

                        {/* Description */}
                        <div className="flex items-center gap-2">
                            <label className="w-20 text-xs text-muted-foreground shrink-0">Description</label>
                            <input value={description} onChange={(e) => setDescription(e.target.value)}
                                className="flex-1 px-2.5 py-1.5 text-xs bg-muted/30 rounded-md border border-border" />
                        </div>
                    </div>

                    {/* Right Column: Logo, Size, Preview - centered */}
                    <div className="w-32 p-3 flex flex-col items-center justify-center gap-1 border-l border-border">
                        <div className="w-20 h-20 bg-muted rounded-xl flex items-center justify-center border border-border">
                            {fileExt ? (
                                <span className="text-lg font-bold text-muted-foreground uppercase">.{fileExt}</span>
                            ) : (
                                <span className="text-xs text-muted-foreground">file</span>
                            )}
                        </div>
                        <span className="text-xs text-primary font-medium mt-5">
                            {filesize ? formatFileSize(filesize) : 'Unknown size'}
                        </span>
                        <button onClick={handlePreview} className="px-3 py-1.5 text-xs bg-card hover:bg-muted/80 rounded-md border border-border mt-4">
                            preview
                        </button>
                    </div>
                </div>

                {/* Row 2: Download Now button */}
                <div className="border-t border-border">
                    <button
                        onClick={handleStart}
                        disabled={isLoading}
                        className="w-full py-3.5 text-sm font-semibold bg-emerald-100/50 dark:bg-emerald-950/30 hover:bg-emerald-200/70 dark:hover:bg-emerald-900/40 text-emerald-700 dark:text-emerald-400 transition-colors disabled:opacity-50 rounded-b-2xl"
                    >
                        {isLoading ? 'Starting...' : 'Download Now'}
                    </button>
                </div>
            </div>
        </div>
    );
}
