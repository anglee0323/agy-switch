import { dashboardPools, type DashboardAccount, type DashboardSnapshot, type DashboardPool } from './accountDashboard';
import { DEFAULT_MENU_BAR_PREFERENCES, type MenuBarPreferences, type MenuBarQuotaScope } from '../types/config';

export type QuotaWindow = '5h' | 'weekly';
export type QuotaFamily = 'gemini' | 'other';
export function quotaTone(value: number | null, preferences: MenuBarPreferences): 'healthy' | 'warning' | 'critical' | 'unknown' {
  if (value === null || !Number.isFinite(value) || value < 0 || value > 100) return 'unknown';
  if (value > (preferences.green_above ?? DEFAULT_MENU_BAR_PREFERENCES.green_above)) return 'healthy';
  return value < (preferences.red_below ?? DEFAULT_MENU_BAR_PREFERENCES.red_below) ? 'critical' : 'warning';
}
export type MenuBarSnapshot = Omit<DashboardSnapshot, 'current_identity_source'> & {
  current_identity_source: 'tools_record' | 'running_app' | 'unavailable';
};
export type QuotaReason = 'disabled' | 'blocked' | 'forbidden' | 'unreadable' | 'stale' | 'protected' | 'unknown' | 'expired' | 'conflict';
export interface FamilyQuota {
  remaining: number | null;
  reason: QuotaReason | null;
  resets: string[];
  pools: number;
}
export interface AccountQuotaView {
  account: DashboardAccount;
  switchable: boolean;
  windows: Record<QuotaWindow, Record<QuotaFamily, FamilyQuota>>;
}
export const familyOf = (name: string): QuotaFamily | null =>
  /gemini/i.test(name) ? 'gemini' : /claude|gpt/i.test(name) ? 'other' : null;
const unknown = (reason: QuotaReason): FamilyQuota => ({ remaining: null, reason, resets: [], pools: 0 });
const validReset = (value: string, now: number) => Number.isFinite(Date.parse(value)) && Date.parse(value) > now;

export function menuBarAccount(account: DashboardAccount, now: number, staleMinutes = 15): AccountQuotaView {
  const blocked = account.validation_blocked && (!account.validation_blocked_until || account.validation_blocked_until * 1000 > now);
  const unavailable: QuotaReason | null = account.read_status !== 'loaded' ? 'unreadable'
    : account.disabled ? 'disabled' : blocked ? 'blocked' : account.quota?.is_forbidden ? 'forbidden' : null;
  const timestamp = account.quota?.last_updated;
  const interval = Number.isFinite(staleMinutes) && staleMinutes > 0 ? staleMinutes : 15;
  const stale = !timestamp || !Number.isFinite(timestamp) || timestamp * 1000 > now + 300_000
    || now - timestamp * 1000 > Math.max(60_000, interval * 60_000);
  const blockedData = unavailable || (stale ? 'stale' : account.protected_models.length ? 'protected' : null);
  const pools = dashboardPools(account.quota).filter(pool => pool.source === 'group');
  // A bucket named as two independent families is ambiguous, even if the two
  // observations agree. The dashboard deduplicates it; aggregation must not guess ownership.
  const owners = new Map<string, Set<QuotaFamily>>();
  for (const group of account.quota?.groups || []) {
    const family = familyOf(group.display_name);
    if (!family) continue;
    for (const bucket of group.buckets) {
      if (!bucket.bucket_id) continue;
      const key = JSON.stringify([bucket.bucket_id, bucket.window.trim().toLowerCase()]);
      const families = owners.get(key) || new Set<QuotaFamily>();
      families.add(family); owners.set(key, families);
    }
  }
  function quota(family: QuotaFamily, window: QuotaWindow): FamilyQuota {
    if (blockedData) return unknown(blockedData);
    if ([...owners].some(([key, values]) => values.size > 1 && values.has(family) && JSON.parse(key)[1] === window)) return unknown('conflict');
    const rows = pools.filter(pool => familyOf(pool.name) === family)
      .flatMap(pool => pool.windows.filter(row => row.window.trim().toLowerCase() === window));
    if (!rows.length || rows.some(row => row.remaining === null || row.key.startsWith('unidentified:'))) return unknown(rows.some(row => row.conflict) ? 'conflict' : 'unknown');
    if (rows.some(row => !validReset(row.resetTime, now))) return unknown(rows.some(row => Number.isFinite(Date.parse(row.resetTime)) && Date.parse(row.resetTime) <= now) ? 'expired' : 'unknown');
    return { remaining: rows.reduce((sum, row) => sum + (row.remaining as number), 0) / rows.length,
      reason: null, resets: [...new Set(rows.map(row => row.resetTime))], pools: rows.length };
  }
  return { account, switchable: !unavailable,
    windows: { '5h': { gemini: quota('gemini', '5h'), other: quota('other', '5h') }, weekly: { gemini: quota('gemini', 'weekly'), other: quota('other', 'weekly') } } };
}

/** Account/family observations receive equal weight. This is relative mean
 * headroom, never a token capacity estimate or a sum of independent allowances. */
export function aggregateMenuBar(accounts: AccountQuotaView[], scope: MenuBarQuotaScope, window: QuotaWindow, threshold = 10) {
  const families: QuotaFamily[] = scope === 'all' ? ['gemini', 'other'] : [scope];
  const valid = accounts.filter(view => families.every(family => view.windows[window][family].remaining !== null));
  const values = valid.flatMap(view => families.map(family => view.windows[window][family].remaining as number));
  const cutoff = Number.isFinite(threshold) && threshold >= 1 && threshold <= 99 ? threshold : 10;
  return { remaining: values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : null,
    usable: valid.filter(view => families.every(family => (view.windows[window][family].remaining as number) > cutoff)).length,
    covered: valid.length, total: accounts.length };
}

export function quotaDisplay(value: number | null): string {
  if (value === null) return '—';
  return `${Math.round(value)}%`;
}

export function reportedQuotaDetails(account: DashboardAccount): DashboardPool[] {
  return dashboardPools(account.quota);
}
