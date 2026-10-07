// Runs a real AppKit/Tauri fixture. Its only writable data is synthetic config;
// the installed app, login registration, accounts and credential stores are untouched.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

assert.equal(process.platform, 'darwin', 'Requires native macOS');
const output = resolve('artifacts/macos-desktop');
mkdirSync(output, { recursive: true });
const root = mkdtempSync(join(tmpdir(), 'agy-desktop-fixture-'));
writeFileSync(join(root, '.desktop-fixture'), 'Synthetic desktop lifecycle data only\n');
writeFileSync(join(root, 'gui_config.json'), JSON.stringify({
  desktop: { launch_at_login: false, hide_dock_icon: true, start_minimized: true },
  auto_refresh: false, auto_sync: false, check_updates_on_startup: false,
}));
const build = spawnSync('cargo', ['build', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', '--example', 'macos-desktop-check'], { encoding: 'utf8', timeout: 180000 });
writeFileSync(join(output, 'build.log'), build.stdout + build.stderr);
assert.equal(build.status, 0, build.stderr);
const run = spawnSync(resolve('src-tauri/target/debug/examples/macos-desktop-check'), [], {
  encoding: 'utf8', timeout: 30000, env: { ...process.env, AGY_DESKTOP_FIXTURE_DATA: root },
});
writeFileSync(join(output, 'native.log'), run.stdout + run.stderr);
const stages = run.stdout.split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
writeFileSync(join(output, 'verification.json'), JSON.stringify({ fixture: root, stages, exit_code: run.status, signal: run.signal }, null, 2) + '\n');
console.log(stages);
assert.equal(run.status, 0, run.stderr);
assert.equal(stages.length, 12, 'Every native lifecycle stage must execute');
assert.ok(stages.every(stage => stage.passed));
console.log('macOS native desktop lifecycle: 12 stages passed');
