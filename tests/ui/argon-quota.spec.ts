import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

for (const language of ['zh', 'en']) for (const view of ['grid', 'list']) test(`unknown Argon model never displays its old percentage in ${view} (${language})`, async ({ page }) => {
  await page.addInitScript(setupSettingsFixture, { language });
  await page.addInitScript(({ view }) => {
    localStorage.setItem('accounts_view_mode', view);
    localStorage.setItem('accounts_quota_window', '5h');
    const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
      const value = await original(command, args);
      if (command === 'load_config') return { ...value, pinned_quota_models: {models:['gemini-4-argon-high']} };
      if (command === 'list_accounts') return [{id:'fixture',email:'fixture@example.invalid',protected_models:[],created_at:1,last_used:1,
        quota:{last_updated:1,models:[{name:'gemini-4-argon-high',percentage:100,percentage_known:false,reset_time:''}]}}];
      return value;
    };
  }, { view });
  await page.goto('/accounts');
  await expect(page.getByText('—', {exact:true})).toHaveCount(1);
  await expect(page.getByText('100%', {exact:true})).toHaveCount(0);
});

for (const language of ['zh', 'en']) for (const view of ['grid', 'list']) test(`Argon has a dedicated weekly bar in account ${view} (${language})`, async ({ page }, info) => {
  await page.addInitScript(setupSettingsFixture, { language });
  await page.addInitScript(({ view }) => {
    localStorage.setItem('accounts_view_mode', view);
    localStorage.setItem('accounts_quota_window', 'weekly');
    const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
      if (command === 'list_accounts') return [{ id: 'fixture', email: 'fixture@example.invalid', name: 'Fixture', protected_models: [], created_at: 1, last_used: 1,
        quota: { last_updated: Date.now() / 1000, models: [], quota_groups: [
          { display_name: 'Gemini Models', buckets: [{ bucket_id: 'gemini-weekly', window: 'weekly', remaining_fraction: .9, remaining_fraction_known: true, reset_time: '2030-01-01T00:00:00Z' }] },
          { display_name: 'Gemini 4 Argon', buckets: [{ bucket_id: 'gemini-argon-wtu-weekly', window: 'weekly', remaining_fraction: .1, remaining_fraction_known: true, reset_time: '2030-01-01T00:00:00Z' }] },
        ] } }];
      return original(command, args);
    };
  }, { view });
  await page.goto('/accounts');
  const argon = page.locator(`div[title="Gemini 4 Argon (${language === 'zh' ? '独立池' : 'Dedicated Pool'})"]`);
  await expect(argon).toContainText('10%');
  await expect(page.locator(`div[title="Gemini (${language === 'zh' ? '共享池' : 'Shared Pool'})"]`)).toContainText('90%');
  await page.screenshot({ path: info.outputPath(`account-argon-${language}-${view}.png`) });
});
