import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';
for (const language of ['zh', 'en']) for (const partial of [false, true]) test(`Claude thinking cost and ${partial ? 'partial' : 'complete'} estimate (${language})`, async ({ page }) => {
    const override = (partial: boolean) => {
        const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
        const record = { input_tokens: 1000000, output_tokens: 0, cached_tokens: 0, total_tokens: 1000000, request_count: 1 };
        const names = ['gemini-2.5-flash', 'claude-opus-4-6-thinking'];
        if (partial) names.push('gemini-2.5-flash-lite');
        const models = names.map(model => ({ ...record, model }));
        const totals = { ...record, input_tokens: record.input_tokens * names.length, total_tokens: record.total_tokens * names.length, request_count: names.length };
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
            if (command === 'get_api_pricing') return { prices: [{ model: 'gemini-2.5-flash', input: .3, output: 2.5, cached: .03 }, { model: 'Claude Opus 4.6', input: 5, output: 25, cached: .5 }], fetched_at: 1, stale: false, source: 'fixture' };
            if (command === 'get_local_token_usage') return { today: totals, yesterday: totals, last_3_days: totals, last_7_days: totals, last_30_days: totals, by_model_today: models, by_model_yesterday: models, by_model_3_days: models, by_model_7_days: models, by_model: models, daily: [], hourly: [], unreadable_databases: 0, generated_at: 1 };
            return original(command, args);
        };
    };
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language })});(${override.toString()})(${partial});` });
    await page.goto('/');
    const table = page.getByRole('table'), card = page.locator('[data-dashboard-card="api_cost"]');
    await expect(table.getByRole('row').filter({ hasText: 'claude-opus' })).toContainText('$5.00');
    await expect(table.getByRole('row').filter({ hasText: 'gemini-2.5-flash' }).first()).toContainText('$0.30');
    await expect(card).toContainText('$5.30');
    await expect(card.locator('[data-card-detail]')).toContainText(language === 'zh' ? `${partial ? 3 : 2} 次请求` : `${partial ? 3 : 2} requests`);
    await expect(card.locator('[data-card-detail]')).not.toContainText('Google');
    if (partial) {
        await expect(table.getByRole('row').filter({ hasText: 'gemini-2.5-flash-lite' })).toContainText(language === 'zh' ? '未计价' : 'Unpriced');
        await expect(card).toContainText(language === 'zh' ? '1 个模型未计价' : '1 model unpriced');
    } else await expect(card).not.toContainText(language === 'zh' ? '未计价' : 'unpriced');
    await page.getByRole('button', { name: language === 'zh' ? '预估费用' : 'Estimated cost' }).click();
    await expect(page.getByText(language === 'zh' ? partial ? '部分估算' : '预估费用' : partial ? 'Partial estimate' : 'Estimated cost', { exact: true }).last()).toBeVisible();
    for (const width of [1046, 1440]) {
        await page.setViewportSize({ width, height: 900 });
        expect(await card.locator('[data-card-detail]').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
    }
    if (language === 'zh') await page.screenshot({ path: test.info().outputPath(`dashboard-cost-${partial ? 'partial' : 'complete'}.png`) });
    if (language === 'en') expect(await page.locator('main').innerText()).not.toMatch(/[\u3400-\u9fff]/);
});
