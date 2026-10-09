// Actual PTY / Windows ConPTY acceptance. Synthetic data only; no submit/auth/switch.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync, chmodSync, lstatSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import pty from 'node-pty';
import { createRequire } from 'node:module';
// node-pty 1.1.0’s npm prebuild omits the Unix helper executable bit.
// Repair only this pinned test dependency; never touch app trust or OS policy.
if (process.platform !== 'win32') {
  const packageRoot = resolve(createRequire(import.meta.url).resolve('node-pty/package.json'), '..');
  const helper = join(packageRoot, 'prebuilds', process.platform + '-' + process.arch, 'spawn-helper');
  if (existsSync(helper) && lstatSync(helper).isFile()) chmodSync(helper, lstatSync(helper).mode | 0o100);
}
const binary = resolve(process.argv[2] || 'src-tauri/target/debug/agy-switch');
const root = mkdtempSync(join(tmpdir(), 'agy-terminal-'));
const data = join(root, 'data'); mkdirSync(data);
const config = JSON.stringify({ language: 'en', theme: 'light' });
writeFileSync(join(data, 'gui_config.json'), config);
const env = {};
for (const key of ['PATH', 'Path', 'SystemRoot', 'WINDIR', 'COMSPEC', 'PATHEXT', 'TMP', 'TEMP', 'LANG']) if (process.env[key]) env[key] = process.env[key];
Object.assign(env, { HOME: root, ABV_DATA_DIR: data, TERM: 'xterm-256color' });
const watchdog = setTimeout(() => { console.error('Terminal harness exceeded 90 seconds'); process.exit(1); }, 90000);
watchdog.unref();
let terminal, output = '', allOutput = '', exited;
let checks = 0;
async function until(text) {
  for (let i = 0; i < 100; i++) { if (output.includes(text)) { checks++; console.log('Terminal reached: ' + text); return; } await delay(100); }
  throw new Error(`Terminal did not reach ${text}; captured ${output.length} bytes`);
}
async function send(keys, text) { output = ''; terminal.write(keys); await until(text); }
try {
  terminal = pty.spawn(binary, [], { cols: 110, rows: 32, cwd: root, env, useConptyDll: process.platform === 'win32' });
  const done = new Promise(resolve => terminal.onExit(event => { exited = event; resolve(event); }));
  terminal.onData(chunk => { output += chunk; allOutput += chunk; });
  await until('Select a section:');
  // Down enters Statistics, Left returns. Right acts like Enter on menus.
  await send('\x1b[B\x1b[C', 'Local Token Usage & Estimated Cost');
  await send('\x1b[D', 'Select a section:');
  await send('4', 'Manually enter Refresh Token');
  await send('\x1b[B\r', 'Enter Google Refresh Token:');
  output = ''; terminal.write('SYNTHETIC-NOT-A-CREDENTIAL');
  await until('********');
  assert.ok(!allOutput.includes('SYNTHETIC-NOT-A-CREDENTIAL'), 'Secret prompt must never echo its input'); checks++;
  await send('\x1b', 'Manually enter Refresh Token');
  await send('\x1b', 'Select a section:');
  // Exercise real setting/order writes through the terminal with synthetic accounts.
  mkdirSync(join(data, 'accounts'));
  const accountFiles = {};
  for (const id of ['A', 'B', 'C']) {
    const content = JSON.stringify({ id, email: `${id.toLowerCase()}@example.invalid`, token: { refresh_token: 'SYNTHETIC-SECRET' }, quota: null });
    accountFiles[id] = content; writeFileSync(join(data, 'accounts', `${id}.json`), content);
  }
  writeFileSync(join(data, 'accounts.json'), JSON.stringify({ version: 'fixture', accounts: ['A', 'B', 'C'].map(id => ({ id })), current_account_id: 'A', extra: 'keep' }));
  await send('6', 'Settings & Order');
  await send('\r', 'Smart switching: Off');
  await send('7', 'Space selects a candidate');
  await send(' \x1b[B \x1b[1;2A\r', 'Candidates & Order: 2');
  await send('3', 'Priority: start at the top');
  await send('\x1b[B\r', 'Account selection order: Round robin');
  await send('2', 'may interrupt running work');
  await send('2', 'Switch timing: Switch at threshold');
  await send('1', 'Smart switching: On');
  await send('6', 'Reserve (1');
  await send('25\x1b[H\x1b[3~1\r', 'Backup minimum');
  await send('40\r', 'Reserve / Backup minimum: 15% / 40%');
  await send('8', 'Settings saved.');
  const savedPolicy = JSON.parse(readFileSync(join(data, 'auto_switch.json'), 'utf8'));
  assert.deepEqual(savedPolicy.candidate_account_ids, ['B', 'A']);
  assert.equal(savedPolicy.strategy, 'round_robin'); assert.equal(savedPolicy.mode, 'stop'); assert.equal(savedPolicy.enabled, true); checks++;
  assert.equal(savedPolicy.reserve_percentage, 15); assert.equal(savedPolicy.candidate_min_percentage, 40); checks++;
  await send('\x1b', 'Settings & Order');
  await send('2', 'U/D reorder');
  await send('\x1b[B\x1b[1;2A\r', 'Settings saved.');
  const savedIndex = readFileSync(join(data, 'accounts.json'), 'utf8');
  assert.deepEqual(JSON.parse(savedIndex).accounts.map(a => a.id), ['B', 'A', 'C']);
  assert.equal(JSON.parse(savedIndex).current_account_id, 'A'); assert.equal(JSON.parse(savedIndex).extra, 'keep'); checks++;
  await send('\x1b', 'Settings & Order');
  await send('2', 'U/D reorder');
  await send('\x1b[B\x1b[1;2A\x1b', 'Settings & Order');
  assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), savedIndex); checks++;
  await send('\x1b', 'Select a section:');
  // Experimental Features is a primary section. Its secondary switch shares
  // the GUI's file, while a foreground runner returns here after Ctrl+C.
  await send('8', 'Chinese interface: Off');
  assert.ok(output.includes('Experimental Features'));
  assert.ok(output.includes('does not modify App installation/source files')); checks++;
  await send('2', 'Turn on Chinese interface first.');
  await send('\x1b', 'Chinese interface: Off');
  await send('1', 'Settings saved.');
  assert.equal(JSON.parse(readFileSync(join(data, 'app_experiments.json'), 'utf8')).translation_enabled, true); checks++;
  await send('\x1b', 'Chinese interface: On');
  if (process.platform === 'darwin' || process.platform === 'win32') {
    await send('2', 'Maintaining translation; waiting for Antigravity App.');
    await send('\x03', 'Foreground renewals stopped.');
    await send('\x1b', 'Chinese interface: On');
  }
  await send('\x1b', 'Select a section:');
  await send('1', 'Accounts & Quotas Hub');
  await send('\x1b[C', 'Account actions');
  await send('\x1b[D', 'Accounts & Quotas Hub');
  await send('\x1b', 'Select a section:');
  for (const id of ['A', 'B', 'C']) assert.equal(readFileSync(join(data, 'accounts', `${id}.json`), 'utf8'), accountFiles[id]);
  assert.ok(!allOutput.includes('SYNTHETIC-SECRET')); checks++;
  terminal.resize(72, 24);
  // Confirm input is still processed after the asynchronous resize event.
  await send('\x1b[B', 'Select a section:');
  output = ''; terminal.write('\x03');
  await Promise.race([done, delay(10000).then(() => { throw new Error('Ctrl+C did not exit TUI'); })]);
  assert.equal(exited.exitCode, 0); checks++;
  assert.doesNotMatch(allOutput, /[\u3400-\u9fff]/); checks++;
  assert.equal(readFileSync(join(data, 'gui_config.json'), 'utf8'), config); checks++;
  const policyBytes = readFileSync(join(data, 'auto_switch.json'), 'utf8');
  writeFileSync(join(data, 'gui_config.json'), JSON.stringify({ language: 'zh' }));
  output = ''; exited = undefined;
  let chineseOutput = '';
  terminal = pty.spawn(binary, [], { cols: 100, rows: 32, cwd: root, env, useConptyDll: process.platform === 'win32' });
  const chineseDone = new Promise(resolve => terminal.onExit(event => { exited = event; resolve(event); }));
  terminal.onData(chunk => { output += chunk; chineseOutput += chunk; });
  await until('选择功能:');
  await send('8', '界面汉化: 开启');
  assert.ok(output.includes('实验功能')); assert.ok(output.includes('不修改 App 安装文件或源文件')); checks++;
  await send('1', '设置已保存。');
  assert.equal(JSON.parse(readFileSync(join(data, 'app_experiments.json'), 'utf8')).translation_enabled, false); checks++;
  await send('\x1b', '界面汉化: 关闭');
  await send('\x1b', '选择功能:');
  await send('6', '策略与排序');
  await send('\r', '切换时机:');
  assert.ok(output.includes('账号选择顺序:')); checks++;
  await send('3', '优先顺序：');
  await send('\x1b', '候选账号与排序:');
  await send('\x1b', '策略与排序');
  await send('\x1b', '选择功能:');
  terminal.write('\x03');
  await Promise.race([chineseDone, delay(10000).then(() => { throw new Error('Chinese TUI did not exit'); })]);
  assert.equal(exited.exitCode, 0);
  assert.equal(readFileSync(join(data, 'auto_switch.json'), 'utf8'), policyBytes);
  assert.doesNotMatch(chineseOutput, /Account selection order|Settings saved|Smart switching|Chinese interface|Experimental Features/); checks++;
  console.log(`${checks} real ${process.platform === 'win32' ? 'ConPTY' : 'PTY'} checks passed: menus, experiments, foreground cancellation, policy editing, selection/order, cancel, masking, resize and exit; synthetic settings writes only, no credential changes`);
} finally {
  clearTimeout(watchdog);
  if (terminal && !exited) terminal.kill();
  await delay(200);
  rmSync(root, { recursive: true, force: true });
}
// node-pty's Windows collector worker can outlive its already exited child.
// All exit and fixture-cleanup assertions above must pass before ending this test process.
process.exit(0);
