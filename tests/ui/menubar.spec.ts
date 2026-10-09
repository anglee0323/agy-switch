import { expect, test, type Page } from '@playwright/test';
// Synthetic IPC acceptance. Native macOS material and installed CLI are checked
// separately on the host; these fixtures never switch real credentials/processes.
test.beforeEach(async ({ page }) => {
  await page.setViewportSize({ width: 424, height: 640 });
  await page.addInitScript(() => {
    const w = window as any; const callbacks: Record<number, Function> = {}; const listeners: Record<number, any> = {}; let sequence = 1;
    let current: string | null = 'B'; let identity = 'running_app'; let preferences: any = JSON.parse(localStorage.getItem('fixture-menu-preferences') || '{"quota_scope":"all"}'); let theme = 'light'; let language = 'zh';
    let failRead = false, failRefresh = false, failSave = false, hold = false, release: (() => void) | null = null;
    let usage: any = { today: { input_tokens: 12000, output_tokens: 1200, cached_tokens: 135000, total_tokens: 148200, request_count: 86 }, estimated_usd: 0.023625, unpriced_models: 0, pricing_stale: false, incomplete: false };
    const calls: any[] = [];
    const now = Math.floor(Date.now() / 1000);
    const account = (id: string, g = 80, c = 70) => ({ id, email: id.toLowerCase() + '@example.invalid', name: null, custom_label: '账号 ' + id, read_status: 'loaded', read_error: null, disabled: false, validation_blocked: false, validation_blocked_until: null, protected_models: [],
      quota: { last_updated: now, subscription_tier: 'PRO', provenance: 'observed', is_forbidden: false, models: [], groups: ['Gemini Models', 'Claude / GPT'].map((display_name, i) => ({ display_name, buckets: ['5h', 'weekly'].map(window => ({ bucket_id: (i ? 'c' : 'g') + window, window, remaining_fraction: (i ? c : g) / 100, reset_time: new Date(Date.now() + (window === '5h' ? 7200000 : 259200000)).toISOString() })) })) } });
    let accounts: any[] = [account('A', 80, 6), account('B'), { ...account('C'), disabled: true }, account('D', 8, 60), { ...account('E'), quota: null }, account('F'), { ...account('G'), validation_blocked: true }];
    accounts[5].quota.last_updated -= 3600;
    let appearance: any = { platform: 'macos', native_material: true, material_kind: 'liquid_glass', reduced_transparency: false, high_contrast: false };
    let status: any = { phase: 'disabled', reason: null, source_account_id: 'B', source_email: 'b@example.invalid', target_account_id: 'A', target_email: 'a@example.invalid', remaining_percentage: 6, pending_id: null, mode: 'wait', process_state: 'running', last_checked: now };
    const emit = (event: string, payload: any = {}) => Object.entries(listeners).forEach(([id, listener]) => { if (listener.event === event) callbacks[listener.handler]?.({ id: Number(id), event, payload }); });
    w.__menuFixture = { calls, emit, setUsage: (next: any) => { usage = next; emit('menubar://opened'); }, current: () => current, setIdentity: (id: string | null, source = 'running_app') => { current = id; identity = source; emit('menubar://opened'); }, empty: () => { accounts = []; current = null; emit('menubar://data-updated'); }, failRead: () => { failRead = true; emit('menubar://data-updated'); }, recover: () => { failRead = false; emit('menubar://data-updated'); }, failRefresh: () => { failRefresh = true; }, failSave: () => { failSave = true; }, holdSwitch: () => { hold = true; }, releaseSwitch: () => release?.(), setStatus: (patch: any) => { status = { ...status, ...patch }; emit('menubar://opened'); }, setPreferences: (patch: any) => { preferences = { ...preferences, ...patch.menu_bar, ...(patch.scope ? { quota_scope: patch.scope } : {}) }; theme = patch.theme || theme; language = patch.language || language; emit('config://updated'); }, setAppearance: (patch: any) => { appearance = { ...appearance, ...patch }; emit('menubar://appearance', appearance); }, manyModels: () => { accounts[0].quota.models = Array.from({length: 14}, (_, i) => ({ name: 'extra-model-' + i, display_name: null, percentage: 50, reset_time: new Date(Date.now() + 7200000).toISOString(), inferred_bucket_id: null })); emit('menubar://data-updated'); } };
    w.__TAURI_INTERNALS__ = {
      transformCallback: (fn: Function) => { const id = sequence++; callbacks[id] = fn; return id; }, unregisterCallback: (id: number) => { delete callbacks[id]; }, convertFileSrc: (s: string) => s,
      metadata: { currentWindow: { label: 'menubar' }, currentWebview: { label: 'menubar' } },
      invoke: async (cmd: string, args: any = {}) => {
        calls.push({ cmd, args });
        if (cmd === 'load_config') return { language, theme, auto_refresh: false, auto_sync: false, refresh_interval: 15, sync_interval: 5, pinned_quota_models: { models: [] }, quota_protection: { enabled: false, threshold_percentage: 10, monitored_models: [] }, menu_bar: preferences };
        if (cmd === 'get_menu_bar_snapshot') { if (failRead) throw new Error('Fixture unreadable'); return { indexed_total: accounts.length, loaded_count: accounts.length, failed_count: 0, accounts, current_account_id: current, current_identity_source: identity }; }
        if (cmd === 'get_menu_bar_appearance') return appearance;
        if (cmd === 'get_menu_bar_usage') return usage;
        if (cmd === 'get_auto_switch_config') return { enabled: false, mode: 'wait', reserve_percentage: 10, candidate_min_percentage: 30, monitored_model: 'gemini-test', candidate_account_ids: ['A'], target: 'app' };
        if (cmd === 'get_auto_switch_status' || cmd === 'check_auto_switch_now') return status;
        if (cmd === 'cancel_auto_switch') { if (args.pendingId !== status.pending_id) throw new Error('Stale cancellation'); status = { ...status, phase: 'canceled', reason: 'canceled_until_recovery', pending_id: null }; return status; }
        if (cmd === 'switch_account') { if (hold) await new Promise<void>(resolve => { release = resolve; }); current = args.accountId; emit('tray://account-switched'); return; }
        if (cmd === 'refresh_all_quotas') return { total: accounts.length, success: failRefresh ? 0 : accounts.length, failed: failRefresh ? accounts.length : 0, details: [] };
        if (cmd === 'set_menu_bar_preferences') { if (failSave) throw new Error('Fixture save failure'); preferences = { ...preferences, ...args.patch, ...(args.quotaScope ? { quota_scope: args.quotaScope } : {}) }; localStorage.setItem('fixture-menu-preferences', JSON.stringify(preferences)); emit('config://updated'); return preferences; }
        if (cmd === 'get_desktop_settings') return { platform: 'macos', tray_available: true, autostart_supported: true, launch_at_login: false, hide_dock_icon: false, start_minimized: false };
        if (cmd === 'list_accounts') return accounts; if (cmd === 'get_current_account') return null;
        if (cmd === 'get_data_dir_path') return '/fixture';
        if (cmd === 'plugin:event|listen') { const id = sequence++; listeners[id] = args; return id; }
        if (cmd === 'plugin:event|unlisten') { delete listeners[args.eventId]; return; }
        return null;
      },
    };
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }; localStorage.setItem('i18nextLng', 'zh');
  });
  await page.goto('/menubar'); await expect(page.getByRole('heading', { name: '剩余额度' })).toBeVisible();
  await expect(page.locator('.mb-account-row')).not.toHaveCount(0);
});
async function bounded(page: Page) {
  await expect.poll(() => page.evaluate(() => [document.documentElement, document.body, document.getElementById('root'), ...document.querySelectorAll('.menubar-app,.mb-content,.mb-accounts')].filter(Boolean).filter((el: any) => el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1).map((el: any) => ({ class: el.className, height: [el.clientHeight, el.scrollHeight], width: [el.clientWidth, el.scrollWidth] }))), { timeout: 5000 }).toEqual([]);
  expect(await page.locator('.mb-footer').evaluate(el => el.getBoundingClientRect().bottom <= innerHeight + 1)).toBe(true);
}
async function calls(page: Page, command: string) { return page.evaluate(command => (window as any).__menuFixture.calls.filter((call: any) => call.cmd === command), command); }

