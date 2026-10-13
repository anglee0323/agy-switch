import { listen } from '@tauri-apps/api/event';
import { request } from './request';
import { ensureFullViewState } from './windowManager';

interface MainWindowRequest {
    request_id: number;
    route: '/' | '/accounts' | '/settings' | null;
}

// Rust owns visibility. A committed React page acknowledges only an explicit
// open request; mounting in the background must never reveal the main window.
export async function installMainWindowPresentation(navigate: (route: string) => Promise<void> | void): Promise<() => void> {
    let disposed = false;
    let latest = 0;
    // Restore native properties once while still hidden, not on every menu
    // click (changing decorations can recreate the native title-bar surface).
    await ensureFullViewState();
    const prepare = async (intent: MainWindowRequest | null) => {
        if (!intent || disposed || intent.request_id <= latest) return;
        latest = intent.request_id;
        try {
            if (intent.route) {
                if (!['/', '/accounts', '/settings'].includes(intent.route)) return;
                await navigate(intent.route);
            }
            // Do not wait for hidden-window animation frames: WebKit can
            // suspend them. Navigation uses flushSync to commit the page first.
            if (!disposed && latest === intent.request_id) {
                await request('present_main_window', { requestId: intent.request_id });
            }
        } catch (error) {
            console.error('Failed to prepare main window:', error);
        }
    };
    const unlisten = await listen<MainWindowRequest>('app://present-main', event => { void prepare(event.payload); });
    void request<MainWindowRequest | null>('main_window_ready').then(prepare).catch(error => {
        console.error('Failed to register main window readiness:', error);
    });
    return () => { disposed = true; unlisten(); };
}
