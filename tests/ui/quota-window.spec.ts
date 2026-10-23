import {expect, test} from '@playwright/test';
import {setupSettingsFixture} from './settings-fixture';

for (const language of ['zh','en']) for (const view of ['grid','list']) {
  test(`cached weekly Gemini quota does not replace the 5h window (${language}, ${view})`, async ({page}, info) => {
    await page.addInitScript(setupSettingsFixture,{language});
    await page.addInitScript(view => {
      localStorage.setItem('accounts_view_mode',view);
      localStorage.setItem('accounts_quota_window','5h');
      const w=window as any, original=w.__TAURI_INTERNALS__.invoke;
      w.__TAURI_INTERNALS__.invoke=async(command:string,args:any={})=>{
        if(command==='load_config')return {...await original(command,args),pinned_quota_models:{models:['gemini-3.8-flash']}};
        if(command==='list_accounts'){
          const short=new Date(Date.now()+2*3600000).toISOString(), week=new Date(Date.now()+4*86400000).toISOString();
          return [{id:'fixture',email:'fixture@example.invalid',protected_models:[],created_at:1,last_used:1,quota:{last_updated:Date.now()/1000,
            models:[{name:'gemini-3.8-flash',percentage:25,percentage_known:true,observed_remaining_fraction:.25,quota_bucket_id:'gemini-weekly',reset_time:week}],
            quota_groups:[{display_name:'Gemini Models',buckets:[
              {bucket_id:'gemini-weekly',window:'weekly',remaining_fraction:.25,remaining_fraction_known:true,reset_time:week},
              {bucket_id:'gemini-5h',window:'5h',remaining_fraction:.8,remaining_fraction_known:true,reset_time:short},
            ]}],
          }}];
        }
        return original(command,args);
      };
    },view);
    await page.goto('/accounts');
    const model=page.locator('div[title="Gemini 3.8 Flash"]');
    await expect(model).toContainText('80%');
    await expect(model).toContainText(/\d+h \d+m/);
    await expect(model).not.toContainText(/\d+d \d+h/);
    await page.screenshot({path:info.outputPath(`gemini-5h-${language}-${view}.png`)});
    await page.getByTitle(language==='zh'?'7 天配额':'7-day quota',{exact:true}).click();
    const weekly=page.locator(`div[title="Gemini (${language==='zh'?'共享池':'Shared Pool'})"]`);
    await expect(weekly).toContainText('25%');
    await expect(weekly).toContainText(/\d+d \d+h/);
    await expect(page.getByText('80%',{exact:true})).toHaveCount(0);
  });
}

test('reset sorting follows the selected window instead of the effective weekly reset', async ({page}) => {
  await page.addInitScript(setupSettingsFixture,{language:'en'});
  await page.addInitScript(() => {
    localStorage.setItem('accounts_view_mode','list');
    localStorage.setItem('accounts_quota_window','5h');
    const w=window as any,original=w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke=async(command:string,args:any={})=>{
      if(command==='load_config')return {...await original(command,args),pinned_quota_models:{models:['gemini-3.8-flash']}};
      if(command==='list_accounts')return ['long','short'].map((id,i)=>{
        const short=new Date(Date.now()+(i?2:4)*3600000).toISOString(),week=new Date(Date.now()+(i?4:1)*86400000).toISOString();
        return {id,email:`${id}-first@example.invalid`,protected_models:[],created_at:1,last_used:1,quota:{last_updated:Date.now()/1000,
          models:[{name:'gemini-3.8-flash',percentage:25,percentage_known:true,observed_remaining_fraction:.25,quota_bucket_id:'gemini-weekly',reset_time:week}],
          quota_groups:[{display_name:'Gemini Models',buckets:[
            {bucket_id:'gemini-weekly',window:'weekly',remaining_fraction:.25,remaining_fraction_known:true,reset_time:week},
            {bucket_id:'gemini-5h',window:'5h',remaining_fraction:.8,remaining_fraction_known:true,reset_time:short},
          ]}],
        }};
      });
      return original(command,args);
    };
  });
  await page.goto('/accounts');
  await page.getByTitle(/sort by quota reset time/i).click();
  await expect(page.locator('tbody tr').first()).toContainText('short-first@example.invalid');
  await page.getByTitle('7-day quota',{exact:true}).click();
  await expect(page.locator('tbody tr').first()).toContainText('long-first@example.invalid');
});
