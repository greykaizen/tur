import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import PageTransition from '@/components/PageTransition';
import { Play, Square, X, Pause, List, Grid3x3, ChevronDown, ListOrdered } from 'lucide-react';
import { toast } from 'sonner';
import { useDownloads, formatSize, formatSpeed, formatTimeLeft } from '@/hooks/useDownloads';

type ViewType = 'list' | 'grid';

export default function Detail() {
  const navigate = useNavigate();
  const [view, setView] = useState<ViewType>('list');
  const [viewMenuOpen, setViewMenuOpen] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // Get downloads from hook
  const { downloads, loadHistory } = useDownloads();

  // Load history from database on mount
  useEffect(() => {
    loadHistory();
  }, [loadHistory]);

  // Convert hook data to display format
  const allDownloads = downloads.map(d => ({
    ...d,
    source: d.status === 'completed' || d.status === 'failed' ? 'history' as const : 'active' as const
  }));

  // Sort: active (downloading/paused) on top, completed at bottom
  const sortedDownloads = allDownloads.sort((a, b) => {
    // Active downloads first
    const aIsActive = a.status === 'downloading' || a.status === 'paused';
    const bIsActive = b.status === 'downloading' || b.status === 'paused';

    if (aIsActive && !bIsActive) return -1;
    if (!aIsActive && bIsActive) return 1;

    // Same status, sort by progress
    return b.progress - a.progress;
  });

  const toggleSelect = (id: string) => {
    const newSelected = new Set(selectedIds);
    if (newSelected.has(id)) {
      newSelected.delete(id);
    } else {
      newSelected.add(id);
    }
    setSelectedIds(newSelected);
  };

  const toggleSelectAll = () => {
    if (selectedIds.size === sortedDownloads.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(sortedDownloads.map(d => d.id)));
    }
  };

  const getFileExtension = (filename: string) => {
    const parts = filename.split('.');
    return parts.length > 1 ? parts[parts.length - 1].toUpperCase() : 'FILE';
  };

  const truncateFilename = (filename: string, maxLength: number = 50) => {
    if (filename.length <= maxLength) return filename;
    const ext = filename.split('.').pop() || '';
    const nameWithoutExt = filename.substring(0, filename.lastIndexOf('.'));
    const truncatedName = nameWithoutExt.substring(0, maxLength - ext.length - 4) + '...';
    return truncatedName + '.' + ext;
  };

  const handleQueueSelected = () => {
    toast.success(`${selectedIds.size} downloads queued`);
    setSelectedIds(new Set());
  };

  const handleStopSelected = () => {
    toast.success(`${selectedIds.size} downloads stopped`);
    setSelectedIds(new Set());
  };

  const handleResumeSelected = () => {
    toast.success(`${selectedIds.size} downloads resumed`);
    setSelectedIds(new Set());
  };

  const handleCancelSelected = () => {
    toast.success(`${selectedIds.size} downloads cancelled`);
    setSelectedIds(new Set());
  };

  const handlePauseResume = (e: React.MouseEvent, status: string) => {
    e.stopPropagation();
    if (status === 'downloading') {
      toast.success('Download paused');
    } else {
      toast.success('Download resumed');
    }
  };

  const handleCancel = (e: React.MouseEvent) => {
    e.stopPropagation();
    toast.success('Download cancelled');
  };

  const handleDownloadClick = (downloadId: string) => {
    // Find the download object
    const download = sortedDownloads.find(d => d.id === downloadId);
    if (download) {
      // Navigate to home page with the selected download object
      navigate('/', { state: { download } });
    }
  };

  return (
    <PageTransition className="h-full w-full flex flex-col overflow-hidden relative">
      {/* Floating Action Bar - appears when items are selected */}
      {selectedIds.size > 0 && (
        <div className="absolute bottom-4 left-1/2 -translate-x-1/2 z-50 bg-primary text-primary-foreground px-4 py-2 rounded-full shadow-lg flex items-center gap-4">
          <span className="text-sm font-medium">{selectedIds.size} selected</span>
          <div className="flex items-center gap-2">
            <button
              onClick={handleQueueSelected}
              className="p-1.5 hover:bg-primary-foreground/20 rounded-md transition-colors"
              title="Queue selected"
            >
              <ListOrdered className="size-4" />
            </button>
            <button
              onClick={handleResumeSelected}
              className="p-1.5 hover:bg-primary-foreground/20 rounded-md transition-colors"
              title="Resume selected"
            >
              <Play className="size-4" />
            </button>
            <button
              onClick={handleStopSelected}
              className="p-1.5 hover:bg-primary-foreground/20 rounded-md transition-colors"
              title="Stop selected"
            >
              <Square className="size-4" />
            </button>
            <button
              onClick={handleCancelSelected}
              className="p-1.5 hover:bg-destructive/80 rounded-md transition-colors"
              title="Cancel selected"
            >
              <X className="size-4" />
            </button>
          </div>
        </div>
      )}

      {/* Header with select all and view switcher */}
      <div className="shrink-0 border-b border-border px-4 py-3 flex items-center justify-between">
        <label className="flex items-center gap-2 text-sm cursor-pointer hover:text-primary transition-colors">
          <input
            type="checkbox"
            className="rounded border-border w-4 h-4 cursor-pointer"
            checked={selectedIds.size === sortedDownloads.length && sortedDownloads.length > 0}
            onChange={toggleSelectAll}
          />
          Select all
        </label>

        {/* View Switcher */}
        <div className="relative">
          <button
            onClick={() => setViewMenuOpen(!viewMenuOpen)}
            className="flex items-center gap-2 px-3 py-1.5 rounded-md text-sm font-medium bg-muted hover:bg-muted/80 transition-colors"
          >
            {view === 'list' ? <List className="size-4" /> : <Grid3x3 className="size-4" />}
            <span>{view === 'list' ? 'List' : 'Grid'}</span>
            <ChevronDown className="size-3" />
          </button>

          {viewMenuOpen && (
            <>
              <div
                className="fixed inset-0 z-[90]"
                onClick={() => setViewMenuOpen(false)}
              />
              <div className="absolute right-0 top-full mt-1 w-32 bg-popover border border-border rounded-md shadow-lg z-[95]">
                <div className="py-1">
                  <button
                    onClick={() => {
                      setView('list');
                      setViewMenuOpen(false);
                    }}
                    className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
                  >
                    <List className="size-4" />
                    List
                  </button>
                  <button
                    onClick={() => {
                      setView('grid');
                      setViewMenuOpen(false);
                    }}
                    className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
                  >
                    <Grid3x3 className="size-4" />
                    Grid
                  </button>
                </div>
              </div>
            </>
          )}
        </div>
      </div>

      {/* List View */}
      {view === 'list' && (
        <div className="flex-1 overflow-y-auto p-3">
          {sortedDownloads.length === 0 ? (
            <div className="flex items-center justify-center h-full">
              <p className="text-muted-foreground">No downloads</p>
            </div>
          ) : (
            <div className="space-y-2">
              {sortedDownloads.map((download) => (
                <div
                  key={download.id}
                  onClick={() => handleDownloadClick(download.id)}
                  className="relative border border-border rounded-lg overflow-hidden hover:border-primary/50 transition-all cursor-pointer group"
                >
                  {/* Progress background - opaque */}
                  <div
                    className="absolute inset-0 bg-green-500/5"
                    style={{ width: `${download.progress}%` }}
                  />

                  <div className="relative p-3">
                    <div className="flex items-center gap-3">
                      {/* Checkbox */}
                      <input
                        type="checkbox"
                        checked={selectedIds.has(download.id)}
                        onChange={(e) => {
                          e.stopPropagation();
                          toggleSelect(download.id);
                        }}
                        onClick={(e) => e.stopPropagation()}
                        className="rounded border-border w-4 h-4 cursor-pointer"
                      />

                      {/* File extension icon */}
                      <div className="flex items-center justify-center w-10 h-10 rounded-lg bg-muted shrink-0">
                        <span className="text-[10px] font-bold text-muted-foreground">
                          {getFileExtension(download.filename)}
                        </span>
                      </div>

                      {/* Download info */}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center justify-between gap-3 mb-1.5">
                          <h3 className="text-sm font-medium truncate flex-1">
                            {truncateFilename(download.filename, 60)}
                          </h3>
                          <span className="text-sm font-medium shrink-0">{download.progress}%</span>
                        </div>

                        {/* Progress bars */}
                        <div className="space-y-1 mb-1.5">
                          <div className="w-full h-1.5 bg-muted rounded-full overflow-hidden">
                            <div
                              className="h-full bg-green-500 transition-all duration-300"
                              style={{ width: `${download.progress}%` }}
                            />
                          </div>
                          {download.segments && download.segments.length > 0 && (
                            <div className="w-full h-1 bg-muted rounded-full overflow-hidden relative">
                              {download.segments.map((segment, idx) => (
                                <div
                                  key={idx}
                                  className="absolute h-full bg-blue-500/60"
                                  style={{
                                    left: `${segment.start}%`,
                                    width: `${segment.end - segment.start}%`
                                  }}
                                />
                              ))}
                            </div>
                          )}
                        </div>

                        {/* Stats */}
                        <div className="flex items-center justify-between text-xs text-muted-foreground">
                          <div className="flex items-center gap-2">
                            <span>{formatSize(download.downloaded)}</span>
                            <span>•</span>
                            <span>{formatSpeed(download.speed)}</span>
                            <span>•</span>
                            <span>{formatTimeLeft(download.downloaded, download.size || 0, download.speed)}</span>
                          </div>
                          <span>{download.size ? formatSize(download.size) : 'Unknown'}</span>
                        </div>
                      </div>

                      {/* Action buttons */}
                      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                        {download.status === 'downloading' ? (
                          <button
                            onClick={(e) => handlePauseResume(e, download.status)}
                            className="p-1.5 hover:bg-muted rounded-md transition-colors"
                            title="Pause"
                          >
                            <Pause className="size-4" />
                          </button>
                        ) : download.status === 'paused' ? (
                          <button
                            onClick={(e) => handlePauseResume(e, download.status)}
                            className="p-1.5 hover:bg-muted rounded-md transition-colors"
                            title="Resume"
                          >
                            <Play className="size-4" />
                          </button>
                        ) : null}
                        {(download.status === 'downloading' || download.status === 'paused') && (
                          <button
                            onClick={handleCancel}
                            className="p-1.5 hover:bg-destructive/10 text-destructive rounded-md transition-colors"
                            title="Cancel"
                          >
                            <X className="size-4" />
                          </button>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Grid View */}
      {view === 'grid' && (
        <div className="flex-1 overflow-auto p-3">
          {sortedDownloads.length === 0 ? (
            <div className="flex items-center justify-center h-full">
              <p className="text-muted-foreground">No downloads</p>
            </div>
          ) : (
            <div className="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-6 lg:grid-cols-8 xl:grid-cols-10 gap-2">
              {sortedDownloads.map((download) => (
                <div
                  key={download.id}
                  onClick={() => handleDownloadClick(download.id)}
                  className="relative group border border-border rounded-lg overflow-hidden hover:shadow-md hover:border-primary/50 transition-all cursor-pointer"
                >
                  {/* Progress background */}
                  <div
                    className="absolute inset-0 bg-green-500/10"
                    style={{ width: `${download.progress}%` }}
                  />

                  {/* Content */}
                  <div className="relative p-2">
                    {/* Checkbox */}
                    <div className="absolute top-1 left-1 z-10">
                      <input
                        type="checkbox"
                        checked={selectedIds.has(download.id)}
                        onChange={(e) => {
                          e.stopPropagation();
                          toggleSelect(download.id);
                        }}
                        onClick={(e) => e.stopPropagation()}
                        className="rounded border-border w-3.5 h-3.5 cursor-pointer"
                      />
                    </div>

                    {/* Action buttons */}
                    <div className="absolute top-1 right-1 z-10 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                      {download.status === 'downloading' || download.status === 'paused' ? (
                        download.status === 'downloading' ? (
                          <button
                            onClick={(e) => handlePauseResume(e, download.status)}
                            className="p-0.5 bg-background/80 hover:bg-muted rounded-md transition-colors"
                            title="Pause"
                          >
                            <Pause className="size-3" />
                          </button>
                        ) : (
                          <button
                            onClick={(e) => handlePauseResume(e, download.status)}
                            className="p-0.5 bg-background/80 hover:bg-muted rounded-md transition-colors"
                            title="Resume"
                          >
                            <Play className="size-3" />
                          </button>
                        )
                      ) : null}
                    </div>

                    <div className="flex flex-col items-center justify-center pt-4 pb-1.5">
                      {/* File extension badge */}
                      <div className="mb-2 flex items-center justify-center w-10 h-10 rounded-lg bg-muted">
                        <span className="text-[10px] font-bold text-muted-foreground">
                          {getFileExtension(download.filename)}
                        </span>
                      </div>

                      {/* Filename */}
                      <p className="text-xs font-medium text-center line-clamp-2 mb-0.5 px-0.5">
                        {truncateFilename(download.filename, 20)}
                      </p>

                      {/* Progress */}
                      <p className="text-xs font-semibold text-primary mb-0.5">
                        {download.progress}%
                      </p>

                      {/* Speed or Status */}
                      <p className="text-[10px] text-muted-foreground">
                        {download.status === 'completed' ? 'Completed' : download.speed}
                      </p>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </PageTransition>
  );
}
