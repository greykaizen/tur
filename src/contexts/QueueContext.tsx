import { createContext, useContext, useState, useEffect, ReactNode, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface Queue {
    id: string;
    name: string;
    color?: string;
    mode: 'sequential' | 'concurrent';
    parallel_count: number;
    status: 'active' | 'paused' | 'completed';
    created_at: number;
}

interface QueueContextType {
    queues: Queue[];
    refreshQueues: () => Promise<void>;
    createQueue: (name: string, color: string | undefined, mode: 'sequential' | 'concurrent', parallelCount: number) => Promise<Queue>;
    deleteQueue: (id: string) => Promise<void>;
    addToQueue: (downloadId: string, queueId: string, position?: number) => Promise<void>;
    removeFromQueue: (downloadId: string) => Promise<void>;
    updateQueueStatus: (queueId: string, status: string) => Promise<void>;
}

const QueueContext = createContext<QueueContextType | undefined>(undefined);

export function QueueProvider({ children }: { children: ReactNode }) {
    const [queues, setQueues] = useState<Queue[]>([]);

    const refreshQueues = useCallback(async () => {
        try {
            const data = await invoke<Queue[]>('get_queues');
            setQueues(data);
        } catch (error) {
            console.error('Failed to fetch queues:', error);
        }
    }, []);

    const createQueue = useCallback(async (
        name: string,
        color: string | undefined,
        mode: 'sequential' | 'concurrent',
        parallelCount: number
    ) => {
        const queue = await invoke<Queue>('create_queue', {
            name,
            color,
            mode,
            parallelCount
        });
        await refreshQueues();
        return queue;
    }, [refreshQueues]);

    const deleteQueue = useCallback(async (id: string) => {
        await invoke('delete_queue', { id });
        await refreshQueues();
    }, [refreshQueues]);

    const addToQueue = useCallback(async (downloadId: string, queueId: string, position?: number) => {
        await invoke('add_to_queue', { downloadId, queueId, position: position ?? null });
        await refreshQueues();
    }, [refreshQueues]);

    const removeFromQueue = useCallback(async (downloadId: string) => {
        await invoke('remove_from_queue', { downloadId });
        await refreshQueues();
    }, [refreshQueues]);

    const updateQueueStatus = useCallback(async (queueId: string, status: string) => {
        await invoke('update_queue_status', { queueId, status });
        await refreshQueues();
    }, [refreshQueues]);

    useEffect(() => {
        refreshQueues();
    }, [refreshQueues]);

    return (
        <QueueContext.Provider value={{
            queues,
            refreshQueues,
            createQueue,
            deleteQueue,
            addToQueue,
            removeFromQueue,
            updateQueueStatus
        }}>
            {children}
        </QueueContext.Provider>
    );
}

export function useQueues() {
    const context = useContext(QueueContext);
    if (context === undefined) {
        throw new Error('useQueues must be used within a QueueProvider');
    }
    return context;
}
