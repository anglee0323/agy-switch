import { expect, test } from '@playwright/test';

// Synthetic Tauri IPC only: no system keyring, real account, process control, or
// network quota service is involved. These are UI acceptance tests, not native
// authentication tests. The Rust suite exercises the real quota/safety guards.
test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
        const w = window as any;
        const callbacks: Record<number, Function> = {}; let callback = 1;
        const model = { name: 'gemini-test', display_name: 'Gemini test model', percentage: 8, reset_time: '2027-02-01T00:00:00Z' };
        const accounts = ['current', 'backup'].map((name, i) => ({ id: i ? 'B' : 'A', email: `${name}@example.invalid`, created_at: 1, last_used: 1,
            token: { access_token: 'fixture-only', refresh_token: 'fixture-only', expires_in: 3600, expiry_timestamp: 1, token_type: 'Bearer' },
            quota: { models: [{ ...model, percentage: i ? 80 : 8 }], last_updated: 1800000000 } }));
        let config = { enabled: false, mode: 'wait', reserve_percentage: 10, candidate_min_percentage: 30, monitored_model: '', candidate_account_ids: [], target: 'app' };
        let status = { phase: 'disabled', reason: null, source_account_id: 'A', source_email: accounts[0].email, target_account_id: 'B', target_email: accounts[1].email,
            remaining_percentage: 8, pending_id: null, mode: 'wait', process_state: 'running', last_checked: 1800000000 };
        const calls: string[] = [];
        w.__fixture = { calls, setStatus: (patch: any) => { status = { ...status, ...patch }; }, config: () => config };
        w.__TAURI_INTERNALS__ = {
            transformCallback: (fn: Function) => { const id = callback++; callbacks[id] = fn; return id; },
            unregisterCallback: (id: number) => { delete callbacks[id]; },
            convertFileSrc: (s: string) => s,
            invoke: async (cmd: string, args: any = {}) => {
                calls.push(cmd);
                if (cmd === 'list_accounts') return accounts;
                if (cmd === 'get_current_account') return accounts[0];
                if (cmd === 'get_auto_switch_config') return config;
                if (cmd === 'set_auto_switch_config') { config = args.config; status = { ...status, mode: config.mode, phase: config.enabled ? 'pending' : 'disabled', reason: 'closing_clients', pending_id: config.enabled ? 'fixture-pending' : null }; return config; }
                if (cmd === 'get_auto_switch_status' || cmd === 'check_auto_switch_now') return status;
                if (cmd === 'cancel_auto_switch') { if (args.pendingId !== status.pending_id) throw new Error('Stale request'); status = { ...status, phase: 'canceled', reason: 'canceled_until_recovery', pending_id: null }; return status; }
                if (cmd === 'get_data_dir_path') return '/fixture/antigravity-tools';
                if (cmd === 'load_config') return { language: 'zh', theme: 'light', auto_refresh: false, refresh_interval: 15, auto_sync: false, sync_interval: 5, quota_protection: { enabled: false, threshold_percentage: 10, monitored_models: [] }, pinned_quota_models: { models: [] } };
                if (cmd === 'plugin:event|listen') return callback++;
                if (cmd === 'plugin:window|available_monitors') return [];
                return null;
            },
            metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
        };
        w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
        localStorage.setItem('i18nextLng', 'zh');
    });
    await page.goto('/settings');
    await page.getByRole('tab', { name: '智能切换策略', exact: true }).click();
    await expect(page.getByRole('tabpanel', { name: '智能切换策略', exact: true })).toBeVisible();
});

async function enable(page: any, stop = false) {
    if (!await page.getByLabel('启用智能切换策略').isChecked()) await page.getByText('启用智能切换策略', { exact: true }).click();
    await expect(page.getByText('已自动保存', { exact: true })).toBeVisible();
    await page.getByLabel('监测模型', { exact: true }).selectOption('gemini');
    await page.getByLabel('backup@example.invalid', { exact: true }).check();
    if (stop) await page.getByRole('radio').nth(1).check();
    await expect(page.getByText('已自动保存', { exact: true })).toBeVisible();
    await expect(page.getByText('正在自动关闭客户端，随后切号并重启').first()).toBeVisible();
}

