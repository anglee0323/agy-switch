import { expect, test, type Page } from '@playwright/test';
import { settleKeyboardDrag, setupSettingsFixture } from './settings-fixture';

const ids = ['total_tokens', 'input_tokens', 'output_tokens', 'cache_hit_rate', 'api_cost', 'first_text_latency', 'body_speed'];
const catalog = [...ids, 'account_status', 'aggregate_quota', 'average_input'];

async function setup(page: Page, cards = ids, language = 'zh') {
    const usage = () => {
        const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
        const totals = { input_tokens: 0, output_tokens: 0, cached_tokens: 0, total_tokens: 0, request_count: 0 };
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
            if (command === 'get_local_token_usage') return {
                today: totals, yesterday: totals, last_3_days: totals, last_7_days: totals, last_30_days: totals,
                by_model_today: [], by_model_yesterday: [], by_model_3_days: [], by_model_7_days: [], by_model: [],
                daily: [], hourly: [], unreadable_databases: 0, generated_at: 1, recent_performance: null,
            };
            if (command === 'get_api_pricing') return { prices: [], source: 'fixture' };
            return original(command, args);
        };
    };
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language, dashboardCards: cards })});(${usage.toString()})();` });
    await page.goto('/settings');
}

async function savedCards(page: Page) {
    return page.evaluate(async () => (await (window as any).__TAURI_INTERNALS__.invoke('load_config')).dashboard.cards);
}

async function expectCards(page: Page, cards: string[]) {
    await expect.poll(() => page.locator('[data-dashboard-card]').evaluateAll(rows => rows.map(row => row.getAttribute('data-dashboard-card')))).toEqual(cards);
}

for (const language of ['zh', 'en']) test(`checkboxes keep all ten options in place and restore their display position (${language})`, async ({ page }) => {
    await setup(page, catalog, language);
    const section = page.getByRole('region', { name: language === 'zh' ? '首页卡片' : 'Dashboard cards', exact: true });
    const rows = section.locator('[data-card-option]');
    const order = () => rows.evaluateAll(nodes => nodes.map(node => node.getAttribute('data-card-option')));
    for (const width of [1046, 420]) {
        await page.setViewportSize({ width, height: 800 });
        const checkbox = section.locator('[data-card-option="api_cost"] input');
        await checkbox.scrollIntoViewIfNeeded();
        const geometry = () => rows.evaluateAll(nodes => nodes.map(node => {
            const rect = node.getBoundingClientRect(), parent = node.closest('section')!.getBoundingClientRect();
            return { x: rect.x - parent.x, y: rect.y - parent.y };
        }));
        const before = await geometry();
        await checkbox.uncheck(); await expect(checkbox).toBeEnabled();
        expect(await order()).toEqual(catalog);
        const after = await geometry();
        for (let i = 0; i < before.length; i++) {
            expect(Math.abs(before[i].x - after[i].x)).toBeLessThan(1);
            expect(Math.abs(before[i].y - after[i].y)).toBeLessThan(1);
        }
        await expect(checkbox).toBeFocused();
        await page.keyboard.press('Space'); await expect(checkbox).toBeChecked(); await expect(checkbox).toBeEnabled();
        expect(await order()).toEqual(catalog);
        await expect.poll(() => savedCards(page)).toEqual(catalog);
        await page.setViewportSize({ width: 1046, height: 800 });
        await page.locator('a[href="/"]').first().click(); await expectCards(page, catalog);
        await page.locator('a[href="/settings"]').first().click(); await expect.poll(order).toEqual(catalog);
    }
});

for (const count of [2, 3, 4, 6]) {
    test(`${count} selected cards persist and fill the available row`, async ({ page }) => {
        await setup(page);
        const section = page.getByRole('region', { name: '首页卡片', exact: true });
        await expect(section.getByRole('checkbox')).toHaveCount(10);
        for (const id of ids.slice(count)) {
            const checkbox = section.locator(`[data-card-option="${id}"] input`);
            await expect(checkbox).toBeEnabled(); await checkbox.uncheck();
        }
        await expect.poll(() => savedCards(page)).toEqual(ids.slice(0, count));
        await page.locator('a[href="/"]').first().click();
        await expectCards(page, ids.slice(0, count));
        expect(await page.locator('[data-dashboard-cards]').evaluate(el => getComputedStyle(el).gridTemplateColumns.split(' ').length)).toBe(Math.min(count, 5));
        await page.locator('a[href="/settings"]').first().click();
        await expect(section.getByRole('checkbox', { checked: true })).toHaveCount(count);
    });
}

test('pointer and keyboard dragging save order without selecting hidden cards', async ({ page }) => {
    await setup(page, ['first_text_latency', 'body_speed', 'total_tokens']);
    const section = page.getByRole('region', { name: '首页卡片', exact: true });
    await expect(section.locator('[data-card-option="input_tokens"] button')).toBeEnabled();
    const speed = section.locator('[data-card-option="body_speed"] button');
    const latency = section.locator('[data-card-option="first_text_latency"] button');
    await latency.scrollIntoViewIfNeeded();
    const from = (await speed.boundingBox())!, to = (await latency.boundingBox())!;
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down(); await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 12 }); await page.mouse.up();
    await expect.poll(() => savedCards(page)).toEqual(['body_speed', 'first_text_latency', 'total_tokens']);
    await expect(speed).toBeEnabled(); await speed.focus(); await page.keyboard.press('Space');
    await expect(speed).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByRole('status').filter({ hasText: '移动到正文速度的位置' })).toBeVisible();
    await settleKeyboardDrag(page);
    await page.keyboard.press('ArrowRight');
    await expect(page.getByRole('status').filter({ hasText: '移动到首字延迟的位置' })).toBeVisible();
    await page.keyboard.press('Space');
    await expect.poll(() => savedCards(page)).toEqual(['first_text_latency', 'body_speed', 'total_tokens']);
    await page.locator('a[href="/"]').first().click(); await expectCards(page, ['first_text_latency', 'body_speed', 'total_tokens']);
    await page.screenshot({ path: 'test-results/auto-switch/dashboard-three-cards.png' });
    await page.setViewportSize({ width: 760, height: 520 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('failed saves roll back and retry without overlapping writes', async ({ page }) => {
    await setup(page);
    const section = page.getByRole('region', { name: '首页卡片', exact: true });
    const checkbox = section.locator('[data-card-option="body_speed"] input');
    await page.evaluate(() => { (window as any).__settingsFixture.holdDashboardSave = true; });
    await checkbox.uncheck(); await expect(checkbox).toBeDisabled();
    await expect(section.getByRole('checkbox', { disabled: true })).toHaveCount(10);
    await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.dashboardSavePending())).toBe(true);
    await page.evaluate(() => (window as any).__settingsFixture.rejectDashboardSave());
    await expect(section.getByRole('alert')).toContainText('synthetic card save rejection');
    await expect(checkbox).toBeChecked(); await expect(checkbox).toBeEnabled();
    expect(await savedCards(page)).toEqual(ids);
    await checkbox.uncheck(); await expect(checkbox).toBeDisabled();
    await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.dashboardSavePending())).toBe(true);
    await page.evaluate(() => (window as any).__settingsFixture.resolveDashboardSave());
    await expect(checkbox).toBeEnabled(); await expect(section.getByRole('alert')).toHaveCount(0);
    await expect.poll(() => savedCards(page)).toEqual(ids.slice(0, -1));
});

test('hidden cards keep their stored position across navigation and can be reordered without enabling them', async ({ page }) => {
    await setup(page, ['first_text_latency', 'body_speed', 'total_tokens']);
    const section = page.getByRole('region', { name: '首页卡片', exact: true });
    const input = section.locator('[data-card-option="input_tokens"] input');
    const handle = section.locator('[data-card-option="input_tokens"] button');
    await handle.scrollIntoViewIfNeeded(); await handle.focus(); await page.keyboard.press('Space');
    await expect(page.getByRole('status').filter({ hasText: '移动到输入 Token的位置' })).toBeVisible();
    await settleKeyboardDrag(page);
    await page.keyboard.press('ArrowUp');
    await expect(page.getByRole('status').filter({ hasText: '移动到正文速度的位置' })).toBeVisible();
    await page.keyboard.press('Space'); await expect(handle).toBeEnabled();
    expect(await savedCards(page)).toEqual(['first_text_latency', 'body_speed', 'total_tokens']);
    await expect(input).not.toBeChecked();
    const order = () => section.locator('[data-card-option]').evaluateAll(rows => rows.map(row => row.getAttribute('data-card-option')));
    await expect.poll(async () => (await order()).slice(0, 4)).toEqual(['first_text_latency', 'input_tokens', 'body_speed', 'total_tokens']);
    await input.check(); await expect(input).toBeEnabled();
    expect(await savedCards(page)).toEqual(['first_text_latency', 'input_tokens', 'body_speed', 'total_tokens']);
    await input.uncheck(); await expect(input).toBeEnabled();
    await page.locator('a[href="/"]').first().click();
    await expectCards(page, ['first_text_latency', 'body_speed', 'total_tokens']);
    await page.locator('a[href="/settings"]').first().click();
    await expect.poll(async () => (await order()).slice(0, 4)).toEqual(['first_text_latency', 'input_tokens', 'body_speed', 'total_tokens']);
    await input.check(); await expect(input).toBeEnabled();
    expect(await savedCards(page)).toEqual(['first_text_latency', 'input_tokens', 'body_speed', 'total_tokens']);
});

test('a delayed appearance save cannot restore old cards', async ({ page }) => {
    await setup(page);
    await page.evaluate(() => { (window as any).__settingsFixture.holdGeneralSave = true; });
    await page.getByRole('button', { name: '深色', exact: true }).click();
    await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((c: any) => c.command === 'save_config').length)).toBe(1);
    const checkbox = page.locator('[data-card-option="body_speed"] input');
    await checkbox.uncheck(); await expect(checkbox).toBeEnabled();
    await page.evaluate(() => (window as any).__settingsFixture.resolveGeneralSave());
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    await expect(checkbox).not.toBeChecked();
    await page.locator('a[href="/"]').first().click(); await expectCards(page, ids.slice(0, -1));
    expect(await savedCards(page)).toEqual(ids.slice(0, -1));
});

for (const language of ['zh', 'en']) test(`empty selection keeps charts and settings are localized (${language})`, async ({ page }) => {
    await setup(page, [], language);
    const section = page.getByRole('region', { name: language === 'zh' ? '首页卡片' : 'Dashboard cards', exact: true });
    await expect(section.getByRole('checkbox', { checked: true })).toHaveCount(0);
    expect(await section.innerText()).not.toMatch(/[·•]/);
    if (language === 'en') expect(await section.innerText()).not.toMatch(/[\u3400-\u9fff]/);
    await page.screenshot({ path: `test-results/auto-switch/dashboard-settings-${language}.png` });
    await page.locator('a[href="/"]').first().click(); await expectCards(page, []);
    await expect(page.locator('[data-dashboard-cards]')).toHaveCount(0);
    await expect(page.getByRole('heading', { name: language === 'zh' ? '模型分布' : 'Model breakdown', exact: true })).toBeVisible();
});

for (const language of ['zh', 'en']) test(`compact card options and readable menu settings adapt to window width (${language})`, async ({ page }) => {
    await setup(page, ids, language);
    const cards = page.getByRole('region', { name: language === 'zh' ? '首页卡片' : 'Dashboard cards', exact: true });
    const menu = page.getByRole('region', { name: language === 'zh' ? '菜单栏显示' : 'Menu bar display', exact: true });
    for (const width of [1046, 420]) {
        await page.setViewportSize({ width, height: 700 });
        await cards.scrollIntoViewIfNeeded();
        const rows = cards.locator('[data-card-option]');
        const first = (await rows.nth(0).boundingBox())!, second = (await rows.nth(1).boundingBox())!;
        if (width > 600) { expect(Math.abs(first.y - second.y)).toBeLessThan(2); expect(first.height).toBeLessThan(52); }
        else expect(second.y).toBeGreaterThan(first.y + first.height);
        expect(await cards.locator('label span').first().evaluate(el => parseFloat(getComputedStyle(el).fontSize))).toBeGreaterThanOrEqual(14);
        await page.screenshot({ path: `test-results/auto-switch/cards-compact-${language}-${width}.png` });
        await menu.scrollIntoViewIfNeeded();
        expect(await menu.locator('select').evaluateAll(fields => fields.every(el => el.getBoundingClientRect().height >= 40 && parseFloat(getComputedStyle(el).fontSize) >= 14))).toBe(true);
        expect(await menu.locator('p').evaluateAll(fields => fields.every(el => parseFloat(getComputedStyle(el).fontSize) >= 14))).toBe(true);
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
        await page.screenshot({ path: `test-results/auto-switch/menu-settings-readable-${language}-${width}.png` });
    }
});
