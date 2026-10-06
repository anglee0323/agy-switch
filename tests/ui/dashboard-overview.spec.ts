import { expect, test, type Page } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';
import { makeDashboardSnapshot } from './dashboard-overview-fixture';

const originalCards = ['total_tokens', 'input_tokens', 'output_tokens', 'cache_hit_rate', 'api_cost', 'first_text_latency', 'body_speed'];
const newCards = ['account_status', 'aggregate_quota', 'quota_reset'];
const allCards = [...originalCards, ...newCards];
async function setup(page: Page, cards = allCards, language = 'zh', withModels = false, quotaScope = 'all') {
    const install = (snapshot: ReturnType<typeof makeDashboardSnapshot>, withModels: boolean) => {
        const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
        w.__overviewFixture = { snapshot, fail: false, hold: false, resolve: null };
        const totals = { input_tokens: 0, output_tokens: 0, cached_tokens: 0, total_tokens: 0, request_count: 0 };
        const models = withModels ? [{ ...totals, model: 'gemini-dashboard-test' }] : [];
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
            if (command === 'get_local_token_usage') return { today: totals, yesterday: totals, last_3_days: totals, last_7_days: totals, last_30_days: totals,
                by_model_today: models, by_model_yesterday: models, by_model_3_days: models, by_model_7_days: models, by_model: models, daily: [], hourly: [], unreadable_databases: 0, generated_at: 1, recent_performance: null };
            if (command === 'get_api_pricing') return { prices: [], source: 'fixture' };
            if (command === 'get_account_dashboard_snapshot') {
                w.__settingsFixture.calls.push({ command, args });
                if (w.__overviewFixture.fail) throw 'synthetic snapshot failure';
                if (w.__overviewFixture.hold) return new Promise(resolve => { w.__overviewFixture.resolve = () => resolve(w.__overviewFixture.snapshot); });
                return structuredClone(w.__overviewFixture.snapshot);
            }
            return original(command, args);
        };
    };
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language, dashboardCards: cards, quotaScope })});(${install.toString()})((${makeDashboardSnapshot.toString()})(Date.now()),${withModels});` });
    await page.goto('/');
}

for (const language of ['zh', 'en']) for (const count of [5, 8, 10]) test(`model details remain reachable with ${count} cards in short windows (${language})`, async ({ page }) => {
    await setup(page, allCards.slice(0, count), language, true);
    const cell = page.getByRole('cell', { name: 'gemini-dashboard-test', exact: true });
    await expect(cell).toBeVisible();
    for (const width of [1440, 1046, 760, 420]) {
        await page.setViewportSize({ width, height: 520 });
        await cell.scrollIntoViewIfNeeded(); await expect(cell).toBeInViewport({ ratio: 1 });
        const geometry = await cell.evaluate(el => {
            const section = el.closest('section')!;
            return { section: section.clientHeight, scroller: section.querySelector('.overflow-y-auto')!.clientHeight };
        });
        if (width >= 1024) { expect(geometry.section).toBeGreaterThanOrEqual(158); expect(geometry.scroller).toBeGreaterThanOrEqual(64); }
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
        await page.screenshot({ path: `test-results/auto-switch/dashboard-model-details-${language}-${count}-${width}.png` });
    }
});

for (const language of ['zh', 'en']) test(`ten selectable cards remain readable and show shared quota summaries (${language})`, async ({ page }) => {
    await setup(page, allCards, language);
    const status = page.locator('[data-dashboard-card="account_status"]');
    const quota = page.locator('[data-dashboard-card="aggregate_quota"]');
    const reset = page.locator('[data-dashboard-card="quota_reset"]');
    await expect(status).toContainText(language === 'zh' ? '5 个账号' : '5 accounts');
    await expect(status).toContainText(language === 'zh' ? '可用 2，不可用 2，未知 1' : 'Available 2, unavailable 2, unknown 1');
    await expect(quota.locator('[data-quota-window="5h"]')).toContainText('53%');
    await expect(quota.locator('[data-quota-window="weekly"]')).toContainText('67%');
    await expect(quota.getByRole('meter', { name: language === 'zh' ? '5 小时已用' : '5-hour used', exact: true })).toHaveAttribute('aria-valuetext', '53%');
    await expect(quota.getByRole('meter', { name: language === 'zh' ? '周额度已用' : 'Weekly used', exact: true })).toHaveAttribute('aria-valuetext', '67%');
    await expect(reset).toContainText(language === 'zh' ? '3 小时 0 分' : '3h 0m');
    await expect(reset).toContainText(language === 'zh' ? '5 小时额度，3 个账号' : '5-hour quota, 3 account(s)');
    expect(await quota.getAttribute('title')).toContain('3/4');
    const range = page.getByRole('button', { name: language === 'zh' ? '近 30 天' : 'Last 30 days', exact: true });
    await range.click(); await expect(quota.locator('[data-quota-window="5h"]')).toContainText('53%');
    for (const width of [1440, 1046, 760, 420]) {
        await page.setViewportSize({ width, height: 520 });
        await expect(page.locator('[data-dashboard-card]')).toHaveCount(10);
        const columns = await page.locator('[data-dashboard-cards]').evaluate(el => getComputedStyle(el).gridTemplateColumns.split(' ').length);
        expect(columns).toBe(width >= 1280 ? 5 : width >= 768 ? 3 : 2);
        expect(await page.locator('[data-dashboard-card]').evaluateAll(cards => cards.every(card => {
            const detail = card.querySelector('[data-card-detail]');
            return card.scrollWidth <= card.clientWidth + 1 && (!detail || (detail.scrollHeight <= detail.clientHeight + 1 && getComputedStyle(detail).textOverflow !== 'ellipsis'));
        }))).toBe(true);
        const heights = await page.locator('[data-dashboard-card]').evaluateAll(cards => cards.map(card => card.getBoundingClientRect().height));
        expect(Math.max(...heights) - Math.min(...heights)).toBeLessThanOrEqual(1);
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
        await reset.scrollIntoViewIfNeeded(); await expect(reset).toBeVisible();
        await page.screenshot({ path: `test-results/auto-switch/dashboard-ten-${language}-${width}.png`, fullPage: true });
    }
    expect(await page.locator('[data-dashboard-cards]').innerText()).not.toMatch(/[·•]/);
    if (language === 'en') expect(await page.locator('[data-dashboard-cards]').innerText()).not.toMatch(/[\u3400-\u9fff]/);
    expect(await page.evaluate(() => (window as any).__settingsFixture.calls.map((call: any) => call.command))).not.toContain('refresh_all_quotas');
});

for (const language of ['zh', 'en']) test(`eight cards have equal dimensions without a quota scope footer (${language})`, async ({ page }) => {
    const saved = ['total_tokens', 'cache_hit_rate', 'first_text_latency', 'body_speed', 'api_cost', 'account_status', 'quota_reset', 'aggregate_quota'];
    await setup(page, saved, language, false, 'gemini');
    const quota = page.locator('[data-dashboard-card="aggregate_quota"]');
    await expect(quota.locator('[data-quota-window]')).toHaveCount(2);
    await expect(quota.locator('[data-card-detail]')).toHaveCount(0);
    await expect(quota).not.toContainText('Gemini');
    expect(await quota.getAttribute('title')).toContain('Gemini');
    for (const width of [1440, 1046, 760, 420]) {
        await page.setViewportSize({ width, height: 520 });
        const sizes = await page.locator('[data-dashboard-card]').evaluateAll(cards => cards.map(card => ({
            width: card.getBoundingClientRect().width, height: card.getBoundingClientRect().height,
        })));
        expect(sizes).toHaveLength(8);
        for (const dimension of ['width', 'height'] as const) {
            const values = sizes.map(size => size[dimension]);
            expect(Math.max(...values) - Math.min(...values)).toBeLessThanOrEqual(1);
        }
        const bars = await quota.getByRole('meter').evaluateAll(meters => meters.map(meter => ({
            expected: Number(meter.getAttribute('aria-valuenow')),
            shown: meter.firstElementChild!.getBoundingClientRect().width / meter.getBoundingClientRect().width * 100,
        })));
        expect(bars).toHaveLength(2);
        for (const bar of bars) expect(Math.abs(bar.expected - bar.shown)).toBeLessThan(0.5);
        await quota.scrollIntoViewIfNeeded(); await expect(quota).toBeInViewport({ ratio: 1 });
        await page.screenshot({ path: `test-results/auto-switch/dashboard-uniform-eight-${language}-${width}.png`, fullPage: true });
    }
    expect(await page.locator('[data-dashboard-card]').evaluateAll(cards => cards.map(card => card.getAttribute('data-dashboard-card')))).toEqual(saved);
});

test('quota meters distinguish unused, exhausted and unknown observations', async ({ page }) => {
    await setup(page, ['aggregate_quota']);
    const quota = page.locator('[data-dashboard-card="aggregate_quota"]');
    await expect(quota.getByRole('meter')).toHaveCount(2);
    for (const [remaining, expected] of [[1, '0'], [0, '100']]) {
        await page.evaluate(value => {
            (window as any).__overviewFixture.snapshot.accounts.forEach((account: any) => {
                account.quota.groups.forEach((group: any) => group.buckets.forEach((bucket: any) => { bucket.remaining_fraction = value; }));
            });
        }, remaining);
        await page.getByRole('button', { name: '刷新', exact: true }).click();
        for (const meter of await quota.getByRole('meter').all()) {
            await expect(meter).toHaveAttribute('aria-valuenow', expected);
            await expect(meter).toHaveAttribute('aria-valuetext', `${expected}%`);
        }
        await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
    }
    await page.evaluate(() => { (window as any).__overviewFixture.snapshot.accounts.forEach((account: any) => { account.quota.groups = []; }); });
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await expect(quota.getByRole('meter')).toHaveCount(0);
    await expect(quota.getByRole('img')).toHaveCount(2);
    await expect(quota).not.toContainText('0%');
});

test('existing five-card selection survives expansion; new choices and keyboard order persist', async ({ page }) => {
    const saved = ['total_tokens', 'cache_hit_rate', 'first_text_latency', 'body_speed', 'api_cost'];
    await setup(page, saved);
    await expect(page.locator('[data-dashboard-card]')).toHaveCount(5);
    expect(await page.evaluate(() => (window as any).__settingsFixture.calls.some((c: any) => c.command === 'get_account_dashboard_snapshot'))).toBe(false);
    await page.locator('a[href="/settings"]').first().click();
    const section = page.getByRole('region', { name: '首页卡片', exact: true });
    await expect(section.getByRole('checkbox')).toHaveCount(10);
    await expect(section.getByRole('checkbox', { checked: true })).toHaveCount(5);
    for (const id of newCards) {
        const checkbox = section.locator(`[data-card-option="${id}"] input`);
        await expect(checkbox).not.toBeChecked(); await checkbox.check(); await expect(checkbox).toBeEnabled();
    }
    for (const id of saved) { const checkbox = section.locator(`[data-card-option="${id}"] input`); await checkbox.uncheck(); await expect(checkbox).toBeEnabled(); }
    const handle = section.locator('[data-card-option="quota_reset"] button');
    await handle.scrollIntoViewIfNeeded(); await handle.focus(); await page.keyboard.press('Space');
    await expect(handle).toHaveAttribute('aria-pressed', 'true'); await page.keyboard.press('ArrowUp');
    await expect(page.getByRole('status').filter({ hasText: '移动到账号状态的位置' })).toBeVisible(); await page.keyboard.press('Space');
    const order = ['quota_reset', 'account_status', 'aggregate_quota'];
    await expect.poll(() => page.evaluate(async () => (await (window as any).__TAURI_INTERNALS__.invoke('load_config')).dashboard.cards)).toEqual(order);
    await page.locator('a[href="/"]').first().click();
    await expect.poll(() => page.locator('[data-dashboard-card]').evaluateAll(cards => cards.map(card => card.getAttribute('data-dashboard-card')))).toEqual(order);
});

test('read failure clears cached values, recovery reads locally, and no data is not zero quota', async ({ page }) => {
    await setup(page, newCards);
    const status = page.locator('[data-dashboard-card="account_status"]');
    const quota = page.locator('[data-dashboard-card="aggregate_quota"]');
    await expect(status).toContainText('可用 2');
    const initialCalls = await page.evaluate(() => (window as any).__settingsFixture.calls.length);
    await page.evaluate(() => { (window as any).__overviewFixture.fail = true; });
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await expect(status).toContainText('本地观测读取失败'); await expect(quota).not.toContainText('53%');
    await page.evaluate(() => {
        const f = (window as any).__overviewFixture; f.fail = false;
        f.snapshot.accounts.forEach((account: any) => { account.quota.last_updated -= 3600; account.disabled = false; });
    });
    await expect(page.getByRole('button', { name: '刷新', exact: true })).toBeEnabled();
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await expect(status).toContainText('可用 0，不可用 0，未知 5');
    await expect(quota.locator('[data-quota-window="5h"]')).toContainText('—');
    await expect(quota.getByRole('meter')).toHaveCount(0);
    await expect(quota.getByRole('img')).toHaveCount(2);
    await expect(page.locator('[data-dashboard-card="quota_reset"]')).toContainText('暂无有效恢复记录');
    const calls = await page.evaluate(() => (window as any).__settingsFixture.calls.map((c: any) => c.command));
    expect(calls.filter((c: string) => c === 'get_account_dashboard_snapshot')).toHaveLength(3);
    expect(calls.slice(initialCalls)).not.toContain('list_accounts'); expect(calls).not.toContain('refresh_all_quotas');
});

test('elapsed freshness expires displayed quota and recovery instead of retaining old values', async ({ page }) => {
    await page.clock.install({ time: new Date('2026-10-06T00:00:00Z') });
    await setup(page, newCards);
    await expect(page.locator('[data-dashboard-card="account_status"]')).toContainText('可用 2');
    await page.clock.fastForward(16 * 60_000);
    await expect(page.locator('[data-dashboard-card="account_status"]')).toContainText('可用 0，不可用 1，未知 4');
    await expect(page.locator('[data-dashboard-card="aggregate_quota"] [data-quota-window="5h"]')).toContainText('—');
    await expect(page.locator('[data-dashboard-card="quota_reset"]')).toContainText('暂无有效恢复记录');
});

test('leaving dashboard ignores an old read; returning uses new observations', async ({ page }) => {
    await setup(page, newCards);
    await expect(page.locator('[data-dashboard-card="account_status"]')).toContainText('可用 2');
    await page.evaluate(() => { (window as any).__overviewFixture.hold = true; });
    await page.getByRole('button', { name: '刷新', exact: true }).click();
    await expect.poll(() => page.evaluate(() => Boolean((window as any).__overviewFixture.resolve))).toBe(true);
    await page.locator('a[href="/settings"]').first().click();
    await page.evaluate(() => {
        const f = (window as any).__overviewFixture; f.hold = false;
        f.snapshot = { ...f.snapshot, indexed_total: 0, loaded_count: 0, accounts: [] }; f.resolve();
    });
    await page.locator('a[href="/"]').first().click();
    await expect(page.locator('[data-dashboard-card="account_status"]')).toContainText('0 个账号');
    await expect(page.locator('[data-dashboard-card="aggregate_quota"] [data-quota-window="5h"]')).toContainText('—');
});
