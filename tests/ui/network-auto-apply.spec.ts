import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

for (const language of ['zh', 'en']) {
    test(`proxy modes apply immediately and invalid manual drafts retain routing (${language})`, async ({ page }, info) => {
        await page.addInitScript(setupSettingsFixture, { language });
        await page.goto('/settings');
        const section = page.getByRole('heading', { name: language === 'zh' ? '网络代理' : 'Network proxy', exact: true }).locator('..');
        const mode = section.getByRole('combobox');
        const address = section.getByRole('textbox');
        const stored = () => page.evaluate(() => (window as any).__settingsFixture.general().network);
        await expect(mode).toHaveValue('system');
        await expect(section.getByRole('button')).toHaveCount(0);
        await mode.selectOption('direct');
        await expect.poll(stored).toEqual({ proxy_mode: 'direct', proxy_url: '' });
        await mode.selectOption('manual');
        await address.fill('http://');
        await address.blur();
        await expect(section.getByRole('alert')).toBeVisible();
        expect(await stored()).toEqual({ proxy_mode: 'direct', proxy_url: '' });
        await address.fill('http://user:password@127.0.0.1:7890');
        await address.press('Enter');
        await expect(address).toHaveAttribute('aria-invalid', 'true');
        expect(await stored()).toEqual({ proxy_mode: 'direct', proxy_url: '' });
        await address.fill('socks5h://127.0.0.1:7891');
        await expect.poll(stored).toEqual({ proxy_mode: 'manual', proxy_url: 'socks5h://127.0.0.1:7891' });
        await expect(section.getByRole('alert')).toHaveCount(0);
        await address.fill('http://127.0.0.1:7892');
        await address.blur();
        await expect.poll(stored).toEqual({ proxy_mode: 'manual', proxy_url: 'http://127.0.0.1:7892' });
        await mode.selectOption('system');
        await expect.poll(stored).toEqual({ proxy_mode: 'system', proxy_url: '' });
        await page.getByRole('link', { name: language === 'zh' ? '账号管理' : 'Accounts', exact: true }).click();
        await page.getByRole('link', { name: language === 'zh' ? '设置' : 'Settings', exact: true }).click();
        await expect(mode).toHaveValue('system');
        await page.setViewportSize({ width: 760, height: 700 });
        await section.screenshot({ path: info.outputPath(`network-auto-${language}.png`) });
    });
}

test('ordered proxy writes preserve newer edits and recover after a failure', async ({ page }) => {
    await page.addInitScript(setupSettingsFixture);
    await page.goto('/settings');
    const mode = page.getByLabel('代理方式');
    const address = page.getByLabel('代理地址');
    const section = page.getByRole('heading', { name: '网络代理', exact: true }).locator('..');
    const stored = () => page.evaluate(() => (window as any).__settingsFixture.general().network);
    await page.evaluate(() => { (window as any).__settingsFixture.holdGeneralSave = true; });
    await mode.selectOption('direct');
    await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((c: any) => c.command === 'save_config').length)).toBe(1);
    await mode.selectOption('manual');
    await address.fill('socks5://127.0.0.1:7891');
    await address.press('Enter');
    await page.evaluate(() => {
        const fixture = (window as any).__settingsFixture;
        fixture.holdGeneralSave = false;
        fixture.resolveGeneralSave();
    });
    await expect.poll(stored).toEqual({ proxy_mode: 'manual', proxy_url: 'socks5://127.0.0.1:7891' });
    await expect(mode).toHaveValue('manual');
    await expect(address).toHaveValue('socks5://127.0.0.1:7891');
    await page.evaluate(() => {
        const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
        let fail = true;
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any) => {
            if (command === 'save_config' && fail) { fail = false; throw 'synthetic write failure'; }
            return original(command, args);
        };
    });
    await mode.selectOption('system');
    await expect(section.getByRole('alert')).toContainText('保存失败');
    expect(await stored()).toEqual({ proxy_mode: 'manual', proxy_url: 'socks5://127.0.0.1:7891' });
    await mode.selectOption('direct');
    await expect.poll(stored).toEqual({ proxy_mode: 'direct', proxy_url: '' });
    await expect(section.getByRole('alert')).toHaveCount(0);
});
