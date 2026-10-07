// Compile the real launch functions with only discovery/config/logging stubbed.
// No credential code, user configuration, real app or OAuth can be reached.
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const source = readFileSync(new URL('../src-tauri/src/modules/process.rs', import.meta.url), 'utf8');
const start = source.indexOf('/// Start Antigravity using the GUI');
const end = source.indexOf('\nfn get_process_info(', start);
assert.ok(start >= 0 && end > start, 'Update the fixture when the launch-function boundary changes');
const launch = source.slice(start, end);
assert.ok(launch.includes('pub fn start_antigravity_detached'));
const root = mkdtempSync(join(tmpdir(), 'agy-cli-launch-'));
const exe = join(root, process.platform === 'win32' ? 'probe.exe' : 'probe');
const probe = `
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
mod modules {
    pub mod logger {
        pub fn log_info(_: &str) {}
        pub fn log_warn(_: &str) {}
    }
    pub mod config {
        pub struct Config { pub antigravity_executable: Option<String>, pub antigravity_ide_executable: Option<String>, pub antigravity_args: Option<Vec<String>> }
        pub fn load_app_config() -> Result<Config, ()> {
            let mode = std::env::var("PROBE_MODE").unwrap();
            let path = if mode == "auto" { None } else { Some(std::env::var("PROBE_APP").unwrap()) };
            Ok(Config { antigravity_executable: path.clone(), antigravity_ide_executable: path, antigravity_args: Some(vec!["--child".into()]) })
        }
    }
}
fn get_antigravity_executable_path(_: Option<&str>) -> Option<std::path::PathBuf> { Some(std::env::var_os("PROBE_EXE").unwrap().into()) }
#[cfg(target_os = "linux")]
fn clean_appimage_env(_: &mut Command) {}
${launch}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--child") {
        println!("FAKE-APP-STDOUT"); eprintln!("FAKE-APP-STDERR");
        std::thread::sleep(std::time::Duration::from_secs(3));
        return;
    }
    // macOS fake 'open': model a short-lived launcher and a persistent APP.
    if args.get(1).map(String::as_str) == Some("-a") {
        Command::new(std::env::var_os("PROBE_EXE").unwrap()).arg("--child").spawn().unwrap();
        return;
    }
    let target = if std::env::var("PROBE_TARGET").unwrap() == "ide" { Some("ide") } else { None };
    if std::env::var("PROBE_IO").unwrap() == "gui" { start_antigravity(target).unwrap(); }
    else { start_antigravity_detached(target).unwrap(); }
    println!(r#"{{"schema_version":1,"switched":true}}"#);
}
`;
try {
  const file = join(root, 'probe.rs'); writeFileSync(file, probe);
  const build = spawnSync('rustc', ['--edition', '2021', file, '-o', exe], { encoding: 'utf8', timeout: 60000 });
  assert.equal(build.status, 0, build.stderr);
  if (process.platform === 'darwin') symlinkSync(exe, join(root, 'open'));
  const env = { ...process.env, PROBE_EXE: exe, PROBE_APP: exe, PROBE_IO: 'cli', PATH: `${root}${process.platform === 'win32' ? ';' : ':'}${process.env.PATH}` };
  let count = 0;
  function check(mode, target, app = exe) {
    const begin = performance.now();
    const run = spawnSync(exe, [], { env: { ...env, PROBE_MODE: mode, PROBE_TARGET: target, PROBE_APP: app }, encoding: 'utf8', timeout: 8000 });
    const elapsed = performance.now() - begin;
    assert.equal(run.status, 0, run.stderr);
    assert.deepEqual(JSON.parse(run.stdout), { schema_version: 1, switched: true });
    assert.equal(run.stderr, '');
    assert.ok(elapsed < 2000, `Child kept CLI capture pipes open (${elapsed.toFixed(0)}ms)`);
    count++;
  }
  for (const mode of ['manual', 'auto']) for (const target of ['app', 'ide']) check(mode, target);
  if (process.platform === 'darwin') {
    const bundle = join(root, 'Fake.app'); mkdirSync(bundle);
    check('manual', 'app', bundle); check('manual', 'ide', bundle);
  }
  // Control case proves normal GUI launch I/O still inherits as before.
  const control = spawnSync(exe, [], { env: { ...env, PROBE_IO: 'gui', PROBE_MODE: 'manual', PROBE_TARGET: 'app' }, encoding: 'utf8', timeout: 8000 });
  assert.equal(control.status, 0, control.stderr);
  assert.ok(control.stdout.includes('FAKE-APP-STDOUT'));
  assert.ok(control.stderr.includes('FAKE-APP-STDERR'));
  console.log(`${count} CLI launch isolation checks passed; GUI inherited-I/O control passed`);
} finally {
  // The GUI control waits for the child pipes. Windows may release the final
  // pipe before releasing its executable mapping; retry that owned-file lock.
  rmSync(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}
