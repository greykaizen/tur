import { useState, useRef, useEffect } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import PageTransition from '@/components/PageTransition';
import { Paperclip, Download, X, Play, Pause, FolderOpen } from 'lucide-react';
import { useSettings } from '@/contexts/SettingsContext';
import { useDownloads } from '@/hooks/useDownloads';

export default function Home() {
  const location = useLocation();
  const navigate = useNavigate();
  const selectedDownload = location.state?.download;

  // Empty state input handling
  const [urlTags, setUrlTags] = useState<string[]>([]);
  const [inputValue, setInputValue] = useState('');
  const [isDragging, setIsDragging] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { settings, ready } = useSettings();
  const showDownloadProgress = ready ? settings.app.show_download_progress : true;
  const showSegmentProgress = ready ? settings.app.show_segment_progress : true;

  // Notify Layout about empty state - MUST be outside conditional
  useEffect(() => {
    if (!selectedDownload) {
      window.dispatchEvent(new CustomEvent('home-empty-state', { detail: { isEmpty: true } }));
      if (inputRef.current) {
        inputRef.current.focus();
      }
      return () => {
        window.dispatchEvent(new CustomEvent('home-empty-state', { detail: { isEmpty: false } }));
      };
    } else {
      window.dispatchEvent(new CustomEvent('home-empty-state', { detail: { isEmpty: false } }));
    }
  }, [selectedDownload]);

  // Empty state handlers
  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);

    const files = Array.from(e.dataTransfer.files);
    if (files.length > 0) {
      const file = files[0];
      const reader = new FileReader();
      reader.onload = (event) => {
        const content = event.target?.result as string;
        const urls = content.split(/[,\n]/).map(u => u.trim()).filter(u => u);
        setUrlTags(prev => [...prev, ...urls]);
      };
      reader.readAsText(file);
    } else if (e.dataTransfer.getData('text')) {
      const droppedText = e.dataTransfer.getData('text').trim();
      if (droppedText) {
        setUrlTags(prev => [...prev, droppedText]);
      }
    }
  };

  const handleFileSelect = () => {
    fileInputRef.current?.click();
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onload = (event) => {
        const content = event.target?.result as string;
        const urls = content.split(/[,\n]/).map(u => u.trim()).filter(u => u);
        setUrlTags(prev => [...prev, ...urls]);
      };
      reader.readAsText(file);
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;

    if (value.includes(',')) {
      const parts = value.split(',');
      const newTags = parts.slice(0, -1).map(p => p.trim()).filter(p => p);
      if (newTags.length > 0) {
        setUrlTags(prev => [...prev, ...newTags]);
      }
      setInputValue(parts[parts.length - 1].trim());
    } else {
      setInputValue(value);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (inputValue.trim()) {
        setUrlTags(prev => [...prev, inputValue.trim()]);
        setInputValue('');
      } else if (urlTags.length > 0) {
        handleDownload();
      }
    } else if (e.key === 'Backspace' && !inputValue && urlTags.length > 0) {
      setUrlTags(prev => prev.slice(0, -1));
    }
  };

  const removeTag = (index: number) => {
    setUrlTags(prev => prev.filter((_, i) => i !== index));
  };

  // Use real downloads hook
  const { downloads, startDownloads } = useDownloads();

  const handleDownload = async () => {
    const allUrls = [...urlTags];
    if (inputValue.trim()) {
      allUrls.push(inputValue.trim());
    }

    if (allUrls.length === 0) return;

    // Call backend to start downloads
    await startDownloads(allUrls);

    setUrlTags([]);
    setInputValue('');
  };

  // Empty State - No download selected
  if (!selectedDownload) {
    return (
      <PageTransition className="h-full w-full overflow-hidden relative">
        {/* Content */}
        <div
          className="relative h-full flex flex-col px-4 pt-8"
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          {/* Logo + Name at Top - Golden Ratio Sizing */}
          <div className="flex items-center justify-center gap-3 mb-4">
            <img src="/icon.png" alt="tur logo" className="mb-2.5 w-[54px] h-[54px]" />
            <h1
              className="text-5xl"
              style={{
                fontFamily: "'Modak', cursive",
                WebkitTextStroke: '1.5px',
                WebkitTextFillColor: 'transparent',
                letterSpacing: '0.05em'
              }}
            >
              tur
            </h1>
          </div>

          {/* Input Field - Right below logo */}
          <div className="w-full max-w-md mx-auto">
            <div className={`transition-all ${isDragging ? 'scale-105' : ''
              }`}>
              <div className={`bg-card/80 backdrop-blur-sm border-2 rounded-xl shadow-xl transition-all ${isDragging ? 'border-blue-500' : 'border-border'
                }`}>
                <div className="flex items-start gap-2 px-3 py-2">
                  {/* Tag input field */}
                  <div className="flex-1 min-w-0 max-h-[60px] overflow-y-auto">
                    <div className="flex flex-wrap gap-1.5 items-center">
                      {/* URL Tags */}
                      {urlTags.map((url, index) => (
                        <div
                          key={index}
                          className="inline-flex items-center gap-1 bg-blue-600/10 text-blue-600 dark:text-blue-400 px-2 py-0.5 rounded-md text-xs"
                        >
                          <span className="max-w-[180px] truncate">{url}</span>
                          <button
                            onClick={() => removeTag(index)}
                            className="hover:bg-blue-600/20 rounded-sm p-0.5"
                          >
                            <X className="h-2.5 w-2.5" />
                          </button>
                        </div>
                      ))}

                      {/* Input field */}
                      <input
                        ref={inputRef}
                        type="text"
                        value={inputValue}
                        onChange={handleInputChange}
                        onKeyDown={handleKeyDown}
                        placeholder={urlTags.length === 0 ? "Enter URL or drag & drop file" : ""}
                        className="flex-1 min-w-[100px] bg-transparent text-sm focus:outline-none py-1"
                      />
                    </div>
                  </div>

                  {/* File browser button */}
                  <button
                    onClick={handleFileSelect}
                    className="p-1.5 hover:bg-muted rounded-md transition-colors shrink-0"
                    title="Browse File"
                  >
                    <Paperclip className="h-4 w-4 text-muted-foreground" />
                  </button>

                  {/* Download button */}
                  <button
                    onClick={handleDownload}
                    disabled={urlTags.length === 0 && !inputValue.trim()}
                    className="p-1.5 rounded-full bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors shrink-0"
                    title="Download"
                  >
                    <Download className="h-4 w-4 text-white" />
                  </button>
                </div>
              </div>

              {/* Helper text */}
              <p className="text-center text-xs text-muted-foreground mt-3">
                Paste URLs separated by commas or press Enter after each URL
              </p>
            </div>
          </div>

          {/* Hidden file input */}
          <input
            ref={fileInputRef}
            type="file"
            accept=".txt,.csv"
            onChange={handleFileChange}
            className="hidden"
          />
        </div>
      </PageTransition>
    );
  }

  // Download View State - Showing selected download details
  return (
    <PageTransition className="h-full w-full overflow-hidden relative">
      {/* Opaque Progress Background */}
      <div
        className="absolute inset-0 bg-green-500/5 transition-all duration-300"
        style={{ width: `${selectedDownload.progress}%` }}
      />

      <div className="relative h-full flex flex-col p-4 space-y-4">
        {/* Download Header */}
        <div className="space-y-3">
          <div className="flex items-start justify-between gap-3">
            <div className="flex-1 min-w-0 space-y-2">
              <h2 className="text-lg font-semibold truncate">{selectedDownload.filename}</h2>
              <p className="text-xs text-muted-foreground truncate">{selectedDownload.url}</p>

              {/* Stats - Aligned values */}
              <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm max-w-md">
                <span className="font-medium text-muted-foreground">Size:</span>
                <span className="text-foreground">{selectedDownload.size}</span>

                <span className="font-medium text-muted-foreground">Downloaded:</span>
                <span className="text-foreground">{selectedDownload.downloaded}</span>

                <span className="font-medium text-muted-foreground">Speed:</span>
                <span className="text-foreground">{selectedDownload.speed}</span>

                <span className="font-medium text-muted-foreground">Time Left:</span>
                <span className="text-foreground">{selectedDownload.timeLeft}</span>
              </div>
            </div>

            {/* Percentage Badge - Rounded on left, square on right */}
            <div className="shrink-0 bg-muted/40 px-4 py-2 rounded-l-lg border-r-4 border-green-500">
              <span className="text-2xl font-bold text-green-600 dark:text-green-500">
                {selectedDownload.progress}%
              </span>
            </div>
          </div>
        </div>

        {/* Action Buttons - Above progress bars */}
        <div className="flex justify-end gap-2">
          {selectedDownload.status === 'completed' || selectedDownload.progress === 100 ? (
            // Completed: Show Open Folder and Close buttons
            <>
              <button className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors">
                <FolderOpen className="size-3.5" />
                <span>Open Folder</span>
              </button>
              <button
                onClick={() => {
                  // Find the next active download
                  const activeDownload = downloads.find((d) =>
                    d.status !== 'completed' && d.progress < 100 && d.id !== selectedDownload.id
                  );

                  if (activeDownload) {
                    navigate('/', { state: { download: activeDownload }, replace: true });
                  } else {
                    navigate('/', { replace: true });
                  }
                }}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-border hover:bg-muted transition-colors"
              >
                <X className="size-3.5" />
                <span>Close</span>
              </button>
            </>
          ) : (
            // Downloading/Paused: Show Pause/Resume and Cancel buttons
            <>
              {selectedDownload.status === 'downloading' ? (
                <button className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors">
                  <Pause className="size-3.5" />
                  <span>Pause</span>
                </button>
              ) : (
                <button className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors">
                  <Play className="size-3.5" />
                  <span>Resume</span>
                </button>
              )}
              <button className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-destructive text-destructive hover:bg-destructive/10 transition-colors">
                <X className="size-3.5" />
                <span>Cancel</span>
              </button>
            </>
          )}
        </div>

        {/* Progress Bars - Hidden when completed, less rounded */}
        {(selectedDownload.status !== 'completed' && selectedDownload.progress < 100) && (
          <div className="space-y-3">
            {showDownloadProgress && (
              <div className="relative w-full h-6 bg-muted/40 border border-border rounded-sm overflow-hidden">
                <div
                  className="absolute inset-y-0 left-0 bg-green-500/50 dark:bg-green-500/40 transition-all duration-300"
                  style={{ width: `${selectedDownload.progress}%` }}
                />
              </div>
            )}

            {showSegmentProgress && selectedDownload.segments && selectedDownload.segments.length > 0 && (
              <div className="relative w-full h-6 bg-muted/40 border border-border rounded-sm overflow-hidden">
                {selectedDownload.segments.map((segment: any, index: number) => (
                  <div
                    key={index}
                    className="absolute inset-y-0 bg-blue-400/70 dark:bg-blue-400/60"
                    style={{
                      left: `${segment.start}%`,
                      width: `${segment.end - segment.start}%`
                    }}
                  />
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </PageTransition>
  );
}
