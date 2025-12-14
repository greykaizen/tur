import { useSettings } from '@/contexts/SettingsContext';

interface DownloadItemProps {
  url: string;
  size: string;
  downloaded: string;
  speed: string;
  timeLeft: string;
  resume: boolean;
  progress: number;
  segments?: { start: number; end: number }[]; // Segment ranges for connection visualization
}

export default function DownloadItem({
  url,
  size: _size,
  downloaded: _downloaded,
  speed: _speed,
  timeLeft: _timeLeft,
  resume: _resume,
  progress,
  segments = []
}: DownloadItemProps) {
  const { settings, ready } = useSettings();
  const showDownloadProgress = ready ? settings.app.show_download_progress : true;
  const showSegmentProgress = ready ? settings.app.show_segment_progress : true;

  return (
    <div className="w-full bg-muted/20 rounded-lg p-3 space-y-2">
      {/* Top section: URL and percentage */}
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs text-muted-foreground truncate flex-1">{url}</p>
        <div className="shrink-0 bg-muted/40 pl-3 pr-2 py-1.5 rounded-l-md border-r-4 border-green-500">
          <span className="text-base font-semibold text-green-600 dark:text-green-500">{progress}%</span>
        </div>
      </div>

      {/* Download info - single column layout */}
      <div className="space-y-0.5 text-sm">
        <div className="flex gap-1">
          <span className="font-medium">Size</span>
        </div>
        <div className="flex gap-1">
          <span className="font-medium">Downloaded</span>
        </div>
        <div className="flex gap-1">
          <span className="font-medium">Speed</span>
        </div>
        <div className="flex gap-1">
          <span className="font-medium">Time Left</span>
        </div>
        <div className="flex gap-1">
          <span className="font-medium">Resume</span>
        </div>
      </div>

      {/* Progress bars section */}
      <div className="space-y-1.5 pt-1">
        {/* Main progress bar */}
        {showDownloadProgress && (
          <div className="relative w-full h-5 bg-muted/40 border border-border rounded-md overflow-hidden">
            <div 
              className="absolute inset-y-0 left-0 bg-green-500/50 dark:bg-green-500/40 transition-all duration-300"
              style={{ width: `${progress}%` }}
            />
          </div>
        )}

        {/* Segment visualization bar */}
        {showSegmentProgress && (
          <div className="relative w-full h-5 bg-muted/40 border border-border rounded-md overflow-hidden flex gap-1 p-1">
            {segments.length > 0 ? (
              segments.map((segment, index) => (
                <div
                  key={index}
                  className="bg-blue-400/70 dark:bg-blue-400/60 rounded-sm"
                  style={{ 
                    width: `${((segment.end - segment.start) / 100) * 100}%`,
                    marginLeft: index === 0 ? `${segment.start}%` : '0'
                  }}
                />
              ))
            ) : (
              // Default segment visualization when no segments provided
              <>
                <div className="bg-blue-400/70 dark:bg-blue-400/60 rounded-sm h-full" style={{ width: '6%' }} />
                <div className="bg-blue-400/70 dark:bg-blue-400/60 rounded-sm h-full" style={{ width: '5%' }} />
                <div className="bg-blue-400/70 dark:bg-blue-400/60 rounded-sm h-full" style={{ width: '4%' }} />
                <div className="flex-1" />
                <div className="bg-blue-400/70 dark:bg-blue-400/60 rounded-sm h-full" style={{ width: '4%' }} />
                <div className="flex-1" />
                <div className="bg-blue-400/70 dark:bg-blue-400/60 rounded-sm h-full" style={{ width: '8%' }} />
                <div className="flex-1" />
                <div className="bg-blue-400/70 dark:bg-blue-400/60 rounded-sm h-full" style={{ width: '6%' }} />
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
