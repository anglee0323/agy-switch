// Actual PTY/ConPTY account viewport regression. Synthetic files; never submit a switch/refresh.
import assert from 'node:assert/strict';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync, chmodSync, lstatSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve, join} from 'node:path';
import {createRequire} from 'node:module';
import {setTimeout as delay} from 'node:timers/promises';
import pty from 'node-pty';
import xterm from '@xterm/headless';

if (process.platform !== 'win32') {
  const packageRoot = resolve(createRequire(import.meta.url).resolve('node-pty/package.json'), '..');
  const helper = join(packageRoot, 'prebuilds', `${process.platform}-${process.arch}`, 'spawn-helper');
  if (existsSync(helper)) chmodSync(helper, lstatSync(helper).mode | 0o100);
}
const binary = resolve(process.argv[2] || 'src-tauri/target/debug/agy-switch');
const root = mkdtempSync(join(tmpdir(), 'agy-account-viewport-'));
const data = join(root, 'data');
mkdirSync(join(data, 'accounts'), {recursive: true});
const config = JSON.stringify({language: 'en'});
writeFileSync(join(data, 'gui_config.json'), config);
const accounts = Array.from({length: 40}, (_, i) => ({id: `fixture-${i}`, email: `u${i}@example.invalid`, token: {refresh_token: 'SYNTHETIC-VIEWPORT-SECRET'}, quota: null}));
for (const account of accounts) writeFileSync(join(data, 'accounts', `${account.id}.json`), JSON.stringify(account));
const index = JSON.stringify({accounts: accounts.map(({id}) => ({id})), current_account_id: accounts[0].id});
writeFileSync(join(data, 'accounts.json'), index);
const env = {};
for (const key of ['PATH', 'Path', 'SystemRoot', 'WINDIR', 'COMSPEC', 'PATHEXT', 'TMP', 'TEMP', 'LANG']) if (process.env[key]) env[key] = process.env[key];
Object.assign(env, {HOME: root, ABV_DATA_DIR: data, TERM: 'xterm-256color'});
let terminal, output = '', exited, checks = 0;
const screen = new xterm.Terminal({cols: 110, rows: 24, allowProposedApi: true, scrollback: 1000});
const frame = () => Array.from({length: screen.rows}, (_, i) => screen.buffer.active.getLine(screen.buffer.active.viewportY + i)?.translateToString(true) || '').join('\n');
async function until(predicate) {
  for (let i = 0; i < 150; i++) { if (predicate()) return; await delay(100); }
  throw new Error('Account viewport did not reach its expected synthetic state');
}
async function send(keys, text) { output = ''; terminal.write(keys); await until(() => output.includes(text)); }
async function viewport(keys, selected, rows, width, prompt) {
  const scrollback = screen.buffer.active.baseY;
  output = ''; terminal.write(keys);
  await until(() => frame().includes(`u${selected}@`) && frame().includes('/ 40') && frame().includes(prompt));
  const rendered = frame();
  assert.equal(screen.buffer.active.baseY, scrollback, 'account repaint must not scroll the terminal');
  assert.ok(rendered.split(/\r?\n/).every(line => line.length < width), 'frame must not wrap');
  assert.ok(!rendered.includes(`u${selected === 39 ? 0 : 39}@`), 'offscreen accounts must not be rendered');
  checks++;
}
try {
  terminal = pty.spawn(binary, [], {cols: 110, rows: 24, cwd: root, env, useConptyDll: process.platform === 'win32'});
  const done = new Promise(resolve => terminal.onExit(event => { exited = event; resolve(); }));
  terminal.onData(chunk => { output += chunk; screen.write(chunk); });
  await until(() => output.includes('Select a section:'));
  await viewport('1', 0, 24, 110, 'Action:');
  await viewport('\x1b[F', 39, 24, 110, 'Action:');
  await viewport('\x1b[H', 0, 24, 110, 'Action:');
  terminal.resize(48, 8);
  screen.resize(48, 8);
  await viewport('\x1b[F', 39, 8, 48, 'Action:');
  await viewport('\x1b[A', 38, 8, 48, 'Action:');
  await send('\x1b', 'Select a section:');
  await send('3', 'Select Refresh Mode:');
  await viewport('3', 0, 8, 48, '? Select account');
  await viewport('\x1b[F', 39, 8, 48, '? Select account');
  terminal.resize(110, 24);
  screen.resize(110, 24);
  await viewport('\x1b[H', 0, 24, 110, '? Select account');
  await send('\x1b', 'Select Refresh Mode:');
  await send('\x1b', 'Select a section:');
  terminal.write('\x03');
  await Promise.race([done, delay(10000).then(() => { throw new Error('Account viewport did not exit'); })]);
  assert.equal(exited.exitCode, 0);
  assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), index);
  assert.equal(readFileSync(join(data, 'gui_config.json'), 'utf8'), config);
  for (const account of accounts) assert.equal(readFileSync(join(data, 'accounts', `${account.id}.json`), 'utf8'), JSON.stringify(account));
  assert.ok(!output.includes('SYNTHETIC-VIEWPORT-SECRET'));
  console.log(`${checks} account viewport PTY checks passed: 40 accounts, both pickers, Home/End/arrows, resize, cancel and preserved data`);
} finally {
  if (terminal && !exited) terminal.kill();
  screen.dispose();
  await delay(200);
  rmSync(root, {recursive: true, force: true});
}
