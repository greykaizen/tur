import PageTransition from '@/components/PageTransition';

export default function Detail() {
  return (
    <PageTransition>
      <div className="h-full w-full overflow-hidden flex flex-col">
        {/* Action bar */}
        <div className="shrink-0 border-b border-border px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <button className="text-sm hover:text-primary transition-colors">Queue Download</button>
            <button className="text-sm hover:text-primary transition-colors">Download Selected</button>
            <button className="text-sm hover:text-primary transition-colors">Stop Selected</button>
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" className="rounded" />
            select all
          </label>
        </div>

        {/* Download list */}
        <div className="flex-1 overflow-y-auto p-6">
          <div className="space-y-4">
            {/* Download items will go here */}
            <div className="h-12 bg-muted rounded"></div>
            <div className="h-12 bg-muted rounded"></div>
            <div className="h-12 bg-muted rounded"></div>
            <div className="h-12 bg-muted rounded"></div>
            <div className="h-12 bg-muted rounded"></div>
          </div>
        </div>
      </div>
    </PageTransition>
  );
}
