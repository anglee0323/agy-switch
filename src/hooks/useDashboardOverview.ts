import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { DashboardSnapshot } from '../utils/accountDashboard';
import { request } from '../utils/request';
import { isTauri } from '../utils/env';

/** Reads saved observations and policy only. Never refreshes remote quotas or
 * credentials, probes the running client's identity, or triggers switching. */
export function useDashboardOverview(enabled: boolean) {
    const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
    const [reserve, setReserve] = useState<number | null>(null);
    const [loading, setLoading] = useState(true);
    const [failed, setFailed] = useState(false);
    const [now, setNow] = useState(Date.now());
    const read = useRef<(() => Promise<void>) | null>(null);
    const refresh = useCallback(() => read.current?.(), []);
    useEffect(() => {
        if (!enabled) return;
        let active = true, busy = false, queued = false;
        const fetch = async () => {
            if (!active) return;
            if (busy) { queued = true; return; }
            busy = true;
            const [data, policy] = await Promise.allSettled([
                request<DashboardSnapshot>('get_account_dashboard_snapshot'),
                request<{ reserve_percentage: number }>('get_auto_switch_config'),
            ]);
            if (active) {
                setSnapshot(data.status === 'fulfilled' ? data.value : null);
                setReserve(policy.status === 'fulfilled' ? policy.value.reserve_percentage : null);
                setFailed(data.status === 'rejected');
                setLoading(false);
                setNow(Date.now());
            }
            busy = false;
            if (queued && active) { queued = false; void fetch(); }
        };
        setSnapshot(null);
        setFailed(false);
        setNow(Date.now());
        setLoading(true);
        read.current = fetch;
        void fetch();
        const timer = window.setInterval(() => {
            if (document.visibilityState !== 'hidden') void fetch();
        }, 60_000);
        const clock = window.setInterval(() => setNow(Date.now()), 15_000);
        const listeners = isTauri() ? ['menubar://data-updated', 'tray://account-switched', 'accounts://refreshed']
            .map(event => listen(event, () => void fetch())) : [];
        return () => {
            active = false;
            read.current = null;
            window.clearInterval(timer);
            window.clearInterval(clock);
            listeners.forEach(listener => { void listener.then(stop => stop()).catch(() => {}); });
        };
    }, [enabled]);
    return { snapshot, reserve, loading, failed, now, refresh };
}
