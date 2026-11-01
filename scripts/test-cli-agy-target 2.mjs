// Isolated check of `switch --target agy` and `current --target agy` with synthetic accounts.
// HOME and ABV_DATA_DIR are temporary, so no real account, App, session or process is involved.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';

const binary = resolve(process.argv[2] ?? 'src-tauri/target/debug/agy-switch');
const root = mkdtempSync(join(tmpdir(), 'agy-target-'));
const home = join(root, 'home');
const data = join(root, 'data');
const session = join(home, '.gemini', 'antigravity-cli', 'antigravity-oauth-token');
const env = { ...process.env, HOME: home, USERPROFILE: home, ABV_DATA_DIR: data, DISPLAY: '', WAYLAND_DISPLAY: '' };
const macos = process.platform === 'darwin';
let passed = 0;
function run(args, code = 0) {
  const result = spawnSync(binary, args, { env, encoding: 'utf8', timeout: 20000 });
  assert.equal(result.error, undefined);
  assert.equal(result.status, code, `${args.join(' ')} -> ${result.status}: ${result.stderr}`);
  assert.ok(!`${result.stdout}${result.stderr}`.includes('FIXTURE-SECRET'), 'credentials must never be printed');
  passed++;
  return result;
}
const farFuture = Math.floor(Date.now() / 1000) + 365 * 24 * 3600;
const account = (n) => ({
  id: `agy-test-${n}`, email: `agy-test-${n}@example.invalid`, name: null,
  token: { access_token: `FIXTURE-SECRET-ACCESS-${n}`, refresh_token: `FIXTURE-SECRET-REFRESH-${n}`, expires_in: 3600,
    expiry_timestamp: farFuture, token_type: 'Bearer', email: `agy-test-${n}@example.invalid`, project_id: 'fixture-project', is_gcp_tos: false },
  quota: null, disabled: false, validation_blocked: false, created_at: 1, last_used: 1,
});
const sessionPayload = (n) => JSON.stringify({ token: { access_token: `FIXTURE-SECRET-ACCESS-${n}`, token_type: 'Bearer',
  refresh_token: `FIXTURE-SECRET-REFRESH-${n}`, expiry: '2099-01-01T00:00:00.000000Z' }, auth_method: 'consumer' });
try {
  mkdirSync(join(data, 'accounts'), { recursive: true });
  const index = JSON.stringify({ version: '2.0', accounts: [{ id: 'agy-test-1' }, { id: 'agy-test-2' }], current_account_id: 'agy-test-1', current_target_ide: 'app' });
  writeFileSync(join(data, 'accounts.json'), index);
  for (const n of [1, 2]) writeFileSync(join(data, `accounts/agy-test-${n}.json`), JSON.stringify(account(n)));

  if (!macos) {
    // Only macOS is verified; other platforms must refuse before touching anything.
    const refused = run(['switch', 'agy-test-2', '--target', 'agy', '--json'], 1);
    assert.match(refused.stderr, /verified on macOS only/);
    assert.equal(existsSync(join(home, '.gemini')), false);
    console.log(`agy-only target refused as expected (${passed} checks passed)`);
  } else {
    // Without an initialized agy session nothing is created and the switch fails.
    const missing = run(['switch', 'agy-test-2', '--target', 'agy', '--json'], 1);
    assert.match(missing.stderr, /agy/i);
    assert.equal(existsSync(join(home, '.gemini')), false);
    run(['current', '--target', 'agy', '--json'], 1);

    mkdirSync(join(home, '.gemini', 'antigravity-cli'), { recursive: true });
    writeFileSync(session, sessionPayload(1));
    const live = JSON.parse(run(['current', '--target', 'agy', '--json']).stdout);
    assert.equal(live.account.id, 'agy-test-1');
    assert.equal(live.account.is_current, true);
    assert.equal(live.source, 'agy_session');
    assert.equal(live.current_target, 'agy');
    assert.match(run(['current', '--target', 'agy']).stdout, /agy-test-1/);

    // The switch rewrites only the agy session file.
    const result = JSON.parse(run(['switch', 'agy-test-2', '--target', 'agy', '--json']).stdout);
    assert.deepEqual([result.switched, result.account_id, result.target], [true, 'agy-test-2', 'agy']);
    assert.ok(!('token' in result));
    const written = JSON.parse(readFileSync(session, 'utf8'));
    assert.equal(written.token.refresh_token, 'FIXTURE-SECRET-REFRESH-2');
    assert.equal(written.auth_method, 'consumer');
    assert.equal(JSON.parse(run(['current', '--target', 'agy', '--json']).stdout).account.id, 'agy-test-2');
    // The App's recorded selection and the account index stay as they were.
    assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), index);
    assert.equal(JSON.parse(run(['current', '--json']).stdout).account.id, 'agy-test-1');
    assert.equal(JSON.parse(readFileSync(join(data, 'accounts/agy-test-1.json'), 'utf8')).last_used, 1);
    assert.equal(JSON.parse(readFileSync(join(data, 'accounts/agy-test-2.json'), 'utf8')).last_used, 1);
    assert.deepEqual(Object.keys(JSON.parse(readFileSync(join(home, '.gemini', 'antigravity-cli', 'antigravity-oauth-token'), 'utf8'))).sort(), ['auth_method', 'token']);

    // A session that matches no saved account is reported, not guessed.
    writeFileSync(session, sessionPayload(9));
    run(['current', '--target', 'agy', '--json'], 3);
    // Unknown accounts and the removed pseudo-target still fail before any write.
    run(['switch', 'missing@example.invalid', '--target', 'agy', '--json'], 3);
    run(['switch', 'agy-test-1', '--target', 'cli', '--json'], 2);
    assert.equal(JSON.parse(readFileSync(session, 'utf8')).token.refresh_token, 'FIXTURE-SECRET-REFRESH-9');
    console.log(`agy-only target verified (${passed} checks passed)`);
  }
} finally {
  rmSync(root, { recursive: true, force: true });
}
