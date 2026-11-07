import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

const fixtureAccount = {
    id: 'fixture-valuation-account',
    email: 'valuation@example.invalid',
    protected_models: [],
    created_at: 1,
    last_used: 1,
    quota: {
        last_updated: Date.now() / 1000,
        subscription_tier: 'pro',
        models: [
            { name: 'gemini-2.5-pro', percentage: 75, percentage_known: true, reset_time: '2026-11-07T12:00:00Z' },
            { name: 'claude-sonnet-4-6', percentage: 100, percentage_known: true, reset_time: '2026-11-07T12:00:00Z' },
        ],
        quota_groups: [
            {
                display_name: 'Gemini',
                buckets: [
                    { bucket_id: 'gemini-5h', window: '5h', remaining_fraction: 0.75, remaining_fraction_known: true, reset_time: '2026-11-07T12:00:00Z' },
                    { bucket_id: 'gemini-weekly', window: 'weekly', remaining_fraction: 0.8, remaining_fraction_known: true, reset_time: '2026-11-14T00:00:00Z' },
                ],
            },
            {
                display_name: 'Claude / GPT',
                buckets: [
                    { bucket_id: '3p-5h', window: '5h', remaining_fraction: 1.0, remaining_fraction_known: true, reset_time: '2026-11-07T12:00:00Z' },
                    { bucket_id: '3p-weekly', window: 'weekly', remaining_fraction: 1.0, remaining_fraction_known: true, reset_time: '2026-11-14T00:00:00Z' },
                ],
            },
        ],
    },
};

const fixtureValuations = {
    accounts: [
        {
            account_id: 'fixture-valuation-account',
            email: 'valuation@example.invalid',
            subscription_tier: 'pro',
            is_active: true,
            unpriced_events_count: 0,
            buckets: [
                {
                    bucket_id: 'gemini-5h',
                    window: '5h',
                    pool: 'gemini',
                    remaining_fraction: 0.75,
                    reset_time: '2026-11-07T12:00:00Z',
                    provenance: 'observed',
                    used_usd: 1.25,
                    used_cny: 9.0625,
                    used_tokens: 250_000,
                    estimated_total_usd: 5.0,
                    estimated_total_cny: 36.25,
                    estimated_total_tokens: 1_000_000,
                    estimated_remaining_usd: 3.75,
                    estimated_remaining_cny: 27.1875,
                    estimated_remaining_tokens: 750_000,
                },
                {
                    bucket_id: '3p-5h',
                    window: '5h',
                    pool: 'third_party',
                    remaining_fraction: 1.0,
                    reset_time: '2026-11-07T12:00:00Z',
                    provenance: 'calibrated',
                    used_usd: 0,
                    used_cny: 0,
                    used_tokens: 0,
                    estimated_total_usd: 20.0,
                    estimated_total_cny: 145.0,
                    estimated_total_tokens: 1_500_000,
                    estimated_remaining_usd: 20.0,
                    estimated_remaining_cny: 145.0,
                    estimated_remaining_tokens: 1_500_000,
                },
            ],
        },
    ],
    currency: 'usd',
    display_mode: 'remaining',
    usd_to_cny_rate: 7.25,
    rate_stale: false,
    calculated_at: Date.now(),
};

function setupValuationFixture({ account, valuations, configOverride = {} }: any) {
    const w = window as any;
    const original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
        if (command === 'load_config') {
            const base = await original(command, args);
            return {
                ...base,
                pinned_quota_models: { models: ['gemini-2.5-pro', 'claude-sonnet-4-6'] },
                quota_valuation: {
                    enabled: true,
                    currency: 'usd',
                    display_mode: 'remaining',
                    auto_exchange_rate: true,
                    usd_to_cny_rate: 7.25,
                    use_reference_cap: true,
                    reference_caps_usd: {},
                    ...configOverride,
                },
            };
        }
        if (command === 'list_accounts') {
            return [account];
        }
        if (command === 'get_account_quota_valuations') {
            const currency = configOverride.currency ?? 'usd';
            const display_mode = configOverride.display_mode ?? 'remaining';
            return {
                ...valuations,
                currency,
                display_mode,
            };
        }
        if (command === 'get_quota_cycle_history') {
            return {
                cycles: [
                    {
                        cycle_id: 'c1',
                        account_id: 'fixture-valuation-account',
                        pool: 'gemini',
                        window: '5h',
                        start_time: '2026-11-07T10:00:00Z',
                        reset_time: '2026-11-07T15:00:00Z',
                        first_observed_remaining: 1.0,
                        latest_remaining: 0.75,
                        min_remaining: 0.75,
                        consumed_fraction: 0.25,
                        last_delta_fraction: -0.25,
                        sample_count: 2,
                        status: 'active',
                        used_usd: 1.25,
                        estimated_total_usd: 5.0,
                        delta_total_pct: 10.0,
                        checkpoints: [
                            { timestamp: 1731000000, remaining_fraction: 0.75, delta_fraction: -0.25 },
                        ],
                        email: 'valuation@example.invalid',
                        subscription_tier: 'pro',
                    },
                ],
                accounts_summary: [
                    {
                        account_id: 'fixture-valuation-account',
                        email: 'valuation@example.invalid',
                        total_cycles: 1,
                        active_cycles: 1,
                        total_consumed_usd: 1.25,
                    },
                ],
                currency: 'usd',
                usd_to_cny_rate: 7.25,
                queried_at: Date.now(),
            };
        }
        return original(command, args);
    };
}

