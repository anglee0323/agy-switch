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
const { dashboardOverview } = await import(url(compile('../src/utils/dashboardOverview.ts')
    .replaceAll("'./menuBarOverview'", JSON.stringify(menu)).replaceAll("'../types/config'", JSON.stringify(config))));
const { makeDashboardSnapshot } = await import(url(compile('../tests/ui/dashboard-overview-fixture.ts')));
const { DASHBOARD_CARD_IDS, dashboardCards, dashboardCardOrder, dashboardGridClass } = await import(config);
const { averageRequestInput } = await import(url(compile('../src/utils/dashboardUsage.ts')));
const now = Date.parse('2026-10-06T00:00:00Z');
const project = (snapshot, patch = {}, time = now, reserve = 10) => dashboardOverview(snapshot, { quota_scope: 'all', ...patch }, time, 15, reserve);
let passed = 0;
const test = (name, fn) => { fn(); console.log(`PASS ${name}`); passed++; };
test('full option order preserves hidden positions and migrates legacy selection once', () => {
    const selected = ['body_speed', 'total_tokens'];
    const migrated = dashboardCardOrder(selected);
    assert.deepEqual(migrated.slice(0, 2), selected);
    assert.equal(migrated.length, 10);
    assert.deepEqual(dashboardCardOrder([], migrated), migrated);
    assert.deepEqual(dashboardCardOrder(['api_cost'], migrated), migrated);
    assert.deepEqual(dashboardCardOrder(selected, ['unknown']), migrated);
    const aliases = dashboardCardOrder([], ['quota_reset', 'average_input', 'unknown', 'api_cost']);
    assert.deepEqual(aliases.slice(0, 2), ['average_input', 'api_cost']);
    assert.equal(aliases.length, 10);
});
test('ten-card catalog retains explicit five-card order, empty selection and deduplication', () => {
    assert.equal(DASHBOARD_CARD_IDS.length, 10);
    const saved = ['body_speed', 'api_cost', 'total_tokens', 'first_text_latency', 'cache_hit_rate'];
    assert.deepEqual(dashboardCards(saved), saved); assert.deepEqual(dashboardCards([]), []);
    assert.deepEqual(dashboardCards(['quota_reset', 'average_input', 'bad', 'aggregate_quota', 'account_status']), ['average_input', 'aggregate_quota', 'account_status']);
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
    assert.equal(result.accounts.unknown, 5); assert.equal(result.windows['5h'].used, null);
    snapshot.accounts = []; snapshot.indexed_total = 0;
    assert.deepEqual(project(snapshot).accounts, { total: 0, available: 0, unavailable: 0, unknown: 0 });
});
test('ambiguous buckets and invalid future observations cannot report usable quota', () => {
    const snapshot = makeDashboardSnapshot(now); snapshot.accounts = [snapshot.accounts[0]]; snapshot.indexed_total = 1;
    snapshot.accounts[0].quota.groups[1].buckets[0].bucket_id = snapshot.accounts[0].quota.groups[0].buckets[0].bucket_id;
    assert.equal(project(snapshot).windows['5h'].used, null); assert.equal(project(snapshot).accounts.unknown, 1);
    snapshot.accounts[0].quota.last_updated = now / 1000 + 301;
    assert.equal(project(snapshot).windows['5h'].used, null);
});
test('mean request input includes system/input/cache tokens and pools request counts', () => {
    assert.equal(averageRequestInput({ input_tokens: 100_000, cached_tokens: 200_000, request_count: 4 }), 75_000);
    assert.equal(averageRequestInput({ input_tokens: 7, cached_tokens: 0, request_count: 2 }), 4);
    assert.equal(averageRequestInput({ input_tokens: 0, cached_tokens: 0, request_count: 1 }), 0);
});
test('no request count or invalid input is unknown rather than a zero-sized request', () => {
    for (const totals of [
        { input_tokens: 0, cached_tokens: 0, request_count: 0 },
        { input_tokens: 100, cached_tokens: 0, request_count: 0 },
        { input_tokens: 100, cached_tokens: NaN, request_count: 2 },
        { input_tokens: -1, cached_tokens: 100, request_count: 2 },
    ]) assert.equal(averageRequestInput(totals), null);
});
test('projection does not mutate saved observations or return identifiers and credentials', () => {
    const snapshot = makeDashboardSnapshot(now); const before = JSON.stringify(snapshot);
    const output = JSON.stringify(project(snapshot)); assert.equal(JSON.stringify(snapshot), before);
    assert.doesNotMatch(output, /email|example\.invalid|access_token|refresh_token|pool-|secondary|normal/);
});
console.log(`Dashboard overview: ${passed} passed`);
