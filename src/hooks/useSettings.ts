import { useState, useEffect, useCallback } from 'react';
import { Store, getStore as getStoreInstance } from '@tauri-apps/plugin-store';
import { appConfigDir } from '@tauri-apps/api/path';
import { join } from '@tauri-apps/api/path';


export type AppSettings = {
    app: {
        show_tray_icon: boolean;
        quit_on_close: boolean;
        sidebar: 'left' | 'right';
        theme: 'light' | 'dark' | 'system';
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

export type BackendSettings = Pick<AppSettings, 'download' | 'thread' | 'session'>;

const DEFAULT_SETTINGS: AppSettings = {
    app: {
        show_tray_icon: true,
        quit_on_close: false,
        sidebar: 'left',
        theme: 'system'
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

    // Try using Tauri’s built-in smart cache:
    let store = await getStoreInstance(storePath);
    if (!store) {
        // not loaded yet — load it now with defaults + autosave
        store = await Store.load(storePath, {
            defaults: { settings: DEFAULT_SETTINGS },
            autoSave: true,
            overrideDefaults: false 
        });
    }
    return store;
}

export function useSettings() {
    const [ready, setReady] = useState(false);
    const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);

    useEffect(() => {
        (async () => {
            const store = await loadOrGetStore();
            const loaded = await store.get<AppSettings>('settings');
            if (loaded) setSettings(loaded);
            setReady(true);
        })();
    }, []);


    const get = useCallback(
        (path: string): any => {
            return path.split('.').reduce((prev: any, key: string) => prev?.[key], settings);
        },
        [settings]
    );

    const set = useCallback(async (path: string, value: any) => {
        const parts = path.split('.');
        // const root = parts.shift() as keyof AppSettings;

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

    const getBackendSettings = useCallback((): BackendSettings => ({
        download: settings.download,
        thread: settings.thread,
        session: settings.session
    }), [settings]);

    const submitWork = useCallback(async (work: string) => {
        const { invoke } = await import('@tauri-apps/api/core'); // v2
        await invoke('submit_work', {
            payload: { work, settings: getBackendSettings() }
        });
    }, [getBackendSettings]);

    return { ready, settings, get, set, getBackendSettings, submitWork };
}