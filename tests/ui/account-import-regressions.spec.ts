import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

async function installFixture(page: import('@playwright/test').Page, language = 'zh', view = 'list') {
    await page.addInitScript(setupSettingsFixture, { language });
    await page.addInitScript(view => {
        localStorage.setItem('accounts_view_mode', view);
        const w = window as any;
        const original = w.__TAURI_INTERNALS__.invoke;
        w.__importRegressionCalls = [];
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
            w.__importRegressionCalls.push({ command, args });
            const accounts = (await original('list_accounts')).map((account: any, index: number) => ({
                ...account, quota: { ...account.quota, subscription_tier: ['PRO', 'ULTRA-PRO', 'FREE', null][index] },
            }));
            accounts.push({ ...accounts[3], id: 'empty-tier', email: 'empty@example.invalid', quota: { ...accounts[3].quota, subscription_tier: '' } });
            accounts.push({ ...accounts[3], id: 'blank-tier', email: 'blank@example.invalid', quota: { ...accounts[3].quota, subscription_tier: '  ' } });
            const imported = JSON.parse(sessionStorage.getItem('synthetic-oauth-account') || 'null');
            if (imported) accounts.push(imported);
            if (command === 'list_accounts') return accounts;
            if (command === 'prepare_oauth_url') return 'https://example.invalid/oauth';
            if (command === 'cancel_oauth_login') return;
            if (command === 'start_oauth_login') {
                const account = { ...accounts[0], id: 'oauth-fixture', email: 'oauth@example.invalid', quota: undefined };
                sessionStorage.setItem('synthetic-oauth-account', JSON.stringify(account));
                return account;
            }
            if (command === 'fetch_account_quota') {
                imported.quota = { ...accounts[0].quota, subscription_tier: 'PRO' };
                sessionStorage.setItem('synthetic-oauth-account', JSON.stringify(imported));
                return imported.quota;
            }
            if (command === 'import_account_transfer') {
                if (args.item.email === 'failed@example.invalid') throw 'Synthetic import rejected';
                return accounts[0];
            }
            return original(command, args);
        };
    }, view);
    await page.goto('/accounts');
}

for (const button of ['取消', '确认添加']) {
    test(`partial token import can close with ${button} without reimporting`, async ({ page }) => {
        await installFixture(page);
        await page.getByTitle('添加账号', { exact: true }).click();
        await page.getByText('Refresh Token', { exact: true }).first().click();
        await page.getByLabel('选择账号导出文件').setInputFiles({
            name: 'synthetic.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify({ accounts: [
                { email: 'success@example.invalid', refresh_token: '1//synthetic-success' },
                { email: 'failed@example.invalid', refresh_token: '1//synthetic-failure' },
            ] })),
        });
        await page.getByRole('button', { name: '确认添加', exact: true }).click();
        await expect(page.getByText('导入完成: 1 个成功, 1 个失败', { exact: true })).toBeVisible();
        const close = page.getByRole('button', { name: button, exact: true });
        await expect(close).toBeEnabled();
        await close.click();
        await expect(page.getByRole('heading', { name: '添加新账号' })).toHaveCount(0);
        expect(await page.evaluate(() => (window as any).__importRegressionCalls.filter((call: any) => call.command === 'import_account_transfer').length)).toBe(2);
        await page.getByTitle('添加账号', { exact: true }).click();
        await expect(page.getByText('导入完成: 1 个成功, 1 个失败', { exact: true })).toHaveCount(0);
    });

    test(`successful import can close with ${button} and reopen without a stale close timer`, async ({ page }) => {
        await installFixture(page);
        await page.getByTitle('添加账号', { exact: true }).click();
        await page.getByText('Refresh Token', { exact: true }).first().click();
        await page.getByLabel('选择账号导出文件').setInputFiles({
            name: 'synthetic.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify({ accounts: [
                { email: 'success@example.invalid', refresh_token: '1//synthetic-success' },
            ] })),
        });
        await page.getByRole('button', { name: '确认添加', exact: true }).click();
        await expect(page.getByText('成功导入 1 个账户', { exact: true })).toBeVisible();
        await page.getByRole('button', { name: button, exact: true }).click();
        await page.getByTitle('添加账号', { exact: true }).click();
        await page.waitForTimeout(1700);
        await expect(page.getByRole('heading', { name: '添加新账号' })).toBeVisible();
        expect(await page.evaluate(() => (window as any).__importRegressionCalls.filter((call: any) => call.command === 'import_account_transfer').length)).toBe(1);
    });
}

for (const language of ['zh', 'en']) for (const view of ['list', 'grid']) {
    test(`OAuth accounts with missing tiers remain findable after reload and quota refresh (${language}, ${view})`, async ({ page }, info) => {
        await installFixture(page, language, view);
        const labels = language === 'zh'
            ? { add: '添加账号', start: '开始 OAuth 授权', all: '全部', unknown: '待识别', refresh: '刷新此账号配额' }
            : { add: 'Add Account', start: 'Start OAuth', all: 'All', unknown: 'Unknown', refresh: "Refresh this account's quota" };
        await page.getByTitle(labels.add, { exact: true }).click();
        await page.getByRole('button', { name: labels.start, exact: true }).click();
        await expect(page.getByText('oauth@example.invalid', { exact: true })).toBeVisible();
        await expect(page.getByRole('button', { name: labels.start, exact: true })).toHaveCount(0);
        await page.reload();
        await expect(page.getByTitle(`${labels.all} (7)`, { exact: true })).toBeVisible();
        for (const tier of ['PRO', 'ULTRA', 'FREE']) await expect(page.getByTitle(`${tier} (1)`, { exact: true })).toBeVisible();
        await page.getByTitle(`${labels.unknown} (4)`, { exact: true }).click();
        await expect(page.getByRole('checkbox', { name: /oauth@example.invalid/ })).toBeVisible();
        await expect(page.getByRole('checkbox', { name: /^(Select account |选择账号 )/ })).toHaveCount(4);
        const account = view === 'list'
            ? page.getByRole('row').filter({ hasText: 'oauth@example.invalid' })
            : page.locator('.rounded-xl.border').filter({ has: page.getByRole('checkbox', { name: /oauth@example.invalid/ }) });
        await expect(account.getByText(labels.unknown, { exact: true })).toBeVisible();
        await expect(account.getByText('FREE', { exact: true })).toHaveCount(0);
        await page.screenshot({ path: info.outputPath(`unknown-tier-${language}-${view}.png`) });
        await account.getByTitle(labels.refresh, { exact: true }).click();
        await expect(page.getByRole('checkbox', { name: /oauth@example.invalid/ })).toHaveCount(0);
        await expect(page.getByTitle(`${labels.unknown} (3)`, { exact: true })).toBeVisible();
        await page.getByTitle('PRO (2)', { exact: true }).click();
        await expect(page.getByRole('checkbox', { name: /oauth@example.invalid/ })).toBeVisible();
    });
}
