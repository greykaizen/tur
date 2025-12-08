import { useState, useEffect } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { useSettings } from '@/contexts/SettingsContext';
import Header from './header';
import Sidebar from './Sidebar';
import AddDownloadDialog from './AddDownloadDialog';

interface LayoutProps {
  children: React.ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const { settings, ready } = useSettings();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(240);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  
  const isDetailPage = location.pathname === '/detail';
  const sidebarPosition = ready ? settings.app.sidebar : 'left';

  // Listen for keyboard shortcuts
  useEffect(() => {
    const handleOpenAddDialog = () => setAddDialogOpen(true);
    const handleToggleSidebar = () => setSidebarOpen(prev => !prev);
    
    window.addEventListener('open-add-dialog', handleOpenAddDialog);
    window.addEventListener('toggle-sidebar', handleToggleSidebar);
    
    return () => {
      window.removeEventListener('open-add-dialog', handleOpenAddDialog);
      window.removeEventListener('toggle-sidebar', handleToggleSidebar);
    };
  }, []);

  const handleWidthChange = (newWidth: number) => {
    const screenWidth = window.innerWidth;
    const widthPercentage = (newWidth / screenWidth) * 100;
    
    // If dragged more than 50% of screen, navigate to details page
    if (widthPercentage > 50) {
      setSidebarOpen(false);
      setSidebarWidth(240);
      navigate('/detail');
      return;
    }
    
    // If shrunk to less than 85% of original size (240px), auto-close
    if (newWidth < 240 * 0.85) {
      setSidebarOpen(false);
      setSidebarWidth(240);
    } else {
      setSidebarWidth(newWidth);
    }
  };

  return (
    <div className="h-screen w-screen flex overflow-hidden bg-background">
      {!isDetailPage && sidebarPosition === 'left' && (
        <Sidebar 
          open={sidebarOpen}
          onClose={() => {
            setSidebarOpen(false);
            setSidebarWidth(240);
          }}
          width={sidebarWidth}
          onWidthChange={handleWidthChange}
          position="left"
        />
      )}
      <div className="flex-1 flex flex-col overflow-hidden transition-all duration-300 ease-out">
        <Header 
          sidebarOpen={sidebarOpen}
          onToggleSidebar={() => setSidebarOpen(true)}
          showSidebarToggle={!isDetailPage}
          onOpenAddDialog={() => setAddDialogOpen(true)}
        />
        <div className="flex-1 overflow-hidden p-3 pt-2">
          <div className="h-full w-full bg-card border border-border rounded-2xl shadow-sm overflow-hidden transition-all duration-200">
            {children}
          </div>
        </div>
      </div>
      
      {/* Add Download Dialog - rendered at layout level to properly blur page content */}
      <AddDownloadDialog open={addDialogOpen} onClose={() => setAddDialogOpen(false)} />
      {!isDetailPage && sidebarPosition === 'right' && (
        <Sidebar 
          open={sidebarOpen}
          onClose={() => {
            setSidebarOpen(false);
            setSidebarWidth(240);
          }}
          width={sidebarWidth}
          onWidthChange={handleWidthChange}
          position="right"
        />
      )}
    </div>
  );
}
