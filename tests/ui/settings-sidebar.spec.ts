import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';
test.beforeEach(async ({ page }, info) => {
  const failure = info.annotations.find(a => a.type === 'fixture-failure')?.description;
  await page.addInitScript(setupSettingsFixture, { failLoad: failure === 'general', failLowQuotaLoad: failure === 'low-quota' });
  await page.goto('/settings'); await expect(page.getByRole('tab', { name: '常规偏好', exact: true })).toHaveAttribute('aria-selected', 'true');
});
test('three retained panels preserve fields and localization is absent', async ({ page }) => {
  expect(await page.getByRole('tab').allTextContents()).toEqual(['常规偏好', '配额与模型', '智能切换策略']);
  for (const name of ['配额与模型', '智能切换策略', '常规偏好']) {
    await page.getByRole('tab', { name, exact: true }).click(); await expect(page.getByRole('tabpanel')).toHaveCount(1); await expect(page.getByRole('tabpanel', { name, exact: true })).toBeVisible();
  }
  await expect(page.getByText('/synthetic/antigravity-tools', { exact: true })).toBeVisible();
  await expect(page.getByLabel('菜单栏聚合额度')).toBeEnabled();
  await expect(page.getByText(/App 汉化|实验功能/)).toHaveCount(0);
});
test('menu display preferences save partial patches and retain one quota window', async ({ page }) => {
  await expect(page.getByLabel('账号名称格式')).toHaveValue('email_then_label');
  await expect(page.getByLabel('隐藏失效和禁用账号')).toBeChecked();
  await expect(page.getByLabel('重置时间显示')).toHaveValue('hover');
  await page.getByLabel('重置时间显示').selectOption('hidden');
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((call: any) => call.command === 'set_menu_bar_preferences').at(-1)?.args?.patch)).toEqual({ reset_time_display: 'hidden' });
  await page.getByLabel('重置时间显示').selectOption('always');
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((call: any) => call.command === 'set_menu_bar_preferences').at(-1)?.args?.patch)).toEqual({ reset_time_display: 'always' });
  await page.getByLabel('账号区显示系列').selectOption('other');
  await expect(page.getByLabel('账号区显示系列')).toHaveValue('other');
  await expect(page.getByLabel('重置时间显示')).toHaveValue('always');
  await page.getByLabel('显示 5 小时额度').uncheck();
  await expect(page.getByLabel('显示 5 小时额度')).not.toBeChecked();
  await expect(page.getByLabel('显示 5 小时额度')).toBeEnabled();
  await expect(page.getByLabel('显示周额度')).toBeDisabled();
  await page.getByLabel('显示 5 小时额度').check();
  await expect(page.getByLabel('显示周额度')).toBeEnabled();
  await page.getByLabel('绿色额度阈值').fill('75'); await page.getByLabel('绿色额度阈值').blur();
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((call: any) => call.command === 'set_menu_bar_preferences' || call.cmd === 'set_menu_bar_preferences').at(-1)?.args?.patch)).toEqual({ green_above: 75 });
  await expect(page.getByLabel('账号区显示系列')).toHaveValue('other');
  await page.getByLabel('红色额度阈值').fill('90'); await page.getByLabel('红色额度阈值').blur(); await expect(page.getByLabel('红色额度阈值')).toHaveValue('20');
});
test('model selection follows the UI language for actions and shared-pool descriptions', async ({ page }) => {
  await page.getByRole('button', { name: 'English', exact: true }).click();
  await page.locator('#settings-tab-quota').click();
  const panel = page.locator('#settings-panel-quota');
  await expect(panel.getByRole('button', { name: 'Select all', exact: true })).toBeVisible();
  await expect(panel.getByText('Shared quota pool: High / Medium / Low', { exact: true })).toBeVisible();
  expect(await panel.innerText()).not.toMatch(/[\u3400-\u9fff]/);
  await panel.getByRole('button', { name: 'Select all', exact: true }).click();
  await expect(panel.getByRole('button', { name: 'Deselect all', exact: true })).toBeVisible();
  await page.locator('#settings-tab-general').click();
  await page.getByRole('button', { name: '简体中文', exact: true }).click();
  await page.locator('#settings-tab-quota').click();
  await expect(panel.getByRole('button', { name: '取消全选', exact: true })).toBeVisible();
  await expect(panel.getByText('共享额度池：High / Medium / Low', { exact: true })).toBeVisible();
});
test('number edits and account choices auto-save and survive navigation', async ({ page }) => {
  await page.locator('#settings-tab-autoSwitch').click(); const reserve = page.locator('#auto-switch-reserve');
  await reserve.fill('13'); await page.locator('#auto-switch-model').selectOption('gemini'); await page.getByLabel('backup@example.invalid', { exact: true }).check();
  await page.locator('#settings-tab-general').click(); await page.locator('#settings-tab-autoSwitch').click();
  await expect(reserve).toHaveValue('13'); await expect(page.getByLabel('backup@example.invalid', { exact: true })).toBeChecked();
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.lowQuota())).toMatchObject({ reserve_percentage: 13, monitored_model: 'gemini', candidate_account_ids: ['fixture-1'], enabled: false });
  await expect(page.getByRole('button', { name: '保存设置', exact: true })).toHaveCount(0);
});
test('in-flight auto-saves are ordered and old responses cannot erase a newer draft', async ({ page }) => {
  await page.locator('#settings-tab-autoSwitch').click(); await page.evaluate(() => { (window as any).__settingsFixture.holdAutoSave = true; });
  await page.locator('#auto-switch-reserve').fill('14'); await page.locator('#auto-switch-reserve').blur();
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((c: any) => c.command === 'set_auto_switch_config').length)).toBe(1);
  await page.locator('#auto-switch-model').selectOption('claude');
  await page.locator('#settings-tab-general').click(); await page.evaluate(() => (window as any).__settingsFixture.resolveAutoSave());
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((c: any) => c.command === 'set_auto_switch_config').length)).toBe(2);
  await page.locator('#settings-tab-autoSwitch').click(); await expect(page.locator('#auto-switch-model')).toHaveValue('claude'); await expect(page.locator('#auto-switch-reserve')).toHaveValue('14');
  await page.evaluate(() => (window as any).__settingsFixture.resolveAutoSave()); await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.lowQuota())).toMatchObject({ reserve_percentage: 14, monitored_model: 'claude' });
});
test('failed automatic save retains edits and later changes recover', async ({ page }) => {
  await page.locator('#settings-tab-autoSwitch').click(); await page.evaluate(() => { (window as any).__settingsFixture.holdAutoSave = true; });
  await page.locator('#auto-switch-model').selectOption('gemini'); await page.locator('#settings-tab-general').click();
  await page.evaluate(() => (window as any).__settingsFixture.rejectAutoSave()); await page.locator('#settings-tab-autoSwitch').click();
  await expect(page.getByRole('alert').filter({ hasText: 'synthetic save rejection' })).toBeVisible(); await expect(page.locator('#auto-switch-model')).toHaveValue('gemini');
  await page.evaluate(() => { (window as any).__settingsFixture.holdAutoSave = false; }); await page.locator('#auto-switch-model').selectOption('claude'); await expect(page.getByRole('alert')).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.lowQuota().monitored_model)).toBe('claude');
});
for (const failure of ['general', 'low-quota']) test(failure + ' load failure is scoped and retry recovers', { annotation: { type: 'fixture-failure', description: failure } }, async ({ page }) => {
  if (failure === 'general') {
    const alert = page.getByRole('alert').filter({ hasText: 'synthetic load rejection' }); await expect(alert).toBeVisible();
    await page.locator('#settings-tab-autoSwitch').click(); await expect(page.locator('#auto-switch-model')).toBeEnabled();
    await page.evaluate(() => { (window as any).__settingsFixture.failLoad = false; }); await alert.getByRole('button', { name: '重新读取' }).click(); await expect(alert).toHaveCount(0);
  } else {
    await page.locator('#settings-tab-autoSwitch').click(); const alert = page.getByRole('alert').filter({ hasText: '无法读取设置或账号' }); await expect(alert).toBeVisible();
    await page.evaluate(() => { (window as any).__settingsFixture.failLowQuotaLoad = false; }); await page.getByRole('tabpanel').getByRole('button', { name: '重新读取' }).click(); await expect(alert).toHaveCount(0); await expect(page.locator('#auto-switch-model')).toBeEnabled();
  }
});
test('keyboard navigation wraps and mounted hidden panels stay outside the tab order', async ({ page }) => {
  await page.getByRole('tab').first().focus(); await page.keyboard.press('ArrowDown'); await expect(page.locator('#settings-tab-quota')).toBeFocused();
  await page.keyboard.press('End'); await expect(page.locator('#settings-tab-autoSwitch')).toBeFocused();
  await page.setViewportSize({ width: 420, height: 600 }); await expect(page.getByRole('tablist')).toHaveAttribute('aria-orientation', 'horizontal');
  await page.getByRole('tab').first().focus(); await page.keyboard.press('ArrowLeft'); await expect(page.locator('#settings-tab-autoSwitch')).toBeFocused(); await page.keyboard.press('Home'); await expect(page.locator('#settings-tab-general')).toBeFocused();
  await page.keyboard.press('Tab'); await expect(page.getByRole('tabpanel')).toBeFocused();
  for (let i = 0; i < 12; i++) { await page.keyboard.press('Tab'); expect(await page.evaluate(() => document.activeElement?.closest('[role="tabpanel"][hidden]') === null)).toBe(true); }
  await expect(page.locator('[role="tabpanel"][hidden]')).toHaveCount(2);
});
test('desktop preferences survive navigation and ordinary appearance saves', async ({ page }) => {
  const login = page.locator('#desktop-launch_at_login'); const dock = page.locator('#desktop-hide_dock_icon');
  await login.click(); await expect(login).toHaveAttribute('aria-checked', 'true'); await dock.click(); await expect(dock).toHaveAttribute('aria-checked', 'true');
  await page.locator('#settings-tab-quota').click(); await page.locator('#settings-tab-general').click(); await expect(login).toHaveAttribute('aria-checked', 'true'); await expect(dock).toHaveAttribute('aria-checked', 'true');
  await page.getByRole('button', { name: '深色', exact: true }).click(); await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(await page.evaluate(() => (window as any).__TAURI_INTERNALS__.invoke('load_config'))).toMatchObject({ desktop: { launch_at_login: true, hide_dock_icon: true, start_minimized: true } });
});
test('all categories and controls fit short bilingual windows; scrollers reset on navigation', async ({ page }, info) => {
  for (const language of ['zh', 'en']) {
    await page.locator('#settings-tab-general').click(); await page.getByRole('button', { name: language === 'en' ? 'English' : '简体中文', exact: true }).click();
    for (const width of [760, 420]) {
      await page.setViewportSize({ width, height: 520 });
      for (const id of ['general', 'quota', 'autoSwitch']) {
        await page.locator('#settings-tab-' + id).click(); const panel = page.locator('#settings-panel-' + id); await expect(panel).toBeVisible();
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
        expect(await panel.locator('input,select,button').evaluateAll(fields => fields.filter(f => (f as HTMLElement).offsetWidth > 2).every(f => { const r=f.getBoundingClientRect();return r.left>=0 && r.right<=innerWidth+1; }))).toBe(true);
        await panel.locator('p').last().scrollIntoViewIfNeeded(); await page.screenshot({ path: info.outputPath('settings-' + language + '-' + width + '-' + id + '.png') });
      }
    }
  }
});

