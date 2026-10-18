/** Contract for get_account_dashboard_snapshot. Contains no credentials or activation action. */
export interface DashboardQuota {
  last_updated: number;
  is_forbidden: boolean;
  subscription_tier: string | null;
  provenance: "legacy_cache" | "observed";
  models: { name: string; display_name: string | null; percentage: number | null; reset_time: string; inferred_bucket_id: string | null }[];
  groups: { display_name: string; buckets: { bucket_id: string; window: string; remaining_fraction: number | null; reset_time: string }[] }[] | null;
}
export interface DashboardAccount {
  id: string; email: string; name: string | null; custom_label: string | null;
  read_status: "loaded" | "failed"; read_error: string | null;
  disabled: boolean; validation_blocked: boolean; validation_blocked_until: number | null;
  protected_models: string[]; quota: DashboardQuota | null;
}
export interface DashboardSnapshot {
  indexed_total: number; loaded_count: number; failed_count: number;
  current_account_id: string | null; current_identity_source: "tools_record";
  accounts: DashboardAccount[];
}
export type QuotaPoolKind = 'gemini' | 'other' | 'argon';
/** Keep in sync with models::quota::quota_pool. IDs outrank display labels. */
export function quotaPoolKind(bucketId: string, groupName: string): QuotaPoolKind | null {
  const id = bucketId.trim().toLowerCase();
  if (id === 'gemini-5h' || id === 'gemini-weekly') return 'gemini';
  if (id === '3p-5h' || id === '3p-weekly') return 'other';
  if (id === 'gemini-argon-wtu-weekly') return 'argon';
  if (id.startsWith('gemini-') || id.startsWith('3p-')) return null;
  const name = groupName.trim().toLowerCase();
  if (name === 'gemini' || name === 'gemini models') return 'gemini';
  if (['claude', 'claude / gpt', 'claude/gpt', 'claude/gpt models', 'claude and gpt models'].includes(name)) return 'other';
  return name === 'gemini 4 argon' ? 'argon' : null;
}
export interface DashboardPool {
  key: string; name: string; source: "group" | "model";
  mapping: "inferred" | "unmapped"; models: string[];
  windows: { key: string; kind: QuotaPoolKind | null; window: string; remaining: number | null; resetTime: string; conflict: boolean }[];
}
const percentage = (v: unknown, fraction = false): number | null =>
  typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= (fraction ? 1 : 100)
    ? v * (fraction ? 100 : 1) : null;

/** Keys identify response presentation rows, not API pool identities. Shared bucket
 * observations deduplicate globally within the account, regardless of group coverage/name. */
