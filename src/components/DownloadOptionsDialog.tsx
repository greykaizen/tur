import { useState, useMemo } from "react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useQueues } from "@/contexts/QueueContext";
import { useDownloads } from "@/hooks/useDownloads";
import { X, GripVertical } from "lucide-react";

interface DownloadOptionsDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    urls: string[];
    onStart: (queueId: string | null, mode?: 'sequential' | 'concurrent', parallelCount?: number) => void;
}

type DownloadMode = 'start_all' | 'queue' | 'wait';

export function DownloadOptionsDialog({ open, onOpenChange, urls: initialUrls, onStart }: DownloadOptionsDialogProps) {
    const { queues, createQueue } = useQueues();
    const { downloads } = useDownloads();

    // URL list state (editable)
    const [urls, setUrls] = useState<string[]>(initialUrls);
    const [dragIndex, setDragIndex] = useState<number | null>(null);
    const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);

    // Sync with prop changes
    useState(() => {
        setUrls(initialUrls);
    });

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

    const handleDragStart = (index: number) => {
        setDragIndex(index);
    };

    const handleDragOver = (e: React.DragEvent, index: number) => {
        e.preventDefault();
        setDragOverIndex(index);
    };

    const handleDrop = (e: React.DragEvent, dropIndex: number) => {
        e.preventDefault();
        if (dragIndex === null) return;

        const newUrls = [...urls];
        const [draggedItem] = newUrls.splice(dragIndex, 1);
        newUrls.splice(dropIndex, 0, draggedItem);
        setUrls(newUrls);
        setDragIndex(null);
        setDragOverIndex(null);
    };

    const handleDragEnd = () => {
        setDragIndex(null);
        setDragOverIndex(null);
    };

    const handleStart = async () => {
        if (urls.length === 0) return;

        if (mode === 'start_all') {
            onStart(null);
        } else if (mode === 'queue') {
            const name = queueName.trim() || `Queue ${queues.length + 1}`;
            const queue = await createQueue(name, nextColor, queueMode, parallelCount);
            onStart(queue.id, queueMode, parallelCount);
        } else if (mode === 'wait') {
            const name = queueName.trim() || `Queue ${queues.length + 1}`;
            const queue = await createQueue(name, nextColor, 'sequential', 1);
            onStart(queue.id, 'sequential', 1);
        }
        onOpenChange(false);
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-[540px] max-h-[85vh] flex flex-col">
                <DialogHeader className="pb-2">
                    <DialogTitle className="text-base">How would you like to download?</DialogTitle>
                </DialogHeader>

                <div className="flex-1 overflow-hidden flex flex-col gap-3">
                    {/* URL List - Draggable */}
                    <div className="bg-muted/40 rounded-lg border overflow-hidden">
                        <div className="px-3 py-1.5 bg-muted/60 border-b flex justify-between items-center">
                            <span className="text-xs font-medium text-muted-foreground">
                                {urls.length} link{urls.length !== 1 ? 's' : ''}
                            </span>
                        </div>
                        <div className="max-h-[140px] overflow-y-auto">
                            {urls.map((url, index) => (
                                <div
                                    key={index}
                                    draggable
                                    onDragStart={() => handleDragStart(index)}
                                    onDragOver={(e) => handleDragOver(e, index)}
                                    onDrop={(e) => handleDrop(e, index)}
                                    onDragEnd={handleDragEnd}
                                    className={`group flex items-center gap-2 px-2 py-1.5 text-xs border-b border-border/30 last:border-0 cursor-move transition-colors hover:bg-muted/50 ${dragOverIndex === index ? 'bg-primary/10' : ''
                                        } ${dragIndex === index ? 'opacity-50' : ''}`}
                                >
                                    <GripVertical className="h-3 w-3 text-muted-foreground/50 shrink-0" />
                                    <span className="flex-1 truncate font-mono text-muted-foreground">{url}</span>
                                    <button
                                        onClick={() => removeUrl(index)}
                                        className="opacity-0 group-hover:opacity-100 p-0.5 hover:bg-destructive/20 rounded transition-all"
                                        title="Remove"
                                    >
                                        <X className="h-3 w-3 text-destructive" />
                                    </button>
                                </div>
                            ))}
                        </div>
                    </div>

                    {/* Mode Selection - Compact */}
                    <div className="space-y-2">
                        {/* Start All */}
                        <label className={`flex items-center gap-3 p-2.5 rounded-lg border cursor-pointer transition-all ${mode === 'start_all' ? 'border-primary bg-primary/5' : 'hover:bg-muted/40'
                            }`}>
                            <input
                                type="radio"
                                name="mode"
                                checked={mode === 'start_all'}
                                onChange={() => setMode('start_all')}
                                className="shrink-0"
                            />
                            <div className="flex-1 min-w-0">
                                <div className="text-sm font-medium">Start all now</div>
                                <div className="text-xs text-muted-foreground">Download all files immediately</div>
                            </div>
                        </label>

                        {/* Queue */}
                        <div className={`rounded-lg border transition-all ${mode === 'queue' ? 'border-primary bg-primary/5' : ''
                            }`}>
                            <label className={`flex items-center gap-3 p-2.5 cursor-pointer ${mode !== 'queue' ? 'hover:bg-muted/40 rounded-lg' : ''
                                }`}>
                                <input
                                    type="radio"
                                    name="mode"
                                    checked={mode === 'queue'}
                                    onChange={() => setMode('queue')}
                                    className="shrink-0"
                                />
                                <div className="flex-1 min-w-0">
                                    <div className="text-sm font-medium">Queue downloads</div>
                                    <div className="text-xs text-muted-foreground">Controlled execution order</div>
                                </div>
                            </label>

                            {mode === 'queue' && (
                                <div className="px-3 pb-3 pt-1 space-y-2 border-t border-border/50 ml-6">
                                    {/* Queue Mode */}
                                    <div className="flex items-center gap-3 text-xs">
                                        <label className="flex items-center gap-1.5 cursor-pointer">
                                            <input
                                                type="radio"
                                                name="queueMode"
                                                checked={queueMode === 'sequential'}
                                                onChange={() => setQueueMode('sequential')}
                                            />
                                            <span>One by one</span>
                                        </label>
                                        <label className="flex items-center gap-1.5 cursor-pointer">
                                            <input
                                                type="radio"
                                                name="queueMode"
                                                checked={queueMode === 'concurrent'}
                                                onChange={() => setQueueMode('concurrent')}
                                            />
                                            <span>Keep</span>
                                            <Input
                                                type="number"
                                                min={1}
                                                max={10}
                                                value={parallelCount}
                                                onChange={(e) => setParallelCount(parseInt(e.target.value) || 2)}
                                                className="w-12 h-6 text-xs px-2"
                                                onClick={(e) => e.stopPropagation()}
                                            />
                                            <span>active</span>
                                        </label>
                                    </div>
                                    {/* Queue Name */}
                                    <div className="flex items-center gap-2">
                                        <Input
                                            value={queueName}
                                            onChange={(e) => setQueueName(e.target.value)}
                                            placeholder={`Queue ${queues.length + 1}`}
                                            className="h-7 text-xs flex-1"
                                        />
                                        <div
                                            className="w-5 h-5 rounded-full shrink-0 ring-1 ring-border"
                                            style={{ backgroundColor: nextColor }}
                                        />
                                    </div>
                                </div>
                            )}
                        </div>

                        {/* Wait For - Only show if active */}
                        {(activeDownloads.length > 0 || activeQueues.length > 0) && (
                            <div className={`rounded-lg border transition-all ${mode === 'wait' ? 'border-primary bg-primary/5' : ''
                                }`}>
                                <label className={`flex items-center gap-3 p-2.5 cursor-pointer ${mode !== 'wait' ? 'hover:bg-muted/40 rounded-lg' : ''
                                    }`}>
                                    <input
                                        type="radio"
                                        name="mode"
                                        checked={mode === 'wait'}
                                        onChange={() => setMode('wait')}
                                        className="shrink-0"
                                    />
                                    <div className="flex-1 min-w-0">
                                        <div className="text-sm font-medium">Wait for...</div>
                                        <div className="text-xs text-muted-foreground">Start after another completes</div>
                                    </div>
                                </label>

                                {mode === 'wait' && (
                                    <div className="px-3 pb-3 pt-1 space-y-1 border-t border-border/50 ml-6 max-h-[80px] overflow-y-auto">
                                        {activeDownloads.slice(0, 5).map(dl => (
                                            <label key={dl.id} className="flex items-center gap-2 text-xs cursor-pointer py-0.5">
                                                <input
                                                    type="radio"
                                                    name="waitFor"
                                                    checked={waitForId === dl.id}
                                                    onChange={() => setWaitForId(dl.id)}
                                                />
                                                <span className="truncate flex-1">{dl.filename}</span>
                                                <span className="text-muted-foreground">{Math.round(dl.progress)}%</span>
                                            </label>
                                        ))}
                                        {activeQueues.map(q => (
                                            <label key={q.id} className="flex items-center gap-2 text-xs cursor-pointer py-0.5">
                                                <input
                                                    type="radio"
                                                    name="waitFor"
                                                    checked={waitForId === q.id}
                                                    onChange={() => setWaitForId(q.id)}
                                                />
                                                <span className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: q.color || '#3b82f6' }} />
                                                <span className="truncate flex-1">{q.name}</span>
                                            </label>
                                        ))}
                                    </div>
                                )}
                            </div>
                        )}
                    </div>
                </div>

                <DialogFooter className="pt-3 border-t gap-2">
                    <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>Cancel</Button>
                    <Button size="sm" onClick={handleStart} disabled={urls.length === 0}>
                        {mode === 'start_all' ? 'Start All' : mode === 'queue' ? 'Create Queue' : 'Wait & Queue'}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