test('overview shows both independent windows and account labels do not navigate or switch', async ({ page }, info) => {
  await expect(page.locator('.mb-aggregate strong')).toHaveText(['51%', '51%']);
  await expect(page.locator('.mb-availability')).toHaveText(['可用 1/5', '可用 1/5']);
  await expect(page.locator('.mb-account-row.current')).toContainText('账号 B');
  expect(await calls(page, 'list_accounts')).toEqual([]); expect(await calls(page, 'get_current_account')).toEqual([]);
  await bounded(page); await page.screenshot({ path: info.outputPath('menubar-overview-light-browser.png') });
  await expect(page.getByRole('button', { name: /^查看 账号/ })).toHaveCount(0);
  await page.locator('.mb-account-label').first().click(); await expect(page.getByText('a@example.invalid', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: '剩余额度' })).toBeVisible(); await expect(page.getByRole('button', { name: '返回总览' })).toHaveCount(0);
  expect(await calls(page, 'switch_account')).toEqual([]); await bounded(page);
  await page.getByRole('button', { name: '切换到 账号 A' }).click();
  await expect(page.locator('.mb-account-row.current')).toContainText('账号 A'); await expect(page.getByText(/已切换到/)).toHaveCount(0);
  expect((await calls(page, 'switch_account')).map(call => call.args.accountId)).toEqual(['A']);
});
test('responsive pagination reaches every account without clipping', async ({ page }) => {
  for (const size of [{ width: 380, height: 480 }, { width: 320, height: 400 }]) {
    await page.setViewportSize(size); await bounded(page);
    const seen = new Set<string>();
    do {
      (await page.locator('.mb-account-label > span').allTextContents()).forEach(name => seen.add(name)); await bounded(page);
      if (await page.getByRole('button', { name: '下一页' }).isDisabled()) break;
      await page.getByRole('button', { name: '下一页' }).click();
    } while (true);
    expect(seen.size).toBe(5);
    while (await page.getByRole('button', { name: '上一页' }).isEnabled()) await page.getByRole('button', { name: '上一页' }).click();
  }
});
test('unknown live identity never chooses the first account and read failures clear stale current badges', async ({ page }) => {
  await page.evaluate(() => (window as any).__menuFixture.setIdentity(null, 'unavailable')); await expect(page.locator('.mb-account-row.current')).toHaveCount(0);
  await expect(page.getByRole('status')).toHaveText('当前未识别'); await bounded(page);
  await page.evaluate(() => (window as any).__menuFixture.setIdentity(null, 'running_app')); await expect(page.getByRole('status')).toHaveText('当前未识别');
  await page.evaluate(() => (window as any).__menuFixture.setIdentity('B', 'tools_record')); await expect(page.getByRole('button', { name: '切换到 账号 B' })).toHaveAttribute('title', 'Tools 保存的账号');
  await expect(page.getByRole('status')).toHaveCount(0);
  await page.evaluate(() => (window as any).__menuFixture.failRead()); await expect(page.getByRole('alert')).toContainText('账号读取失败'); await expect(page.locator('.mb-account-row.current')).toHaveCount(0);
  await page.evaluate(() => (window as any).__menuFixture.recover()); await expect(page.locator('.mb-account-row.selected')).toContainText('账号 B');
  await page.evaluate(() => (window as any).__menuFixture.empty()); await expect(page.getByText('添加账号后显示额度')).toBeVisible(); await bounded(page);
});
test('a saved selection is not a verified login and can be reapplied in either language', async ({ page }) => {
  for (const language of ['zh', 'en']) {
    await page.evaluate(language => { (window as any).__menuFixture.setPreferences({ language }); (window as any).__menuFixture.setIdentity('B', 'tools_record'); }, language);
    const selected = page.getByRole('button', { name: (language === 'zh' ? '切换到 ' : 'Switch to ') + '账号 B' });
    await expect(selected).toHaveText(language === 'zh' ? '记录' : 'Saved');
    await expect(selected).toBeEnabled(); await expect(page.locator('.mb-account-row.current')).toHaveCount(0);
    const before = (await calls(page, 'switch_account')).length;
    await selected.click(); await expect.poll(async () => (await calls(page, 'switch_account')).length).toBe(before + 1);
    await page.evaluate(() => (window as any).__menuFixture.setIdentity('B', 'running_app'));
    await expect(selected).toHaveText(language === 'zh' ? '当前' : 'Current'); await expect(selected).toBeDisabled();
  }
});
test('English unknown identity stays explicit in the compact menu', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 400 });
  await page.evaluate(() => { (window as any).__menuFixture.setPreferences({ language: 'en' }); (window as any).__menuFixture.setIdentity(null, 'unavailable'); });
  await expect(page.getByRole('status')).toHaveText('Current unknown'); await bounded(page);
  await page.evaluate(() => (window as any).__menuFixture.setIdentity('A'));
  await expect(page.locator('.mb-account-row.current')).toContainText('账号 A');
  await expect(page.getByRole('status')).toHaveCount(0);
});
test('switch locks and coordinator cancellation preserve safety; partial refresh remains explicit', async ({ page }) => {
  await page.evaluate(() => (window as any).__menuFixture.holdSwitch()); await page.getByRole('button', { name: '切换到 账号 A' }).click(); await expect(page.getByRole('button', { name: '切换到 账号 A' })).toBeDisabled(); expect(await calls(page, 'switch_account')).toHaveLength(1);
  await page.locator('.mb-account-label').filter({ hasText: '账号 B' }).click(); await page.evaluate(() => (window as any).__menuFixture.releaseSwitch()); await expect.poll(() => page.evaluate(() => (window as any).__menuFixture.current())).toBe('A'); await expect(page.getByText('b@example.invalid', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '返回总览' })).toHaveCount(0);
  await page.evaluate(() => (window as any).__menuFixture.setStatus({ phase: 'pending', reason: 'clients_running', pending_id: 'fixture-request' })); await page.getByRole('button', { name: '查看低额度换号详情' }).click();
  await expect(page.getByText(/读取实时任务状态/)).toBeVisible(); await page.getByRole('button', { name: '取消本次换号', exact: true }).click(); await expect.poll(async () => (await calls(page, 'cancel_auto_switch'))[0]?.args.pendingId).toBe('fixture-request');
  await page.getByRole('button', { name: '返回总览' }).click(); await page.evaluate(() => (window as any).__menuFixture.failRefresh()); await page.getByRole('button', { name: '刷新全部额度' }).click(); await expect(page.getByRole('alert')).toContainText('7 个账号刷新失败');
});
test('scope, language and accessibility material respond to backend events and settings persist with rollback on failure', async ({ page }, info) => {
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ scope: 'gemini', theme: 'dark' })); await expect(page.locator('.mb-aggregate strong')).toHaveText(['56%', '56%']);
  await expect(page.locator('.mb-availability')).toHaveText(['可用 2/5', '可用 2/5']); await expect(page.locator('.menubar-app')).toHaveAttribute('data-material', 'liquid_glass');
  await page.evaluate(() => (window as any).__menuFixture.setAppearance({ native_material: false, reduced_transparency: true, material_kind: 'opaque' })); await expect(page.locator('.menubar-app')).toHaveClass(/opaque-material/);
  await page.screenshot({ path: info.outputPath('menubar-overview-dark-browser.png') });
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ language: 'en' })); await expect(page.getByRole('heading', { name: 'Remaining quota' })).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click(); await page.goto('/settings'); await page.evaluate(() => (window as any).__menuFixture.setPreferences({ language: 'en' })); await page.getByLabel('Menu bar aggregate quotas').selectOption('other'); await expect(page.getByLabel('Menu bar aggregate quotas')).toHaveValue('other');
  await expect.poll(() => page.evaluate(() => (window as any).__menuFixture.calls.filter((call: any) => call.cmd === 'set_menu_bar_preferences').at(-1)?.args.patch.quota_scope)).toBe('other');
  await page.evaluate(() => (window as any).__menuFixture.failSave()); await page.getByLabel('Menu bar aggregate quotas').selectOption('gemini'); await expect(page.getByRole('alert').filter({ hasText: 'Could not save' })).toBeVisible(); await expect(page.getByLabel('Menu bar aggregate quotas')).toHaveValue('other');
});
test('family selection stays in Settings and disabled quotas show zero without activation', async ({ page }) => {
  await expect(page.locator('.mb-eyebrow')).toHaveText('AntiGravity Switch');
  await expect(page.locator('.mb-mini.other').first()).toBeVisible();
  await expect(page.locator('.mb-account-identity').first()).toContainText('a@example.invalid');
  const identity = page.locator('.mb-account-identity').first();
  expect(await identity.evaluate(el => el.querySelector('.mb-account-switch')!.getBoundingClientRect().left > el.querySelector('.mb-account-label')!.getBoundingClientRect().left)).toBe(true);
  await expect(page.getByRole('button', { name: 'Gemini', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Claude 和 GPT', exact: true })).toHaveCount(0);
  await page.goto('/settings');
  await page.getByLabel('账号区显示系列').selectOption('other');
  await expect(page.getByLabel('账号区显示系列')).toHaveValue('other');
  expect((await calls(page, 'set_menu_bar_preferences')).at(-1).args.patch).toEqual({ display_scope: 'other' });
  await page.goto('/menubar');
  await expect(page.locator('.mb-mini.gemini')).toHaveCount(0);
  await expect(page.locator('.mb-mini.other').first()).toBeVisible();
  await expect(page.locator('.mb-aggregate strong')).toHaveText(['51%', '51%']);
  expect(await calls(page, 'switch_account')).toEqual([]);
  await page.reload(); await expect(page.locator('.mb-mini.gemini')).toHaveCount(0);
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ menu_bar: { hide_unavailable: false, label_style: 'email_only', show_weekly: false } }));
  await expect(page.locator('.mb-availability')).toHaveText(['可用 1/7']);
  await expect(page.locator('.mb-account-identity').first()).not.toContainText('账号 A');
  const disabled = page.locator('.mb-account-row.disabled');
  const locateDisabled = async (values: string[]) => {
    // Capacity varies with platform fonts and the enabled overview sections.
    // Find the account again after layout changes rather than assuming its page.
    await expect.poll(async () => {
      if (await disabled.count()) return disabled.locator('.mb-mini strong').allTextContents();
      const next = page.getByRole('button', { name: '下一页' });
      if (await next.isEnabled()) await next.click();
      else {
        const previous = page.getByRole('button', { name: '上一页' });
        while (await previous.isEnabled()) await previous.click();
      }
      return [];
    }).toEqual(values);
  };
  await locateDisabled(['0%']);
  await expect(disabled).toHaveCount(1);
  await expect(disabled.locator('.mb-account-switch')).toBeDisabled();
  await expect(disabled.locator('.mb-account-switch')).toHaveText('禁用');
  await expect(disabled.locator('.mb-mini strong')).toHaveText(['0%']);
  await expect(page.locator('.mb-availability')).toHaveText(['可用 1/7']);
  await disabled.locator('.mb-account-label').click();
  await expect(page.getByRole('heading', { name: '剩余额度' })).toBeVisible();
  expect(await calls(page, 'switch_account')).toEqual([]);
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ menu_bar: { display_scope: 'all', show_weekly: true } }));
  await expect(page.locator('.mb-aggregate strong')).toHaveCount(2);
  await locateDisabled(['0%', '0%', '0%', '0%']);
  await expect(page.getByRole('button', { name: '关于应用' })).toHaveCount(0); await expect(page.getByRole('button', { name: 'GitHub', exact: true })).toBeVisible();
});

