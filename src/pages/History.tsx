import { useState, useEffect } from 'react';
import PageTransition from '@/components/PageTransition';
import { MoreVertical, FolderOpen, Trash2, Download, Play, X, CheckCircle2, Clock, Loader2, List, Grid3x3, ChevronDown, Copy } from 'lucide-react';
import { toast } from 'sonner';
import { useDownloads, formatSize } from '@/hooks/useDownloads';

type FilterType = 'all' | 'completed' | 'incomplete';
type ViewType = 'list' | 'grid';

export default function History() {
  const [filter, setFilter] = useState<FilterType>('all');
  const [view, setView] = useState<ViewType>('list');
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [viewMenuOpen, setViewMenuOpen] = useState(false);

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

  // Filter downloads
  const filteredDownloads = allDownloads.filter(download => {
    if (filter === 'all') return true;
    if (filter === 'completed') return download.status === 'completed';
    if (filter === 'incomplete') return download.status === 'paused' || download.status === 'downloading';
    return true;
  });

  // Helper to get file extension
  const getFileExtension = (filename: string) => {
    const parts = filename.split('.');
    return parts.length > 1 ? parts[parts.length - 1].toUpperCase() : 'FILE';
  };

  // Helper to truncate filename intelligently
  const truncateFilename = (filename: string, maxLength: number = 40) => {
    if (filename.length <= maxLength) return filename;
    const ext = filename.split('.').pop() || '';
    const nameWithoutExt = filename.substring(0, filename.lastIndexOf('.'));
    const truncatedName = nameWithoutExt.substring(0, maxLength - ext.length - 4) + '...';
    return truncatedName + '.' + ext;
  };

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
    if (selectedIds.size === filteredDownloads.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(filteredDownloads.map(d => d.id)));
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'completed':
        return 'text-green-600 dark:text-green-500';
      case 'downloading':
        return 'text-blue-600 dark:text-blue-500';
      case 'paused':
        return 'text-yellow-600 dark:text-yellow-500';
      default:
        return 'text-muted-foreground';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'completed':
        return <CheckCircle2 className="size-4" />;
      case 'downloading':
        return <Loader2 className="size-4 animate-spin" />;
      case 'paused':
        return <Clock className="size-4" />;
      default:
        return null;
    }
  };

  const getStatusLabel = (status: string) => {
    switch (status) {
      case 'completed':
        return 'Completed';
      case 'downloading':
        return 'Active';
      case 'paused':
        return 'Incomplete';
      default:
        return status;
    }
  };

  const handleCopyLink = (url: string) => {
    navigator.clipboard.writeText(url);
    toast.success('Link copied to clipboard');
    setOpenMenuId(null);
  };

  const handleBulkDelete = () => {
    toast.success(`${selectedIds.size} items deleted`);
    setSelectedIds(new Set());
  };

  const handleBulkResume = () => {
    toast.success(`${selectedIds.size} downloads resumed`);
    setSelectedIds(new Set());
  };

  return (
    <PageTransition className="h-full w-full flex flex-col overflow-hidden relative">
      {/* Bulk Actions Bar - appears when items are selected */}
      {selectedIds.size > 0 && (
        <div className="absolute bottom-4 left-1/2 -translate-x-1/2 z-50 bg-primary text-primary-foreground px-4 py-2 rounded-full shadow-lg flex items-center gap-4">
          <span className="text-sm font-medium">{selectedIds.size} selected</span>
          <div className="flex items-center gap-2">
            <button
              onClick={handleBulkResume}
              className="p-1.5 hover:bg-primary-foreground/20 rounded-md transition-colors"
              title="Resume selected"
            >
              <Play className="size-4" />
            </button>
            <button
              onClick={handleBulkDelete}
              className="p-1.5 hover:bg-destructive/80 rounded-md transition-colors"
              title="Delete selected"
            >
              <Trash2 className="size-4" />
            </button>
            <button
              onClick={() => setSelectedIds(new Set())}
              className="p-1.5 hover:bg-primary-foreground/20 rounded-md transition-colors"
              title="Clear selection"
            >
              <X className="size-4" />
            </button>
          </div>
        </div>
      )}

      {/* Filters and View Switcher */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <div className="flex items-center gap-2">
          <button
            onClick={() => setFilter('all')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${filter === 'all'
              ? 'bg-primary text-primary-foreground'
              : 'bg-muted hover:bg-muted/80'
              }`}
          >
            All
          </button>
          <button
            onClick={() => setFilter('completed')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${filter === 'completed'
              ? 'bg-primary text-primary-foreground'
              : 'bg-muted hover:bg-muted/80'
              }`}
          >
            Completed
          </button>
          <button
            onClick={() => setFilter('incomplete')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${filter === 'incomplete'
              ? 'bg-primary text-primary-foreground'
              : 'bg-muted hover:bg-muted/80'
              }`}
          >
            Incomplete
          </button>
        </div>

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
        <div className="flex-1 overflow-auto">
          <table className="w-full">
            <thead className="sticky top-0 bg-muted/50 backdrop-blur-sm border-b border-border">
              <tr>
                <th className="w-16 px-3 py-3 text-left">
                  <div className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={selectedIds.size === filteredDownloads.length && filteredDownloads.length > 0}
                      onChange={toggleSelectAll}
                      className="rounded border-border w-4 h-4 cursor-pointer"
                    />
                    <span className="text-xs font-medium">#</span>
                  </div>
                </th>
                <th className="px-3 py-3 text-left text-sm font-medium">Name</th>
                <th className="w-20 px-3 py-3 text-left text-sm font-medium">Type</th>
                <th className="w-28 px-3 py-3 text-left text-sm font-medium">Status</th>
                <th className="w-16 px-3 py-3 text-left text-sm font-medium"></th>
              </tr>
            </thead>
            <tbody>
              {filteredDownloads.map((download, index) => (
                <tr
                  key={download.id}
                  className="border-b border-border hover:bg-muted/30 transition-colors group"
                >
                  <td className="px-3 py-3">
                    <div className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={selectedIds.has(download.id)}
                        onChange={() => toggleSelect(download.id)}
                        className="rounded border-border w-4 h-4 cursor-pointer"
                      />
                      <span className="text-xs text-muted-foreground">{index + 1}</span>
                    </div>
                  </td>
                  <td className="px-3 py-3">
                    <div className="flex flex-col min-w-0">
                      <span className="text-sm font-medium truncate">{truncateFilename(download.filename)}</span>
                      <span className="text-xs text-muted-foreground">{download.size ? formatSize(download.size) : 'Unknown'}</span>
                    </div>
                  </td>
                  <td className="px-3 py-3">
                    <span className="text-xs font-mono text-muted-foreground">{getFileExtension(download.filename)}</span>
                  </td>
                  <td className="px-3 py-3">
                    <div className={`flex items-center gap-1.5 ${getStatusColor(download.status)}`}>
                      {getStatusIcon(download.status)}
                      <span className="text-xs font-medium">{getStatusLabel(download.status)}</span>
                    </div>
                  </td>
                  <td className="px-3 py-3">
                    <div className="relative">
                      <button
                        onClick={() => setOpenMenuId(openMenuId === download.id ? null : download.id)}
                        className="p-1.5 hover:bg-muted rounded-md transition-colors opacity-0 group-hover:opacity-100"
                      >
                        <MoreVertical className="size-4" />
                      </button>

                      {openMenuId === download.id && (
                        <>
                          <div
                            className="fixed inset-0 z-[90]"
                            onClick={() => setOpenMenuId(null)}
                          />
                          <div className="absolute right-0 top-full mt-1 w-44 bg-popover border border-border rounded-md shadow-lg z-[95]">
                            <div className="py-1">
                              {download.status === 'completed' && (
                                <button className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors">
                                  <Download className="size-4" />
                                  Redownload
                                </button>
                              )}
                              {download.status === 'paused' && (
                                <button className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors">
                                  <Play className="size-4" />
                                  Resume
                                </button>
                              )}
                              {download.status === 'downloading' && (
                                <button className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors">
                                  <X className="size-4" />
                                  Cancel
                                </button>
                              )}
                              <button className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors">
                                <FolderOpen className="size-4" />
                                Open Location
                              </button>
                              <button
                                onClick={() => handleCopyLink(download.url)}
                                className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
                              >
                                <Copy className="size-4" />
                                Copy Link
                              </button>
                              <div className="border-t border-border my-1" />
                              <button className="w-full flex items-center gap-2 px-3 py-2 text-sm text-destructive hover:bg-destructive/10 transition-colors">
                                <Trash2 className="size-4" />
                                Delete
                              </button>
                            </div>
                          </div>
                        </>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {filteredDownloads.length === 0 && (
            <div className="flex items-center justify-center h-32">
              <p className="text-muted-foreground">No downloads found</p>
            </div>
          )}
        </div>
      )}

      {/* Grid View */}
      {view === 'grid' && (
        <div className="flex-1 overflow-auto p-4">
          {filteredDownloads.length === 0 ? (
            <div className="flex items-center justify-center h-32">
              <p className="text-muted-foreground">No downloads found</p>
            </div>
          ) : (
            <div className="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-6 lg:grid-cols-8 xl:grid-cols-10 gap-2">
              {filteredDownloads.map((download) => {
                const isCompleted = download.status === 'completed';
                const isIncomplete = download.status === 'paused' || download.status === 'downloading';
                const progressPercent = download.progress || 0;

                return (
                  <div
                    key={download.id}
                    className="relative group border border-border rounded-lg overflow-hidden hover:shadow-md transition-all"
                  >
                    {/* Progress bar background - solid color */}
                    <div
                      className={`absolute inset-0 ${isCompleted
                        ? 'bg-green-500/10'
                        : isIncomplete
                          ? 'bg-yellow-500/15'
                          : ''
                        }`}
                      style={
                        isIncomplete && !isCompleted
                          ? { width: `${progressPercent}%` }
                          : undefined
                      }
                    />

                    {/* Content wrapper */}
                    <div className="relative p-2">
                      {/* Selection checkbox - top left */}
                      <div className="absolute top-1 left-1 z-10">
                        <input
                          type="checkbox"
                          checked={selectedIds.has(download.id)}
                          onChange={() => toggleSelect(download.id)}
                          className="rounded border-border w-3.5 h-3.5 cursor-pointer"
                        />
                      </div>

                      {/* Three-dot menu - top right */}
                      <div className="absolute top-1 right-1 z-10">
                        <button
                          onClick={() => setOpenMenuId(openMenuId === download.id ? null : download.id)}
                          className="p-0.5 hover:bg-muted rounded-md transition-colors opacity-0 group-hover:opacity-100"
                        >
                          <MoreVertical className="size-3" />
                        </button>

                        {openMenuId === download.id && (
                          <>
                            <div
                              className="fixed inset-0 z-[90]"
                              onClick={() => setOpenMenuId(null)}
                            />
                            <div className="absolute right-0 top-full mt-1 w-44 bg-popover border border-border rounded-md shadow-lg z-[95]">
                              <div className="py-1">
                                {download.status === 'completed' && (
                                  <button className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors">
                                    <Download className="size-4" />
                                    Redownload
                                  </button>
                                )}
                                {download.status === 'paused' && (
                                  <button className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors">
                                    <Play className="size-4" />
                                    Resume
                                  </button>
                                )}
                                {download.status === 'downloading' && (
                                  <button className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors">
                                    <X className="size-4" />
                                    Cancel
                                  </button>
                                )}
                                <button className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors">
                                  <FolderOpen className="size-4" />
                                  Open Location
                                </button>
                                <button
                                  onClick={() => handleCopyLink(download.url)}
                                  className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-accent transition-colors"
                                >
                                  <Copy className="size-4" />
                                  Copy Link
                                </button>
                                <div className="border-t border-border my-1" />
                                <button className="w-full flex items-center gap-2 px-3 py-2 text-sm text-destructive hover:bg-destructive/10 transition-colors">
                                  <Trash2 className="size-4" />
                                  Delete
                                </button>
                              </div>
                            </div>
                          </>
                        )}
                      </div>

                      {/* Content */}
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

                        {/* Size */}
                        <p className="text-[10px] text-muted-foreground">
                          {download.size ? formatSize(download.size) : 'Unknown'}
                        </p>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}
    </PageTransition>
  );
}
