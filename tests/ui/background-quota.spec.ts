import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

test('quota settings save without starting duplicate WebView refresh timers', async ({ page }) => {
  await page.addInitScript(setupSettingsFixture, { autoRefresh: true });
  await page.clock.install();
  await page.goto('/settings');
  await page.getByRole('tab', { name: '配额与模型', exact: true }).click();
  const toggle = page.getByRole('switch', { name: '自动刷新账号配额', exact: true });
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await page.clock.fastForward(3600001);
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await page.clock.fastForward(3600001);
  const calls = await page.evaluate(() => (window as any).__settingsFixture.calls);
  expect(calls.filter((call: any) => call.command === 'refresh_all_quotas')).toHaveLength(0);
  expect(calls.filter((call: any) => call.command === 'save_config').map((call: any) => call.args.config.auto_refresh)).toEqual([false, true]);
});
