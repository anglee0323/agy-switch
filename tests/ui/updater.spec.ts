import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';
for (const language of ['zh', 'en']) test(`update prompt and manual check are localized (${language})`, async ({ page }) => {
    const override = () => {
        const w=window as any, original=w.__TAURI_INTERNALS__.invoke;
        w.__updateFixture={fail:false,newer:true,calls:[]};
        w.__TAURI_INTERNALS__.invoke=async (command:string,args:any)=>{
            w.__updateFixture.calls.push(command);
            if(command==='check_for_updates'){
                if(w.__updateFixture.fail)throw 'raw backend language should not appear';
                return {current_version:'4.7.8',latest_version:'v4.7.9',has_update:w.__updateFixture.newer,release_url:'https://github.com/anglee0323/agy-switch/releases/tag/v4.7.9'};
            }
            if(command==='get_running_version')return '4.7.8';
            if(command==='download_and_install_update'){
                args.progress.onmessage({stage:'downloading',downloaded:50,total:100});
                await new Promise((_, reject)=>{w.__updateFixture.rejectDownload=()=>reject('update_mac_trust_required');});
            }
            return original(command,args);
        };
    };
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language })});(${override.toString()})();` });
    await page.goto('/settings');
    const check=page.getByRole('button',{name:language==='zh'?'检查更新':'Check for updates',exact:true});
    await check.click(); const dialog=page.getByRole('dialog'); await expect(dialog).toBeVisible();
    if(language==='en')expect(await dialog.innerText()).not.toMatch(/[\u3400-\u9fff]/);
    expect(await page.evaluate(()=>(window as any).__updateFixture.calls)).not.toContain('download_and_install_update');
    await dialog.getByRole('button',{name:language==='zh'?'下载并安装':'Download and install'}).click();
    await expect(dialog.getByRole('status')).toContainText('50%');
    expect(await page.evaluate(()=>Boolean(document.activeElement?.closest('[role=dialog]')))).toBe(true);
    await page.keyboard.press('Escape'); await expect(dialog).toBeVisible();
    await page.evaluate(()=>(window as any).__updateFixture.rejectDownload());
    await expect(dialog.getByRole('alert')).toContainText(language==='zh'?'未通过 macOS':'did not pass macOS');
    await expect(dialog.getByRole('status')).toHaveCount(0);
    await page.keyboard.press('Escape'); await expect(dialog).toHaveCount(0);
    expect(await page.evaluate(()=>localStorage.getItem('dismissed_release'))).toBe('v4.7.9');
    await page.getByLabel(language==='zh'?'启动时检查更新':'Check for updates on startup').uncheck();
    await expect.poll(()=>page.evaluate(()=>(window as any).__settingsFixture.calls.filter((c:any)=>c.command==='save_config').at(-1)?.args.config.check_updates_on_startup)).toBe(false);
    await page.evaluate(()=>{(window as any).__updateFixture.newer=false;}); await check.click();
    await expect(page.getByText(language==='zh'?'当前已是最新版本':'You are up to date',{exact:true})).toBeVisible();
    await page.evaluate(()=>{(window as any).__updateFixture.fail=true;}); await check.click();
    await expect(page.getByRole('alert')).toContainText(language==='zh'?'暂时无法检查更新':'Could not check for updates');
    expect(await page.locator('main').innerText()).not.toContain('raw backend');
    expect((await page.evaluate(()=>(window as any).__updateFixture.calls)).filter((c:string)=>c==='download_and_install_update')).toHaveLength(1);
});

for (const enabled of [true, false]) test(`startup check respects the saved preference (${enabled})`, async ({ page }) => {
    await page.clock.install();
    const override = (startup: boolean) => {
        const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
        w.__startupChecks = 0;
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
            if (command === 'load_config') return { ...await original(command, args), check_updates_on_startup: startup };
            if (command === 'check_for_updates') {
                w.__startupChecks++;
                return { current_version: '4.9.0', latest_version: 'v4.9.1', has_update: true, release_url: 'https://github.com/anglee0323/agy-switch/releases/tag/v4.9.1' };
            }
            return original(command, args);
        };
    };
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})({language:'en'});(${override.toString()})(${enabled});` });
    await page.goto('/settings');
    await expect(page.getByLabel('Check for updates on startup')).toBeVisible();
    await page.clock.fastForward(4500);
    if (enabled) {
        await expect(page.getByRole('dialog')).toContainText('4.9.1');
        expect(await page.evaluate(() => (window as any).__startupChecks)).toBe(1);
        await page.getByRole('dialog').getByRole('button', { name: 'Later', exact: true }).click();
        await page.reload();
        await expect(page.getByLabel('Check for updates on startup')).toBeVisible();
        await page.clock.fastForward(4500);
        await expect.poll(() => page.evaluate(() => (window as any).__startupChecks)).toBe(1);
        await expect(page.getByRole('dialog')).toHaveCount(0);
    } else {
        expect(await page.evaluate(() => (window as any).__startupChecks)).toBe(0);
        await expect(page.getByRole('dialog')).toHaveCount(0);
    }
});

for (const language of ['zh', 'en']) test(`manual-required Mac installs explain the browser redirect without downloading (${language})`, async ({ page }) => {
    const override = () => {
        const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
        w.__updateFixture = { calls: [], openFail: false };
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
            w.__updateFixture.calls.push(command);
            if (command === 'check_for_updates')
                return { current_version: '4.9.0', latest_version: 'v4.9.1', has_update: true, release_url: 'https://github.com/anglee0323/agy-switch/releases/tag/v4.9.1' };
            if (command === 'get_running_version') return '4.9.0';
            if (command === 'download_and_install_update') {
                // The Rust pre-download gate fails before any progress is reported.
                throw w.__updateFixture.openFail ? 'update_open_failed' : 'update_mac_manual_required';
            }
            return original(command, args);
        };
    };
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language })});(${override.toString()})();` });
    await page.goto('/settings');
    await page.getByRole('button', { name: language === 'zh' ? '检查更新' : 'Check for updates', exact: true }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: language === 'zh' ? '下载并安装' : 'Download and install' }).click();
    await expect(dialog.getByRole('alert')).toContainText(language === 'zh' ? '已在浏览器中打开' : 'opened in your browser');
    await expect(dialog.getByRole('status')).toHaveCount(0);
    expect(await dialog.innerText()).not.toContain('update_mac');
    await page.evaluate(() => { (window as any).__updateFixture.openFail = true; });
    await dialog.getByRole('button', { name: language === 'zh' ? '下载并安装' : 'Download and install' }).click();
    await expect(dialog.getByRole('alert')).toContainText(language === 'zh' ? '无法打开下载页' : 'Could not open the download page');
    await expect(dialog.getByRole('status')).toHaveCount(0);
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
});
