import { expect, test, type Page } from '@playwright/test';
import { settleKeyboardDrag, setupSettingsFixture } from './settings-fixture';

// Synthetic accounts and an in-memory IPC double: no native bridge, no real data.
// Odd-numbered accounts are named "odd#", even-numbered ones "even#", so a search can narrow the list.
async function setup(page: Page, { failReorder = false } = {}) {
    await page.addInitScript(setupSettingsFixture, { language: 'en' });
    await page.addInitScript(fail => {
        const w = window as any, original = w.__TAURI_INTERNALS__.invoke;
        const make = (i: number) => ({ id: `fixture-${i}`, email: `${i % 2 ? 'odd' : 'even'}${i}@example.invalid`, created_at: 1, last_used: 1,
            token: { access_token: '', refresh_token: '', expires_in: 0, expiry_timestamp: 0, token_type: 'Bearer' },
            quota: { models: [{ name: 'm', display_name: 'M', percentage: 80, reset_time: '2027-01-01T00:00:00Z' }], last_updated: 1800000000 } });
        let order = [0, 1, 2, 3, 4, 5];
        w.__reorderCalls = [];
        w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
            if (command === 'list_accounts') return order.map(make);
            if (command === 'reorder_accounts') {
                w.__reorderCalls.push(args.accountIds);
                if (fail) throw 'synthetic reorder rejection';
                order = args.accountIds.map((id: string) => Number(id.split('-')[1]));
                return null;
            }
            return original(command, args);
        };
        localStorage.setItem('accounts_page_size', '3');
        localStorage.setItem('accounts_view_mode', 'list');
    }, failReorder);
    await page.goto('/accounts');
    await expect(page.locator('tbody tr')).toHaveCount(3);
}

const rows = (page: Page) => page.locator('tbody tr').evaluateAll(items => items.map(item => /(odd|even)\d/.exec(item.textContent || '')?.[0]));
const sent = (page: Page) => page.evaluate(() => (window as any).__reorderCalls);
const ids = (...numbers: number[]) => numbers.map(number => `fixture-${number}`);

async function dragFirstRowDown(page: Page, over: string) {
    const handle = page.locator('tbody tr').first().getByRole('button', { name: 'Drag to reorder' });
    await handle.focus();
    await page.keyboard.press('Space');
    await settleKeyboardDrag(page);
    await page.keyboard.press('ArrowDown');
    await expect(page.getByRole('status').filter({ hasText: new RegExp(`moved over droppable area ${over}`) })).toBeVisible();
    await page.keyboard.press('Space');
}

test('reordering on page 2 keeps page 1 and saves the complete order', async ({ page }) => {
    await setup(page);
    await page.getByRole('button', { name: /^2$/ }).click();
    await expect.poll(() => rows(page)).toEqual(['odd3', 'even4', 'odd5']);
    await dragFirstRowDown(page, 'fixture-4');
    await expect.poll(() => sent(page)).toEqual([ids(0, 1, 2, 4, 3, 5)]);
    await expect.poll(() => rows(page)).toEqual(['even4', 'odd3', 'odd5']);
    await page.getByRole('button', { name: /^1$/ }).click();
    await expect.poll(() => rows(page)).toEqual(['even0', 'odd1', 'even2']);
});

test('reordering a search result keeps hidden accounts in their positions', async ({ page }) => {
    await setup(page);
    await page.getByPlaceholder('Search email...').fill('odd');
    await expect.poll(() => rows(page)).toEqual(['odd1', 'odd3', 'odd5']);
    await dragFirstRowDown(page, 'fixture-3');
    await expect.poll(() => sent(page)).toEqual([ids(0, 3, 2, 1, 4, 5)]);
    await page.getByPlaceholder('Search email...').fill('');
    await expect.poll(() => rows(page)).toEqual(['even0', 'odd3', 'even2']);
});

test('a rejected save restores the previous complete order', async ({ page }) => {
    await setup(page, { failReorder: true });
    await page.getByRole('button', { name: /^2$/ }).click();
    await dragFirstRowDown(page, 'fixture-4');
    await expect.poll(() => sent(page).then((calls: string[][]) => calls.length)).toBe(1);
    await expect.poll(() => rows(page)).toEqual(['odd3', 'even4', 'odd5']);
    await page.getByRole('button', { name: /^1$/ }).click();
    await expect.poll(() => rows(page)).toEqual(['even0', 'odd1', 'even2']);
});
