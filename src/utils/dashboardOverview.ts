import type { DashboardSnapshot } from './accountDashboard';
import { aggregateMenuBar, menuBarAccount, type QuotaFamily, type QuotaWindow } from './menuBarOverview';
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
    return { accounts, windows, threshold };
}
