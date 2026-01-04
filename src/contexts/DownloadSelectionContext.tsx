/**
 * DownloadSelectionContext
 * 
 * Single source of truth for:
 * - Which download is currently selected (selectedDownloadId)
 * - Whether we're in empty state (no download selected)
 * - Whether to show welcome screen vs in-app empty state
 * 
 * Logic:
 * - showWelcomeScreen: true only on first launch before user navigates anywhere
 * - After user visits any other page, they get the simpler in-app empty state
 * - Can be disabled via settings
 */
import { createContext, useContext, useState, useEffect, useRef, ReactNode } from 'react';
import { useLocation } from 'react-router-dom';
import { useDownloads } from '@/hooks/useDownloads';
import { useSettings } from '@/contexts/SettingsContext';

interface DownloadSelectionContextType {
    selectedDownloadId: string | null;
    setSelectedDownloadId: (id: string | null) => void;
    isEmptyState: boolean;
    showWelcomeScreen: boolean;
    selectedDownload: ReturnType<typeof useDownloads>['downloads'][number] | null;
}

const DownloadSelectionContext = createContext<DownloadSelectionContextType | null>(null);

export function DownloadSelectionProvider({ children }: { children: ReactNode }) {
    const location = useLocation();
    const { downloads } = useDownloads();
    const { settings, ready } = useSettings();

    // Initialize from location state if present (e.g., navigating with download)
    const [selectedDownloadId, setSelectedDownloadId] = useState<string | null>(
        () => location.state?.download?.id || null
    );

    // Track if user has interacted (navigated away from home or started a download)
    // Once true, never goes back to false (session-based)
    const [hasInteracted, setHasInteracted] = useState(false);

    // Track initial path - only show welcome if started on home
    const initialPathRef = useRef(location.pathname);

    // Detect navigation away from home
    useEffect(() => {
        // If user navigated to any page other than home, they've interacted
        if (location.pathname !== '/') {
            setHasInteracted(true);
        }
    }, [location.pathname]);

    // If user starts a download, they've interacted
    useEffect(() => {
        if (downloads.length > 0) {
            setHasInteracted(true);
        }
    }, [downloads.length]);

    // Listen for select-download events from sidebar
    useEffect(() => {
        const handleSelectDownload = (e: CustomEvent<{ id: string }>) => {
            setSelectedDownloadId(e.detail.id);
            setHasInteracted(true); // Selecting a download = interaction
        };
        window.addEventListener('select-download', handleSelectDownload as EventListener);
        return () => {
            window.removeEventListener('select-download', handleSelectDownload as EventListener);
        };
    }, []);

    // Auto-select first active download when downloads arrive (for deep links)
    useEffect(() => {
        if (downloads.length > 0 && !selectedDownloadId) {
            const activeDownload = downloads.find(d => d.status === 'downloading' || d.status === 'queued');
            if (activeDownload) {
                setSelectedDownloadId(activeDownload.id);
            }
        }
    }, [downloads, selectedDownloadId]);

    // Get the actual selected download object
    const selectedDownload = selectedDownloadId
        ? downloads.find(d => d.id === selectedDownloadId) || null
        : null;

    // Empty state = no download selected
    const isEmptyState = !selectedDownload;

    // Welcome screen setting from config (default to true if not loaded yet)
    const welcomeScreenEnabled = ready ? settings.app.show_welcome_screen : true;

    // Welcome screen = setting enabled + empty state + never interacted + on home page + started on home
    const showWelcomeScreen = welcomeScreenEnabled &&
        isEmptyState &&
        !hasInteracted &&
        location.pathname === '/' &&
        initialPathRef.current === '/';

    return (
        <DownloadSelectionContext.Provider value={{
            selectedDownloadId,
            setSelectedDownloadId,
            isEmptyState,
            showWelcomeScreen,
            selectedDownload
        }}>
            {children}
        </DownloadSelectionContext.Provider>
    );
}

export function useDownloadSelection() {
    const context = useContext(DownloadSelectionContext);
    if (!context) {
        throw new Error('useDownloadSelection must be used within DownloadSelectionProvider');
    }
    return context;
}
