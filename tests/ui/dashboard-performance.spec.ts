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
                        recent_performance: hasTiming ? { model: 'Gemini fixture', source: 'antigravity', sample_count: 7,
                            first_text_seconds: 4.75, body_tokens_per_second: 147.3, last_activity: 1 } : null,
                    };
                    if (command === 'get_api_pricing') return { prices: [], source: 'fixture' };
                    return original(command, args);
                };
            };
            await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language })});(${override.toString()})(${available});` });
            await page.goto('/');
            const label = language === 'zh' ? '首字延迟' : 'First-text latency';
            const latency = page.getByText(label, { exact: true }).locator('../..');
            const speed = page.getByText(language === 'zh' ? '正文速度' : 'Body output speed', { exact: true }).locator('../..');
            await expect(latency).toBeVisible();
            if (available) {
                await expect(latency).toContainText(language === 'zh' ? '4.75 秒' : '4.75 s');
                await expect(speed).toContainText('147.3 token/s');
                await expect(latency).toContainText('Gemini fixture');
                await expect(latency).toContainText(language === 'zh' ? '近 7 次' : '7 samples');
                await page.getByRole('button', { name: language === 'zh' ? '近 30 天' : 'Last 30 days', exact: true }).click();
                await expect(speed).toContainText('147.3 token/s');
            } else {
                await expect(latency).toContainText('—');
                await expect(speed).toContainText('—');
                await expect(speed).toContainText(language === 'zh' ? '暂无有效正文计时' : 'No valid text timing');
            }
            if (language === 'en') expect(await page.locator('main').innerText()).not.toMatch(/[\u3400-\u9fff]/);
            await page.screenshot({ path: `test-results/auto-switch/performance-wide-${language}-${available}.png` });
            await page.setViewportSize({ width: 1024, height: 768 });
            await expect(speed).toBeVisible();
            expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
            await page.screenshot({ path: `test-results/auto-switch/performance-${language}-${available}.png` });
        });
    }
}
