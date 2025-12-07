import { useState } from 'react';
import Header from './header';
import Sidebar from './Sidebar';

interface LayoutProps {
  children: React.ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  const [sidebarOpen, setSidebarOpen] = useState(false);

  return (
    <div className="h-screen flex flex-col bg-background">
      <Header 
        sidebarOpen={sidebarOpen}
        onToggleSidebar={() => setSidebarOpen(true)}
      />
      <div className="flex flex-1 overflow-hidden p-4">
        <main className={`flex-1 overflow-auto transition-all duration-300 ${sidebarOpen ? 'mr-80' : ''}`}>
          <div className="h-full bg-card border border-border rounded-2xl shadow-sm">
            {children}
          </div>
        </main>
        {sidebarOpen && (
          <Sidebar 
            open={sidebarOpen}
            onClose={() => setSidebarOpen(false)}
          />
        )}
      </div>
    </div>
  );
}
