// Actual Windows/Linux tray and secondary-window runtime, in fresh hosted VMs.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, chmodSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

const selfTest = process.argv.includes('--self-test');
if (!selfTest) {
  assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Run only in a fresh GitHub-hosted VM');
  assert.equal(process.env.RUNNER_ENVIRONMENT, 'github-hosted', 'Never run on a user machine');
  assert.ok(['win32', 'linux'].includes(process.platform));
}
function isolatedEnv(root, data) {
  const env = {};
  for (const key of ['PATH', 'Path', 'SystemRoot', 'SYSTEMROOT', 'WINDIR', 'COMSPEC', 'PATHEXT', 'DISPLAY', 'XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS', 'LANG', 'LC_ALL']) {
    if (process.env[key]) env[key] = process.env[key];
  }
  return { ...env, HOME: join(root, 'home'), USERPROFILE: join(root, 'home'),
    APPDATA: join(root, 'home/AppData/Roaming'), LOCALAPPDATA: join(root, 'home/AppData/Local'),
    XDG_CONFIG_HOME: join(root, 'home/.config'), XDG_CACHE_HOME: join(root, 'home/.cache'),
    XDG_DATA_HOME: join(root, 'home/.local/share'), XDG_RUNTIME_DIR: join(root, 'runtime'),
    ABV_DATA_DIR: data, GITHUB_ACTIONS: 'true', RUNNER_ENVIRONMENT: 'github-hosted',
    WEBKIT_DISABLE_DMABUF_RENDERER: '1', GDK_BACKEND: 'x11' };
}
function verifyStages(stages, noTray) {
  const names = ['startup', ...(noTray ? [] : ['prewarm_hidden', 'panel_open_and_bounds', 'second_click_hides', 'panel_reused',
    'panel_close_hides', 'blur_hides_panel', 'main_close_keeps_runtime', 'main_reopen_hides_panel']),
    'tray_unavailable_recovery', 'background_start_rejected_without_tray', 'close_without_tray'];
  assert.deepEqual(stages.map(stage => stage.stage), names, 'Every lifecycle stage must run exactly once');
  assert.ok(stages.every(stage => stage.passed === true), 'A native lifecycle assertion failed');
}
if (selfTest) {
  assert.equal(isolatedEnv('/fixture', '/fixture/data').ABV_DATA_DIR, '/fixture/data');
  assert.ok(!('GITHUB_TOKEN' in isolatedEnv('/fixture', '/fixture/data')));
  assert.throws(() => verifyStages([{ stage: 'startup', passed: true }], false));
  assert.throws(() => verifyStages(['startup', 'tray_unavailable_recovery', 'background_start_rejected_without_tray', 'close_without_tray']
    .map(stage => ({ stage, passed: stage !== 'close_without_tray' })), true));
  console.log('Native lifecycle harness: environment isolation and incomplete/failed report rejection passed');
} else {
  const output = resolve('artifacts/desktop-lifecycle'); mkdirSync(output, { recursive: true });
  const source = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  const build = spawnSync('cargo', ['build', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', '--features', 'native-gui-test', '--example', 'desktop-lifecycle-check'], { encoding: 'utf8', timeout: 180000 });
  writeFileSync(join(output, 'build.log'), build.stdout + build.stderr);
  assert.equal(build.status, 0, build.stderr);
  const binary = resolve('src-tauri/target/debug/examples/desktop-lifecycle-check' + (process.platform === 'win32' ? '.exe' : ''));
  const report = { source_head: process.env.SOURCE_HEAD, checkout_commit: source, platform: process.platform,
    executable_sha256: createHash('sha256').update(readFileSync(binary)).digest('hex'), passed: false, runs: [],
    limits: ['Own tray constructed through production code; physical icon/menu clicks and taskbar presence are not asserted',
      'Single hosted display; additional monitors, Wayland compositors and authenticated account operations are separate acceptance'] };
  writeFileSync(join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
  assert.equal(report.source_head, source, 'Must test the exact checked-out source');
  for (const noTray of [false, true]) {
    const root = mkdtempSync(join(tmpdir(), 'agy-lifecycle-')); const data = join(root, 'data');
    for (const path of [data, join(root, 'home/.config'), join(root, 'home/.cache'), join(root, 'home/.local/share'),
      join(root, 'home/AppData/Roaming'), join(root, 'home/AppData/Local'), join(root, 'runtime')]) mkdirSync(path, { recursive: true });
    if (process.platform !== 'win32') chmodSync(join(root, 'runtime'), 0o700);
    writeFileSync(join(data, '.desktop-fixture'), 'Synthetic lifecycle fixture only\n');
    const config = JSON.stringify({ language: 'en', desktop: { launch_at_login: false, hide_dock_icon: false, start_minimized: true },
      auto_refresh: false, auto_sync: false, check_updates_on_startup: false });
    writeFileSync(join(data, 'gui_config.json'), config);
    writeFileSync(join(data, 'accounts.json'), JSON.stringify({ version: '2.0', accounts: [], current_account_id: null }));
    const run = spawnSync(binary, noTray ? ['--no-tray'] : [], { encoding: 'utf8', timeout: 30000, env: isolatedEnv(root, data) });
    writeFileSync(join(output, noTray ? 'without-tray.log' : 'with-tray.log'), run.stdout + run.stderr);
    const stages = run.stdout.split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
    report.runs.push({ no_tray: noTray, stages, exit_code: run.status, signal: run.signal,
      config_unchanged: readFileSync(join(data, 'gui_config.json'), 'utf8') === config });
    writeFileSync(join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
    assert.equal(run.status, 0, run.stderr);
    verifyStages(stages, noTray);
    assert.ok(report.runs.at(-1).config_unchanged, 'Lifecycle checks must not write preferences');
    rmSync(root, { recursive: true }); // Only the freshly created, owned fixture.
  }
  report.passed = true; writeFileSync(join(output, 'acceptance.json'), JSON.stringify(report, null, 2));
  console.log(`${process.platform} native lifecycle: 16 stages passed, with and without tray`);
}
