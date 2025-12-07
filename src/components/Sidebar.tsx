import { X, FileDown } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface SidebarProps {
  open: boolean;
  onClose: () => void;
}

export default function Sidebar({ open, onClose }: SidebarProps) {
  return (
    <div 
      className={`w-80 bg-background border-l border-border flex flex-col transition-all duration-300 ${
        open ? 'translate-x-0' : 'translate-x-full'
      }`}
    >
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border">
        <div className="flex items-center gap-2">
          <FileDown className="h-5 w-5" />
          <span className="font-semibold">Details</span>
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={onClose}
          aria-label="Close sidebar"
        >
          <X className="h-5 w-5" />
        </Button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-4">
        <p className="text-sm text-muted-foreground text-center">
          Active downloads will appear here
        </p>
      </div>
    </div>
  );
}
