import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

test.beforeEach(async ({ page }) => {
    await page.addInitScript(setupSettingsFixture);
    await page.goto('/accounts');
    await page.evaluate(() => {
        const w = window as any;
        const original = w.__TAURI_INTERNALS__.invoke;
        w.__transferCalls = [];
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
            w.__transferCalls.push({ command, args });
            if (command === 'plugin:dialog|save') return '/synthetic/export.json';
            if (command === 'save_account_export') return args.accountIds.length;
            if (command === 'switch_account') { if (args.targetIde === 'ide') w.__ideId = args.accountId; return; }
            if (command === 'get_current_account' && args.targetIde === 'ide') return (await original('list_accounts')).find((account: any) => account.id === w.__ideId) || null;
            if (command === 'import_local_accounts_report') return { accounts: [{ id: 'fixture-1' }], sources: [{ source: 'App', status: 'imported' }, { source: 'IDE', status: 'validation_failed' }] };
            if (command === 'import_account_transfer') return { id: 'fixture-1' };
            return original(command, args);
        };
    });
});

test('App/CLI and IDE switch choices use distinct shared service targets', async ({ page }, info) => {
    await expect(page.getByLabel('切换目标')).toHaveValue('app');
    await page.getByTitle('切换到此账号', { exact: true }).first().click();
    await expect.poll(() => page.evaluate(() => (window as any).__transferCalls.filter((call: any) => call.command === 'switch_account').map((call: any) => call.args.targetIde))).toEqual(['app']);
    await page.getByLabel('切换目标').selectOption('ide');
    await page.getByTitle('切换到此账号', { exact: true }).nth(1).click();
    await expect.poll(() => page.evaluate(() => (window as any).__transferCalls.filter((call: any) => call.command === 'switch_account').map((call: any) => call.args.targetIde))).toEqual(['app', 'ide']);
    await expect(page.getByText(/Unimplemented synthetic command/)).toHaveCount(0);
    await page.screenshot({ path: info.outputPath('account-scopes.png') });
});

test('single and selected account exports reach the save command', async ({ page }) => {
    await page.getByRole('button', { name: '导出账号', exact: true }).first().click();
    await expect.poll(() => page.evaluate(() => (window as any).__transferCalls.filter((call: any) => call.command === 'save_account_export').map((call: any) => call.args.accountIds.length))).toEqual([1]);
    await page.locator('tbody input[type="checkbox"]').nth(0).check();
    await page.locator('tbody input[type="checkbox"]').nth(1).check();
    await page.getByRole('button', { name: '导出所选 2 个账号', exact: true }).click();
    await expect.poll(() => page.evaluate(() => (window as any).__transferCalls.filter((call: any) => call.command === 'save_account_export').map((call: any) => call.args.accountIds.length))).toEqual([1, 2]);
});

test('local import displays partial failures without closing the results', async ({ page }, info) => {
    await page.getByTitle('添加账号', { exact: true }).click();
    await page.getByText('从本机导入', { exact: true }).click();
    await page.getByRole('button', { name: '扫描并导入本机账号', exact: true }).click();
    await expect(page.getByText('已导入 1 个账号，1 个来源需要检查', { exact: true })).toBeVisible();
    await expect(page.getByText('IDE: 凭据验证失败（网络或登录已失效）', { exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: '取消', exact: true })).toBeEnabled();
    await expect(page.getByRole('button', { name: '扫描并导入本机账号', exact: true })).toBeEnabled();
    await page.screenshot({ path: info.outputPath('local-import-report.png') });
});

test('JSON file import preserves OAuth and remark metadata', async ({ page }) => {
    await page.getByTitle('添加账号', { exact: true }).click();
    await page.getByText('Refresh Token', { exact: true }).first().click();
    await page.getByLabel('选择账号导出文件').setInputFiles({ name: 'fixture.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify({ schema_version: 2, accounts: [{ email: 'fixture@example.test', refresh_token: '1//fixture', oauth_client_key: 'fixture-client', custom_label: 'fixture remark' }] })) });
    await page.getByRole('button', { name: '确认添加', exact: true }).click();
    await expect.poll(() => page.evaluate(() => (window as any).__transferCalls.find((call: any) => call.command === 'import_account_transfer')?.args.item)).toEqual({ email: 'fixture@example.test', refresh_token: '1//fixture', oauth_client_key: 'fixture-client', custom_label: 'fixture remark' });
});

test('network defaults to system and saves a manual fallback', async ({ page }, info) => {
    await page.goto('/settings');
    await expect(page.getByLabel('代理方式')).toHaveValue('system');
    await page.getByLabel('代理方式').selectOption('manual');
    await page.getByLabel('代理地址').fill('socks5h://127.0.0.1:7891');
    await page.getByRole('button', { name: '保存', exact: true }).first().click();
    await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((call: any) => call.command === 'save_config').at(-1)?.args.config.network)).toEqual({ proxy_mode: 'manual', proxy_url: 'socks5h://127.0.0.1:7891' });
    await page.screenshot({ path: info.outputPath('network-settings.png') });
});
