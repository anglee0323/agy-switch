import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

for (const language of ['zh', 'en']) for (const view of ['grid', 'list']) test(`5h and weekly account views retain their own values in ${view} (${language})`, async ({ page }) => {
  await page.addInitScript(setupSettingsFixture, { language });
  await page.addInitScript(({ view }) => {
    localStorage.setItem('accounts_view_mode', view);
    localStorage.setItem('accounts_quota_window', '5h');
    const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
      if (command === 'load_config') return {...await original(command,args),pinned_quota_models:{models:['claude-sonnet-4-6']}};
      if (command === 'list_accounts') return [{id:'fixture',email:'fixture@example.invalid',protected_models:[],created_at:1,last_used:1,
        quota:{last_updated:Date.now()/1000,models:[{name:'claude-sonnet-4-6',percentage:100,percentage_known:true,reset_time:''}],quota_groups:[
          {display_name:'Claude / GPT',buckets:[
            {bucket_id:'3p-5h',window:'5h',remaining_fraction:1,remaining_fraction_known:true,reset_time:'2030-01-01T00:00:00Z'},
            {bucket_id:'3p-weekly',window:'weekly',remaining_fraction:0,remaining_fraction_known:true,reset_time:'2030-01-07T00:00:00Z'},
          ]},
        ]}}];
      return original(command,args);
    };
  }, {view});
  await page.goto('/accounts');
  await expect(page.getByText('100%',{exact:true})).toHaveCount(1);
  await expect(page.getByText('0%',{exact:true})).toHaveCount(0);
  await page.getByTitle(language === 'zh' ? '7 天配额' : '7-day quota', {exact:true}).click();
  await expect(page.getByText('0%',{exact:true})).toHaveCount(1);
  await expect(page.getByText('100%',{exact:true})).toHaveCount(0);
});

for (const language of ['zh','en']) test(`Argon selection is available before the source reports a model (${language})`, async ({page}) => {
  await page.addInitScript(setupSettingsFixture,{language});
  await page.goto('/settings');
  await page.locator('#settings-tab-quota').click();
  await expect(page.getByText('Gemini 4 Argon',{exact:true})).toBeVisible();
  await expect(page.getByText(language === 'zh'
    ? '勾选模型不会开通使用权限。Argon 使用独立周额度；接口尚未返回其额度时显示 —，刷新不会解锁模型。'
    : 'Selecting a model does not grant access. Argon uses its own weekly quota; an unreported allowance shows —. Refreshing does not unlock models.', {exact:true})).toBeVisible();
});

for (const view of ['grid','list']) test(`Argon model selector uses a reported dedicated pool without a catalog model (${view})`, async({page},info)=>{
  await page.addInitScript(setupSettingsFixture,{language:'en'});
  await page.addInitScript(view=>{
    localStorage.setItem('accounts_view_mode',view);
    localStorage.setItem('accounts_quota_window','5h');
    const w=window as any, original=w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke=async(command:string,args:any={})=>{
      if(command==='load_config')return {...await original(command,args),pinned_quota_models:{models:['gemini-4-argon']}};
      if(command==='list_accounts')return [{id:'fixture',email:'fixture@example.invalid',protected_models:[],created_at:1,last_used:1,quota:{last_updated:1,models:[],quota_groups:[
        {display_name:'Gemini 4 Argon',buckets:[{bucket_id:'gemini-argon-wtu-weekly',window:'weekly',remaining_fraction:.23,remaining_fraction_known:true,reset_time:'2030-01-07T00:00:00Z'}]},
      ]}}];
      return original(command,args);
    };
  },view);
  await page.goto('/accounts');
  await expect(page.getByText('23%',{exact:true})).toHaveCount(1);
  await expect(page.locator('div[title="Gemini 4 Argon"]')).toContainText('Weekly');
  await page.screenshot({path:info.outputPath(`argon-weekly-model-${view}.png`)});
});

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