test('switch timing and account selection order are separate labelled choices', async ({ page }, testInfo) => {
    const timing = page.getByRole('group', { name: '切换时机', exact: true });
    const order = page.getByRole('group', { name: '账号选择顺序', exact: true });
    await expect(timing).toBeVisible(); await expect(order).toBeVisible();
    await expect(timing.getByRole('radio')).toHaveCount(2);
    await expect(order.getByRole('radio')).toHaveCount(2);
    await order.getByRole('radio').nth(1).check();
    await expect(timing.getByRole('radio').first()).toBeChecked();
    await expect(order.getByRole('radio').nth(1)).toBeChecked();
    await page.screenshot({ path: testInfo.outputPath('policy-dimensions.png'), fullPage: true });
    await page.setViewportSize({ width: 760, height: 1000 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('policy groups use natural English without Chinese labels', async ({ page }, testInfo) => {
    await page.getByRole('button', { name: 'ZH', exact: true }).click();
    await page.getByRole('button', { name: /EN.*English/ }).click();
    await expect(page.getByRole('group', { name: 'Switch timing', exact: true })).toBeVisible();
    await expect(page.getByRole('group', { name: 'Account selection order', exact: true })).toBeVisible();
    expect(await page.getByRole('tabpanel').innerText()).not.toMatch(/[\u3400-\u9fff]/);
    await page.screenshot({ path: testInfo.outputPath('policy-dimensions-en.png'), fullPage: true });
});

test('default off, explicit configuration, real command contract, cancellation', async ({ page }, testInfo) => {
    await expect(page.getByLabel('启用智能切换策略')).not.toBeChecked();
    await expect(page.getByRole('button', { name: '保存设置', exact: true })).toHaveCount(0);
    await enable(page);
    await expect(page.getByText('当前账号剩余 8%').first()).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath('waiting.png'), fullPage: true });
    await page.getByRole('button', { name: '取消本次换号', exact: true }).first().click();
    await expect(page.getByText('已取消本次换号；额度恢复后重新监测，或保存设置重新启用').first()).toBeVisible();
    const calls = await page.evaluate(() => (window as any).__fixture.calls);
    expect(calls).toContain('set_auto_switch_config'); expect(calls).toContain('cancel_auto_switch');
    expect(calls).not.toContain('switch_account');
});

test('stop mode gives truthful native instructions, keyboard dismissal and no stop call', async ({ page }, testInfo) => {
    await enable(page, true);
    await page.getByRole('button', { name: '查看步骤', exact: true }).first().click();
    const dialog = page.getByRole('dialog'); await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('button', { name: '知道了', exact: true })).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath('stop-guidance.png') });
    await page.keyboard.press('Escape'); await expect(dialog).not.toBeVisible();
    const calls = await page.evaluate(() => (window as any).__fixture.calls);
    expect(calls.some((c: string) => /stop|kill|switch_account/.test(c))).toBe(false);
});

test('unknown blocks, completion says next launch, narrow layout has no horizontal overflow', async ({ page }, testInfo) => {
    await enable(page);
    await page.evaluate(() => (window as any).__fixture.setStatus({ phase: 'blocked', reason: 'process_unknown', process_state: 'unknown' }));
    await expect(page.getByText('无法确认客户端状态，暂不换号').first()).toBeVisible();
    await page.evaluate(() => (window as any).__fixture.setStatus({ phase: 'completed', reason: 'credentials_updated', process_state: 'closed', pending_id: null }));
    await expect(page.getByText('账号凭据已安全写入').first()).toBeVisible();
    await expect(page.getByText(/请在客户端确认账号/).first()).toBeVisible();
    await page.setViewportSize({ width: 760, height: 1000 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath('completed-narrow.png'), fullPage: true });
});

test('wait mode reports automatic closing, restart and a distinct relaunch failure', async ({ page }) => {
    await enable(page);
    await page.evaluate(() => (window as any).__fixture.setStatus({ reason: 'waiting_task_finish' }));
    await expect(page.getByText('任务仍在运行，等待结束后切换').first()).toBeVisible();
    await page.evaluate(() => (window as any).__fixture.setStatus({ reason: 'closing_clients' }));
    await expect(page.getByText('正在自动关闭客户端，随后切号并重启').first()).toBeVisible();
    await page.evaluate(() => (window as any).__fixture.setStatus({ phase: 'completed', reason: 'restarted', pending_id: null }));
    await expect(page.getByText('已切换账号并重启客户端').first()).toBeVisible();
    await page.evaluate(() => (window as any).__fixture.setStatus({ reason: 'restart_failed' }));
    await expect(page.getByText('账号已切换，客户端重启失败，请手动打开').first()).toBeVisible();
});