export function dashboardPools(quota: DashboardQuota | null): DashboardPool[] {
  if (!quota || quota.is_forbidden) return [];
  const pools: DashboardPool[] = [];
  const observations = new Map<string, { owner: DashboardPool; row: DashboardPool["windows"][number] }>();
  const bucketOwners = new Map<string, Set<DashboardPool>>();
  (quota.groups || []).forEach((group, groupIndex) => {
    const pool: DashboardPool = { key: `reported-group:${groupIndex}`, name: group.display_name, source: "group", mapping: "unmapped", models: [], windows: [] };
    group.buckets.forEach((bucket, bucketIndex) => {
      const key = bucket.bucket_id ? JSON.stringify([bucket.bucket_id, bucket.window.trim().toLowerCase()]) : `unidentified:${groupIndex}:${bucketIndex}`;
      const remaining = percentage(bucket.remaining_fraction, true);
      let existing = observations.get(key);
      if (existing) {
        const kind = quotaPoolKind(bucket.bucket_id, group.display_name);
        if (existing.row.kind === null) existing.row.kind = kind;
        // A second group cannot create a second copy of the same bucket/window.
        if (existing.row.remaining !== remaining || existing.row.resetTime !== bucket.reset_time || (kind !== null && existing.row.kind !== kind)) {
          existing.row.remaining = null;
          existing.row.conflict = true;
        }
      } else {
        const row = { key, kind: quotaPoolKind(bucket.bucket_id, group.display_name), window: bucket.window, remaining, resetTime: bucket.reset_time, conflict: false };
        pool.windows.push(row);
        existing = { owner: pool, row };
        observations.set(key, existing);
      }
      if (bucket.bucket_id) {
        const owners = bucketOwners.get(bucket.bucket_id) || new Set<DashboardPool>();
        owners.add(existing.owner);
        bucketOwners.set(bucket.bucket_id, owners);
      }
    });
    pools.push(pool);
  });
  const models = new Map<string, DashboardPool>();
  quota.models.forEach((model, index) => {
    const owners = model.inferred_bucket_id ? bucketOwners.get(model.inferred_bucket_id) : undefined;
    if (owners?.size === 1) {
      const shared = [...owners][0];
      shared.mapping = "inferred";
      if (!shared.models.includes(model.name)) shared.models.push(model.name);
      return;
    }
    const key = `model:${model.name}`;
    const remaining = model.inferred_bucket_id ? null : percentage(model.percentage);
    const existing = models.get(key);
    if (existing) {
      if (existing.windows[0].remaining !== remaining || existing.windows[0].resetTime !== model.reset_time) {
        existing.windows[0].remaining = null;
        existing.windows[0].conflict = true;
      }
      return;
    }
    const pool: DashboardPool = { key, name: model.display_name || model.name || `Model ${index + 1}`, source: "model", mapping: "unmapped", models: [model.name],
      windows: [{ key, kind: null, window: "model", remaining, resetTime: model.reset_time, conflict: false }] };
    models.set(key, pool);
    pools.push(pool);
  });
  return pools.filter(pool => pool.windows.length || pool.models.length);
}
export function dashboardAccountState(account: DashboardAccount, now = Date.now(), staleMinutes = 15) {
  const authentication = account.read_status === "failed" ? "unknown"
    : account.disabled ? "disabled"
    : account.validation_blocked && (!account.validation_blocked_until || account.validation_blocked_until * 1000 > now)
      ? "verification_required" : "not_verified";
  const quota = account.quota;
  const timestamp = quota?.last_updated;
  const freshness = timestamp == null || timestamp === 0 ? "missing"
    : !Number.isFinite(timestamp) || timestamp < 0 || timestamp * 1000 > now + 300_000 ? "invalid"
    : now - timestamp * 1000 > Math.max(60_000, (Number.isFinite(staleMinutes) && staleMinutes > 0 ? staleMinutes : 15) * 60_000) ? "stale" : "fresh";
  const pools = dashboardPools(quota);
  const values = pools.flatMap(p => p.windows);
  // The existing grouped-summary contract is weekly + 5h. Other window labels
  // are preserved but their coverage cannot be established from this response.
  const incompleteGroups = Boolean(quota?.groups?.some(g => {
    const windows = new Set(g.buckets.map(b => b.window.trim().toLowerCase()));
    const weeklyOnly = g.buckets.length > 0 && g.buckets.every(b => quotaPoolKind(b.bucket_id, g.display_name) === 'argon');
    return (!weeklyOnly && !windows.has("5h")) || !windows.has("weekly")
      || [...windows].some(window => window !== "5h" && window !== "weekly");
  }));
  const unresolvedMapping = Boolean(quota?.groups?.length && pools.some(p => p.source === "model"));
  const quotaState = quota?.is_forbidden ? "forbidden"
    : !values.length ? "unknown"
    : values.some(v => v.remaining === null) || incompleteGroups || unresolvedMapping ? "partial"
    : values.some(v => v.remaining === 0) ? "exhausted" : "reported";
  const resetExpired = values.some(v => {
    const reset = Date.parse(v.resetTime);
    return Number.isFinite(reset) && reset <= now;
  });
  return { authentication, quota: quotaState, freshness, resetExpired, provenance: quota?.provenance || "none", pools };
}
/** Trusted totals describe file reads, never total token usage or summed percentages. */
export function dashboardCounts(snapshot: DashboardSnapshot) {
  return { indexed: snapshot.indexed_total, loaded: snapshot.loaded_count, failed: snapshot.failed_count,
    currentAccountId: snapshot.current_account_id, currentIdentitySource: snapshot.current_identity_source };
}
