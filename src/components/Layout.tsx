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
  const [isEmptyState, setIsEmptyState] = useState(false);
  
  const isDetailPage = location.pathname === '/detail';
  const sidebarPosition = ready ? settings.app.sidebar : 'left';

  // Listen for keyboard shortcuts and empty state
  useEffect(() => {
    const handleOpenAddDialog = () => setAddDialogOpen(true);
    const handleToggleSidebar = () => setSidebarOpen(prev => !prev);
    const handleEmptyState = (e: CustomEvent) => setIsEmptyState(e.detail.isEmpty);
    
    window.addEventListener('open-add-dialog', handleOpenAddDialog);
    window.addEventListener('toggle-sidebar', handleToggleSidebar);
    window.addEventListener('home-empty-state', handleEmptyState as EventListener);
    
    return () => {
      window.removeEventListener('open-add-dialog', handleOpenAddDialog);
      window.removeEventListener('toggle-sidebar', handleToggleSidebar);
      window.removeEventListener('home-empty-state', handleEmptyState as EventListener);
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
    <div className="h-screen w-screen flex overflow-hidden bg-background relative">
      {/* Animated Gradient Mesh Background - Only in Empty State */}
      {isEmptyState && (
        <div className="absolute inset-0 overflow-hidden pointer-events-none">
          <div 
            className="absolute w-[600px] h-[600px] rounded-full blur-3xl opacity-45 animate-float-1"
            style={{
              background: 'radial-gradient(circle, rgba(59, 130, 246, 0.4) 0%, transparent 70%)',
              top: '10%',
              left: '15%',
            }}
          />
          <div 
            className="absolute w-[650px] h-[650px] rounded-full blur-3xl opacity-40 animate-float-2"
            style={{
              background: 'radial-gradient(circle, rgba(168, 85, 247, 0.4) 0%, transparent 70%)',
              bottom: '15%',
              right: '20%',
            }}
          />
          <div 
            className="absolute w-[600px] h-[600px] rounded-full blur-3xl opacity-45 animate-float-3"
            style={{
              background: 'radial-gradient(circle, rgba(236, 72, 153, 0.3) 0%, transparent 70%)',
              top: '50%',
              left: '50%',
              transform: 'translate(-50%, -50%)',
            }}
          />
        </div>
      )}

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
      <div className="flex-1 flex flex-col overflow-hidden transition-all duration-300 ease-out relative z-10">
        <Header 
          sidebarOpen={sidebarOpen}
          onToggleSidebar={() => setSidebarOpen(true)}
          showSidebarToggle={!isDetailPage}
          onOpenAddDialog={() => setAddDialogOpen(true)}
          isEmptyState={isEmptyState}
        />
        <div className="flex-1 overflow-hidden p-3 pt-2">
          <div className={`h-full w-full rounded-2xl shadow-sm overflow-hidden transition-all duration-200 ${
            isEmptyState ? 'bg-transparent border-0' : 'bg-card border border-border'
          }`}>
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
