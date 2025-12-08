import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { Store, getStore as getStoreInstance } from '@tauri-apps/plugin-store';
import { appConfigDir } from '@tauri-apps/api/path';
import { join } from '@tauri-apps/api/path';

export type AppSettings = {
    app: {
        show_tray_icon: boolean;
        quit_on_close: boolean;
        sidebar: 'left' | 'right';
        theme: 'light' | 'dark' | 'system';
        button_label: 'text' | 'icon' | 'both';
        show_download_progress: boolean;
        show_segment_progress: boolean;
    };
    shortcuts: {
        goHome: string;
        openSettings: string;
        addDownload: string;
        openDetails: string;
        openHistory: string;
        toggleSidebar: string;
        cancelDownload: string;
        quitApp: string;
    };
    download: {
        num_threads: number;
        chunk_size: number;
        socket_buffer_size: number;
        speed_limit: number;
    };
    thread: {
        total_connections: number;
        per_task_connections: number;
    };
    session: {
        history: boolean;
        metadata: boolean;
    };
    sendAnonymousMetrics: boolean;
    showNotifications: boolean;
};

const DEFAULT_SETTINGS: AppSettings = {
    app: {
        show_tray_icon: true,
        quit_on_close: false,
        sidebar: 'left',
        theme: 'system',
        button_label: 'both',
        show_download_progress: true,
        show_segment_progress: true
    },
    shortcuts: {
        goHome: 'Ctrl+K',
        openSettings: 'Ctrl+P',
        addDownload: 'Ctrl+N',
        openDetails: 'Ctrl+D',
        openHistory: 'Ctrl+H',
        toggleSidebar: 'Ctrl+L',
        cancelDownload: 'Ctrl+C',
        quitApp: 'Ctrl+Q'
    },
    download: {
        num_threads: 8,
        chunk_size: 16,
        socket_buffer_size: 0,
        speed_limit: 0
    },
    thread: {
        total_connections: 1,
        per_task_connections: 1
    },
    session: {
        history: false,
        metadata: false
    },
    sendAnonymousMetrics: false,
    showNotifications: true
};

async function loadOrGetStore(): Promise<Store> {
    const dir = await appConfigDir();
    const storePath = await join(dir, 'config.json');

    let store = await getStoreInstance(storePath);
    if (!store) {
        store = await Store.load(storePath, {
            defaults: { settings: DEFAULT_SETTINGS },
            autoSave: true,
            overrideDefaults: false 
        });
    }
    return store;
}

interface SettingsContextType {
    ready: boolean;
    settings: AppSettings;
    set: (path: string, value: any) => Promise<void>;
}

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export function SettingsProvider({ children }: { children: ReactNode }) {
    const [ready, setReady] = useState(false);
    const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);

    useEffect(() => {
        (async () => {
            const store = await loadOrGetStore();
            const loaded = await store.get<AppSettings>('settings');
            if (loaded) {
                // Merge with defaults to ensure shortcuts exist
                const merged = {
                    ...DEFAULT_SETTINGS,
                    ...loaded,
                    shortcuts: {
                        ...DEFAULT_SETTINGS.shortcuts,
                        ...(loaded.shortcuts || {})
                    }
                };
                setSettings(merged);
                // Save merged settings back to store
                await store.set('settings', merged);
            }
            setReady(true);
        })();
    }, []);

    const set = useCallback(async (path: string, value: any) => {
        const parts = path.split('.');
        const updated = structuredClone(settings);
        let node = updated as any;
        parts.forEach((p, i) => {
            if (i === parts.length - 1) node[p] = value;
            else node = node[p];
        });

        const store = await loadOrGetStore();
        await store.set('settings', updated);
        setSettings(updated);
    }, [settings]);

    return (
        <SettingsContext.Provider value={{ ready, settings, set }}>
            {children}
        </SettingsContext.Provider>
    );
}

export function useSettings() {
    const context = useContext(SettingsContext);
    if (context === undefined) {
        throw new Error('useSettings must be used within a SettingsProvider');
    }
    return context;
}
