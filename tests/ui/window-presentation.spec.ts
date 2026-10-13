import { expect, test } from '@playwright/test';
import { setupSettingsFixture } from './settings-fixture';

// Native visibility is simulated here; the macOS fixture separately checks the
// actual opaque frame and stale/closed request acknowledgements.
for (const language of ['zh', 'en']) {
    test(`menu requests commit the destination before revealing and idle boots stay hidden (${language})`, async ({ page }) => {
        await page.addInitScript({ content: `(${setupSettingsFixture.toString()})(${JSON.stringify({ language })});(${presentationFixture.toString()})(null);` });
        await page.goto('/');
        await expect.poll(() => page.evaluate(() => (window as any).__presentation.ready)).toBe(true);
        expect(await page.evaluate(() => (window as any).__presentation.shown)).toEqual([]);
        await page.evaluate(() => (window as any).__presentation.open(1, '/accounts'));
        await expect.poll(() => page.evaluate(() => (window as any).__presentation.shown)).toEqual([{ id: 1, route: '/accounts', content: true }]);
        await page.evaluate(() => (window as any).__presentation.open(2, '/'));
        await expect.poll(() => page.evaluate(() => (window as any).__presentation.shown.at(-1))).toEqual({ id: 2, route: '/', content: true });
        await page.evaluate(() => (window as any).__presentation.open(3, '/settings'));
        await expect.poll(() => page.evaluate(() => (window as any).__presentation.shown.at(-1))).toEqual({ id: 3, route: '/settings', content: true });
        // A delayed duplicate boot response/event may not replay old navigation.
        await page.evaluate(() => (window as any).__presentation.open(2, '/accounts'));
        await expect(page).toHaveURL(/\/settings$/);
        expect(await page.evaluate(() => (window as any).__presentation.shown)).toHaveLength(3);
    });
}

test('an open request received before listener readiness survives a cold background boot', async ({ page }) => {
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})();(${presentationFixture.toString()})({request_id: 7, route: '/accounts'});` });
    await page.goto('/');
    await expect.poll(() => page.evaluate(() => (window as any).__presentation.shown)).toEqual([{ id: 7, route: '/accounts', content: true }]);
});

test('rapid menu choices reveal only the newest committed page', async ({ page }) => {
    await page.addInitScript({ content: `(${setupSettingsFixture.toString()})();(${presentationFixture.toString()})(null);` });
    await page.goto('/');
    await expect.poll(() => page.evaluate(() => (window as any).__presentation.ready)).toBe(true);
    await page.evaluate(() => {
        (window as any).__presentation.open(10, '/accounts');
        (window as any).__presentation.open(11, '/settings');
    });
    await expect.poll(() => page.evaluate(() => (window as any).__presentation.shown)).toEqual([{ id: 11, route: '/settings', content: true }]);
});

function presentationFixture(pending: { request_id: number; route: string } | null) {
    const w = window as any;
    const original = w.__TAURI_INTERNALS__.invoke;
    let listener: Function | undefined;
    const bridge = w.__TAURI_INTERNALS__.transformCallback;
    const callbacks: Record<number, Function> = {};
    w.__TAURI_INTERNALS__.transformCallback = (fn: Function) => { const id = bridge(fn); callbacks[id] = fn; return id; };
    w.__presentation = { ready: false, shown: [] as any[], open: (id: number, route: string) => {
        pending = { request_id: id, route };
        listener?.({ payload: pending });
    } };
    w.__TAURI_INTERNALS__.invoke = async (command: string, args: any = {}) => {
        if (command === 'plugin:event|listen' && args.event === 'app://present-main') listener = callbacks[args.handler];
        if (command === 'main_window_ready') { w.__presentation.ready = true; return pending; }
        if (command === 'present_main_window') {
            if (pending?.request_id !== args.requestId) return null;
            pending = null;
            const route = location.pathname;
            const content = route === '/accounts' ? Boolean(document.querySelector('main input[type="text"]'))
                : route === '/settings' ? document.body.innerText.includes('网络代理') || document.body.innerText.includes('Network proxy')
                : Boolean(document.querySelector('[data-usage-freshness]'));
            w.__presentation.shown.push({ id: args.requestId, route, content });
            return null;
        }
        return original(command, args);
    };
}
