// useMenuEvents.ts - Listen for global menu actions from Tauri
import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { listen } from '@tauri-apps/api/event';

export function useMenuEvents() {
    const navigate = useNavigate();

    useEffect(() => {
        const unlisten = listen<string>('menu-action', (event) => {
            const action = event.payload;

            switch (action) {
                // View navigation
                case 'view_home':
                    navigate('/');
                    break;
                case 'view_details':
                    navigate('/detail');
                    break;
                case 'view_history':
                    navigate('/history');
                    break;

                // File actions - will be handled by components
                case 'add_url':
                    // Emit to open add URL dialog
                    window.dispatchEvent(new CustomEvent('menu:add-url'));
                    break;
                case 'add_clipboard':
                    // Paste from clipboard
                    navigator.clipboard.readText().then(text => {
                        if (text) {
                            window.dispatchEvent(new CustomEvent('menu:add-url', { detail: text }));
                        }
                    }).catch(() => { });
                    break;

                // Downloads actions - emit to download context
                case 'start_all':
                case 'pause_all':
                case 'resume_all':
                case 'cancel_all':
                    window.dispatchEvent(new CustomEvent(`menu:${action.replace('_', '-')}`));
                    break;

                // Queue actions
                case 'create_queue':
                    window.dispatchEvent(new CustomEvent('menu:create-queue'));
                    break;
                case 'manage_queues':
                    window.dispatchEvent(new CustomEvent('menu:manage-queues'));
                    break;

                // Toggle sidebar
                case 'toggle_sidebar':
                    window.dispatchEvent(new CustomEvent('menu:toggle-sidebar'));
                    break;

                // Help actions
                case 'documentation':
                    window.open('https://github.com/greykaizen/tur', '_blank');
                    break;
                case 'shortcuts':
                    window.dispatchEvent(new CustomEvent('menu:shortcuts'));
                    break;
                case 'about':
                    window.dispatchEvent(new CustomEvent('menu:about'));
                    break;

                default:
                    console.log('[MenuEvents] Unhandled action:', action);
            }
        });

        return () => {
            unlisten.then(fn => fn());
        };
    }, [navigate]);
}
