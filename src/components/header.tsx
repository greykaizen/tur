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
}

export default function Header({ sidebarOpen, onToggleSidebar, showSidebarToggle = true, onOpenAddDialog }: HeaderProps) {
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
    <header className="bg-background/80 backdrop-blur-sm">
      <div className="flex items-center justify-between px-4 h-14">
        {/* Left side: Sidebar toggle (if left), Logo, App name */}
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
            onClick={() => navigate('/')}
            className="flex items-center gap-3 hover:opacity-80 transition-opacity"
          >
            <img src="/icon.png" alt="tur logo" className="w-8 h-8" />
            <span className="font-bold text-xl tracking-tight">tur</span>
          </button>
        </div>

        {/* Right side: Add button, History button, Menu, Sidebar toggle (if right) */}
        <div className="flex items-center gap-2">
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

                  <button className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm">
                    <Info className="mr-2 h-5 w-5" />
                    About
                  </button>

                  <button className="w-full flex items-center px-3 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-sm">
                    <Heart className="mr-2 h-5 w-5" />
                    Donate us
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

