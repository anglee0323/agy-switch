// Real executable smoke tests with synthetic local files. No OAuth or switch calls.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, readdirSync, rmSync, symlinkSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';

const binary = resolve(process.argv[2] ?? 'src-tauri/target/debug/agy-switch-desktop');
const root = mkdtempSync(join(tmpdir(), 'agy-lite-cli-'));
const data = join(root, 'data');
const env = { ...process.env, ABV_DATA_DIR: data, DISPLAY: '', WAYLAND_DISPLAY: '' };
let passed = 0;
function run(args, code = 0, exe = binary) {
  const result = spawnSync(exe, args, { env, encoding: 'utf8', timeout: 10000 });
  assert.equal(result.error, undefined, 'CLI must exit without a display');
  assert.equal(result.status, code, `${args.join(' ')}: ${result.stderr}`);
  assert.ok(!`${result.stdout}${result.stderr}`.includes('FIXTURE-SECRET'));
  passed++;
  return result;
}
try {
  assert.match(run(['--version']).stdout, /^agy-switch \d+\.\d+\.\d+/);
  assert.match(run(['--help']).stdout, /cached data/);
  assert.deepEqual(JSON.parse(run(['accounts', 'list', '--json']).stdout).accounts, []);
  assert.equal(JSON.parse(run(['policy', 'show', '--json']).stdout).policy.enabled, false);
  assert.deepEqual(JSON.parse(run(['accounts', 'order', '--json']).stdout).account_ids, []);
  const experiments = JSON.parse(run(['experiments', 'show', '--json']).stdout);
  assert.equal(experiments.experiments.translation_enabled, false);
  assert.equal(experiments.experiments.scope, 'antigravity_app');
  assert.equal(experiments.runtime_checked, false);
  assert.deepEqual(readdirSync(root), []); // Not even a log/data directory is created.
  run(['current'], 3);
  run(['quota', '--refresh'], 2);
  run(['switch', 'unused', '--target', 'cli', '--json'], 2);
  run(['experiments', 'run', '--json'], 2);
  run(['experiments', 'translation', 'maybe'], 2);
  assert.deepEqual(readdirSync(root), []); // Unsupported target must fail before any state access.
  assert.equal(JSON.parse(run(['--unknown', '--json'], 2).stderr).error.code, 2);
  mkdirSync(join(data, 'accounts'), { recursive: true });
  const index = JSON.stringify({ accounts: [{ id: 'test-1' }], current_account_id: 'test-1', current_target_ide: 'agy' });
  const account = JSON.stringify({ id: 'test-1', email: 'test@example.invalid', token: { access_token: 'FIXTURE-SECRET-ACCESS', refresh_token: 'FIXTURE-SECRET-REFRESH' }, quota: { last_updated: 123, models: [{ name: 'test-model', percentage: 75, reset_time: '2030-01-01T00:00:00Z' }] } });
  writeFileSync(join(data, 'accounts.json'), index);
  writeFileSync(join(data, 'accounts/test-1.json'), account);
  assert.equal(JSON.parse(run(['current', '--json']).stdout).account.id, 'test-1');
  assert.equal(JSON.parse(run(['quota', '--json']).stdout).quota.models[0].percentage, 75);
  assert.match(run(['quota', 'TEST@example.invalid']).stdout, /75% remaining/);
  run(['quota', 'missing@example.invalid'], 3);
  assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), index);
  assert.equal(readFileSync(join(data, 'accounts/test-1.json'), 'utf8'), account);
  assert.deepEqual(readdirSync(data).sort(), ['accounts', 'accounts.json']);
  const noQuota = JSON.parse(account); delete noQuota.quota;
  writeFileSync(join(data, 'accounts/test-1.json'), JSON.stringify(noQuota));
  run(['quota'], 4);
  // Real settings/order writes stay inside synthetic local storage. Credential files
  // and the current account must remain unchanged, including unknown index metadata.
  const second = JSON.stringify({ ...JSON.parse(account), id: 'test-2', email: 'second@example.invalid' });
  writeFileSync(join(data, 'accounts/test-2.json'), second);
  writeFileSync(join(data, 'accounts.json'), JSON.stringify({ ...JSON.parse(index), accounts: [{ id: 'test-1', extra: 'retain' }, { id: 'test-2' }], extra: 'retain' }));
  assert.deepEqual(JSON.parse(run(['accounts', 'order', 'second@example.invalid', 'test-1', '--json']).stdout).account_ids, ['test-2', 'test-1']);
  const orderedIndex = readFileSync(join(data, 'accounts.json'), 'utf8');
  assert.equal(JSON.parse(orderedIndex).current_account_id, 'test-1');
  assert.equal(JSON.parse(orderedIndex).accounts[1].extra, 'retain');
  run(['accounts', 'order', 'test-1', '--json'], 2);
  run(['accounts', 'order', 'test-1', 'test-1'], 2);
  assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), orderedIndex);
  const policy = JSON.parse(run(['policy', 'set', '--enabled', 'true', '--mode', 'wait', '--strategy', 'round-robin', '--reserve', '15', '--minimum', '40', '--model', 'gemini', '--target', 'app-cli', '--candidates', 'second@example.invalid', 'test-1', '--json']).stdout).policy;
  assert.equal(policy.enabled, true); assert.equal(policy.strategy, 'round_robin');
  assert.equal(policy.target, 'app_cli'); assert.equal(policy.reserve_percentage, 15);
  assert.deepEqual(policy.candidate_account_ids, ['test-2', 'test-1']);
  assert.deepEqual(JSON.parse(run(['policy', 'order', 'test-1', 'test-2', '--json']).stdout).policy.candidate_account_ids, ['test-1', 'test-2']);
  const beforePolicy = readFileSync(join(data, 'auto_switch.json'), 'utf8');
  run(['policy', 'set', '--reserve', '90', '--minimum', '40'], 2);
  run(['policy', 'set', '--candidates', 'missing@example.invalid'], 3);
  run(['policy', 'set', '--candidates', 'test-1', 'test@example.invalid'], 2);
  run(['policy', 'order', 'test-1'], 2);
  assert.equal(readFileSync(join(data, 'auto_switch.json'), 'utf8'), beforePolicy);
  assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), orderedIndex);
  assert.equal(readFileSync(join(data, 'accounts/test-1.json'), 'utf8'), JSON.stringify(noQuota));
  assert.equal(readFileSync(join(data, 'accounts/test-2.json'), 'utf8'), second);
  // The experimental switch belongs to Switch. It must not read an account,
  // contact the App, alter client preferences or require a desktop session.
  const savedGui = JSON.stringify({ language: 'en', theme: 'light' });
  writeFileSync(join(data, 'gui_config.json'), savedGui);
  assert.equal(JSON.parse(run(['experiments', 'translation', 'on', '--json']).stdout).experiments.translation_enabled, true);
  assert.deepEqual(JSON.parse(readFileSync(join(data, 'app_experiments.json'), 'utf8')), { translation_enabled: true });
  assert.match(run(['experiments', 'show']).stdout, /does not modify App installation\/source files/);
  assert.equal(JSON.parse(run(['experiments', 'translation', 'off', '--json']).stdout).experiments.translation_enabled, false);
  run(['experiments', 'run'], 1);
  assert.equal(readFileSync(join(data, 'gui_config.json'), 'utf8'), savedGui);
  assert.equal(readFileSync(join(data, 'auto_switch.json'), 'utf8'), beforePolicy);
  assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), orderedIndex);
  assert.equal(readFileSync(join(data, 'accounts/test-2.json'), 'utf8'), second);
  writeFileSync(join(data, 'app_experiments.json'), 'corrupt');
  run(['experiments', 'translation', 'off'], 1);
  run(['experiments', 'show'], 1);
  assert.equal(readFileSync(join(data, 'app_experiments.json'), 'utf8'), 'corrupt');
  writeFileSync(join(data, 'auto_switch.json'), JSON.stringify({ ...policy, candidate_account_ids: ['removed-account'] }));
  assert.equal(JSON.parse(run(['policy', 'set', '--enabled', 'false', '--json']).stdout).policy.enabled, false);
  writeFileSync(join(data, 'auto_switch.json'), 'corrupt');
  run(['policy', 'set', '--enabled', 'false'], 1);
  assert.equal(readFileSync(join(data, 'auto_switch.json'), 'utf8'), 'corrupt');
  writeFileSync(join(data, 'accounts.json'), 'corrupt');
  run(['list', '--json'], 1);
  assert.equal(readFileSync(join(data, 'accounts.json'), 'utf8'), 'corrupt');
  if (process.platform !== 'win32') {
    const linkSwitch = join(root, 'agy-switch'); symlinkSync(binary, linkSwitch);
    assert.match(run([], 0, linkSwitch).stdout, /Usage:/);
  }
  console.log(`${passed} CLI executable smoke checks passed; no GUI, network or credential changes`);
} finally { rmSync(root, { recursive: true, force: true }); }
