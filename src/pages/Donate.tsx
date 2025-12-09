import PageTransition from '@/components/PageTransition';
import { Heart, Github as GithubIcon, Coffee } from 'lucide-react';
import { toast } from 'sonner';

export default function Donate() {
  const copyToClipboard = (text: string, label: string) => {
    navigator.clipboard.writeText(text);
    toast.success(`${label} copied to clipboard`);
  };

  return (
    <PageTransition className="h-full w-full overflow-auto">
      <div className="h-full w-full p-3 grid grid-cols-2 gap-3">
        {/* Left Column */}
        <div className="space-y-3">
          {/* Hero Section - Logo and Title Side by Side */}
          <div className="flex flex-col items-center space-y-3 p-4 rounded-lg border border-border bg-card">
            <div className="flex items-center gap-3">
              <div className="w-12 h-12 rounded-full bg-gradient-to-br from-pink-500 to-red-500 flex items-center justify-center">
                <Heart className="size-6 text-white" />
              </div>
              <h1 className="text-2xl font-bold tracking-tight">Support tur</h1>
            </div>
            <p className="text-sm text-muted-foreground text-center">
              Help us keep tur free, fast, and awesome
            </p>
          </div>

          {/* GitHub Sponsors */}
          <div className="p-3 rounded-lg border-2 border-primary/20 bg-gradient-to-br from-primary/5 to-transparent">
            <div className="flex items-start gap-2">
              <div className="w-9 h-9 rounded-lg bg-primary flex items-center justify-center shrink-0">
                <GithubIcon className="size-5 text-primary-foreground" />
              </div>
              <div className="flex-1 min-w-0">
                <h3 className="text-sm font-semibold mb-1">GitHub Sponsors</h3>
                <p className="text-xs text-muted-foreground mb-2 leading-tight">
                  Monthly or one-time contributions
                </p>
                <a
                  href="https://github.com/sponsors"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors text-xs"
                >
                  <Heart className="size-3" />
                  Sponsor
                </a>
              </div>
            </div>
          </div>

          {/* Other Ways */}
          <div className="p-3 rounded-lg border border-border bg-card space-y-2">
            <h3 className="text-sm font-semibold">Your Impact</h3>
            <div className="space-y-2">
              <div className="flex items-start gap-2">
                <div className="w-6 h-6 rounded bg-green-500/10 text-green-500 flex items-center justify-center shrink-0 text-xs font-bold">
                  ✓
                </div>
                <div>
                  <p className="text-xs font-medium">Faster Development</p>
                  <p className="text-[10px] text-muted-foreground leading-tight">More time for features</p>
                </div>
              </div>
              <div className="flex items-start gap-2">
                <div className="w-6 h-6 rounded bg-blue-500/10 text-blue-500 flex items-center justify-center shrink-0 text-xs font-bold">
                  ✓
                </div>
                <div>
                  <p className="text-xs font-medium">Better Support</p>
                  <p className="text-[10px] text-muted-foreground leading-tight">Quick bug fixes</p>
                </div>
              </div>
              <div className="flex items-start gap-2">
                <div className="w-6 h-6 rounded bg-purple-500/10 text-purple-500 flex items-center justify-center shrink-0 text-xs font-bold">
                  ✓
                </div>
                <div>
                  <p className="text-xs font-medium">Free Forever</p>
                  <p className="text-[10px] text-muted-foreground leading-tight">Always open source</p>
                </div>
              </div>
            </div>
          </div>
          
        </div>

        {/* Right Column */}
        <div className="space-y-3">
          {/* Donation Badges - Clickable */}
          <div className="p-3 rounded-lg border border-border bg-card space-y-2">
            <h3 className="text-sm font-semibold mb-2">Quick Donate</h3>
            <div className="grid grid-cols-2 gap-2">
              {/* Bitcoin Badge */}
              <button
                onClick={() => copyToClipboard('bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh', 'Bitcoin address')}
                className="flex flex-col items-center gap-1.5 p-3 rounded-lg border border-border bg-gradient-to-br from-orange-500/5 to-transparent hover:from-orange-500/10 transition-colors group"
                title="Click to copy Bitcoin address"
              >
                <div className="w-10 h-10 rounded-full bg-orange-500/10 flex items-center justify-center group-hover:bg-orange-500/20 transition-colors">
                  <span className="text-lg font-bold text-orange-500">₿</span>
                </div>
                <span className="text-xs font-semibold">Bitcoin</span>
              </button>

              {/* Ethereum Badge */}
              <button
                onClick={() => copyToClipboard('0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb', 'Ethereum address')}
                className="flex flex-col items-center gap-1.5 p-3 rounded-lg border border-border bg-gradient-to-br from-blue-500/5 to-transparent hover:from-blue-500/10 transition-colors group"
                title="Click to copy Ethereum address"
              >
                <div className="w-10 h-10 rounded-full bg-blue-500/10 flex items-center justify-center group-hover:bg-blue-500/20 transition-colors">
                  <span className="text-lg font-bold text-blue-500">Ξ</span>
                </div>
                <span className="text-xs font-semibold">Ethereum</span>
              </button>

              {/* Ko-fi Badge */}
              <a
                href="https://ko-fi.com"
                target="_blank"
                rel="noopener noreferrer"
                className="flex flex-col items-center gap-1.5 p-3 rounded-lg border border-border bg-gradient-to-br from-red-500/5 to-transparent hover:from-red-500/10 transition-colors group"
                title="Support on Ko-fi"
              >
                <div className="w-10 h-10 rounded-full bg-red-500/10 flex items-center justify-center group-hover:bg-red-500/20 transition-colors">
                  <Coffee className="size-5 text-red-500" />
                </div>
                <span className="text-xs font-semibold">Ko-fi</span>
              </a>

              {/* Buy Me a Coffee Badge */}
              <a
                href="https://buymeacoffee.com"
                target="_blank"
                rel="noopener noreferrer"
                className="flex flex-col items-center gap-1.5 p-3 rounded-lg border border-border bg-gradient-to-br from-yellow-500/5 to-transparent hover:from-yellow-500/10 transition-colors group"
                title="Buy me a coffee"
              >
                <div className="w-10 h-10 rounded-full bg-yellow-500/10 flex items-center justify-center group-hover:bg-yellow-500/20 transition-colors">
                  <Coffee className="size-5 text-yellow-600" />
                </div>
                <span className="text-xs font-semibold">BMC</span>
              </a>
            </div>
          </div>

          {/* Other Ways to Help */}
          <div className="p-3 rounded-lg border border-border bg-card">
            <h3 className="text-sm font-semibold mb-2">Other Ways to Help</h3>
            <ul className="space-y-1.5 text-xs text-muted-foreground">
              <li className="flex items-start gap-2">
                <span className="text-primary mt-0.5">•</span>
                <span>Star on GitHub</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-primary mt-0.5">•</span>
                <span>Share with friends</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-primary mt-0.5">•</span>
                <span>Report bugs</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-primary mt-0.5">•</span>
                <span>Contribute code</span>
              </li>
            </ul>
          </div>
        </div>
      </div>
    </PageTransition>
  );
}
