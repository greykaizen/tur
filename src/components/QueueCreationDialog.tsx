import { useState } from "react";

import { useQueues } from "@/contexts/QueueContext";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";

interface QueueCreationDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onQueueCreated?: () => void;
}

export function QueueCreationDialog({ open, onOpenChange, onQueueCreated }: QueueCreationDialogProps) {
    const { createQueue } = useQueues();
    const [name, setName] = useState("");
    const [color, setColor] = useState("#3b82f6");
    const [mode, setMode] = useState<'sequential' | 'concurrent'>('sequential');
    const [parallelCount, setParallelCount] = useState(2);
    const [isLoading, setIsLoading] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!name.trim()) return;

        setIsLoading(true);
        try {
            await createQueue(name, color, mode, parallelCount);
            toast.success("Queue created successfully");
            setName("");
            setMode('sequential');
            setParallelCount(2);
            onOpenChange(false);
            onQueueCreated?.();
        } catch (error) {
            toast.error(`Failed to create queue: ${error}`);
        } finally {
            setIsLoading(false);
        }
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-[460px]">
                <DialogHeader>
                    <DialogTitle>Create New Queue</DialogTitle>
                    <DialogDescription>
                        Queues let you organize and control how downloads are processed.
                    </DialogDescription>
                </DialogHeader>
                <form onSubmit={handleSubmit} className="grid gap-5 py-4">
                    {/* Queue Name */}
                    <div className="grid grid-cols-4 items-center gap-4">
                        <Label htmlFor="name" className="text-right">
                            Name
                        </Label>
                        <Input
                            id="name"
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            className="col-span-3"
                            placeholder="e.g., Nightly Builds"
                            autoFocus
                        />
                    </div>

                    {/* Queue Color */}
                    <div className="grid grid-cols-4 items-center gap-4">
                        <Label htmlFor="color" className="text-right">
                            Color
                        </Label>
                        <div className="col-span-3 flex items-center gap-2">
                            <Input
                                id="color"
                                type="color"
                                value={color}
                                onChange={(e) => setColor(e.target.value)}
                                className="w-12 h-8 p-1 cursor-pointer"
                            />
                            <span className="text-sm text-muted-foreground">{color}</span>
                        </div>
                    </div>

                    {/* Queue Mode */}
                    <div className="grid grid-cols-4 items-start gap-4">
                        <Label className="text-right pt-2">Mode</Label>
                        <div className="col-span-3 space-y-3">
                            {/* Sequential Option */}
                            <label className="flex items-start gap-3 cursor-pointer group">
                                <input
                                    type="radio"
                                    name="mode"
                                    value="sequential"
                                    checked={mode === 'sequential'}
                                    onChange={() => setMode('sequential')}
                                    className="mt-1"
                                />
                                <div>
                                    <div className="font-medium group-hover:text-primary transition-colors">
                                        Sequential
                                    </div>
                                    <div className="text-xs text-muted-foreground">
                                        Downloads one at a time. Next starts when current finishes.
                                    </div>
                                </div>
                            </label>

                            {/* Concurrent Option */}
                            <label className="flex items-start gap-3 cursor-pointer group">
                                <input
                                    type="radio"
                                    name="mode"
                                    value="concurrent"
                                    checked={mode === 'concurrent'}
                                    onChange={() => setMode('concurrent')}
                                    className="mt-1"
                                />
                                <div className="flex-1">
                                    <div className="font-medium group-hover:text-primary transition-colors">
                                        Concurrent
                                    </div>
                                    <div className="text-xs text-muted-foreground mb-2">
                                        Keep multiple downloads active at the same time.
                                    </div>
                                    {mode === 'concurrent' && (
                                        <div className="flex items-center gap-2">
                                            <Label htmlFor="parallelCount" className="text-xs whitespace-nowrap">
                                                Keep active:
                                            </Label>
                                            <Input
                                                id="parallelCount"
                                                type="number"
                                                min={1}
                                                max={10}
                                                value={parallelCount}
                                                onChange={(e) => setParallelCount(parseInt(e.target.value) || 2)}
                                                className="w-16 h-7 text-sm"
                                            />
                                        </div>
                                    )}
                                </div>
                            </label>
                        </div>
                    </div>

                    <DialogFooter>
                        <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
                            Cancel
                        </Button>
                        <Button type="submit" disabled={isLoading || !name.trim()}>
                            {isLoading ? "Creating..." : "Create Queue"}
                        </Button>
                    </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
}
