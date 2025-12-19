import { useState, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { PanelRightClose, LayoutList, Pause, Play, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useDownloads } from '@/hooks/useDownloads';

interface SidebarProps {
  open: boolean;
  onClose: () => void;
  width: number;
  onWidthChange: (width: number) => void;
  position?: 'left' | 'right';
}

export default function Sidebar({ open, onClose, width, onWidthChange, position = 'left' }: SidebarProps) {
  const navigate = useNavigate();
  const [isResizing, setIsResizing] = useState(false);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const sidebarRef = useRef<HTMLDivElement>(null);

  // Use real downloads from hook
  const { downloads: allDownloads, pauseDownload, cancelDownload, resumeDownloads } = useDownloads();

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;

      let newWidth: number;
      if (position === 'left') {
        newWidth = e.clientX;
      } else {
        newWidth = window.innerWidth - e.clientX;
      }

      const maxWidth = window.innerWidth * 0.8;
      if (newWidth >= 200 && newWidth <= maxWidth) {
        onWidthChange(newWidth);
      }
    };

    const handleMouseUp = () => {
      setIsResizing(false);
    };

    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isResizing, onWidthChange]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          ref={sidebarRef}
          initial={{ width: 0, opacity: 0 }}
          animate={{
            width: isResizing ? width : width,
            opacity: 1,
            transition: isResizing ? { duration: 0 } : { duration: 0.3, ease: "easeOut" }
          }}
          exit={{ width: 0, opacity: 0, transition: { duration: 0.25, ease: "easeIn" } }}
          className={`h-full bg-sidebar flex flex-col overflow-hidden relative ${position === 'left' ? 'border-r' : 'border-l'
            } border-sidebar-border`}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-sidebar-border shrink-0">
            {/* Detail View button */}
            <button
              onClick={() => {
                navigate('/detail');
                onClose();
              }}
              className="flex items-center gap-2 px-3 py-1.5 rounded-md text-sm font-medium hover:bg-sidebar-accent transition-colors"
              title="Open detail page"
            >
              <LayoutList className="size-4" />
              <span>Detail View</span>
            </button>

            {/* Close button */}
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={onClose}
              title="Close sidebar"
            >
              <PanelRightClose className={`size-5 ${position === 'left' ? 'scale-x-[-1]' : ''}`} />
            </Button>
          </div>

          {/* Content - Download List */}
          <div className="flex-1 overflow-y-auto overflow-x-hidden">
            {allDownloads.map((download) => (
              <div
                key={download.id}
                onClick={() => {
                  // Dispatch event for Home page to select this download
                  window.dispatchEvent(new CustomEvent('select-download', { detail: { id: download.id } }));
                  navigate('/');
                }}
                onMouseEnter={() => setHoveredId(download.id)}
                onMouseLeave={() => setHoveredId(null)}
                className="px-3 py-2 hover:bg-sidebar-accent cursor-pointer transition-colors border-b border-sidebar-border/50"
              >
                {/* Top row: Filename and Percentage/Actions */}
                <div className="flex items-center justify-between gap-2 mb-1">
                  {/* Filename - truncated */}
                  <p className="text-xs font-medium truncate flex-1 min-w-0" title={download.filename}>
                    {download.filename}
                  </p>

                  {/* Progress percentage or Action buttons on hover */}
                  <div className="relative shrink-0 flex items-center justify-end w-14">
                    {/* Progress percentage - hidden on hover */}
                    <span
                      className={`text-xs font-semibold text-primary transition-opacity duration-200 ${hoveredId === download.id ? 'opacity-0' : 'opacity-100'
                        }`}
                    >
                      {Math.round(download.progress)}%
                    </span>

                    {/* Action Buttons - shown on hover */}
                    <div
                      className={`absolute right-0 flex items-center gap-1 transition-opacity duration-200 ${hoveredId === download.id ? 'opacity-100' : 'opacity-0 pointer-events-none'
                        }`}
                    >
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          if (download.status === 'downloading') {
                            pauseDownload(download.id);
                          } else if (download.status === 'paused') {
                            resumeDownloads([download.id]);
                          }
                        }}
                        className="p-1 hover:bg-sidebar rounded transition-colors"
                        title={download.status === 'downloading' ? 'Pause' : 'Resume'}
                      >
                        {download.status === 'downloading' ? (
                          <Pause className="size-3.5" />
                        ) : download.status === 'paused' ? (
                          <Play className="size-3.5" />
                        ) : null}
                      </button>
                      {download.status !== 'completed' && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            cancelDownload(download.id);
                          }}
                          className="p-1 hover:bg-destructive/10 hover:text-destructive rounded transition-colors"
                          title="Cancel"
                        >
                          <X className="size-3.5" />
                        </button>
                      )}
                    </div>
                  </div>
                </div>

                {/* Progress Bar - below filename */}
                <div className="relative w-full h-1.5 bg-muted/40 rounded-full overflow-hidden">
                  <div
                    className="absolute inset-y-0 left-0 bg-primary transition-all duration-300 rounded-full"
                    style={{ width: `${download.progress}%` }}
                  />
                </div>
              </div>
            ))}
          </div>

          {/* Resize handle */}
          <div
            className={`absolute top-0 ${position === 'left' ? 'right-0' : 'left-0'} w-1 h-full cursor-col-resize hover:bg-primary/50`}
            onMouseDown={() => setIsResizing(true)}
          />
        </motion.div>
      )}
    </AnimatePresence>
  );
}
