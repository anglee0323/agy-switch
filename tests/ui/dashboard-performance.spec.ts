import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

for (const language of ['zh', 'en']) {
    for (const available of [true, false]) {
        test(`recent response performance uses local summaries (${language}, ${available})`, async ({ page }) => {
            const override = (hasTiming: boolean) => {
                const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
                const totals = { input_tokens: 0, output_tokens: 0, cached_tokens: 0, total_tokens: 0, request_count: 0 };
                w.__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
                    if (command === 'get_local_token_usage') return {
                        today: totals, yesterday: totals, last_3_days: totals, last_7_days: totals, last_30_days: totals,
                        by_model_today: [], by_model_yesterday: [], by_model_3_days: [], by_model_7_days: [], by_model: [],
                        daily: [], hourly: [], unreadable_databases: 0, generated_at: 1,
                        recent_performance: hasTiming ? { model_count: 2, source_count: 2, sample_count: 7,
                            first_text_seconds: 4.75, body_tokens_per_second: 147.3, last_activity: 1 } : null,
                    };
                    if (command === 'get_api_pricing') return { prices: [], source: 'fixture' };
                    return original(command, args);
                };
            };
            await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language })});(${override.toString()})(${available});` });
            await page.goto('/');
            const label = language === 'zh' ? '首字延迟' : 'First-text latency';
            const latency = page.locator('[data-dashboard-card="first_text_latency"]');
            const speed = page.locator('[data-dashboard-card="body_speed"]');
            await expect(latency).toContainText(label);
            await expect(latency).toBeVisible();
            if (available) {
                await expect(latency).toContainText(language === 'zh' ? '4.75 秒' : '4.75 s');
                await expect(speed).toContainText('147.3 token/s');
                await expect(latency).not.toContainText(/Gemini|Claude/);
                await expect(latency).toContainText(language === 'zh' ? '最近 7 次回复的中位数' : 'Median of 7 recent responses');
                await expect(latency.locator('[data-card-detail]')).toHaveAttribute('title', language === 'zh' ? /2 个模型、2 个本地来源/ : /2 model\(s\), 2 local source\(s\)/);
                await page.getByRole('button', { name: language === 'zh' ? '近 30 天' : 'Last 30 days', exact: true }).click();
                await expect(speed).toContainText('147.3 token/s');
            } else {
                await expect(latency).toContainText('—');
                await expect(speed).toContainText('—');
                await expect(speed).toContainText(language === 'zh' ? '暂无有效样本' : 'No eligible samples');
            }
            if (language === 'en') expect(await page.locator('main').innerText()).not.toMatch(/[\u3400-\u9fff]/);
            await page.screenshot({ path: `test-results/auto-switch/performance-wide-${language}-${available}.png` });
            await page.setViewportSize({ width: 1024, height: 768 });
            await expect(speed).toBeVisible();
            expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
            await page.screenshot({ path: `test-results/auto-switch/performance-${language}-${available}.png` });
            for (const width of [1440, 1046, 760, 420]) {
                await page.setViewportSize({ width, height: 520 });
                for (const card of [latency, speed]) {
                    await card.scrollIntoViewIfNeeded(); await expect(card).toBeVisible();
                    expect(await card.locator('[data-card-detail]').evaluate(el => el.scrollWidth <= el.clientWidth && el.scrollHeight <= el.clientHeight)).toBe(true);
                    expect(await card.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
                }
                expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
            }
        });
    }
}
