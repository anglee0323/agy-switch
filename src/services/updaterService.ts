import { request } from '../utils/request';
export interface UpdateInfo { current_version: string; latest_version: string; has_update: boolean; release_url: string }
export const checkForUpdates = () => request<UpdateInfo>('check_for_updates');
export const openRelease = (url: string) => {
    if (!/^https:\/\/github\.com\/anglee0323\/agy-switch\/releases\/tag\/v?\d+\.\d+\.\d+$/.test(url)) return Promise.reject(new Error('invalid_release'));
    return request('plugin:opener|open_url', { url });
};

export interface UpdateProgress { stage: 'downloading' | 'installing'; downloaded: number; total: number | null }
export const downloadAndInstallUpdate = async (expectedVersion: string, onProgress: (progress: UpdateProgress) => void) => {
    const { Channel } = await import('@tauri-apps/api/core');
    const progress = new Channel<UpdateProgress>();
    progress.onmessage = onProgress;
    await request('download_and_install_update', { expectedVersion, progress });
};
export const getRunningVersion = () => request<string>('get_running_version');
