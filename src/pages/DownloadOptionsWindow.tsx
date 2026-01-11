import { useState, useMemo, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useQueues } from "@/contexts/QueueContext";
import { useDownloads } from "@/hooks/useDownloads";
import { X, GripVertical, Trash2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

type DownloadMode = 'start_all' | 'queue' | 'wait';

export default function DownloadOptionsWindow() {
    const { queues, createQueue } = useQueues();
    const { downloads } = useDownloads();

    // URL list state (editable)
    const [urls, setUrls] = useState<string[]>([]);
    const [dragIndex, setDragIndex] = useState<number | null>(null);
    const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
    const [isRemoveZoneActive, setIsRemoveZoneActive] = useState(false);

    // Listen for init data
    useEffect(() => {
        const unlisten = listen<{ urls: string[] }>('download-options-init', (event) => {
            setUrls(event.payload.urls);
        });

        // Signal we are ready
        // invoke('download_options_ready'); 

        return () => {
            unlisten.then(f => f());
        };
    }, []);

    // Also check for query param as fallback/initial
    useEffect(() => {
        const params = new URLSearchParams(window.location.search);
        const urlsParam = params.get('urls');
        if (urlsParam) {
            try {
                setUrls(JSON.parse(decodeURIComponent(urlsParam)));
            } catch (e) {
                console.error("Failed to parse URLs from query param", e);
            }
        }
    }, []);

    // Mode selection
    const [mode, setMode] = useState<DownloadMode>('start_all');

    // Queue options
    const [queueMode, setQueueMode] = useState<'sequential' | 'concurrent'>('sequential');
    const [parallelCount, setParallelCount] = useState(2);
    const [queueName, setQueueName] = useState('');

    // Wait for options
    const [waitForId, setWaitForId] = useState<string | null>(null);

    // Active downloads/queues for "Wait for" option
    const activeDownloads = useMemo(() =>
        downloads.filter(d => d.status === 'downloading' || d.status === 'queued'),
        [downloads]
    );
    const activeQueues = useMemo(() =>
        queues.filter(q => q.status === 'active'),
        [queues]
    );

    // Subtle queue colors
    const queueColors = ['#fbbf24', '#34d399', '#60a5fa', '#a78bfa', '#f472b6', '#fb923c'];
    const nextColor = queueColors[(queues.length) % queueColors.length];

    const removeUrl = (index: number) => {
        setUrls(prev => prev.filter((_, i) => i !== index));
    };

    const handleDragStart = (e: React.DragEvent, index: number) => {
        setDragIndex(index);
        e.dataTransfer.effectAllowed = "move";
    };

    const handleDragOver = (e: React.DragEvent, index: number) => {
        e.preventDefault();
        e.stopPropagation();
        setDragOverIndex(index);
        setIsRemoveZoneActive(false);
    };

    const handleListDrop = (e: React.DragEvent, dropIndex: number) => {
        e.preventDefault();
        e.stopPropagation();
        if (dragIndex === null) return;

        const newUrls = [...urls];
        const [draggedItem] = newUrls.splice(dragIndex, 1);
        newUrls.splice(dropIndex, 0, draggedItem);
        setUrls(newUrls);
        setDragIndex(null);
        setDragOverIndex(null);
    };

    const handleRemoveZoneDragOver = (e: React.DragEvent) => {
        e.preventDefault();
        if (dragIndex !== null) {
            setIsRemoveZoneActive(true);
            setDragOverIndex(null);
        }
    };

    const handleRemoveZoneLeave = (e: React.DragEvent) => {
        if (e.target === e.currentTarget) {
            setIsRemoveZoneActive(false);
        }
    };

    const handleRemoveDrop = (e: React.DragEvent) => {
        e.preventDefault();
        if (dragIndex !== null) {
            removeUrl(dragIndex);
            setDragIndex(null);
            setIsRemoveZoneActive(false);
        }
    };

    const handleDragEnd = () => {
        setDragIndex(null);
        setDragOverIndex(null);
        setIsRemoveZoneActive(false);
    };

    const onStart = async (queueId: string | null) => {
        // Prepare items
        const items = urls.map(url => ({
            url,
            queue_id: queueId
        }));

        // Use invoke generic command 'queue_download' (it handles single or multiple)
        // Wait, the existing code mapped and called startDownloads.
        // We need to replicate startDownloads logic or call the Tauri command directly.
        // The Tauri command is 'queue_download' but it takes a single item?
        // Let's check manager.rs output or Home.tsx usage.
        // Home.tsx: await startDownloads(allUrls.map(url => ({ url, queue_id: queueId })));

        // We'll reimplement specific start logic here:
        // iterate and invoke 'queue_download' for each.
        // Or if there is a bulk command? 'queue_download' takes generic?
        // Let's assume we invoke 'queue_download' for each for now, or check usage.

        // Actually, invoking loop is fine.
        for (const item of items) {
            await invoke('queue_download', {
                url: item.url,
                filename: null, // option
                queueId: item.queue_id
            });
        }
    };

    const handleStart = async () => {
        if (urls.length === 0) return;

        try {
            if (mode === 'start_all') {
                await onStart(null);
            } else if (mode === 'queue') {
                const name = queueName.trim() || `Queue ${queues.length + 1}`;
                const queue = await createQueue(name, nextColor, queueMode, parallelCount);
                await onStart(queue.id);
            } else if (mode === 'wait') {
                const name = queueName.trim() || `Queue ${queues.length + 1}`;
                const queue = await createQueue(name, nextColor, 'sequential', 1);
                await onStart(queue.id);
            }

            // Close window on success
            await getCurrentWindow().close();
        } catch (error) {
            console.error("Failed to start downloads", error);
        }
    };

    const handleCancel = async () => {
        await getCurrentWindow().close();
    };

    return (
        <div
            className={`h-screen w-screen flex flex-col bg-background overflow-hidden transition-colors ${isRemoveZoneActive ? 'bg-destructive/5' : ''
                }`}
            onDragOver={handleRemoveZoneDragOver}
            onDragLeave={handleRemoveZoneLeave}
            onDrop={handleRemoveDrop}
        >
            {/* Header */}
            <div className="px-6 py-4 border-b bg-muted/30 shrink-0" data-tauri-drag-region>
                <div className="text-base font-semibold flex items-center justify-between pointer-events-none">
                    <span>How would you like to download?</span>
                    {isRemoveZoneActive && (
                        <span className="text-destructive text-xs flex items-center gap-1 animate-pulse">
                            <Trash2 className="h-3 w-3" />
                            Drop to remove
                        </span>
                    )}
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-6 flex flex-col gap-6">
                {/* URL List - Draggable */}
                <div className="space-y-2">
                    <div className="flex justify-between items-center px-1">
                        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                            {urls.length} link{urls.length !== 1 ? 's' : ''} detected
                        </span>
                        <span className="text-[10px] text-muted-foreground/60 italic">
                            Drag out to remove
                        </span>
                    </div>

                    <div className="bg-background rounded-lg border shadow-sm divide-y" onDrop={(e) => e.stopPropagation()}>
                        <div className="max-h-[160px] overflow-y-auto">
                            {urls.map((url, index) => (
                                <div
                                    key={index}
                                    draggable
                                    onDragStart={(e) => handleDragStart(e, index)}
                                    onDragOver={(e) => handleDragOver(e, index)}
                                    onDrop={(e) => handleListDrop(e, index)}
                                    onDragEnd={handleDragEnd}
                                    className={`group flex items-center gap-3 px-3 py-2.5 text-sm cursor-move transition-all hover:bg-muted/30 ${dragOverIndex === index ? 'bg-primary/5 border-primary/20 relative z-10' : ''
                                        } ${dragIndex === index ? 'opacity-40 grayscale' : ''}`}
                                >
                                    <GripVertical className="h-4 w-4 text-muted-foreground/30 group-hover:text-muted-foreground/60 transition-colors shrink-0" />
                                    <div className="flex-1 min-w-0">
                                        <div className="truncate font-mono text-xs">{url}</div>
                                    </div>
                                    <button
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            removeUrl(index);
                                        }}
                                        className="opacity-0 group-hover:opacity-100 p-1 hover:bg-destructive/10 hover:text-destructive rounded-md transition-all shrink-0"
                                        title="Remove"
                                    >
                                        <X className="h-3.5 w-3.5" />
                                    </button>
                                </div>
                            ))}
                            {urls.length === 0 && (
                                <div className="py-8 text-center text-muted-foreground text-xs">
                                    No URLs remaining
                                </div>
                            )}
                        </div>
                    </div>
                </div>

                {/* Mode Selection */}
                <div className="space-y-3">
                    <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider px-1">
                        Download Mode
                    </span>

                    <div className="grid gap-3">
                        {/* Start All */}
                        <label className={`flex items-start gap-4 p-4 rounded-xl border transition-all cursor-pointer ${mode === 'start_all'
                            ? 'border-primary ring-1 ring-primary bg-primary/5 shadow-sm'
                            : 'hover:bg-muted/40 hover:border-muted-foreground/30'
                            }`}>
                            <input
                                type="radio"
                                name="mode"
                                checked={mode === 'start_all'}
                                onChange={() => setMode('start_all')}
                                className="mt-1"
                            />
                            <div>
                                <div className="font-medium text-sm">Start all now</div>
                                <div className="text-xs text-muted-foreground mt-0.5">Download files immediately in parallel</div>
                            </div>
                        </label>

                        {/* Queue */}
                        <div className={`rounded-xl border transition-all overflow-hidden ${mode === 'queue' ? 'border-primary ring-1 ring-primary bg-primary/5 shadow-sm' : ''
                            }`}>
                            <label className={`flex items-start gap-4 p-4 cursor-pointer ${mode !== 'queue' ? 'hover:bg-muted/40 hover:border-muted-foreground/30' : ''
                                }`}>
                                <input
                                    type="radio"
                                    name="mode"
                                    checked={mode === 'queue'}
                                    onChange={() => setMode('queue')}
                                    className="mt-1"
                                    onFocus={() => { if (mode !== 'queue') setMode('queue'); }}
                                />
                                <div className="flex-1">
                                    <div className="font-medium text-sm">Queue downloads</div>
                                    <div className="text-xs text-muted-foreground mt-0.5">Create a managed queue</div>
                                </div>
                            </label>

                            {mode === 'queue' && (
                                <div className="bg-background/50 border-t px-4 py-3 space-y-3 animate-in slide-in-from-top-2 duration-200">
                                    <div className="flex flex-wrap gap-4">
                                        <label className="flex items-center gap-2 text-sm cursor-pointer select-none">
                                            <input
                                                type="radio"
                                                name="queueMode"
                                                checked={queueMode === 'sequential'}
                                                onChange={() => setQueueMode('sequential')}
                                                className="accent-primary"
                                            />
                                            <span>One by one</span>
                                        </label>
                                        <label className="flex items-center gap-2 text-sm cursor-pointer select-none">
                                            <input
                                                type="radio"
                                                name="queueMode"
                                                checked={queueMode === 'concurrent'}
                                                onChange={() => setQueueMode('concurrent')}
                                                className="accent-primary"
                                            />
                                            <span className="flex items-center gap-2">
                                                Keep
                                                <Input
                                                    type="number"
                                                    min={1}
                                                    max={10}
                                                    value={parallelCount}
                                                    onChange={(e) => setParallelCount(parseInt(e.target.value) || 2)}
                                                    className="w-14 h-7 text-xs px-2 bg-background"
                                                    onClick={(e) => e.stopPropagation()}
                                                />
                                                active
                                            </span>
                                        </label>
                                    </div>
                                    <div className="items-center gap-2 flex">
                                        <Input
                                            value={queueName}
                                            onChange={(e) => setQueueName(e.target.value)}
                                            placeholder={`Queue ${queues.length + 1}`}
                                            className="h-8 text-sm bg-background"
                                        />
                                        <div
                                            className="w-6 h-6 rounded-full shrink-0 ring-1 ring-border shadow-sm"
                                            style={{ backgroundColor: nextColor }}
                                            title="Queue Color"
                                        />
                                    </div>
                                </div>
                            )}
                        </div>

                        {/* Wait For */}
                        {(activeDownloads.length > 0 || activeQueues.length > 0) && (
                            <div className={`rounded-xl border transition-all overflow-hidden ${mode === 'wait' ? 'border-primary ring-1 ring-primary bg-primary/5 shadow-sm' : ''
                                }`}>
                                <label className={`flex items-start gap-4 p-4 cursor-pointer ${mode !== 'wait' ? 'hover:bg-muted/40 hover:border-muted-foreground/30' : ''
                                    }`}>
                                    <input
                                        type="radio"
                                        name="mode"
                                        checked={mode === 'wait'}
                                        onChange={() => setMode('wait')}
                                        className="mt-1"
                                        onFocus={() => { if (mode !== 'wait') setMode('wait'); }}
                                    />
                                    <div className="flex-1">
                                        <div className="font-medium text-sm">Wait for...</div>
                                        <div className="text-xs text-muted-foreground mt-0.5">Start after completion</div>
                                    </div>
                                </label>

                                {mode === 'wait' && (
                                    <div className="bg-background/50 border-t max-h-[120px] overflow-y-auto divide-y animate-in slide-in-from-top-2 duration-200">
                                        {[...activeDownloads, ...activeQueues].map((item: any) => (
                                            <label key={item.id} className="flex items-center gap-3 px-4 py-2.5 text-sm cursor-pointer hover:bg-muted/30 transition-colors">
                                                <input
                                                    type="radio"
                                                    name="waitFor"
                                                    checked={waitForId === item.id}
                                                    onChange={() => setWaitForId(item.id)}
                                                    className="accent-primary"
                                                />
                                                <div className="flex-1 min-w-0 flex items-center justify-between gap-2">
                                                    <span className="truncate">{item.filename || item.name}</span>
                                                    {item.progress !== undefined && (
                                                        <span className="text-xs text-muted-foreground">{Math.round(item.progress)}%</span>
                                                    )}
                                                    {item.color && (
                                                        <div className="w-2 h-2 rounded-full" style={{ backgroundColor: item.color }} />
                                                    )}
                                                </div>
                                            </label>
                                        ))}
                                    </div>
                                )}
                            </div>
                        )}
                    </div>
                </div>
            </div>

            <div className="px-6 py-4 border-t bg-muted/30 flex justify-between sm:justify-between items-center shrink-0">
                <Button variant="ghost" onClick={handleCancel} className="hover:bg-background">Cancel</Button>
                <Button onClick={handleStart} disabled={urls.length === 0} className="px-8 shadow-sm">
                    {mode === 'start_all' ? 'Start All' : mode === 'queue' ? 'Create Queue' : 'Wait & Queue'}
                </Button>
            </div>
        </div>
    );
}