test('daily usage distinguishes empty records from unknown API pricing', async ({ page }) => {
  const usage = page.getByRole('region', { name: '今日本机用量' });
  await expect(usage.getByRole('heading')).toHaveText('今日用量');
  await expect(usage.locator('.mb-usage-ring strong')).toHaveText('148.2K');
  await expect(usage.locator('.mb-usage-type strong')).toHaveText(['12.0K', '1.2K', '135.0K']);
  await expect(usage.getByText('USD', { exact: true })).toHaveCount(0);
  await page.evaluate(() => (window as any).__menuFixture.setUsage({ today: { input_tokens: 1000, output_tokens: 0, cached_tokens: 0, total_tokens: 1000, request_count: 1 }, estimated_usd: null, unpriced_models: 1, pricing_stale: true, incomplete: true }));
  await expect(usage.locator('.mb-usage-cost strong')).toHaveText('未计价');
  await expect(usage).toContainText('统计不完整');
  await page.evaluate(() => (window as any).__menuFixture.setUsage({ today: { input_tokens: 0, output_tokens: 0, cached_tokens: 0, total_tokens: 0, request_count: 0 }, estimated_usd: 0, unpriced_models: 0, pricing_stale: true, incomplete: false }));
  await expect(usage.locator('.mb-usage-cost strong')).toHaveText('$ 0.00');
  await expect(usage.locator('.mb-usage-ring circle')).toHaveCount(1);
  expect(await calls(page, 'switch_account')).toEqual([]);
});

