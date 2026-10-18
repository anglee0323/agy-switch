import type { Account, QuotaData } from "../types/account";
import { quotaPoolKind } from './accountDashboard';

/** Shared by account cards and tables; preserve dedicated and future pool names. */
export function weeklyQuotaItems(quota?: QuotaData) {
  return (quota?.quota_groups || []).flatMap(group => group.buckets
    .filter(b => b.window.trim().toLowerCase() === 'weekly' || b.bucket_id.toLowerCase().endsWith('-weekly'))
    .map(b => {
      const kind = quotaPoolKind(b.bucket_id, group.display_name);
      return { id: `${group.display_name}-${b.bucket_id}`, title: kind === 'argon' ? 'Gemini 4 Argon' : kind === 'gemini' ? 'Gemini' : kind === 'other' ? 'Claude / GPT' : group.display_name || b.display_name || b.bucket_id,
        shared: kind === 'gemini' || kind === 'other', isClaude: kind === 'other',
        percentage: observedPercentage(b.remaining_fraction, b.remaining_fraction_known, true), resetTime: b.reset_time };
    }));
}

export interface CompactQuotaRow {
  id: string;
  label: string;
  window?: string;
  remaining: number | null;
  resetTime: string;
}
export interface CompactQuotaGroup {
  name: string;
  rows: CompactQuotaRow[];
}

export function validPercentage(
  value: unknown,
  fraction = false,
): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  const scaled = fraction ? value * 100 : value;
  return scaled >= 0 && scaled <= 100 ? Math.round(scaled) : null;
}

/** Legacy zeros are ambiguous: previous parsers substituted zero for missing API data. */
export function observedPercentage(value: unknown, known?: boolean, fraction = false): number | null {
  if (known === false || (known === undefined && value === 0)) return null;
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  const scaled = fraction ? value * 100 : value;
  return scaled >= 0 && scaled <= 100 ? scaled : null;
}

/** Independent pools stay independent. Unknown data never becomes an empty bar. */
export function compactQuotaGroups(
  quota?: QuotaData,
  pinned: string[] = [],
): CompactQuotaGroup[] {
  if (!quota || quota.is_forbidden) return [];
  const groups = quota.quota_groups
    ?.filter((group) => group.buckets?.length)
    .map((group, groupIndex) => ({
      name: group.display_name,
      rows: group.buckets.map((bucket, index) => ({
        id: `${groupIndex}-${bucket.bucket_id || index}`,
        label: bucket.display_name || bucket.window,
        window: bucket.window,
        remaining: observedPercentage(bucket.remaining_fraction, bucket.remaining_fraction_known, true),
        resetTime: bucket.reset_time,
      })),
    }));
  if (groups?.length) return groups;
  const models = quota.models || [];
  const selected = pinned.length
    ? models.filter((model) => pinned.includes(model.name))
    : [];
  return (selected.length ? selected : models).map((model) => ({
    name: model.display_name || model.name,
    rows: [
      {
        id: model.name,
        label: model.display_name || model.name,
        remaining: model.observed_remaining_fraction !== undefined
          ? observedPercentage(model.observed_remaining_fraction, model.percentage_known, true)
          : observedPercentage(model.percentage, model.percentage === 0 ? undefined : model.percentage_known),
        resetTime: model.reset_time,
      },
    ],
  }));
}

export function lowestKnownQuota(account: Account): number | null {
  const known = compactQuotaGroups(account.quota)
    .flatMap((group) => group.rows)
    .map((row) => row.remaining)
    .filter((v): v is number => v !== null);
  return known.length ? Math.min(...known) : null;
}

export function isAccountSwitchable(
  account: Account,
  now = Date.now(),
): boolean {
  const blocked =
    account.validation_blocked &&
    (!account.validation_blocked_until ||
      account.validation_blocked_until * 1000 > now);
  return !account.disabled && !blocked;
}

export function isQuotaStale(
  timestamp?: number,
  intervalMinutes = 15,
  now = Date.now(),
): boolean {
  return (
    !timestamp ||
    !Number.isFinite(timestamp) ||
    timestamp * 1000 > now + 300_000 ||
    now - timestamp * 1000 > Math.max(intervalMinutes * 60_000, 60_000)
  );
}

export function resetTimestamp(value: string): number | null {
  if (!value) return null;
  const valueMs = Date.parse(value);
  return Number.isFinite(valueMs) ? valueMs : null;
}

export interface PagedQuotaRow extends CompactQuotaRow {
  group: string;
}
/** Bounded pages keep every data item accessible without an unbounded popover. */
export function quotaPages(
  groups: CompactQuotaGroup[],
  pageSize = 4,
): PagedQuotaRow[][] {
  const size = Math.max(1, Math.floor(pageSize));
  const rows = groups.flatMap((group) =>
    group.rows.map((row) => ({ ...row, group: group.name })),
  );
  return Array.from({ length: Math.ceil(rows.length / size) }, (_, index) =>
    rows.slice(index * size, (index + 1) * size),
  );
}

