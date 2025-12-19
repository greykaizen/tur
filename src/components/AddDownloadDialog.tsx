import { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Paperclip, Download, X } from 'lucide-react';
import { useDownloads } from '@/hooks/useDownloads';

interface AddDownloadDialogProps {
  open: boolean;
  onClose: () => void;
}

export default function AddDownloadDialog({ open, onClose }: AddDownloadDialogProps) {
  const [urlTags, setUrlTags] = useState<string[]>([]);
  const [inputValue, setInputValue] = useState('');
  const [isDragging, setIsDragging] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { startDownloads } = useDownloads();

  useEffect(() => {
    if (open && inputRef.current) {
      inputRef.current.focus();
    }
  }, [open]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) {
        handleClose();
      }
    };
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [open]);

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

    // Check if user typed a comma
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

    if (allUrls.length === 0) return;

    // Call backend to start downloads
    await startDownloads(allUrls);

    onClose();
    setUrlTags([]);
    setInputValue('');
  };

  const handleClose = () => {
    onClose();
    setUrlTags([]);
    setInputValue('');
  };

  return (
    <AnimatePresence>
      {open && (
        <>
          {/* Backdrop - blurs everything including page content */}
          <motion.div
            initial={{ opacity: 0, backdropFilter: 'blur(0px)' }}
            animate={{ opacity: 1, backdropFilter: 'blur(8px)' }}
            exit={{ opacity: 0, backdropFilter: 'blur(0px)' }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 bg-background/40 z-50"
            style={{ backdropFilter: 'blur(8px)', WebkitBackdropFilter: 'blur(3px)' }}
            onClick={handleClose}
          />

          {/* Compact input panel with tag system */}
          <motion.div
            initial={{ opacity: 0, y: -20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            className="fixed top-20 left-1/2 -translate-x-1/2 w-full max-w-xl z-[60] px-4"
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
          >
            <div className={`bg-card border-2 rounded-xl shadow-xl transition-all ${isDragging ? 'border-blue-500 scale-[1.02]' : 'border-border'
              }`}>
              <div className="flex items-start gap-3 px-4 py-2.5">
                {/* Tag input field with max height and scroll */}
                <div className="flex-1 min-w-0 max-h-[84px] overflow-y-auto">
                  <div className="flex flex-wrap gap-1.5 items-center">
                    {/* URL Tags */}
                    {urlTags.map((url, index) => (
                      <div
                        key={index}
                        className="inline-flex items-center gap-1 bg-blue-600/10 text-blue-600 dark:text-blue-400 px-2 py-1 rounded-md text-sm"
                      >
                        <span className="max-w-[200px] truncate">{url}</span>
                        <button
                          onClick={() => removeTag(index)}
                          className="hover:bg-blue-600/20 rounded-sm p-0.5"
                        >
                          <X className="h-3 w-3" />
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
                      placeholder={urlTags.length === 0 ? "Enter URL or Browse File" : ""}
                      className="flex-1 min-w-[120px] bg-transparent text-sm focus:outline-none py-1"
                    />
                  </div>
                </div>

                {/* File browser icon (paperclip) */}
                <button
                  onClick={handleFileSelect}
                  className="p-1.5 hover:bg-muted rounded-md transition-colors shrink-0 mt-1"
                  title="Browse File"
                >
                  <Paperclip className="h-4 w-4 text-muted-foreground" />
                </button>

                {/* Download button */}
                <button
                  onClick={handleDownload}
                  disabled={urlTags.length === 0 && !inputValue.trim()}
                  className="p-1.5 rounded-full bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors shrink-0 mt-1"
                  title="Download"
                >
                  <Download className="h-4 w-4 text-white" />
                </button>
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
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