for (const language of ['zh', 'en']) {
    for (const view of ['grid', 'list']) {
        test(`account card and table render valuation second line in ${view} (${language})`, async ({ page }) => {
            await page.addInitScript(setupSettingsFixture, { language });
            await page.addInitScript(setupValuationFixture, {
                account: fixtureAccount,
                valuations: fixtureValuations,
                configOverride: {},
            });
            await page.addInitScript(({ view }) => {
                localStorage.setItem('accounts_view_mode', view);
                localStorage.setItem('accounts_quota_window', '5h');
            }, { view });

            await page.goto('/accounts');

            // 验证图 1 三段式 UI：已用指标与剩余指标
            await expect(page.getByText('$1.25 / 250K')).toBeVisible();
            const remText = language === 'zh' ? '剩余 $3.75 / 750K' : 'Left $3.75 / 750K';
            await expect(page.getByText(remText)).toBeVisible();

            // 验证 100% 满额的校准折算：剩余 $20.00 / 1.5M 或 Left $20.00 / 1.5M，以及校准徽标
            const refText = language === 'zh' ? '剩余 $20.00 / 1.5M' : 'Left $20.00 / 1.5M';
            await expect(page.getByText(refText)).toBeVisible();
            const badgeText = language === 'zh' ? '校准' : 'Cal';
            await expect(page.getByText(badgeText)).toBeVisible();
        });
    }
}

test('currency mode switching reflects in card display', async ({ page }) => {
    await page.addInitScript(setupSettingsFixture, { language: 'zh' });
    await page.addInitScript(setupValuationFixture, {
        account: fixtureAccount,
        valuations: fixtureValuations,
        configOverride: { currency: 'cny' },
    });
    await page.addInitScript(() => {
        localStorage.setItem('accounts_view_mode', 'grid');
        localStorage.setItem('accounts_quota_window', '5h');
    });

    await page.goto('/accounts');
    // 人民币模式应包含 ¥
    await expect(page.getByText('剩余 ¥27.19 / 750K')).toBeVisible();
});

test('display mode used_and_total shows cost and total bounds', async ({ page }) => {
    await page.addInitScript(setupSettingsFixture, { language: 'zh' });
    await page.addInitScript(setupValuationFixture, {
        account: fixtureAccount,
        valuations: fixtureValuations,
        configOverride: { display_mode: 'used_and_total' },
    });
    await page.addInitScript(() => {
        localStorage.setItem('accounts_view_mode', 'list');
        localStorage.setItem('accounts_quota_window', '5h');
    });

    await page.goto('/accounts');
    // used_and_total 模式：$1.25 / 250K 与 总额 $5.00 / 1M
    await expect(page.getByText('$1.25 / 250K')).toBeVisible();
    await expect(page.getByText('总额 $5.00 / 1M')).toBeVisible();
});

test('settings quota valuation controls interact correctly', async ({ page }) => {
    await page.addInitScript(setupSettingsFixture, { language: 'zh' });
    await page.addInitScript(setupValuationFixture, {
        account: fixtureAccount,
        valuations: fixtureValuations,
        configOverride: {},
    });

    await page.goto('/settings');
    await page.locator('#settings-tab-quota').click();

    // 检查配额折算配置区
    await expect(page.getByText('配额价值与等效 Token 换算')).toBeVisible();
    await expect(page.getByText('计价币种')).toBeVisible();
    await expect(page.getByText('展示模式')).toBeVisible();
    await expect(page.getByText('100% 满额基准折算与自定义参考上限')).toBeVisible();
});

test('quota cycle modal opens and renders cycle records correctly', async ({ page }) => {
    await page.addInitScript(setupSettingsFixture, { language: 'zh' });
    await page.addInitScript(setupValuationFixture, {
        account: fixtureAccount,
        valuations: fixtureValuations,
        configOverride: {},
    });
    await page.addInitScript(() => {
        localStorage.setItem('accounts_view_mode', 'list');
        localStorage.setItem('accounts_quota_window', '5h');
    });

    await page.goto('/accounts');
    // 点击顶部「周期变化」按钮
    const cycleBtn = page.getByRole('button', { name: '周期变化' });
    await expect(cycleBtn).toBeVisible();
    await cycleBtn.click();

    // 弹窗应呈现
    await expect(page.getByText('周期额度变化与消耗统计')).toBeVisible();
    await expect(page.getByText('监控周期数')).toBeVisible();
    await expect(page.getByText('累计周期消耗')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Gemini' })).toBeVisible();
    await expect(page.getByRole('button', { name: '进行中' })).toBeVisible();
    await expect(page.getByText('valuation@example.invalid').first()).toBeVisible();
});
