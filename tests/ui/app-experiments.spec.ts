import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';
test.beforeEach(async ({ page }) => {
    await page.addInitScript(setupSettingsFixture);
    await page.goto('/settings');
    await expect(page.getByRole('switch', { name: '界面汉化' })).toHaveCount(0);
    await page.getByRole('tab', { name: '实验功能', exact: true }).click();
});
test('experiments contain only App translation with the Windows and macOS scope', async ({ page }) => {
    const panel = page.getByRole('tabpanel');
    await expect(panel.getByText(/汉化仅作用于 Windows 和 macOS 的 Antigravity App/)).toBeVisible();
    await expect(panel.getByText(/不修改 App 安装文件或源文件/)).toBeVisible();
    await expect(panel.getByText(/agy-switch experiments run/)).toBeVisible();
    await expect(panel.getByRole('switch')).toHaveCount(1);
    await expect(panel.getByRole('combobox')).toHaveCount(0);
    await expect(panel.getByRole('textbox')).toHaveCount(0);
    await expect(panel.getByText(/懒人配置|推荐配置|编辑权限规则 JSON/)).toHaveCount(0);
    await panel.getByRole('switch', { name: '界面汉化' }).click();
    await expect(panel.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
    expect(await page.evaluate(() => (window as any).__settingsFixture.calls.filter((c: any) => c.command.startsWith('set_app_')))).toEqual([{ command: 'set_app_translation', args: { enabled: true } }]);
});
test('external translation changes refresh the confirmed state', async ({ page }) => {
    await page.evaluate(() => (window as any).__settingsFixture.externalExperimentEdit());
    await page.getByRole('button', { name: '重新读取', exact: true }).click();
    await expect(page.getByRole('switch', { name: '界面汉化' })).toHaveAttribute('aria-checked', 'true');
    await expect(page.getByText('已翻译 12 处界面文字')).toBeVisible();
});
test('pending translation disables competing writes and failures retain confirmed state', async ({ page }) => {
    await page.evaluate(() => { (window as any).__settingsFixture.holdExperimentSave = true; });
    const translation = page.getByRole('switch', { name: '界面汉化' });
    await translation.click();
    await expect(translation).toBeDisabled();
    await expect(page.getByRole('button', { name: '重新读取', exact: true })).toBeDisabled();
    await page.evaluate(() => (window as any).__settingsFixture.resolveExperimentSave());
    await expect(translation).toHaveAttribute('aria-checked', 'true');
    await page.evaluate(() => { const f = (window as any).__settingsFixture; f.holdExperimentSave = false; f.failExperimentSave = true; });
    await translation.click();
    await expect(page.getByRole('alert')).toContainText('synthetic App write rejection');
    await expect(translation).toHaveAttribute('aria-checked', 'true');
    await page.getByRole('button', { name: '重新读取', exact: true }).click();
    await expect(page.getByRole('alert')).toHaveCount(0);
});
test('translation can be disabled while the App is closed', async ({ page }) => {
    await page.evaluate(() => (window as any).__settingsFixture.disconnectApp());
    await page.getByRole('button', { name: '重新读取', exact: true }).click();
    const translation = page.getByRole('switch', { name: '界面汉化' });
    await expect(translation).toHaveAttribute('aria-checked','true');
    await expect(translation).toBeEnabled();
    await translation.click();
    await expect(translation).toHaveAttribute('aria-checked','false');
    await expect(translation).toBeDisabled();
    await expect.poll(() => page.evaluate(() => (window as any).__settingsFixture.experiments().translation_enabled)).toBe(false);
});