test('candidate order supports keyboard dragging and survives saved navigation', async ({ page }) => {
  await page.locator('#settings-tab-autoSwitch').click();
  await expect(page.getByRole('radio', { name: /优先顺序/ })).toBeChecked();
  await page.getByRole('radio', { name: /循环轮换/ }).check();
  await page.getByLabel('primary@example.invalid', { exact: true }).check();
  await page.getByLabel('backup@example.invalid', { exact: true }).check();
  await page.getByLabel('studio@example.invalid', { exact: true }).check();
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.lowQuota().candidate_account_ids)).toEqual(['fixture-0', 'fixture-1', 'fixture-2']);
  const handle = page.getByRole('button', { name: '调整 primary@example.invalid 的顺序', exact: true });
  await handle.focus(); await page.keyboard.press('Space'); await expect(handle).toHaveAttribute('aria-pressed', 'true');
  // The initial collision announcement confirms measured rows and an active sensor.
  await expect(page.locator('[role="status"]').filter({ hasText: '目标位置：primary@example.invalid' })).toBeVisible();
  await page.keyboard.press('ArrowDown'); await expect(page.locator('[role="status"]').filter({ hasText: '目标位置：backup@example.invalid' })).toBeVisible(); await page.keyboard.press('Space');
  await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.lowQuota())).toMatchObject({ strategy: 'round_robin', candidate_account_ids: ['fixture-1', 'fixture-0', 'fixture-2'] });
  await page.locator('#settings-tab-general').click(); await page.locator('#settings-tab-autoSwitch').click();
  await expect(page.locator('[data-candidate-id]').first()).toHaveAttribute('data-candidate-id', 'fixture-1');
});

test('language-change confirmation uses the selected language', async ({ page }) => {
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language: 'zh' })});` });
    await page.goto('/settings');
    await page.getByRole('button', { name: 'English', exact: true }).click();
    await expect(page.getByText('Settings saved', { exact: true })).toBeVisible();
    await expect(page.getByText('设置已保存', { exact: true })).toHaveCount(0);
});
