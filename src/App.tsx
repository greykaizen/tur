import { ThemeProvider } from "@/components/theme-provider"
import { BrowserRouter, Routes, Route, useLocation, useNavigate } from 'react-router-dom';
import { AnimatePresence } from 'framer-motion';
import { useEffect } from 'react';
import { SettingsProvider, useSettings } from '@/contexts/SettingsContext';
import { DownloadSelectionProvider, useDownloadSelection } from '@/contexts/DownloadSelectionContext';
import Layout from '@/components/Layout';
import Home from '@/pages/Home';
import Settings from '@/pages/Settings';
import History from '@/pages/History';
import Detail from '@/pages/Detail';
import About from '@/pages/About';
import Donate from '@/pages/Donate';
import Download from '@/pages/Download';

function AnimatedRoutes() {
  const location = useLocation();

  return (
    <AnimatePresence mode="wait">
      <Routes location={location} key={location.pathname}>
        <Route path="/" element={<Home />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/history" element={<History />} />
        <Route path="/detail" element={<Detail />} />
        <Route path="/about" element={<About />} />
        <Route path="/donate" element={<Donate />} />
        <Route path="/download" element={<Download />} />
      </Routes>
    </AnimatePresence>
  );
}

function AppContent() {
  const navigate = useNavigate();
  const { settings, ready } = useSettings();
  const { isEmptyState } = useDownloadSelection();

  useEffect(() => {
    if (!ready) return;

    const parseShortcut = (shortcut: string) => {
      const parts = shortcut.split('+');
      return {
        ctrl: parts.includes('Ctrl'),
        shift: parts.includes('Shift'),
        alt: parts.includes('Alt'),
        meta: parts.includes('Meta'),
        key: parts[parts.length - 1].toLowerCase()
      };
    };

    const handleKeyDown = async (e: KeyboardEvent) => {
      const goHome = parseShortcut(settings.shortcuts.go_home);
      const openSettings = parseShortcut(settings.shortcuts.open_settings);
      const addDownload = parseShortcut(settings.shortcuts.add_download);
      const openDetails = parseShortcut(settings.shortcuts.open_details);
      const openHistory = parseShortcut(settings.shortcuts.open_history);
      const toggleSidebar = parseShortcut(settings.shortcuts.toggle_sidebar);
      const cancelDownload = parseShortcut(settings.shortcuts.cancel_download);
      const quitApp = parseShortcut(settings.shortcuts.quit_app);

      const matchesShortcut = (shortcut: ReturnType<typeof parseShortcut>) => {
        return (
          e.ctrlKey === shortcut.ctrl &&
          e.shiftKey === shortcut.shift &&
          e.altKey === shortcut.alt &&
          e.metaKey === shortcut.meta &&
          e.key.toLowerCase() === shortcut.key
        );
      };

      if (matchesShortcut(goHome)) {
        e.preventDefault();
        navigate('/');
      } else if (matchesShortcut(openSettings)) {
        e.preventDefault();
        navigate('/settings');
      } else if (matchesShortcut(addDownload)) {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('open-add-dialog'));
      } else if (matchesShortcut(openDetails)) {
        e.preventDefault();
        navigate('/detail');
      } else if (matchesShortcut(openHistory)) {
        e.preventDefault();
        navigate('/history');
      } else if (matchesShortcut(toggleSidebar)) {
        // Don't toggle sidebar when in home empty state
        if (!isEmptyState) {
          e.preventDefault();
          window.dispatchEvent(new CustomEvent('toggle-sidebar'));
        }
      } else if (matchesShortcut(cancelDownload)) {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('cancel-download'));
      } else if (matchesShortcut(quitApp)) {
        e.preventDefault();
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('request_shutdown');
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [navigate, settings, ready, isEmptyState]);

  return (
    <Layout>
      <AnimatedRoutes />
    </Layout>
  );
}

// Standalone routes that don't use Layout
function StandaloneRoutes() {
  const location = useLocation();

  // Check if this is a download window (no Layout needed)
  if (location.pathname === '/download') {
    return <Download />;
  }

  return <AppContent />;
}

export default function App() {
  return (
    <ThemeProvider defaultTheme="system" storageKey="vite-ui-theme">
      <SettingsProvider>
        <BrowserRouter>
          <DownloadSelectionProvider>
            <StandaloneRoutes />
          </DownloadSelectionProvider>
        </BrowserRouter>
      </SettingsProvider>
    </ThemeProvider>
  )
}