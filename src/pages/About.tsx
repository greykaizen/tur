import PageTransition from '@/components/PageTransition';
import { Github as GithubIcon, Globe, Mail, Heart } from 'lucide-react';

export default function About() {
  return (
    <PageTransition className="h-full w-full overflow-auto">
      <div className="h-full w-full p-3 grid grid-cols-2 gap-3">
        {/* Left Column */}
        <div className="space-y-3">
          {/* Logo + Name + Version */}
          <div className="flex flex-col items-center space-y-2 p-3 rounded-lg border border-border bg-card">
            <div className="flex items-center gap-2">
              <img src="/icon.png" alt="tur logo" className="w-10 h-10" />
              <h1 className="text-xl font-bold tracking-tight">tur</h1>
            </div>
            <p className="text-xs text-muted-foreground text-center">
              A modern, fast, and beautiful download manager
            </p>
            <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
              <span>Version 1.0.0</span>
              <span>•</span>
              <span className="text-green-500">Up to date</span>
            </div>
            <button className="px-3 py-1 text-[10px] rounded-lg border border-border hover:bg-muted transition-colors">
              Check for Updates
            </button>
          </div>

          {/* Connect Links */}
          <div className="text-center p-3 text-[10px] text-muted-foreground border border-border rounded-lg bg-card">
            <p>Made with <Heart className="inline size-2.5 text-red-500" /> by the tur team</p>
            <p className="mt-1">© 2025 tur. Open source and free forever.</p>
          </div>
        </div>

        {/* Right Column */}
        <div className="space-y-3">
          {/* About Section */}
          <div className="p-3 rounded-lg border border-border bg-card space-y-2">
            <h2 className="text-sm font-bold">About tur</h2>
            <p className="text-xs text-muted-foreground leading-relaxed">
              tur is a modern download manager built with cutting-edge technologies. It combines the performance of native applications with the flexibility of web technologies to provide a fast, secure, and intuitive downloading experience.
            </p>
          </div>

          {/* Footer */}
          <div className="p-3 rounded-lg border border-border bg-card space-y-2">
            <h2 className="text-sm font-bold">Connect</h2>
            <div className="space-y-1.5">
              <a
                href="https://github.com"
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 px-2 py-1.5 rounded-lg border border-border hover:bg-muted transition-colors text-xs"
              >
                <GithubIcon className="size-3.5" />
                <span>GitHub</span>
              </a>
              <a
                href="https://example.com"
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 px-2 py-1.5 rounded-lg border border-border hover:bg-muted transition-colors text-xs"
              >
                <Globe className="size-3.5" />
                <span>Website</span>
              </a>
              <a
                href="mailto:hello@example.com"
                className="flex items-center gap-2 px-2 py-1.5 rounded-lg border border-border hover:bg-muted transition-colors text-xs"
              >
                <Mail className="size-3.5" />
                <span>hello@example.com</span>
              </a>
            </div>
          </div>
        </div>
      </div>
    </PageTransition>
  );
}
