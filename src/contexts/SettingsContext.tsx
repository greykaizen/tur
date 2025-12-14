import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';

export type AppSettings = {
    app: {
        show_tray_icon: boolean;
        quit_on_close: boolean;
        sidebar: 'left' | 'right';
        theme: 'light' | 'dark' | 'system';
        button_label: 'text' | 'icon' | 'both';
        show_download_progress: boolean;
        show_segment_progress: boolean;
        autostart: boolean;
    };
    shortcuts: {
        go_home: string;
        open_settings: string;
        add_download: string;
        open_details: string;
        open_history: string;
        toggle_sidebar: string;
        cancel_download: string;
        quit_app: string;
    };
    download: {
        download_location: string;
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
    send_anonymous_metrics: boolean;
    show_notifications: boolean;
};

const DEFAULT_SETTINGS: AppSettings = {
    app: {
        show_tray_icon: true,
        quit_on_close: false,
        sidebar: 'left',
        theme: 'system',
        button_label: 'both',
        show_download_progress: true,
        show_segment_progress: true,
        autostart: false,
    },
    shortcuts: {
        go_home: 'Ctrl+K',
        open_settings: 'Ctrl+P',
        add_download: 'Ctrl+N',
        open_details: 'Ctrl+D',
        open_history: 'Ctrl+H',
        toggle_sidebar: 'Ctrl+L',
        cancel_download: 'Ctrl+C',
        quit_app: 'Ctrl+Q',
    },
    download: {
        download_location: '',
        num_threads: 8,
        chunk_size: 16,
        socket_buffer_size: 0,
        speed_limit: 0,
    },
    thread: {
        total_connections: 1,
        per_task_connections: 1,
    },
    session: {
        history: false,
        metadata: false,
    },
    send_anonymous_metrics: false,
    show_notifications: true,
};

const UI_CACHE_KEY = 'tur_ui_settings';

function getCachedUISettings(): Partial<AppSettings['app']> | null {
    try {
        const cached = localStorage.getItem(UI_CACHE_KEY);
        return cached ? JSON.parse(cached) : null;
    } catch {
        return null;
    }
}

function cacheUISettings(app: AppSettings['app']) {
    try {
        localStorage.setItem(UI_CACHE_KEY, JSON.stringify({
            theme: app.theme,
            sidebar: app.sidebar,
        }));
    } catch {
        // localStorage not available
    }
}

interface SettingsContextType {
    ready: boolean;
    settings: AppSettings;
    set: (path: string, value: unknown) => Promise<void>;
}

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export function SettingsProvider({ children }: { children: ReactNode }) {
    const cachedUI = getCachedUISettings();
    const initialSettings: AppSettings = {
        ...DEFAULT_SETTINGS,
        app: {
            ...DEFAULT_SETTINGS.app,
            ...(cachedUI || {}),
        },
    };

    const [ready, setReady] = useState(false);
    const [settings, setSettings] = useState<AppSettings>(initialSettings);

    useEffect(() => {
        (async () => {
            try {
                const loaded = await invoke<AppSettings>('get_settings');
                setSettings(loaded);
                cacheUISettings(loaded.app);
            } catch (err) {
                console.error('Failed to load settings:', err);
            }
            setReady(true);
        })();
    }, []);

    const set = useCallback(async (path: string, value: unknown) => {
        try {
            await invoke('update_setting', { key: path, value });
            
            const parts = path.split('.');
            const updated = structuredClone(settings);
            let node = updated as Record<string, unknown>;
            
            for (let i = 0; i < parts.length - 1; i++) {
                node = node[parts[i]] as Record<string, unknown>;
            }
            node[parts[parts.length - 1]] = value;
            
            setSettings(updated);
            
            if (path.startsWith('app.')) {
                cacheUISettings(updated.app);
            }
            
            if (path === 'app.autostart') {
                await invoke('set_autostart', { enabled: value });
            }
        } catch (err) {
            console.error('Failed to update setting:', err);
            throw err;
        }
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
