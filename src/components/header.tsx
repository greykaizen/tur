import { useNavigate } from 'react-router-dom';
import { useState, useEffect, useRef } from 'react';
import { EllipsisVertical, CirclePlus, History, PanelLeft, Settings, Sun, Moon, Laptop, Info, Heart, LogOut, ChevronRight } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useTheme } from '@/components/theme-provider';

interface HeaderProps {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
}

export default function Header({ sidebarOpen, onToggleSidebar }: HeaderProps) {
  const navigate = useNavigate();
  const { setTheme } = useTheme();
  const [menuOpen, setMenuOpen] = useState(false);
  const [themeSubmenuOpen, setThemeSubmenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const themeItemRef = useRef<HTMLDivElement>(null);
  const themeSubmenuRef = useRef<HTMLDivElement>(null);

  const handleQuit = async () => {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().close();
  };

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setMenuOpen(false);
        setThemeSubmenuOpen(false);
      }
    };

    if (menuOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [menuOpen]);

  return (
    <header className="bg-background/80 backdrop-blur-sm">
      <div className="flex items-center justify-between px-4 h-14">
        {/* Left side: Sidebar toggle, Logo, App name */}
        <div className="flex items-center gap-3">
          {!sidebarOpen && (
            <Button
              variant="ghost"
              size="icon"
              onClick={onToggleSidebar}
              aria-label="Toggle sidebar"
            >
              <PanelLeft className="h-5 w-5" />
            </Button>
          )}
          
          <button
            onClick={() => navigate('/')}
            className="flex items-center gap-3 hover:opacity-80 transition-opacity"
          >
            <img src="/icon.png" alt="tur logo" className="w-10 h-10" />
            <span className="font-bold text-2xl tracking-tight">tur</span>
          </button>
        </div>

        {/* Right side: Add button, History button, Menu */}
        <div className="flex items-center gap-2">
          <Button
            onClick={() => {/* TODO: Open add download dialog */}}
            className="gap-2 rounded-full bg-blue-600 hover:bg-blue-700 text-white"
          >
            <CirclePlus className="h-4 w-4" />
            Add
          </Button>

          <Button
            variant="ghost"
            size="sm"
            onClick={() => navigate('/history')}
            className="gap-2"
          >
            <History className="h-4 w-4" />
            History
          </Button>

          <div className="relative" ref={menuRef}>
            <Button 
              variant="ghost" 
              size="icon" 
              aria-label="Menu"
              onClick={() => setMenuOpen(!menuOpen)}
            >
              <EllipsisVertical className="h-5 w-5" />
            </Button>
            
            {menuOpen && (
              <div className="absolute right-0 mt-2 w-48 bg-popover text-popover-foreground border border-border rounded-md shadow-lg z-50">
                <div className="py-1">
                  <button
                    onClick={() => {
                      navigate('/settings');
                      setMenuOpen(false);
                    }}
                    className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm"
                  >
                    <Settings className="mr-2 h-4 w-4" />
                    Settings
                  </button>

                  <div 
                    ref={themeItemRef}
                    className="relative"
                    onMouseEnter={() => setThemeSubmenuOpen(true)}
                    onMouseLeave={() => setThemeSubmenuOpen(false)}
                  >
                    <button 
                      className="w-full flex items-center justify-between px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm transition-colors"
                    >
                      <div className="flex items-center">
                        <Sun className="mr-2 h-4 w-4" />
                        Theme
                      </div>
                      <ChevronRight className="h-4 w-4" />
                    </button>
                    
                    {themeSubmenuOpen && (
                      <div 
                        ref={themeSubmenuRef}
                        className="absolute right-full top-0 mr-1 w-40 bg-popover text-popover-foreground border border-border rounded-md shadow-lg z-[60]"
                        onMouseEnter={() => setThemeSubmenuOpen(true)}
                        onMouseLeave={() => setThemeSubmenuOpen(false)}
                      >
                        <div className="py-1">
                          <button
                            onClick={() => {
                              setTheme('light');
                              setMenuOpen(false);
                              setThemeSubmenuOpen(false);
                            }}
                            className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm transition-colors"
                          >
                            <Sun className="mr-2 h-4 w-4" />
                            Light
                          </button>
                          <button
                            onClick={() => {
                              setTheme('dark');
                              setMenuOpen(false);
                              setThemeSubmenuOpen(false);
                            }}
                            className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm transition-colors"
                          >
                            <Moon className="mr-2 h-4 w-4" />
                            Dark
                          </button>
                          <button
                            onClick={() => {
                              setTheme('system');
                              setMenuOpen(false);
                              setThemeSubmenuOpen(false);
                            }}
                            className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm transition-colors"
                          >
                            <Laptop className="mr-2 h-4 w-4" />
                            System
                          </button>
                        </div>
                      </div>
                    )}
                  </div>

                  <button className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm">
                    <Info className="mr-2 h-4 w-4" />
                    About
                  </button>

                  <button className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm">
                    <Heart className="mr-2 h-4 w-4" />
                    Donate us
                  </button>

                  <div className="border-t border-border my-1"></div>

                  <button
                    onClick={() => {
                      handleQuit();
                      setMenuOpen(false);
                    }}
                    className="w-full flex items-center px-3 py-2 text-sm text-destructive hover:bg-destructive/10 rounded-sm"
                  >
                    <LogOut className="mr-2 h-4 w-4" />
                    Quit
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </header>
  );
}