export function pageSlice<T>(items: T[], page: number, pageSize = 4): T[] {
  const size = Math.max(1, Math.floor(pageSize));
  const lastPage = Math.max(0, Math.ceil(items.length / size) - 1);
  const index = Math.max(0, Math.min(Math.floor(page), lastPage));
  return items.slice(index * size, (index + 1) * size);
}

export function pageSizeForHeight(height: number): number {
  if (height >= 560) return 6;
  if (height >= 440) return 4;
  if (height >= 360) return 3;
  if (height >= 300) return 2;
  return 1;
}

export type AccountReadiness = "healthy" | "low" | "unavailable" | "unknown";
export interface OverviewPool {
  key: string;
  name: string;
  windows: string[];
  total: number;
  usable: number;
  low: number;
  unavailable: number;
  unknown: number;
}
export interface AccountOverview {
  statuses: Record<string, AccountReadiness>;
  total: number;
  healthy: number;
  low: number;
  unavailable: number;
  unknown: number;
  pools: OverviewPool[];
}
export const quotaThreshold = (value?: number): number =>
  typeof value === "number" &&
  Number.isFinite(value) &&
  value >= 1 &&
  value <= 99
    ? value
    : 10;

function unavailableAccount(account: Account, now: number): boolean {
  return (
    !isAccountSwitchable(account, now) || Boolean(account.quota?.is_forbidden)
  );
}
function knownCurrentRows(rows: CompactQuotaRow[], now: number): boolean {
  return (
    rows.length > 0 &&
    rows.every(
      (row) =>
        row.remaining !== null &&
        (resetTimestamp(row.resetTime) === null ||
          (resetTimestamp(row.resetTime) as number) > now),
    )
  );
}
export function accountReadiness(
  account: Account,
  threshold = 10,
  interval = 15,
  now = Date.now(),
): AccountReadiness {
  if (unavailableAccount(account, now)) return "unavailable";
  if (
    isQuotaStale(account.quota?.last_updated, interval, now) ||
    account.protected_models?.length
  )
    return "unknown";
  const rows = compactQuotaGroups(account.quota).flatMap((group) => group.rows);
  if (!knownCurrentRows(rows, now)) return "unknown";
  return rows.some(
    (row) => (row.remaining as number) <= quotaThreshold(threshold),
  )
    ? "low"
    : "healthy";
}

/** Counts are over ALL saved accounts. Independent pool percentages are never summed.
 * Missing windows, stale/expired data and locally protected scopes are not usable.
 */
export function summarizeAccounts(
  accounts: Account[],
  threshold = 10,
  interval = 15,
  now = Date.now(),
): AccountOverview {
  const result: AccountOverview = {
    statuses: {},
    total: accounts.length,
    healthy: 0,
    low: 0,
    unavailable: 0,
    unknown: 0,
    pools: [],
  };
  const pools = new Map<string, { name: string; windows: Set<string> }>();
  const accountPools = accounts.map((account) => {
    const grouped = Boolean(
      account.quota?.quota_groups?.some((group) => group.buckets?.length),
    );
    const values = new Map<string, CompactQuotaRow[]>();
    for (const group of compactQuotaGroups(account.quota)) {
      const key = `${grouped ? "pool" : "model"}:${group.name.trim().toLowerCase()}`;
      const definition = pools.get(key) || {
        name: group.name,
        windows: new Set<string>(),
      };
      group.rows.forEach((row) =>
        definition.windows.add(row.window || "model"),
      );
      pools.set(key, definition);
      values.set(key, [...(values.get(key) || []), ...group.rows]);
    }
    return values;
  });
  accounts.forEach((account, index) => {
    let state = accountReadiness(account, threshold, interval, now);
    if (state === "healthy" || state === "low") {
      const incomplete = [...accountPools[index]].some(([key, rows]) => {
        const reported = new Set(rows.map((row) => row.window || "model"));
        return [...(pools.get(key)?.windows || [])].some(
          (window) => !reported.has(window),
        );
      });
      if (incomplete) state = "unknown";
    }
    result.statuses[account.id] = state;
    result[state]++;
  });
  for (const [key, definition] of pools) {
    const row: OverviewPool = {
      key,
      name: definition.name,
      windows: [...definition.windows],
      total: accounts.length,
      usable: 0,
      low: 0,
      unavailable: 0,
      unknown: 0,
    };
    accounts.forEach((account, index) => {
      if (unavailableAccount(account, now)) {
        row.unavailable++;
        return;
      }
      const values = accountPools[index].get(key) || [];
      const windows = new Set(values.map((value) => value.window || "model"));
      const protectedScope =
        Boolean(account.protected_models?.length) &&
        (key.startsWith("pool:") ||
          values.some((value) => account.protected_models?.includes(value.id)));
      if (
        isQuotaStale(account.quota?.last_updated, interval, now) ||
        protectedScope ||
        !knownCurrentRows(values, now) ||
        [...definition.windows].some((window) => !windows.has(window))
      ) {
        row.unknown++;
      } else if (
        values.some(
          (value) => (value.remaining as number) <= quotaThreshold(threshold),
        )
      ) {
        row.low++;
      } else {
        row.usable++;
      }
    });
    result.pools.push(row);
  }
  return result;
}
