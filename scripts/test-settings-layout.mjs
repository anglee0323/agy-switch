// Executes real Settings and navigation render/handlers with synthetic hooks.
// This is a component-contract test, not a browser or screenshot pass.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import ts from 'typescript';
const jsx = (type, props) => ({ type, props });
function walk(node, predicate, output = []) {
    if (Array.isArray(node)) node.forEach(n => walk(n, predicate, output));
    else if (node && typeof node === 'object') { if (predicate(node)) output.push(node); walk(node.props?.children, predicate, output); }
    return output;
}
function hooks() {
    const state = [], refs = [], effects = []; let si = 0, ri = 0;
    return { reset: () => { si = ri = 0; }, state, effects, react: {
        useState(initial) { const i = si++; if (!(i in state)) state[i] = initial; return [state[i], v => { state[i] = typeof v === 'function' ? v(state[i]) : v; }]; },
        useRef(initial) { const i = ri++; return refs[i] ||= { current: initial }; },
        useEffect(fn) { effects.push(fn); },
    } };
}
const icons = new Proxy({}, { get: (_, name) => `Icon:${String(name)}` });
function compile(file, imports, extra = {}) {
    const js = ts.transpileModule(readFileSync(new URL(file, import.meta.url), 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2020 } }).outputText;
    const module = { exports: {} };
    runInNewContext(js, { exports: module.exports, require(name) { assert.ok(name in imports, `Unexpected import ${name}`); return imports[name]; }, ...extra });
    return module.exports;
}
let passed = 0;
const test = (name, fn) => { fn(); console.log(`PASS ${name}`); passed++; };
let narrow = false, listener;
const selected = [];
const navHooks = hooks();
const nav = compile('../src/components/settings/SettingsNavigation.tsx', {
    react: navHooks.react, 'react/jsx-runtime': { jsx, jsxs: jsx }, 'lucide-react': icons, 'react-i18next': { useTranslation: () => ({ t: key => key }) },
}, { window: { matchMedia: () => ({ get matches() { return narrow; }, addEventListener: (_name, fn) => { listener = fn; }, removeEventListener() {} }) } });
const ids = Array.from(nav.SETTINGS_SECTIONS, section => section.id);
function renderNav(current) { navHooks.reset(); return nav.default({ selected: current, onSelect: id => selected.push(id) }); }
const h = hooks(), saves = [], calls = [];
const settingsStore = { config: { language: 'zh', theme: 'light', auto_refresh: false, auto_sync: false }, loading: false, error: null, loadConfig: async () => { calls.push('load_config'); }, saveConfig: async c => { saves.push(c); } };
const Settings = compile('../src/pages/Settings.tsx', {
    react: h.react, 'react/jsx-runtime': { jsx, jsxs: jsx }, 'lucide-react': icons,
    'react-i18next': { useTranslation: () => ({ t: key => key }) },
    '../stores/useConfigStore': { useConfigStore: () => settingsStore },
    '../utils/request': { request: async command => { calls.push(command); return '/synthetic/data'; } },
    '../components/common/ToastContainer': { showToast() {} }, '@tauri-apps/plugin-dialog': { open: async () => null },
    '../components/settings/UpdateSettings': { default: 'UpdateSettings' },
    '../components/settings/DesktopSettings': { default: 'DesktopSettings' },
    '../components/settings/DashboardSettings': { default: 'DashboardSettings' },
    '../components/settings/ModelDisplaySettings': { default: 'ModelDisplaySettings' },
    '../components/autoSwitch/AutoSwitch': { AutoSwitchSettings: 'AutoSwitchSettings' },
    '../components/settings/SettingsNavigation': { ...nav, default: 'SettingsNavigation' },
    '../components/settings/SettingsLayout.css': {},
});
function renderSettings() { h.reset(); return Settings.default(); }
test('three categories have one selected roving tab and matching panel targets', () => {
    assert.deepEqual(ids, ['general', 'quota', 'autoSwitch']);
    const tree = renderNav('general');
    const tabs = walk(tree, n => n.props?.role === 'tab');
    assert.equal(tabs.length, 3); assert.equal(tabs.filter(t => t.props.tabIndex === 0).length, 1);
    assert.equal(tabs[0].props['aria-controls'], 'settings-panel-general');
});
test('desktop arrow/End/Home focus and select the expected categories', () => {
    const tree = renderNav('general'); const tabs = walk(tree, n => n.props?.role === 'tab');
    let focused;
    tabs.forEach((tab, index) => tab.props.ref({ focus: () => { focused = ids[index]; } }));
    for (const [index, key, expected] of [[0, 'ArrowDown', 'quota'], [0, 'ArrowUp', 'autoSwitch'], [0, 'End', 'autoSwitch'], [2, 'Home', 'general']]) {
        let prevented = false; tabs[index].props.onKeyDown({ key, preventDefault: () => { prevented = true; } });
        assert.equal(prevented, true); assert.equal(selected.at(-1), expected); assert.equal(focused, expected);
    }
});
test('narrow navigation changes orientation and uses left/right keys', () => {
    navHooks.effects[0](); narrow = true; listener();
    const tree = renderNav('general'); assert.equal(tree.props['aria-orientation'], 'horizontal');
    const tabs = walk(tree, n => n.props?.role === 'tab'); tabs[0].props.onKeyDown({ key: 'ArrowRight', preventDefault() {} });
    assert.equal(selected.at(-1), 'quota');
});
test('all category panels remain rendered while only one is visible', () => {
    const tree = renderSettings(); const panels = walk(tree, n => n.props?.role === 'tabpanel');
    assert.equal(panels.length, 3); assert.equal(panels.filter(p => !p.props.hidden).length, 1);
    assert.equal(walk(tree, n => n.type === 'AutoSwitchSettings').length, 1);
    const navigation = walk(tree, n => n.type === 'SettingsNavigation')[0]; navigation.props.onSelect('autoSwitch');
    const next = renderSettings(); const nextPanels = walk(next, n => n.props?.role === 'tabpanel');
    assert.equal(nextPanels.length, 3); assert.equal(nextPanels.find(p => !p.props.hidden).props.id, 'settings-panel-autoSwitch');
    assert.equal(walk(next, n => n.type === 'AutoSwitchSettings').length, 1);
    assert.equal(saves.length, 0); assert.equal(calls.length, 0);
});
test('data, startup and background controls remain present', () => {
    const tree = renderSettings();
    for (const type of ['DesktopSettings', 'DashboardSettings', 'ModelDisplaySettings']) assert.equal(walk(tree, n => n.type === type).length, 1);
    for (const id of ['refresh-interval', 'sync-interval']) assert.equal(walk(tree, n => n.props?.id === id).length, 1);
});
test('category scroll reset clears both scrollers without invoking config writes', () => {
    const tree = renderSettings(); const scroller = walk(tree, n => n.props?.className === 'settings-content')[0];
    tree.props.ref.current = { scrollTop: 480 }; scroller.props.ref.current = { scrollTop: 900 };
    h.effects[0](); assert.equal(tree.props.ref.current.scrollTop, 0); assert.equal(scroller.props.ref.current.scrollTop, 0);
    assert.equal(saves.length, 0);
});
test('general store errors provide a disabled-during-load retry and recover visibly', () => {
    settingsStore.error = 'synthetic load rejection'; settingsStore.loading = true;
    let alerts = walk(renderSettings(), n => n.props?.role === 'alert'); assert.equal(alerts.length, 1);
    let retry = walk(alerts[0], n => n.type === 'button')[0]; assert.equal(retry.props.disabled, true);
    settingsStore.loading = false; retry = walk(walk(renderSettings(), n => n.props?.role === 'alert')[0], n => n.type === 'button')[0];
    assert.equal(retry.props.disabled, false); retry.props.onClick(); assert.equal(calls.at(-1), 'load_config');
    settingsStore.error = null; assert.equal(walk(renderSettings(), n => n.props?.role === 'alert').length, 0);
});
const fixtureWindow = { fetch: async () => { throw new Error('No network expected'); } };
const fixture = compile('../tests/ui/settings-fixture.ts', {}, { window: fixtureWindow, localStorage: { setItem() {} }, location: { href: 'http://127.0.0.1:1421/settings', origin: 'http://127.0.0.1:1421' }, URL });
fixture.setupSettingsFixture();
const invoke = fixtureWindow.__TAURI_INTERNALS__.invoke;
for (const key of ['launch_at_login', 'hide_dock_icon', 'start_minimized']) {
    for (const value of [true, false]) {
        const result = await invoke('set_desktop_preferences', { patch: { [key]: value } });
        assert.equal(result[key], value); assert.equal('patch' in result, false);
        assert.equal((await invoke('get_desktop_settings'))[key], value);
        assert.equal((await invoke('load_config')).desktop[key], value);
        for (const other of ['launch_at_login', 'hide_dock_icon', 'start_minimized'].filter(k => k !== key)) assert.equal(result[other], false);
        assert.equal(result.platform, 'macos'); assert.equal(result.tray_available, true);
    }
}
const writes = fixtureWindow.__settingsFixture.calls.filter(c => c.command === 'set_desktop_preferences');
assert.equal(writes.length, 6);
assert.deepEqual(Object.keys(writes[0].args), ['patch']);
console.log('PASS actual synthetic desktop IPC merges args.patch and reconciles load_config'); passed++;
fixture.setupSettingsFixture({ failLoad: true, failLowQuotaLoad: true });
const failedInvoke = fixtureWindow.__TAURI_INTERNALS__.invoke;
await assert.rejects(failedInvoke('load_config'), e => e === 'synthetic load rejection');
await assert.rejects(failedInvoke('get_auto_switch_config'), e => e === 'synthetic low-quota load rejection');
fixtureWindow.__settingsFixture.failLoad = false; fixtureWindow.__settingsFixture.failLowQuotaLoad = false;
assert.equal((await failedInvoke('load_config')).language, 'zh'); assert.equal((await failedInvoke('get_auto_switch_config')).reserve_percentage, 10);
console.log('PASS general and low-quota fixture failures are independent and recoverable'); passed++;
fixtureWindow.__settingsFixture.holdAutoSave = true;
const draft = { ...(await failedInvoke('get_auto_switch_config')), reserve_percentage: 14, candidate_account_ids: ['fixture-1'] };
const deferredSave = failedInvoke('set_auto_switch_config', { config: draft });
assert.equal(fixtureWindow.__settingsFixture.lowQuota().reserve_percentage, 10);
fixtureWindow.__settingsFixture.resolveAutoSave();
assert.equal((await deferredSave).reserve_percentage, 14); assert.deepEqual(Array.from(fixtureWindow.__settingsFixture.lowQuota().candidate_account_ids), ['fixture-1']);
console.log('PASS pending synthetic save resolves to stored draft without native or network access'); passed++;
console.log(`${passed} settings component-contract tests passed; browser acceptance remains separate.`);
