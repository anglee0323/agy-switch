import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
const runtime = readFileSync('src-tauri/resources/app-experiments/runtime.js', 'utf8');
const dictionary = JSON.parse(readFileSync('src-tauri/resources/app-experiments/zh-CN.json', 'utf8'));
async function install(page: import('@playwright/test').Page) {
    await page.evaluate(({ runtime, dictionary }) => {
        const factory = (0, eval)(runtime);
        (window as any).controller = factory(window, { appVersion: '2.21.1', locale: 'zh-CN', dictionary });
        (window as any).controller.apply();
    }, { runtime, dictionary });
}
test('inner settings, dynamic labels, input hints and menus translate reversibly', async ({ page }) => {
    await page.goto('/');
    await page.setContent(`<div class="settings-modal-container"><h2>General</h2><section id="content"><h3>Plan Review Policy</h3><p>See 13 more skills and rules</p><input placeholder="Search customizations..." value="General" /><code>General</code><pre>Settings</pre><div data-user-content>General</div></section></div><div role="menu"><button>Request Review</button></div><div class="markdown">General</div>`);
    await install(page);
    await expect(page.locator('h3')).toHaveText('计划审批');
    await expect(page.locator('p')).toHaveText('查看另外 13 项技能和规则');
    await expect(page.locator('input')).toHaveAttribute('placeholder','搜索自定义功能…');
    await expect(page.locator('input')).toHaveValue('General');
    await expect(page.locator('code')).toHaveText('General');
    await expect(page.locator('pre')).toHaveText('Settings');
    await expect(page.locator('[data-user-content]')).toHaveText('General');
    await expect(page.locator('.markdown')).toHaveText('General');
    await page.locator('#content').evaluate(el => { el.innerHTML = '<h3>Tool Permissions</h3><button title="Open">Open</button>'; });
    await expect(page.locator('h3')).toHaveText('工具权限');
    await expect(page.locator('#content button')).toHaveAttribute('title','打开');
    await page.evaluate(() => (window as any).controller.dispose());
    await expect(page.locator('h3')).toHaveText('Tool Permissions');
    await expect(page.locator('#content button')).toHaveAttribute('title','Open');
    await expect(page.locator('h2')).toHaveText('General');
});
test('application writes retain ownership and moved user content restores original text', async ({ page }) => {
    await page.goto('/'); await page.setContent('<div class="settings-modal-container"><h3>General</h3><p id="external">Settings</p></div><div class="markdown" id="user"></div>');
    await install(page);
    await page.locator('#external').evaluate(el => { el.firstChild!.nodeValue = 'User text'; });
    await page.locator('h3').evaluate(el => document.querySelector('#user')!.append(el));
    await expect(page.locator('#user h3')).toHaveText('General');
    await page.evaluate(() => (window as any).controller.dispose());
    await expect(page.locator('#external')).toHaveText('User text');
});
test('translation lease restores labels when the manager stops renewing it', async ({ page }) => {
    await page.goto('/'); await page.clock.install();
    await page.setContent('<div class="settings-modal-container"><h2>Settings</h2></div>');
    await install(page); await expect(page.locator('h2')).toHaveText('设置');
    await page.clock.fastForward(16000); await expect(page.locator('h2')).toHaveText('Settings');
});
test('new and older sidebar layouts translate controls while project names stay intact', async ({ page }) => {
    await page.goto('/');
    await page.setContent(`<div role="navigation" aria-label="Sidebar"><a data-testid="new-conversation-button">New Conversation</a><button data-testid="automations-button">Automations</button><h3><button>Projects</button></h3><button data-project-card><span>General</span></button><a aria-label="New Conversation in Project"></a></div><div class="settings-modal-container"><h1>Settings</h1><button data-testid="settings-nav-item-General">General</button><h2>Projects</h2><div><button data-testid="settings-nav-item-CLI Project">CLI Project</button><button data-testid="settings-nav-item-General">General</button></div><h2>Not in Project</h2><button data-testid="settings-nav-item-Conversations">Conversations</button><h2><span>General</span><button aria-label="Edit project name"></button></h2><div role="combobox" class="font-mono">Ask</div><span class="font-mono">Deny</span></div>`);
    await install(page);
    await expect(page.locator('[data-testid="new-conversation-button"]')).toHaveText('新建对话');
    await expect(page.locator('[data-testid="automations-button"]')).toHaveText('自动化');
    await expect(page.locator('[data-project-card]')).toHaveText('General');
    await expect(page.locator('[data-testid="settings-nav-item-General"]').first()).toHaveText('常规');
    await expect(page.locator('[data-testid="settings-nav-item-General"]').last()).toHaveText('General');
    await expect(page.locator('[data-testid="settings-nav-item-CLI Project"]')).toHaveText('CLI Project');
    await expect(page.locator('h2 span')).toHaveText('General');
    await expect(page.locator('[role="combobox"]')).toHaveText('询问');
    await expect(page.locator('span.font-mono')).toHaveText('Deny');
    await page.evaluate(() => (window as any).controller.dispose());
    await expect(page.locator('[data-testid="new-conversation-button"]')).toHaveText('New Conversation');
});
test('composer hints and cached detached controls restore without translating messages', async ({ page }) => {
    await page.goto('/');
    await page.setContent('<p class="pointer-events-none">Ask anything, @ to mention, / for actions</p><div contenteditable="true" aria-label="Message input">General</div><div class="settings-modal-container"><h2>Settings</h2></div>');
    await install(page);
    await expect(page.locator('p')).toHaveText('输入问题，@ 引用内容，/ 选择操作');
    await expect(page.locator('[contenteditable]')).toHaveText('General');
    await expect(page.locator('[contenteditable]')).toHaveAttribute('aria-label','消息输入框');
    await page.locator('h2').evaluate(el => { (window as any).detachedControl = el; el.remove(); });
    await expect.poll(() => page.evaluate(() => (window as any).detachedControl.textContent)).toBe('Settings');
    await page.evaluate(() => { (window as any).controller.dispose(); document.body.append((window as any).detachedControl); });
    await expect(page.locator('h2')).toHaveText('Settings');
    await expect(page.locator('[contenteditable]')).toHaveAttribute('aria-label','Message input');
});
