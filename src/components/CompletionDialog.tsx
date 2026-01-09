import { invoke } from '@tauri-apps/api/core';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { FolderOpen, ExternalLink, Play, X } from 'lucide-react';
import { formatSize } from '@/hooks/useDownloads';

interface CompletionDialogProps {
    open: boolean;
    onClose: () => void;
    download: {
        filename: string;
        size: number | null;
        destination: string;
    } | null;
}

export function CompletionDialog({ open, onClose, download }: CompletionDialogProps) {
    if (!download) return null;

    const handleOpen = async () => {
        try {
            await invoke('open_path', { path: download.destination });
            onClose();
        } catch (e) {
            console.error('Failed to open file:', e);
        }
    };

    const handleOpenWith = async () => {
        // Open with default file picker - platform specific
        try {
            // On Linux, use xdg-open with --ask flag or similar
            // For now, just open with default
            await invoke('open_path', { path: download.destination });
            onClose();
        } catch (e) {
            console.error('Failed to open file:', e);
        }
    };

    const handleOpenLocation = async () => {
        try {
            // Get the directory containing the file
            const dir = download.destination.substring(0, download.destination.lastIndexOf('/'));
            await invoke('open_path', { path: dir || download.destination });
            onClose();
        } catch (e) {
            console.error('Failed to open location:', e);
        }
    };

    return (
        <Dialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
            <DialogContent className="sm:max-w-md">
                <DialogHeader>
                    <DialogTitle className="flex items-center gap-2">
                        <span className="text-green-500">✓</span>
                        Download Complete
                    </DialogTitle>
                </DialogHeader>

                <div className="space-y-4 py-4">
                    {/* File Info */}
                    <div className="space-y-2">
                        <p className="font-medium truncate" title={download.filename}>
                            {download.filename}
                        </p>
                        <div className="text-sm text-muted-foreground space-y-1">
                            <p>Size: {download.size ? formatSize(download.size) : 'Unknown'}</p>
                            <p className="truncate" title={download.destination}>
                                Location: {download.destination}
                            </p>
                        </div>
                    </div>

                    {/* Action Buttons */}
                    <div className="flex flex-wrap gap-2">
                        <Button onClick={handleOpen} className="gap-2">
                            <Play className="size-4" />
                            Open
                        </Button>
                        <Button onClick={handleOpenWith} variant="outline" className="gap-2">
                            <ExternalLink className="size-4" />
                            Open With
                        </Button>
                        <Button onClick={handleOpenLocation} variant="outline" className="gap-2">
                            <FolderOpen className="size-4" />
                            Open Location
                        </Button>
                        <Button onClick={onClose} variant="ghost" className="gap-2">
                            <X className="size-4" />
                            Close
                        </Button>
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
}
