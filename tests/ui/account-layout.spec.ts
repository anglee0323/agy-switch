import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

for (const language of ['zh', 'en']) for (const view of ['list', 'grid']) {
    test(`account selection and trailing export stay usable in ${view} (${language})`, async ({ page }, info) => {
        const labels = language === 'zh'
            ? { select: '选择账号 primary@example.invalid', selectPage: '选择本页账号', export: '导出账号', exportSelected: '导出所选 1 个账号', refresh: '刷新 (1)', switch: '切换到此账号' }
            : { select: 'Select account primary@example.invalid', selectPage: 'Select accounts on this page', export: 'Export account', exportSelected: 'Export 1 selected accounts', refresh: 'Refresh (1)', switch: 'Switch to this account' };
        await page.addInitScript(setupSettingsFixture, { language });
        await page.addInitScript(mode => localStorage.setItem('accounts_view_mode', mode), view);
        await page.goto('/accounts');
        await expect(page.getByRole('combobox', { name: /切换目标|Switch target/ })).toHaveCount(0);
        const selection = page.getByRole('checkbox', { name: labels.select, exact: true });
        await expect(selection).toBeVisible();
        await selection.focus();
        await page.keyboard.press('Space');
        await expect(selection).toBeChecked();
        await expect(selection).toBeFocused();
        if (view === 'list') {
            const selectPage = page.getByRole('checkbox', { name: labels.selectPage, exact: true });
            await expect(selectPage).toHaveJSProperty('indeterminate', true);
            await selectPage.check();
            await expect(page.locator('tbody').getByRole('checkbox', { checked: true })).toHaveCount(4);
            await selectPage.uncheck();
            await expect(page.locator('tbody').getByRole('checkbox', { checked: true })).toHaveCount(0);
            await selection.check();
        }
        const rowExport = page.getByRole('button', { name: labels.export, exact: true }).first();
        expect(await rowExport.evaluate(button => button.parentElement?.lastElementChild === button)).toBe(true);
        const batchExport = page.getByRole('button', { name: labels.exportSelected, exact: true });
        expect(await batchExport.evaluate(button => button.parentElement?.lastElementChild === button)).toBe(true);
        await expect(page.getByTitle(labels.refresh, { exact: true })).toBeVisible();
        await page.screenshot({ path: info.outputPath(`accounts-${view}-${language}-light.png`) });
        await page.setViewportSize({ width: 1024, height: 700 });
        await expect(selection).toBeVisible();
        await expect(batchExport).toBeVisible();
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
        await page.getByTitle(language === 'zh' ? '切换到深色模式' : 'Switch to Dark Mode', { exact: true }).click();
        await expect(page.locator('html')).toHaveClass(/dark/);
        await page.evaluate(async () => {
            await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
            await Promise.all(document.getAnimations().map(animation => animation.finished.catch(() => {})));
        });
        await page.screenshot({ path: info.outputPath(`accounts-${view}-${language}-dark-1024.png`) });
        await page.getByTitle(labels.switch, { exact: true }).first().click();
        await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.calls.filter((call: any) => call.command === 'switch_account').map((call: any) => call.args.targetIde))).toEqual(['app']);
    });
}