test('platform dashboard labels its quota columns and honors icon visibility', async ({ page }) => {
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ language: 'en', menu_bar: { show_icons: false } }));
  await expect(page.locator('.mb-family-heading')).toHaveCount(0);
  const row = page.locator('.mb-account-row').first();
  await expect(row.locator('.mb-period')).toHaveText(['5 hours', '5 hours', 'Weekly', 'Weekly']);
  await expect(row.locator('.mb-period').nth(0)).toHaveAttribute('title', 'Gemini · 5 hours');
  await expect(row.locator('.mb-period').nth(1)).toHaveAttribute('title', 'Claude / GPT · 5 hours');
  await expect(page.locator('.mb-brand img')).toHaveCount(0);
  await expect(page.locator('.mb-footer-actions svg')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'GitHub', exact: true })).toHaveText('GitHub');
  await expect(page.getByRole('button', { name: 'Quit', exact: true })).toHaveText('Quit');
  await expect(page.locator('.mb-account-switch svg').first()).toBeVisible();
  await bounded(page);
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ menu_bar: { show_icons: true, display_scope: 'other' } }));
  await expect(row.locator('.mb-period')).toHaveText(['5 hours', 'Weekly']);
  await expect(row.locator('.mb-period').first()).toHaveAttribute('title', 'Claude / GPT · 5 hours');
  await expect(page.locator('.mb-brand img')).toBeVisible();
  await expect(page.locator('.mb-footer-actions svg')).toHaveCount(4);
  await bounded(page);
});

