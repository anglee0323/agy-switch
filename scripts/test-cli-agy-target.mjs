// Checks `switch --target agy` paths that must fail before any credential write.
// A successful switch writes the system credential store, so it is never run here;
// it is verified manually on a real machine (see docs/cli.md).
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';

const binary = resolve(process.argv[2] ?? 'src-tauri/target/debug/agy-switch');
const root = mkdtempSync(join(tmpdir(), 'agy-target-'));
const home = join(root, 'home');
const data = join(root, 'data');
const env = { ...process.env, HOME: home, USERPROFILE: home, ABV_DATA_DIR: data, DISPLAY: '', WAYLAND_DISPLAY: '' };
let passed = 0;
function run(args, code) {
  const result = spawnSync(binary, args, { env, encoding: 'utf8', timeout: 20000 });
  assert.equal(result.error, undefined);
  assert.equal(result.status, code, `${args.join(' ')} -> ${result.status}: ${result.stderr}`);
  assert.ok(!`${result.stdout}${result.stderr}`.includes('FIXTURE-SECRET'), 'credentials must never be printed');
  passed++;
  return result;
}
try {
  assert.match(run(['--help'], 0).stdout, /switch \[ACCOUNT_ID\|EMAIL\] \[--target app\|ide\|agy\]/);
  assert.match(run(['--help'], 0).stdout, /without closing or restarting any client/);
  mkdirSync(join(data, 'accounts'), { recursive: true });
  const index = JSON.stringify({ version: '2.0', accounts: [{ id: 'agy-test-1' }], current_account_id: 'agy-test-1', current_target_ide: 'app' });
  const account = JSON.stringify({ id: 'agy-test-1', email: 'agy-test-1@example.invalid', name: null,
    token: { access_token: 'FIXTURE-SECRET-ACCESS', refresh_token: 'FIXTURE-SECRET-REFRESH', expires_in: 3600, expiry_timestamp: 1, token_type: 'Bearer' },
    quota: null, disabled: false, validation_blocked: false, created_at: 1, last_used: 1 });
  writeFileSync(join(data, 'accounts.json'), index);
  writeFileSync(join(data, 'accounts/agy-test-1.json'), account);
  // Unknown accounts, ambiguous use and the old pseudo-targets stop before any write.
  run(['switch', 'missing@example.invalid', '--target', 'agy', '--json'], 3);
  run(['switch', 'agy-test-1', '--target', 'cli', '--json'], 2);
  run(['current', '--target', 'agy', '--json'], 2);
  run(['switch', '--target', 'agy', '--json'], 2);
  assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), index);
  assert.equal(readFileSync(join(data, 'accounts/agy-test-1.json'), 'utf8'), account);
  assert.equal(existsSync(join(home, '.gemini')), false);
  console.log(`agy target pre-write checks passed (${passed}); a successful switch is verified manually`);
} finally {
  rmSync(root, { recursive: true, force: true });
}
