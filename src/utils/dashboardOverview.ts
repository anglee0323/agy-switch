import type { DashboardSnapshot } from './accountDashboard';
import { aggregateMenuBar, familyOf, menuBarAccount, reportedQuotaDetails, type QuotaFamily, type QuotaWindow } from './menuBarOverview';
import { DEFAULT_MENU_BAR_PREFERENCES, type MenuBarPreferences } from '../types/config';

/** Read-only projection of the same cached quota observations used by the menu.
 * Unknown observations are never counted as zero or as a failed login. */
export function dashboardOverview(snapshot: DashboardSnapshot, preferences: MenuBarPreferences, now: number, staleMinutes = 15, reserve = 10) {
    const families: QuotaFamily[] = preferences.quota_scope === 'all' ? ['gemini', 'other'] : [preferences.quota_scope];
    const threshold = Number.isFinite(reserve) && reserve >= 1 && reserve <= 99 ? reserve : 10;
    const views = snapshot.accounts.map(account => menuBarAccount(account, now, staleMinutes));
    let available = 0, unavailable = 0;
    for (const view of views) {
        if (view.account.read_status !== 'loaded') continue;
        const values = (['5h', 'weekly'] as const).flatMap(window => families.map(family => view.windows[window][family].remaining));
        if (!view.switchable || values.some(value => value !== null && value <= threshold)) unavailable++;
        else if (values.every(value => value !== null && value > threshold)) available++;
    }
    const total = snapshot.indexed_total;
    const accounts = { total, available, unavailable, unknown: Math.max(0, total - available - unavailable) };
    const visible = views.filter(view => !(preferences.hide_unavailable ?? DEFAULT_MENU_BAR_PREFERENCES.hide_unavailable) || view.switchable);
    const windows = Object.fromEntries((['5h', 'weekly'] as const).map(window => {
        const summary = aggregateMenuBar(visible, preferences.quota_scope, window, threshold);
        return [window, { ...summary, used: summary.remaining === null ? null : 100 - summary.remaining }];
    })) as Record<QuotaWindow, ReturnType<typeof aggregateMenuBar> & { used: number | null }>;
    const resets = visible.flatMap(view => reportedQuotaDetails(view.account).flatMap(pool => {
        const family = familyOf(pool.name);
        if (pool.source !== 'group' || !family || !families.includes(family)) return [];
        return pool.windows.flatMap(row => {
            const window = row.window.trim().toLowerCase();
            // Validate the whole family/window with the shared menu projection,
            // then exclude each full pool rather than relying on its family mean.
            if ((window !== '5h' && window !== 'weekly') || view.windows[window][family].remaining === null
                || row.remaining === null || row.remaining >= 100) return [];
            return [{ at: Date.parse(row.resetTime), window, id: view.account.id }];
        });
    }));
    const at = resets.length ? Math.min(...resets.map(reset => reset.at)) : null;
    const next = resets.filter(reset => reset.at === at);
    const reset = at === null ? null : {
        at, window: new Set(next.map(reset => reset.window)).size > 1 ? 'mixed' as const : next[0].window,
        accounts: new Set(next.map(reset => reset.id)).size,
    };
    return { accounts, windows, reset, threshold };
}

export function resetCountdown(at: number, now: number) {
    const minutes = Math.max(1, Math.ceil((at - now) / 60_000));
    return minutes >= 1440 ? { key: 'reset_days_hours', days: Math.floor(minutes / 1440), hours: Math.floor(minutes % 1440 / 60) }
        : minutes >= 60 ? { key: 'reset_hours_minutes', hours: Math.floor(minutes / 60), minutes: minutes % 60 }
        : { key: 'reset_minutes', minutes };
}