test('hover reset times stay inside the row without moving percentages and can be disabled', async ({ page }) => {
  const row = page.locator('.mb-account-row').first(); const numbers = row.locator('.mb-mini strong');
  const geometry = () => numbers.evaluateAll(fields => fields.map(field => { const rect = field.getBoundingClientRect(); return { top: rect.top, right: rect.right }; }));
  const before = await geometry(); const values = await numbers.allTextContents();
  const columnGap = () => row.locator('.mb-account-window').first().evaluate(el => el.querySelector('.mb-mini.other .mb-meter')!.getBoundingClientRect().left - el.querySelector('.mb-mini.gemini strong')!.getBoundingClientRect().right);
  expect(await columnGap()).toBeGreaterThanOrEqual(24);
  const divider = () => row.locator('.mb-mini.other').first().evaluate(el => {
    const style = getComputedStyle(el, '::before');
    return { content: style.content, width: style.width, left: style.left };
  });
  expect(await divider()).toEqual({ content: '""', width: '1px', left: '-12px' });
  const meter = row.locator('.mb-meter').first(); const fullWidth = await meter.evaluate(el => el.getBoundingClientRect().width);
  const control = row.locator('.mb-account-switch');
  const initialBackground = await control.evaluate(el => getComputedStyle(el).backgroundColor);
  await control.hover();
  await expect(row.locator('.mb-reset-label').first()).toBeVisible();
  await expect(row.locator('.mb-reset-label').first()).toContainText(/\dh/);
  await expect(meter).toBeVisible(); expect(await meter.evaluate(el => el.getBoundingClientRect().width)).toBeLessThan(fullWidth);
  await expect.poll(() => control.evaluate(el => getComputedStyle(el).backgroundColor)).not.toBe(initialBackground);
  expect(await geometry()).toEqual(before); expect(await numbers.allTextContents()).toEqual(values);
  expect(await columnGap()).toBeGreaterThanOrEqual(24);
  await bounded(page); await expect(page.getByRole('tooltip')).toHaveCount(0);
  await page.mouse.move(0, 0); await expect(row.locator('.mb-reset-label').first()).toBeHidden();
  expect(await meter.evaluate(el => el.getBoundingClientRect().width)).toBe(fullWidth);
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ menu_bar: { reset_time_display: 'always' } }));
  await expect(row.locator('.mb-reset-label').first()).toBeVisible(); await expect(meter).toBeVisible();
  expect(await meter.evaluate(el => el.getBoundingClientRect().width)).toBeLessThan(fullWidth);
  expect(await geometry()).toEqual(before); await control.hover(); await page.mouse.move(0, 0);
  await expect(row.locator('.mb-reset-label').first()).toBeVisible(); await bounded(page);
  for (const width of [320, 424]) {
    await page.setViewportSize({ width, height: 640 });
    await bounded(page);
    expect(await row.locator('.mb-meter').first().evaluate(el => el.getBoundingClientRect().width)).toBeGreaterThanOrEqual(18);
  }
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ menu_bar: { reset_time_display: 'hidden' } }));
  await control.hover(); await expect(row.locator('.mb-reset-label')).toHaveCount(0); await expect(row.locator('.mb-meter').first()).toBeVisible();
  expect(await meter.evaluate(el => el.getBoundingClientRect().width)).toBe(fullWidth);
  await page.evaluate(() => (window as any).__menuFixture.setPreferences({ menu_bar: { display_scope: 'other' } }));
  await expect(row.locator('.mb-mini')).toHaveCount(2);
  expect(await row.locator('.mb-mini').first().evaluate(el => getComputedStyle(el, '::before').content)).toBe('none');
  await bounded(page);
  expect(await calls(page, 'switch_account')).toEqual([]);
});
