import { useState, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { PanelRightClose, PanelRightOpen, LayoutGrid, List } from 'lucide-react';
import { Button } from '@/components/ui/button';

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
  const sidebarRef = useRef<HTMLDivElement>(null);

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
          className={`h-full bg-sidebar flex flex-col overflow-hidden relative ${
            position === 'left' ? 'border-r' : 'border-l'
          } border-sidebar-border`}
        >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-sidebar-border shrink-0">
        <div className="flex items-center gap-2">
          <LayoutGrid className="size-5" />
          <List className="size-5" />
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="size-5"
            onClick={() => {
              onClose();
              navigate('/detail');
            }}
            aria-label="Open details page"
          >
            <PanelRightOpen className="size-5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8"
            onClick={onClose}
            aria-label="Close sidebar"
          >
            <PanelRightOpen className={`size-5 ${position === 'left' ? 'scale-x-[-1]' : ''}`} />
          </Button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden p-3">
        <p className="text-xs text-muted-foreground text-center py-8">
          Active downloads will appear here
        </p>
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
