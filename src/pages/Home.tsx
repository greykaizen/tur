import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useLocation } from 'react-router-dom';
import PageTransition from '@/components/PageTransition';
import { CompletionDialog } from '@/components/CompletionDialog';
import { Paperclip, Download, X, Play, Pause, FolderOpen, ChevronDown } from 'lucide-react';
import { useSettings } from '@/contexts/SettingsContext';
import { useDownloadSelection } from '@/contexts/DownloadSelectionContext';
import { useDownloads, formatSize, formatSpeed, formatTimeLeft } from '@/hooks/useDownloads';
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
// import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
// import { emit } from '@tauri-apps/api/event';
// import { DownloadOptionsDialog } from "@/components/DownloadOptionsDialog";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';


type FinishAction = 'none' | 'open_file' | 'open_folder' | 'shutdown_app' | 'shutdown_system' | 'sleep';

export default function Home() {
  const location = useLocation();
  // Get selected download state from context (single source of truth)
  const { selectedDownload, setSelectedDownloadId, showWelcomeScreen } = useDownloadSelection();

  // Handle navigation from Detail page
  useEffect(() => {
    if (location.state?.download?.id) {
      setSelectedDownloadId(location.state.download.id);
      // Clear state to avoid re-selecting on re-renders if desired, 
      // but react-router state is persistent so this is fine for now.
      window.history.replaceState({}, document.title);
    }
  }, [location.state, setSelectedDownloadId]);

  // Empty state input handling
  const [urlTags, setUrlTags] = useState<string[]>([]);
  const [inputValue, setInputValue] = useState('');
  const [isDragging, setIsDragging] = useState(false);

  // const [showOptionsDialog, setShowOptionsDialog] = useState(false); // Removed
  const inputRef = useRef<HTMLInputElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);



  const { settings, ready } = useSettings();
  const showDownloadProgress = ready ? settings.app.show_download_progress : true;
  const showSegmentProgress = ready ? settings.app.show_segment_progress : true;

  // Get downloads for actions (startDownloads, pauseDownload, cancelDownload, resumeDownloads)
  const { downloads, pauseDownload, cancelDownload, resumeDownloads } = useDownloads();

  // Completion timer state
  const [completionTimer, setCompletionTimer] = useState<number | null>(null);
  const completionTimerRef = useRef<NodeJS.Timeout | null>(null);
  const completionDisplayDuration = ready ? settings.app.completion_display_duration : 5;
  const completionAction = ready ? settings.app.completion_action : 'popup';

  // Completed download for popup
  const [completedDownload, setCompletedDownload] = useState<{
    filename: string;
    size: number | null;
    destination: string;
  } | null>(null);

  // Per-download finish action (do nothing, open file, open folder)
  const [finishAction, setFinishAction] = useState<FinishAction>('none');

  // Track downloads that have already shown completion (to avoid re-triggering)
  const completionShownRef = useRef<Set<string>>(new Set());

  // Detect download completion and start timer (only for new completions)
  useEffect(() => {
    if (selectedDownload?.status === 'completed' || (selectedDownload && selectedDownload.progress >= 100)) {
      // Only start timer if this download hasn't shown completion yet
      if (!completionShownRef.current.has(selectedDownload.id)) {
        if (completionTimer === null) {
          if (completionDisplayDuration > 0) {
            setCompletionTimer(completionDisplayDuration);
          } else {
            // Duration is 0 = immediate popup
            setCompletionTimer(0);
          }
        }
      }
    } else {
      // Reset timer if download is not completed
      setCompletionTimer(null);
      if (completionTimerRef.current) {
        clearInterval(completionTimerRef.current);
        completionTimerRef.current = null;
      }
    }
  }, [selectedDownload?.status, selectedDownload?.progress, selectedDownload?.id, completionDisplayDuration, completionTimer]);

  // Countdown timer effect
  useEffect(() => {
    if (completionTimer !== null && completionTimer > 0) {
      completionTimerRef.current = setTimeout(() => {
        setCompletionTimer(prev => (prev !== null ? prev - 1 : null));
      }, 1000);
    } else if (completionTimer === 0 && selectedDownload) {
      // Timer expired - mark as shown to prevent re-triggering
      completionShownRef.current.add(selectedDownload.id);

      // Trigger popup or notification based on setting
      if (completionAction === 'popup') {
        setCompletedDownload({
          filename: selectedDownload.filename,
          size: selectedDownload.size,
          destination: selectedDownload.destination,
        });
      }
      // TODO: Handle 'notification' action with Tauri notification API

      const nextDownload = downloads.find(d =>
        d.status !== 'completed' && d.progress < 100 && d.id !== selectedDownload.id
      );
      setSelectedDownloadId(nextDownload?.id || null);
      setCompletionTimer(null);
    }

    return () => {
      if (completionTimerRef.current) {
        clearTimeout(completionTimerRef.current);
      }
    };
  }, [completionTimer, downloads, selectedDownload, setSelectedDownloadId, completionAction]);

  // Focus input in empty state
  useEffect(() => {
    if (!selectedDownload && inputRef.current) {
      inputRef.current.focus();
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

  const handleDownload = async () => {
    const allUrls = [...urlTags];
    if (inputValue.trim()) {
      allUrls.push(inputValue.trim());
    }

    // Only open if we have URLs
    if (allUrls.length === 0) return;

    try {
      await invoke('open_download_options_window', { urls: allUrls });

      // Clear input from Home immediately
      setUrlTags([]);
      setInputValue('');
    } catch (error) {
      console.error("Failed to open download options window", error);
    }
  };

  // handleConfirmDownload removed

  // Empty State - No download selected
  if (!selectedDownload) {
    // Show Input UI (Welcome Screen OR if User interacted via Drag/Drop/Type)
    if (showWelcomeScreen || urlTags.length > 0 || inputValue) {
      return (
        <PageTransition
          className="h-full w-full overflow-hidden relative"
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          {/* Content */}
          <div
            className="relative h-full flex flex-col px-4 pt-8"
          >
            {/* Logo + Name at Top - Golden Ratio Sizing */}
            <div className="flex items-center justify-center gap-3 mb-4">
              <img src="/icon.png" alt="tur logo" className="w-[54px] h-[54px]" />
              <h1
                className="text-7xl tracking-tight"
                style={{
                  fontFamily: "'Margin', sans-serif",
                }}
              >
                tur
              </h1>
            </div>

            {/* Input Field - Right below logo */}
            <div className="w-full max-w-md mx-auto">
              <div className={`transition-all ${isDragging ? 'scale-105' : ''}`}>
                <div className={`bg-card/80 backdrop-blur-sm border-2 rounded-xl shadow-xl transition-all ${isDragging ? 'border-blue-500' : 'border-border'}`}>
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
          {/* Dialog Removed */}
        </PageTransition>
      );
    }

    // In-App Empty State - Simple wireframe style
    return (
      <PageTransition
        className="h-full w-full overflow-hidden relative"
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        <div
          className="relative h-full flex flex-col items-center pt-5 p-4"
        >
          <div className={`text-center transition-all ${isDragging ? 'scale-105' : ''}`}>
            <p className="text-muted-foreground text-sm leading-relaxed">
              Start a New Download by<br />
              clicking on Add button<br />
              or drag & drop a file here
            </p>
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
    <>
      <PageTransition className="h-full w-full overflow-hidden relative">
        {/* Opaque Progress Background */}
        <div
          className="absolute inset-0 bg-green-500/5 transition-all duration-300"
          style={{ width: `${selectedDownload.progress}%` }}
        />

        <div className="relative h-full flex flex-col p-4">
          <Tabs defaultValue="status" className="flex-1 flex flex-col space-y-3">

            {/* Tabs List (Compact) - Now at Top */}
            <div>
              <TabsList className="h-8 p-0 bg-muted/30 gap-1 rounded-md px-1 w-auto inline-flex">
                <TabsTrigger value="status" className="h-6 text-xs px-3 data-[state=active]:bg-background data-[state=active]:shadow-sm">Status</TabsTrigger>
                <TabsTrigger value="options" className="h-6 text-xs px-3 data-[state=active]:bg-background data-[state=active]:shadow-sm">Options</TabsTrigger>
                <TabsTrigger value="speed" className="h-6 text-xs px-3 data-[state=active]:bg-background data-[state=active]:shadow-sm">Speed</TabsTrigger>
              </TabsList>
            </div>

            {/* Tab Contents */}
            <div className="flex-1 relative min-h-0">
              <TabsContent value="status" className="mt-0 space-y-4 h-full">
                {/* Download Header - Only in Status tab */}
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1 min-w-0 space-y-1">
                    <h2 className="text-lg font-semibold truncate">{selectedDownload.filename}</h2>
                    <p className="text-xs text-muted-foreground truncate">{selectedDownload.url}</p>
                  </div>
                  {/* Percentage (Right) */}
                  <div className="shrink-0 bg-muted/40 px-4 py-2 rounded-lg border border-green-500/20 bg-green-500/5">
                    <span className="text-xl font-bold text-green-600 dark:text-green-500">
                      {Math.round(selectedDownload.progress)}%
                    </span>
                  </div>
                </div>

                <div className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm max-w-md">
                  <span className="font-medium text-muted-foreground">Size:</span>
                  <span className="text-foreground">
                    {selectedDownload.size ? formatSize(selectedDownload.size) : 'Unknown'}
                    <span className="text-muted-foreground ml-1">
                      ({formatSize(selectedDownload.downloaded)})
                    </span>
                  </span>

                  <span className="font-medium text-muted-foreground">Speed:</span>
                  <span className="text-foreground">{formatSpeed(selectedDownload.speed)}</span>

                  <span className="font-medium text-muted-foreground">Time Left:</span>
                  <span className="text-foreground">{formatTimeLeft(selectedDownload.downloaded, selectedDownload.size || 0, selectedDownload.speed, selectedDownload.id)}</span>

                  <span className="font-medium text-muted-foreground">Connections:</span>
                  <span className="text-foreground">
                    {selectedDownload.num_connections > 1 ? `${selectedDownload.num_connections} threads` : 'Single'}
                  </span>

                  <span className="font-medium text-muted-foreground">Resume:</span>
                  <span className={selectedDownload.resume_supported ? "text-foreground" : "text-red-400/70"}>
                    {selectedDownload.resume_supported ? 'Yes' : 'No'}
                  </span>
                </div>
              </TabsContent>

              <TabsContent value="options" className="mt-0 space-y-4 pt-2">
                <div className="flex flex-col gap-2">
                  <span className="text-sm font-medium text-muted-foreground">On Completion:</span>
                  <DropdownMenu>
                    <DropdownMenuTrigger className="flex w-full justify-between items-center gap-2 text-sm px-3 py-2 rounded-md border border-input bg-transparent hover:bg-accent hover:text-accent-foreground transition-colors">
                      <span>
                        {finishAction === 'none' && 'Do Nothing'}
                        {finishAction === 'open_file' && 'Open File'}
                        {finishAction === 'open_folder' && 'Open Folder'}
                        {finishAction === 'shutdown_app' && 'Shutdown App'}
                        {finishAction === 'shutdown_system' && 'Shutdown System'}
                        {finishAction === 'sleep' && 'Sleep'}
                      </span>
                      <ChevronDown className="h-4 w-4 opacity-50" />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start" className="w-[var(--radix-dropdown-menu-trigger-width)]">
                      <DropdownMenuItem onSelect={() => setFinishAction('none')}>
                        Do Nothing
                      </DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => setFinishAction('open_file')}>
                        Open File
                      </DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => setFinishAction('open_folder')}>
                        Open Folder
                      </DropdownMenuItem>
                      <div className="h-px bg-border my-1" />
                      <DropdownMenuItem onSelect={() => setFinishAction('shutdown_app')}>
                        Shutdown App
                      </DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => setFinishAction('shutdown_system')}>
                        Shutdown System
                      </DropdownMenuItem>
                      <DropdownMenuItem onSelect={() => setFinishAction('sleep')}>
                        Sleep
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                  <p className="text-xs text-muted-foreground">
                    Action to perform when this download finishes.
                  </p>
                </div>
              </TabsContent>

              <TabsContent value="speed" className="mt-0 space-y-4 pt-2">
                <div className="flex flex-col gap-2">
                  <span className="text-sm font-medium text-muted-foreground">Speed Limit:</span>
                  <DropdownMenu>
                    <DropdownMenuTrigger className="flex w-full justify-between items-center gap-2 text-sm px-3 py-2 rounded-md border border-input bg-transparent hover:bg-accent hover:text-accent-foreground transition-colors">
                      <span>Unlimited</span>
                      <ChevronDown className="h-4 w-4 opacity-50" />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start" className="w-[var(--radix-dropdown-menu-trigger-width)]">
                      <DropdownMenuItem>Unlimited</DropdownMenuItem>
                      <div className="h-px bg-border my-1" />
                      <DropdownMenuItem>10 MB/s</DropdownMenuItem>
                      <DropdownMenuItem>5 MB/s</DropdownMenuItem>
                      <DropdownMenuItem>2 MB/s</DropdownMenuItem>
                      <DropdownMenuItem>1 MB/s</DropdownMenuItem>
                      <DropdownMenuItem>512 KB/s</DropdownMenuItem>
                      <DropdownMenuItem>256 KB/s</DropdownMenuItem>
                      <DropdownMenuItem>128 KB/s</DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                  <p className="text-xs text-muted-foreground">
                    Limit download speed for this file.
                  </p>
                </div>
              </TabsContent>
            </div>
          </Tabs>

          {/* Action Buttons - Above progress bars */}
          <div className="flex justify-end gap-2">
            {selectedDownload.status === 'completed' || selectedDownload.progress >= 100 ? (
              // Completed: Show Timer, Open Folder, and Close
              <>
                {/* Circular Countdown Timer - Above buttons */}
                {completionTimer !== null && completionDisplayDuration > 0 && (
                  <div className="relative flex items-center justify-center w-9 h-9">
                    <svg className="w-9 h-9 -rotate-90" viewBox="0 0 32 32">
                      <circle
                        cx="16"
                        cy="16"
                        r="13"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="3"
                        className="text-muted/20"
                      />
                      <circle
                        cx="16"
                        cy="16"
                        r="13"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="3"
                        strokeDasharray={82}
                        strokeDashoffset={82 - (82 * completionTimer / completionDisplayDuration)}
                        strokeLinecap="round"
                        className="text-white transition-all duration-1000"
                      />
                    </svg>
                    <span className="absolute text-xs font-semibold text-foreground">
                      {completionTimer}
                    </span>
                  </div>
                )}
                <button className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors">
                  <FolderOpen className="size-3.5" />
                  <span>Open Folder</span>
                </button>
                <button
                  onClick={() => {
                    // Find the next active download or clear selection
                    const activeDownload = downloads.find((d) =>
                      d.status !== 'completed' && d.progress < 100 && d.id !== selectedDownload.id
                    );
                    setSelectedDownloadId(activeDownload?.id || null);
                    setCompletionTimer(null);
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
                  <button
                    onClick={() => pauseDownload(selectedDownload.id)}
                    className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
                  >
                    <Pause className="size-3.5" />
                    <span>Pause</span>
                  </button>
                ) : (
                  <button
                    onClick={() => resumeDownloads([selectedDownload.id])}
                    className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
                  >
                    <Play className="size-3.5" />
                    <span>Resume</span>
                  </button>
                )}
                <button
                  onClick={() => {
                    cancelDownload(selectedDownload.id);
                    setSelectedDownloadId(null);
                  }}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-destructive text-destructive hover:bg-destructive/10 transition-colors"
                >
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
      </PageTransition >
      <CompletionDialog
        open={completedDownload !== null}
        onClose={() => setCompletedDownload(null)}
        download={completedDownload}
      />
    </>
  );
}
