import { useNavigate } from 'react-router-dom';
import { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Ellipsis, CirclePlus, History, PanelLeft, PanelRight, Settings, Sun, Moon, Laptop, Info, Heart, LogOut, ChevronRight } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useTheme } from '@/components/theme-provider';
import { useSettings } from '@/contexts/SettingsContext';

interface HeaderProps {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  showSidebarToggle?: boolean;
  onOpenAddDialog: () => void;
  isEmptyState?: boolean;
}

export default function Header({ sidebarOpen, onToggleSidebar, showSidebarToggle = true, onOpenAddDialog, isEmptyState = false }: HeaderProps) {
  const navigate = useNavigate();
  const { setTheme } = useTheme();
  const { settings, ready } = useSettings();
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

  const sidebarPosition = ready ? settings.app.sidebar : 'left';
  const buttonLabel = ready ? settings.app.button_label : 'both';

  return (
    <header className={`relative z-50 ${isEmptyState ? 'bg-transparent' : 'bg-background/80 backdrop-blur-sm'}`}>
      <div className="flex items-center justify-between px-4 h-14">
        {/* Left side: Sidebar toggle (if left), Logo, App name - Hidden in empty state */}
        {!isEmptyState && (
          <div className="flex items-center gap-3">
            {showSidebarToggle && !sidebarOpen && sidebarPosition === 'left' && (
              <Button
                variant="ghost"
                size="icon"
                onClick={onToggleSidebar}
                aria-label="Toggle sidebar"
              >
                <PanelLeft className="size-5" />
              </Button>
            )}
            
            <button
              onClick={() => {
                // Check if there are active downloads
                const activeDownloads = sessionStorage.getItem('activeDownloads');
                if (activeDownloads) {
                  const downloads = JSON.parse(activeDownloads);
                  if (downloads.length > 0) {
                    // Navigate to download view with first download
                    navigate('/', { state: { download: downloads[0] } });
                  } else {
                    // Navigate to empty state
                    navigate('/', { replace: true });
                  }
                } else {
                  // Navigate to empty state
                  navigate('/', { replace: true });
                }
              }}
              className="flex items-center gap-3 hover:opacity-80 transition-opacity"
            >
              <img src="/icon.png" alt="tur logo" className="w-8 h-8" />
              <span className="-mb-1.5 text-4xl tracking-tight" style={{ 
      fontFamily: "'Modak', cursive",
      WebkitTextStroke: '1.5px',
      WebkitTextFillColor: 'transparent',
      letterSpacing: '0.05em'
    }}>tur</span>
            </button>
          </div>
        )}

        {/* Empty state - just a spacer */}
        {isEmptyState && <div />}

        {/* Right side: Donate, History, Menu (no Add button in empty state) */}
        <div className="flex items-center gap-2">
          {/* Donate Button - Only in empty state with microinteraction */}
          {isEmptyState && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => navigate('/donate')}
              className="gap-2 group"
            >
              <Heart className="size-5 transition-all group-hover:scale-110 group-hover:fill-red-500 group-hover:text-red-500" />
              Donate
            </Button>
          )}

          {/* History Button */}
          {buttonLabel === 'text' ? (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => navigate('/history')}
            >
              History
            </Button>
          ) : buttonLabel === 'icon' ? (
            <Button
              variant="ghost"
              size="icon"
              onClick={() => navigate('/history')}
              aria-label="History"
            >
              <History className="size-5" />
            </Button>
          ) : (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => navigate('/history')}
              className="gap-2"
            >
              <History className="size-5" />
              History
            </Button>
          )}

          {/* Add Button - Hidden in empty state */}
          {!isEmptyState && (
            <>
              {buttonLabel === 'text' ? (
                <Button
                  onClick={onOpenAddDialog}
                  className="rounded-full bg-blue-600 hover:bg-blue-700 text-white"
                >
                  Add
                </Button>
              ) : buttonLabel === 'icon' ? (
                <Button
                  onClick={onOpenAddDialog}
                  className="rounded-full bg-blue-600 hover:bg-blue-700 text-white"
                  size="icon"
                  aria-label="Add"
                >
                  <CirclePlus className="size-5" />
                </Button>
              ) : (
                <Button
                  onClick={onOpenAddDialog}
                  className="rounded-full bg-blue-600 hover:bg-blue-700 text-white gap-2"
                >
                  <CirclePlus className="size-5" />
                  Add
                </Button>
              )}
            </>
          )}

          <div className="relative" ref={menuRef}>
            <Button 
              variant="ghost" 
              size="icon" 
              aria-label="Menu"
              onClick={() => setMenuOpen(!menuOpen)}
            >
              <Ellipsis className="size-6" />
            </Button>
            
            <AnimatePresence>
              {menuOpen && (
                <motion.div 
                  initial={{ opacity: 0, scale: 0.95, y: -10 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  exit={{ opacity: 0, scale: 0.95, y: -10 }}
                  transition={{ duration: 0.15, ease: "easeOut" }}
                  className="absolute right-0 mt-2 w-56 bg-popover text-popover-foreground border border-border rounded-md shadow-lg z-[100]"
                >
                  <div className="py-1">
                  <button
                    onClick={() => {
                      navigate('/settings');
                      setMenuOpen(false);
                    }}
                    className="w-full flex items-center justify-between px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm"
                  >
                    <div className="flex items-center">
                      <Settings className="mr-2 h-5 w-5" />
                      Settings
                    </div>
                    <span className="text-xs text-muted-foreground font-mono">Ctrl+P</span>
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
                        <Sun className="mr-2 h-5 w-5" />
                        Theme
                      </div>
                      <ChevronRight className="h-5 w-5" />
                    </button>
                    
                    <AnimatePresence>
                      {themeSubmenuOpen && (
                        <motion.div 
                          ref={themeSubmenuRef}
                          initial={{ opacity: 0, x: 10 }}
                          animate={{ opacity: 1, x: 0 }}
                          exit={{ opacity: 0, x: 10 }}
                          transition={{ duration: 0.15, ease: "easeOut" }}
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
                            <Sun className="mr-2 h-5 w-5" />
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
                            <Moon className="mr-2 h-5 w-5" />
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
                            <Laptop className="mr-2 h-5 w-5" />
                            System
                          </button>
                        </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </div>

                  <button
                    onClick={() => {
                      navigate('/about');
                      setMenuOpen(false);
                    }}
                    className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm"
                  >
                    <Info className="mr-2 h-5 w-5" />
                    About
                  </button>

                  <button
                    onClick={() => {
                      navigate('/donate');
                      setMenuOpen(false);
                    }}
                    className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm"
                  >
                    <Heart className="mr-2 h-5 w-5" />
                    Donate
                  </button>

                  <div className="border-t border-border my-1"></div>

                  <button
                    onClick={() => {
                      handleQuit();
                      setMenuOpen(false);
                    }}
                    className="w-full flex items-center justify-between px-3 py-2 text-sm text-destructive hover:bg-destructive/10 rounded-sm"
                  >
                    <div className="flex items-center">
                      <LogOut className="mr-2 h-5 w-5" />
                      Quit
                    </div>
                    <span className="text-xs text-muted-foreground font-mono">Ctrl+Q</span>
                  </button>
                </div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>

          {showSidebarToggle && !sidebarOpen && sidebarPosition === 'right' && (
            <Button
              variant="ghost"
              size="icon"
              onClick={onToggleSidebar}
              aria-label="Toggle sidebar"
            >
              <PanelRight className="size-5" />
            </Button>
          )}
        </div>
      </div>
    </header>
  );
}

