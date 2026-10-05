// Real Tauri/WebDriver acceptance. No IPC mocks, real accounts, or desktop capture.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, lstatSync, rmSync, readlinkSync, readdirSync, realpathSync } from 'node:fs';
import { join, resolve, dirname, isAbsolute } from 'node:path';
import { tmpdir, platform, release } from 'node:os';
import { spawn, spawnSync, execFileSync } from 'node:child_process';
import { inflateSync, deflateSync } from 'node:zlib';
import { setTimeout as delay } from 'node:timers/promises';

const selfTest = process.argv.includes('--self-test');
const windows = platform() === 'win32';
if (!selfTest) {
    assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Run only in a fresh GitHub-hosted CI VM');
    assert.equal(process.env.RUNNER_ENVIRONMENT, 'github-hosted', 'Self-hosted/user machines are excluded');
    assert.ok(['win32', 'linux'].includes(platform()));
}
const root = mkdtempSync(join(realpathSync(tmpdir()), 'agy-lite-native-'));
const home = join(root, 'home'), data = join(root, 'data');
const output = resolve('artifacts/native-gui');
const owned = [root], ownedFiles = [];
const originals = new Map();
let driver, session, driverExit, binary;
let driverLog = '';
let report = { passed: false, platform: platform(), os_release: release(), capture: 'Actual Tauri native WebView viewport; excludes OS window frame; synthetic example data', screenshots: [], checks: [], cleanup: {} };
let error;
class EnvironmentBlocked extends Error {}
const knownWindowsBlock = (info, wry) => info.is_elevated === true && Number(info.runtime_version.split('.')[0]) >= 150 && wry === '0.54.1';
const json = (path, value) => { mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, JSON.stringify(value, null, 2) + '\n'); };
const exists = path => { try { return lstatSync(path); } catch (e) { if (e.code === 'ENOENT') return null; throw e; } };
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function assertNoLinks(path) {
    if (windows && !selfTest) {
        const escaped = resolve(path).replaceAll("'", "''");
        execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `$p='${escaped}'; while($p) { $i=Get-Item -LiteralPath $p -Force -ErrorAction SilentlyContinue; if($i -and ($i.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Refusing Windows reparse point' }; $p=[IO.Path]::GetDirectoryName($p) }`], { stdio: 'pipe' });
    }
    for (let current = resolve(path); ; current = dirname(current)) {
        assert.ok(!exists(current)?.isSymbolicLink(), `Refusing symlink/junction: ${current}`);
        if (dirname(current) === current) break;
    }
}
function ownFresh(path) {
    assert.ok(isAbsolute(path)); assertNoLinks(path);
    assert.equal(exists(path), null, `Existing app directory: refusing to read or delete ${path}`);
    mkdirSync(path); owned.push(path);
}
function fixture(pricingHome) {
    for (const path of [home, data, join(data, 'accounts'), join(root, 'runtime'), join(home, '.config'), join(home, '.cache'), join(home, '.local/share'), join(home, 'AppData/Roaming'), join(home, 'AppData/Local')]) mkdirSync(path, { recursive: true, mode: 0o700 });
    const now = Math.floor(Date.now() / 1000);
    const parserRevision = Number(readFileSync(new URL('../src-tauri/src/modules/api_pricing.rs', import.meta.url), 'utf8').match(/const PARSER_REVISION: u32 = (\d+);/)?.[1]);
    assert.ok(Number.isInteger(parserRevision) && parserRevision > 0, 'Fixture requires the current pricing parser revision');
    const accounts = [8, 80].map((percentage, i) => ({
        id: `fixture-${i + 1}`, email: `example-${i + 1}@example.invalid`, name: `Example account ${i + 1} (synthetic)`,
        custom_label: 'Synthetic example data', created_at: now, last_used: now, disabled: false,
        token: { access_token: '', refresh_token: '', expires_in: 0, expiry_timestamp: 0, token_type: 'Bearer' },
        quota: { models: [{ name: 'gemini-test', display_name: 'Gemini example model', percentage, reset_time: '2030-01-01T00:00:00Z' }], last_updated: now,
            quota_groups: ['Gemini Models', 'Claude and GPT models'].map((display_name, family) => ({ display_name,
                buckets: ['5h', 'weekly'].map(window => ({ bucket_id: `${family ? '3p' : 'gemini'}-${window}`, window, remaining_fraction: (percentage + family * 10) / 100, remaining_fraction_known: true, reset_time: '2030-01-01T00:00:00Z' })) })) },
    }));
    json(join(data, 'accounts.json'), { version: '2.0', current_account_id: null, current_target_ide: null, accounts: accounts.map(({ token, quota, ...a }) => a) });
    for (const account of accounts) json(join(data, 'accounts', `${account.id}.json`), account);
    json(join(data, 'gui_config.json'), { language: 'en', theme: 'light', auto_refresh: false, refresh_interval: 15, auto_sync: false, sync_interval: 5, quota_protection: { enabled: false, threshold_percentage: 10, monitored_models: [] }, pinned_quota_models: { models: [] } });
    json(join(data, 'auto_switch.json'), { enabled: false, mode: 'wait', reserve_percentage: 10, candidate_min_percentage: 30, monitored_model: '', candidate_account_ids: [], target: 'app' });
    json(join(pricingHome, '.antigravity_tools/api_pricing.json'), { parser_revision: parserRevision, prices: [{ model: 'gemini-test', input: 0, output: 0, cached: 0 }], fetched_at: now, stale: false, source: 'Synthetic local acceptance data, not real pricing', warning: null });
    for (const path of ['accounts.json', 'accounts/fixture-1.json', 'accounts/fixture-2.json', 'auto_switch.json']) originals.set(path, readFileSync(join(data, path), 'utf8'));
}
function isolatedEnv(info) {
    // Do not copy process.env: tokens, cloud credentials, and runner secrets stay out of child processes.
    const env = {};
    for (const key of ['PATH', 'Path', 'SystemRoot', 'SYSTEMROOT', 'WINDIR', 'COMSPEC', 'PATHEXT', 'ProgramFiles', 'ProgramFiles(x86)', 'DISPLAY', 'XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS', 'LANG', 'LC_ALL']) if (process.env[key]) env[key] = process.env[key];
    Object.assign(env, { HOME: home, USERPROFILE: home, APPDATA: join(home, 'AppData/Roaming'), LOCALAPPDATA: join(home, 'AppData/Local'), ABV_DATA_DIR: data,
        XDG_CONFIG_HOME: join(home, '.config'), XDG_DATA_HOME: join(home, '.local/share'), XDG_CACHE_HOME: join(home, '.cache'), XDG_RUNTIME_DIR: join(root, 'runtime'),
        TMP: root, TEMP: root, TMPDIR: root, ANTIGRAVITY_DISABLE_TRAY: '1', MSEDGEDRIVER_TELEMETRY_OPTOUT: '1', WEBVIEW2_USER_DATA_FOLDER: join(root, 'webview'), RUST_LOG: 'warn' });
    if (windows) { env.WEBVIEW2_BROWSER_EXECUTABLE_FOLDER = info.runtime; env.ANTIGRAVITY_NATIVE_GUI_TEST = '1'; }
    return env;
}
// Validate actual screenshot pixels as well as DOM. A successful blank PNG is a failure.
function inspectPng(bytes, panel = false) {
    assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
    const width = bytes.readUInt32BE(16), height = bytes.readUInt32BE(20), depth = bytes[24], type = bytes[25];
    assert.equal(depth, 8); assert.ok([2, 6].includes(type), `Unsupported screenshot PNG type ${type}`); assert.equal(bytes[28], 0);
    const parts = [];
    for (let pos = 8; pos < bytes.length;) { const size = bytes.readUInt32BE(pos); if (bytes.toString('ascii', pos + 4, pos + 8) === 'IDAT') parts.push(bytes.subarray(pos + 8, pos + 8 + size)); pos += size + 12; }
    const raw = inflateSync(Buffer.concat(parts));
    const channels = type === 6 ? 4 : 3, stride = width * channels;
    assert.equal(raw.length, height * (stride + 1));
    let previous = Buffer.alloc(stride), count = 0, sum = 0, squares = 0, opaque = 0; const colors = new Set();
    for (let y = 0; y < height; y++) {
        const filter = raw[y * (stride + 1)], row = Buffer.from(raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1)));
        assert.ok(filter <= 4);
        for (let x = 0; x < stride; x++) {
            const a = x >= channels ? row[x - channels] : 0, b = previous[x], c = x >= channels ? previous[x - channels] : 0;
            const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
            const predictor = filter === 1 ? a : filter === 2 ? b : filter === 3 ? Math.floor((a + b) / 2) : filter === 4 ? (pa <= pb && pa <= pc ? a : pb <= pc ? b : c) : 0;
            row[x] = (row[x] + predictor) & 255;
        }
        for (let x = 0; x < stride; x += channels * 4) { const luminance = (row[x] + row[x + 1] + row[x + 2]) / 3; count++; if (channels === 3 || row[x + 3] >= 250) opaque++; sum += luminance; squares += luminance ** 2; colors.add(`${row[x] >> 4},${row[x + 1] >> 4},${row[x + 2] >> 4}`); }
        previous = row;
    }
    const deviation = Math.sqrt(Math.max(0, squares / count - (sum / count) ** 2));
    assert.ok(width >= (panel ? 400 : 600) && height >= 400 && colors.size >= 24 && deviation >= 8 && opaque / count > 0.99, `Blank/invalid capture: ${width}x${height}, colors=${colors.size}, deviation=${deviation}`);
    return { width, height, colors: colors.size, luminance_deviation: Number(deviation.toFixed(2)), sha256: hash(bytes) };
}
const endpoint = 'http://127.0.0.1:4444';
async function request(method, path, body) {
    const response = await fetch(endpoint + path, { method, headers: { 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(30000) });
    const result = await response.json();
    if (!response.ok || result.value?.error) throw new Error(`${method} ${path}: ${JSON.stringify(result.value)}`);
    return result.value;
}
const command = (method, path, body) => request(method, `/session/${session}${path}`, body);
const execute = (script, args = []) => command('POST', '/execute/sync', { script, args });
async function until(check, label, timeout = 30000) {
    const deadline = Date.now() + timeout; let last;
    do { try { const result = await check(); if (result) return result; } catch (e) { last = e; } await delay(200); } while (Date.now() < deadline);
    throw new Error(`Timed out: ${label}${last ? ': ' + last.message : ''}`);
}
async function click(selector) {
    const element = await until(() => execute(`return [...document.querySelectorAll(arguments[0])].find(e => e.getClientRects().length && !e.disabled)`, [selector]), `visible element: ${selector}`);
    assert.ok(element, `Visible element: ${selector}`);
    await command('POST', `/element/${element['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
}
async function ipc(commandName) {
    assert.ok(['get_data_dir_path', 'get_current_account', 'get_auto_switch_config', 'get_auto_switch_status', 'get_local_token_usage'].includes(commandName));
    const result = await command('POST', '/execute/async', { script: "const done=arguments[arguments.length-1]; window.__TAURI_INTERNALS__.invoke(arguments[0]).then(value=>done({value}),error=>done({error:String(error)}));", args: [commandName] });
    assert.equal(result.error, undefined); return result.value;
}
async function screenshot(name, panel = false) {
    const layout = await execute("return {width:innerWidth,height:innerHeight,overflow:document.documentElement.scrollWidth>innerWidth+1,route:location.pathname,theme:document.documentElement.dataset.theme,text:document.body.innerText.length}");
    assert.equal(layout.overflow, false); assert.ok(layout.text > 100);
    await command('POST', '/execute/async', { script: 'const done=arguments[arguments.length-1]; document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>done(true))));', args: [] });
    let bytes, pixels;
    try {
        await until(async () => {
            bytes = Buffer.from(await command('GET', '/screenshot'), 'base64');
            pixels = inspectPng(bytes, panel); return true;
        }, `nonblank native pixels for ${name}`, 10000);
    } catch (e) {
        // Diagnostic comes only from our app session. Never reuse a rejected image as acceptance evidence.
        if (bytes) writeFileSync(join(output, name + '.rejected.png'), bytes);
        report.rejected_capture = { name, ...layout, diagnostic: await execute("return {ready:document.readyState,visible:document.visibilityState,background:getComputedStyle(document.documentElement).backgroundColor,stylesheets:document.styleSheets.length}") };
        throw e;
    }
    writeFileSync(join(output, name + '.png'), bytes);
    report.screenshots.push({ file: name + '.png', ...layout, ...pixels });
}
function appProcesses(binary) {
    if (windows) {
        const source = "@(Get-Process -Name agy-switch-desktop -ErrorAction SilentlyContinue | ForEach-Object { @{pid=$_.Id; executable=$_.Path} }) | ConvertTo-Json -Compress";
        const result = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', source], { encoding: 'utf8' }).trim();
        return result ? [].concat(JSON.parse(result)).filter(p => p.executable?.toLowerCase() === binary.toLowerCase()) : [];
    }
    return readdirSync('/proc').filter(p => /^\d+$/.test(p)).flatMap(pid => { try { return readlinkSync(`/proc/${pid}/exe`) === binary ? [{ pid: Number(pid), executable: binary }] : []; } catch { return []; } });
}
try {
    const info = windows && !selfTest ? JSON.parse(readFileSync(process.env.GUI_WINDOWS_INFO, 'utf8').replace(/^\uFEFF/, '')) : {};
    if (windows && !selfTest) {
        assert.equal(dirname(process.env.GUI_WINDOWS_INFO), resolve(process.env.RUNNER_TEMP));
        assert.match(process.env.GUI_WINDOWS_INFO, /agy-lite-windows-driver-[a-f0-9-]+\.json$/i);
        ownedFiles.push(process.env.GUI_WINDOWS_INFO);
        if (info.downloaded_driver_directory) {
            assert.equal(dirname(info.downloaded_driver_directory), resolve(process.env.RUNNER_TEMP));
            assert.match(info.downloaded_driver_directory, /agy-lite-edge-[a-f0-9-]+$/i);
            owned.push(info.downloaded_driver_directory);
        }
    }
    if (!selfTest) {
        binary = resolve(process.argv[2] || `src-tauri/target/debug/agy-switch-desktop${windows ? '.exe' : ''}`);
        assertNoLinks(binary);
        report = { ...report, source_head: process.env.SOURCE_HEAD, checkout_commit: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), run_id: process.env.GITHUB_RUN_ID, binary_sha256: hash(readFileSync(binary)), profile: 'debug, custom-protocol; Windows also enables native-gui-test', tauri_driver_version: '2.0.6', native_driver_version: windows ? info.driver_version : execFileSync('dpkg-query', ['-W', '-f=${Version}', 'webkit2gtk-driver'], { encoding: 'utf8' }) };
        assert.match(report.source_head || '', /^[a-f0-9]{40}$/);
        assert.equal(report.checkout_commit, report.source_head, 'Test must use the exact requested source SHA');
    }
    if (windows && !selfTest) {
        report.windows_driver = info;
        const wry = readFileSync('src-tauri/Cargo.lock', 'utf8').match(/\[\[package\]\]\r?\nname = "wry"\r?\nversion = "([^"]+)"/)[1];
        report.automation = 'Debug-only native-gui-test feature passes the driver port through the WebView2 API; release builds cannot use this path';
        if (knownWindowsBlock(info, wry) && process.env.NATIVE_GUI_TEST_FEATURE !== '1') {
            throw new EnvironmentBlocked('Windows native GUI NOT TESTED: elevated hosted runner + WebView2 150+ + Wry 0.54.1 cannot establish a WebDriver session. No registry/security workaround is applied. https://github.com/tauri-apps/wry/issues/1782');
        }
        const paths = [join(info.known_home, '.gemini'), join(info.known_home, '.antigravity_tools'), join(info.known_roaming, 'com.lbjlaq.antigravity-tools-lite'), join(info.known_local, 'com.lbjlaq.antigravity-tools-lite')];
        // Check ALL paths before creating ANY. Never inspect contents of an existing profile directory.
        for (const path of paths) { assertNoLinks(path); assert.equal(exists(path), null, `Pre-existing app directory: ${path}`); }
        for (const path of paths) ownFresh(path);
        report.isolation = 'Fresh GitHub-hosted VM; Windows Known Folders app directories asserted absent and created exclusively for this test. HOME/APPDATA env overrides do not redirect Known Folders.';
        report.windows_driver = info;
    } else report.isolation = 'Fresh GitHub-hosted VM, isolated HOME/XDG/ABV_DATA_DIR, independent Xvfb and D-Bus session';
    fixture(windows && !selfTest ? info.known_home : home);
    const env = isolatedEnv(info);
    assert.ok(!Object.keys(env).some(key => /TOKEN|SECRET|PASSWORD|AWS|AZURE|GITHUB/.test(key)));
    if (selfTest) {
        assert.equal(knownWindowsBlock({ is_elevated: true, runtime_version: '153.0.4234.48' }, '0.54.1'), true);
        assert.equal(knownWindowsBlock({ is_elevated: false, runtime_version: '153.0.4234.48' }, '0.54.1'), false);
        assert.equal(knownWindowsBlock({ is_elevated: true, runtime_version: '149.0.0.0' }, '0.54.1'), false);
        assert.equal(knownWindowsBlock({ is_elevated: true, runtime_version: '153.0.4234.48' }, 'different-version'), false);
        assert.equal(JSON.parse(readFileSync(join(data, 'accounts.json'))).current_account_id, null);
        assert.equal(JSON.parse(readFileSync(join(data, 'gui_config.json'))).auto_refresh, false);
        assert.equal(JSON.parse(readFileSync(join(data, 'auto_switch.json'))).enabled, false);
        const existing = join(root, 'existing'); mkdirSync(existing); writeFileSync(join(existing, 'sentinel'), 'must remain');
        assert.throws(() => ownFresh(existing), /Existing app directory/);
        assert.equal(readFileSync(join(existing, 'sentinel'), 'utf8'), 'must remain');
        // Exercise the pixel decoder against a uniform screenshot and require rejection.
        const ihdr = Buffer.alloc(25); ihdr.writeUInt32BE(13); ihdr.write('IHDR', 4); ihdr.writeUInt32BE(800, 8); ihdr.writeUInt32BE(600, 12); ihdr[16] = 8; ihdr[17] = 2;
        const dataBytes = deflateSync(Buffer.alloc(600 * (800 * 3 + 1))); const idat = Buffer.alloc(dataBytes.length + 12); idat.writeUInt32BE(dataBytes.length); idat.write('IDAT', 4); dataBytes.copy(idat, 8);
        assert.throws(() => inspectPng(Buffer.concat([Buffer.from('89504e470d0a1a0a', 'hex'), ihdr, idat])), /Blank\/invalid capture/);
        report.passed = true; console.log('Fixture, environment allowlist, and blank-image rejection checks passed (no native GUI launched).');
    } else {
        mkdirSync(output, { recursive: true });
        assert.equal(appProcesses(binary).length, 0, 'Refusing a previously running app');
        driver = spawn(process.env.TAURI_DRIVER || (windows ? 'tauri-driver.exe' : 'tauri-driver'), ['--port', '4444', '--native-port', '4445', '--native-host', '127.0.0.1', '--native-driver', process.env.NATIVE_DRIVER || '/usr/bin/WebKitWebDriver'], { env, cwd: root, detached: !windows, stdio: ['ignore', 'pipe', 'pipe'] });
        driverExit = new Promise(resolve => { driver.once('exit', resolve); driver.once('error', resolve); });
        driver.stdout.on('data', d => { driverLog = (driverLog + d).slice(-30000); }); driver.stderr.on('data', d => { driverLog = (driverLog + d).slice(-30000); });
        await until(async () => (await request('GET', '/status')).ready, 'native driver ready');
        const created = await request('POST', '/session', { capabilities: { alwaysMatch: { 'tauri:options': { application: binary, ...(windows ? { webviewOptions: { userDataFolder: join(root, 'webview') } } : {}) } } } });
        session = created.sessionId; assert.ok(session); report.session_capabilities = created.capabilities;
        await command('POST', '/timeouts', { script: 15000, pageLoad: 30000, implicit: 0 });
        await until(() => execute("return document.querySelector('h1')?.textContent === 'Dashboard' && !!window.__TAURI_INTERNALS__"), 'real Tauri dashboard');
        report.processes = await until(() => { const p = appProcesses(binary); return p.length === 1 && p; }, 'owned native app PID');
        assert.equal(await ipc('get_data_dir_path'), data); assert.equal(await ipc('get_current_account'), null);
        assert.equal((await ipc('get_auto_switch_config')).enabled, false); assert.equal((await ipc('get_auto_switch_status')).phase, 'disabled');
        assert.equal((await ipc('get_local_token_usage')).databases_scanned, 0);
        report.checks.push('Native app PID and real IPC confirmed; no active account; switching disabled; zero native usage databases');
        await screenshot('dashboard-light');
        await click('a[href="/accounts"]');
        await until(() => execute("return document.body.innerText.includes('example-1@example.invalid') && document.body.innerText.includes('example-2@example.invalid')"), 'synthetic accounts from Rust');
        await screenshot('accounts-light');
        await click('a[href="/settings"]');
        await click('#settings-tab-autoSwitch');
        await until(() => execute("return !!document.querySelector('#auto-switch-model')"), 'native settings form');
        assert.equal(await execute("return document.querySelector('#settings-panel-autoSwitch input[type=checkbox]').checked"), false);
        assert.equal(await execute("return document.querySelectorAll('input[name=auto-switch-mode]').length"), 2);
        await screenshot('settings-light');
        await click('nav button[title="Switch to Dark Mode"]');
        await until(() => execute("return document.documentElement.dataset.theme === 'dark'"), 'dark theme applied');
        await screenshot('settings-dark');
        assert.equal(JSON.parse(readFileSync(join(data, 'gui_config.json'))).theme, 'dark');
        // WebDriver rect includes window chrome on some drivers; adjust until the actual viewport is exactly 760 CSS px.
        let rect = await command('GET', '/window/rect');
        for (let i = 0; i < 4; i++) { const width = await execute('return innerWidth'); if (width === 760) break; rect = await command('POST', '/window/rect', { width: Math.round(rect.width + 760 - width), height: 900 }); await delay(200); }
        assert.equal(await execute('return innerWidth'), 760, '760px native viewport required');
        await screenshot('settings-dark-760');
        assert.equal(await execute("return [...document.querySelectorAll('main input, main select')].filter(e=>e.offsetWidth>2).every(e=>{const r=e.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth+1})"), true);
        await click('nav button[title="Switch to Light Mode"]');
        await until(() => execute("return document.documentElement.dataset.theme === 'light'"), 'light theme restored');
        await screenshot('settings-light-760');
        // Same native WebView, now exercise the compact cross-platform dashboard route.
        await execute("history.pushState({}, '', '/menubar'); dispatchEvent(new PopStateEvent('popstate')); return true");
        await until(() => execute("return document.querySelector('.mb-eyebrow')?.textContent === 'agy-switch' && document.querySelectorAll('.mb-account-row').length === 2"), 'native quick dashboard');
        rect = await command('POST', '/window/rect', { width: 424, height: 720 });
        for (let i = 0; i < 4; i++) { const width = await execute('return innerWidth'); if (width === 424) break; rect = await command('POST', '/window/rect', { width: Math.round(rect.width + 424 - width), height: 720 }); await delay(200); }
        assert.equal(await execute('return innerWidth'), 424);
        assert.equal(await execute("return ['Today’s usage','Remaining quota','Accounts'].every(t=>document.body.innerText.includes(t)) && !/[\u3400-\u9fff]/.test(document.body.innerText)"), true);
        assert.equal(await execute("return [...document.querySelectorAll('.mb-account-switch')].every(e=>{const r=e.getBoundingClientRect(); const p=e.closest('article').querySelector('.mb-mini:last-child strong').getBoundingClientRect(); return Math.abs(r.right-p.right)<2})"), true);
        assert.deepEqual(await execute("return [...document.querySelectorAll('.mb-account-row .mb-mini strong')].map(e=>e.textContent)"), ['8%', '18%', '8%', '18%', '80%', '90%', '80%', '90%']);
        assert.equal(await execute("return [...document.querySelectorAll('.mb-account-window > span')].every(e=>e.getBoundingClientRect().height < 15)"), true, 'Quota window labels must remain on one line');
        await screenshot('quick-dashboard-light', true);
        report.checks.push('Native 424px compact dashboard: all three sections, English copy, account/action right alignment; viewport test, not tray placement');
        for (const [path, original] of originals) assert.equal(readFileSync(join(data, path), 'utf8'), original, `Unchanged ${path}`);
        const config = JSON.parse(readFileSync(join(data, 'gui_config.json'))); assert.equal(config.auto_refresh, false); assert.equal(config.auto_sync, false); assert.equal(config.quota_protection.enabled, false);
        report.checks.push('Dashboard, synthetic accounts, Settings controls, persisted light/dark theme, exact 760px viewport without horizontal overflow', 'No account/config mutation except theme; no switch/refresh/login/import/autostart action');
        await command('DELETE', ''); session = null;
        await until(() => appProcesses(binary).length === 0, 'native application exit');
        report.checks.push('WebDriver session close terminated the native application');
        report.passed = true;
        writeFileSync(join(output, 'driver.log'), driverLog);
    }
} catch (caught) {
    if (caught instanceof EnvironmentBlocked) { report.status = 'blocked'; report.capture = 'none (native GUI blocked before app launch)'; report.blocker = caught.message; console.log(`::warning::${caught.message}`); }
    else { error = caught; report.error = String(caught.stack || caught); }
}
finally {
    if (session) try { await command('DELETE', ''); } catch { /* Clean up owned process tree below. */ }
    if (driver?.pid) {
        if (windows) spawnSync('taskkill.exe', ['/PID', String(driver.pid), '/T', '/F'], { stdio: 'ignore', timeout: 10000 });
        else try { process.kill(-driver.pid, 'SIGTERM'); } catch (e) { if (e.code !== 'ESRCH') error ||= e; }
        await Promise.race([driverExit, delay(5000)]);
        if (driver.exitCode === null && !windows) try { process.kill(-driver.pid, 'SIGKILL'); } catch { /* Process already exited. */ }
        if (binary) try { await until(() => appProcesses(binary).length === 0, 'owned app cleanup', 10000); report.cleanup.processes_exited = true; } catch (e) { error ||= e; report.cleanup.error = String(e); }
    }
    report.cleanup.owned_files = [...ownedFiles];
    for (const path of ownedFiles) {
        try { assertNoLinks(path); rmSync(path); assert.equal(exists(path), null); }
        catch (e) { error ||= e; report.cleanup.error = String(e); }
    }
    report.cleanup.owned_directories = [...owned];
    for (const path of owned.reverse()) {
        try { assertNoLinks(path); rmSync(path, { recursive: true, force: true, maxRetries: 3 }); assert.equal(exists(path), null); }
        catch (e) { error ||= e; report.cleanup.error = String(e); }
    }
    report.cleanup.completed = !report.cleanup.error;
    if (error) report.passed = false;
    report.status ||= report.passed ? 'passed' : 'failed';
    if (error) report.status = 'failed';
    if (!selfTest && process.env.GITHUB_STEP_SUMMARY) writeFileSync(process.env.GITHUB_STEP_SUMMARY, `Native GUI (${platform()}): **${report.status.toUpperCase()}**\n\n${report.blocker || (report.passed ? 'Real Tauri viewport checks and cleanup passed; synthetic example data.' : 'Native acceptance failed. Inspect acceptance.json and diagnostics.')}\n`, { flag: 'a' });
    if (!selfTest) { mkdirSync(output, { recursive: true }); json(join(output, 'acceptance.json'), report); writeFileSync(join(output, 'driver.log'), driverLog); }
}
if (error) throw error;
