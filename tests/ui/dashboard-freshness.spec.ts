import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

test.use({ timezoneId: 'Asia/Shanghai' });

for (const language of ['zh', 'en']) {
    test(`scan time differs from usage time and returning rescans immediately (${language})`, async ({ page }, info) => {
        await page.clock.install({ time: new Date('2026-10-12T07:00:00Z') });
        await page.clock.pauseAt(new Date('2026-10-12T07:00:10Z'));
        const install = () => {
            const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
            const totals = { input_tokens: 100, output_tokens: 50, cached_tokens: 0, total_tokens: 150, request_count: 1 };
            w.__usageReads = 0;
            w.__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
                if (command === 'get_local_token_usage') {
                    w.__usageReads++;
                    return { today: totals, yesterday: totals, last_3_days: totals, last_7_days: totals, last_30_days: totals,
                        daily: [], hourly: [], by_model_today: [], by_model_yesterday: [], by_model_3_days: [], by_model_7_days: [], by_model: [],
                        unreadable_databases: 0, last_activity: Date.parse('2026-10-12T06:19:49Z') / 1000, generated_at: Date.now() / 1000 };
                }
                if (command === 'get_api_pricing') return { prices: [], source: 'fixture' };
                return original(command, args);
            };
        };
        await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language })});(${install.toString()})();` });
        await page.goto('/');
        const status = page.locator('[data-usage-freshness]');
        const reads = () => page.evaluate(() => (window as any).__usageReads);
        await expect(status).toContainText(language === 'zh' ? '已扫描 15:00:10' : 'Scanned 03:00:10 PM');
        await expect(status).toContainText(language === 'zh' ? '最新用量记录 14:19:49' : 'Latest usage record 02:19:49 PM');
        // Language initialization can trigger another initial scan.
        await expect(page.getByRole('button', { name: language === 'zh' ? '刷新' : 'Refresh', exact: true })).toBeEnabled();
        const initialReads = await reads();
        expect(initialReads).toBeGreaterThan(0);
        await page.clock.fastForward(60_000);
        await expect.poll(reads).toBe(initialReads + 1);
        await page.evaluate(() => {
            Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true });
            document.dispatchEvent(new Event('visibilitychange'));
            window.dispatchEvent(new Event('focus'));
        });
        await page.clock.fastForward(60_000);
        expect(await reads()).toBe(initialReads + 1);
        await page.evaluate(() => {
            Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true });
            document.dispatchEvent(new Event('visibilitychange'));
        });
        await expect.poll(reads).toBe(initialReads + 2);
        await expect(status).toContainText(language === 'zh' ? '已扫描 15:02:10' : 'Scanned 03:02:10 PM');
        await expect(status).toContainText(language === 'zh' ? '最新用量记录 14:19:49' : 'Latest usage record 02:19:49 PM');
        await page.evaluate(() => window.dispatchEvent(new Event('focus')));
        await expect.poll(reads).toBe(initialReads + 3);
        await page.setViewportSize({ width: 760, height: 700 });
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
        await page.screenshot({ path: info.outputPath(`dashboard-freshness-${language}.png`) });
    });
}
