import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
const compile = path => ts.transpileModule(readFileSync(new URL(path, import.meta.url), 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.ES2022, target: ts.ScriptTarget.ES2022 },
}).outputText;
const url = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const config = url(compile('../src/types/config.ts'));
const account = url(compile('../src/utils/accountDashboard.ts'));
const menu = url(compile('../src/utils/menuBarOverview.ts').replaceAll("'./accountDashboard'", JSON.stringify(account)).replaceAll("'../types/config'", JSON.stringify(config)));
const { dashboardOverview, resetCountdown } = await import(url(compile('../src/utils/dashboardOverview.ts')
    .replaceAll("'./menuBarOverview'", JSON.stringify(menu)).replaceAll("'../types/config'", JSON.stringify(config))));
const { makeDashboardSnapshot } = await import(url(compile('../tests/ui/dashboard-overview-fixture.ts')));
const { DASHBOARD_CARD_IDS, dashboardCards, dashboardGridClass } = await import(config);
const now = Date.parse('2026-10-06T00:00:00Z');
const project = (snapshot, patch = {}, time = now, reserve = 10) => dashboardOverview(snapshot, { quota_scope: 'all', ...patch }, time, 15, reserve);
let passed = 0;
const test = (name, fn) => { fn(); console.log(`PASS ${name}`); passed++; };
test('ten-card catalog retains explicit five-card order, empty selection and deduplication', () => {
    assert.equal(DASHBOARD_CARD_IDS.length, 10);
    const saved = ['body_speed', 'api_cost', 'total_tokens', 'first_text_latency', 'cache_hit_rate'];
    assert.deepEqual(dashboardCards(saved), saved); assert.deepEqual(dashboardCards([]), []);
    assert.deepEqual(dashboardCards(['quota_reset', 'quota_reset', 'bad', 'aggregate_quota', 'account_status']), ['quota_reset', 'aggregate_quota', 'account_status']);
    assert.equal(dashboardCards(undefined).length, 10);
    assert.match(dashboardGridClass(10), /xl:grid-cols-5/); assert.match(dashboardGridClass(8), /lg:grid-cols-4/);
});
test('account status covers known usable, disabled, exhausted and unknown accounts', () => {
    const result = project(makeDashboardSnapshot(now));
    assert.deepEqual(result.accounts, { total: 5, available: 2, unavailable: 2, unknown: 1 });
});
test('used percentages are complements of the same menu mean, including observed zero', () => {
    const result = project(makeDashboardSnapshot(now));
    assert.ok(Math.abs(result.windows['5h'].used - 100 * 8 / 15) < 1e-10);
    assert.ok(Math.abs(result.windows.weekly.used - 100 * 2 / 3) < 1e-10);
    assert.equal(result.windows['5h'].covered, 3); assert.equal(result.windows['5h'].total, 4);
    assert.equal(project(makeDashboardSnapshot(now), { hide_unavailable: false }).windows['5h'].total, 5);
});
test('scope and reserve affect readiness without changing the quota mean', () => {
    const snapshot = makeDashboardSnapshot(now);
    assert.equal(project(snapshot, { quota_scope: 'gemini' }).windows['5h'].used, 40);
    assert.deepEqual(project(snapshot, {}, now, 30).accounts, { total: 5, available: 1, unavailable: 3, unknown: 1 });
    assert.equal(project(snapshot, { quota_scope: 'gemini' }, now, 30).accounts.available, 2);
    assert.deepEqual(project(snapshot, {}, now, NaN).accounts, project(snapshot).accounts);
});
test('missing windows and unreadable/protected accounts stay unknown; known low still unavailable', () => {
    const snapshot = makeDashboardSnapshot(now);
    snapshot.accounts[0].quota.groups.forEach(group => group.buckets = group.buckets.filter(bucket => bucket.window === 'weekly'));
    snapshot.accounts[1].protected_models = ['gemini']; snapshot.accounts[2].read_status = 'failed';
    assert.deepEqual(project(snapshot).accounts, { total: 5, available: 0, unavailable: 1, unknown: 4 });
    snapshot.accounts[0].quota.groups[0].buckets[0].remaining_fraction = 0;
    assert.equal(project(snapshot).accounts.unavailable, 2);
});
test('missing all data is unknown, while an empty snapshot reports an accurate zero count', () => {
    const snapshot = makeDashboardSnapshot(now); snapshot.accounts.forEach(account => { account.quota.last_updated -= 3600; account.disabled = false; });
    const result = project(snapshot);
    assert.equal(result.accounts.unknown, 5); assert.equal(result.windows['5h'].used, null); assert.equal(result.reset, null);
    snapshot.accounts = []; snapshot.indexed_total = 0;
    assert.deepEqual(project(snapshot).accounts, { total: 0, available: 0, unavailable: 0, unknown: 0 });
});
test('nearest recovery counts distinct accounts, excludes full/stale/disabled pools and retains known zero', () => {
    const snapshot = makeDashboardSnapshot(now);
    assert.deepEqual(project(snapshot).reset, { at: now + 3 * 3600000, window: '5h', accounts: 3 });
    snapshot.accounts[1].quota.groups.forEach(group => group.buckets.forEach(bucket => bucket.remaining_fraction = 1));
    snapshot.accounts[1].quota.groups[0].buckets[0].reset_time = new Date(now + 60000).toISOString();
    snapshot.accounts[2].quota.groups[0].buckets[0].reset_time = new Date(now + 30000).toISOString();
    assert.deepEqual(project(snapshot).reset, { at: now + 3 * 3600000, window: '5h', accounts: 2 });
});
test('scope, simultaneous windows and passed resets do not invent a recovery', () => {
    const snapshot = makeDashboardSnapshot(now); const first = snapshot.accounts[0].quota.groups[0];
    first.buckets[1].reset_time = first.buckets[0].reset_time;
    assert.equal(project(snapshot).reset.window, 'mixed');
    first.buckets[0].reset_time = new Date(now + 60000).toISOString();
    assert.equal(project(snapshot, { quota_scope: 'gemini' }).reset.at, now + 60000);
    assert.equal(project(snapshot, { quota_scope: 'other' }).reset.at, now + 3 * 3600000);
    assert.equal(project(snapshot, {}, now + 3 * 3600000).reset, null);
});
test('a full pool is excluded even when another pool in its family/window is partially used', () => {
    const snapshot = makeDashboardSnapshot(now);
    snapshot.accounts[0].quota.groups[0].buckets.push({ bucket_id: 'full', window: '5h', remaining_fraction: 1,
        reset_time: new Date(now + 60000).toISOString() });
    assert.equal(project(snapshot).reset.at, now + 3 * 3600000);
});
test('ambiguous buckets and invalid future observations cannot report usable quota or resets', () => {
    const snapshot = makeDashboardSnapshot(now); snapshot.accounts = [snapshot.accounts[0]]; snapshot.indexed_total = 1;
    snapshot.accounts[0].quota.groups[1].buckets[0].bucket_id = snapshot.accounts[0].quota.groups[0].buckets[0].bucket_id;
    assert.equal(project(snapshot).windows['5h'].used, null); assert.equal(project(snapshot).accounts.unknown, 1);
    snapshot.accounts[0].quota.last_updated = now / 1000 + 301;
    assert.equal(project(snapshot).reset, null);
});
test('countdown rounds up minutes and retains hours and days', () => {
    assert.deepEqual(resetCountdown(now + 1, now), { key: 'reset_minutes', minutes: 1 });
    assert.deepEqual(resetCountdown(now + 61 * 60000, now), { key: 'reset_hours_minutes', hours: 1, minutes: 1 });
    assert.deepEqual(resetCountdown(now + 27 * 3600000, now), { key: 'reset_days_hours', days: 1, hours: 3 });
});
test('projection does not mutate saved observations or return identifiers and credentials', () => {
    const snapshot = makeDashboardSnapshot(now); const before = JSON.stringify(snapshot);
    const output = JSON.stringify(project(snapshot)); assert.equal(JSON.stringify(snapshot), before);
    assert.doesNotMatch(output, /email|example\.invalid|access_token|refresh_token|pool-|secondary|normal/);
});
console.log(`Dashboard overview: ${passed} passed`);
