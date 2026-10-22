// Run local HTTP quota fixtures without native version discovery, credentials,
// production settings or an application directory left on the hosted runner.
import assert from 'node:assert/strict';
import {mkdtempSync, existsSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
const root = mkdtempSync(join(tmpdir(), 'agy-quota-fixture-'));
const data = join(root, 'data');
try {
  const result = spawnSync('cargo', ['test', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', '--lib', 'modules::quota::tests'], {
    env: {...process.env, ABV_DATA_DIR: data}, stdio: 'inherit', timeout: 300000,
  });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0, 'Quota regression tests must pass');
  assert.equal(existsSync(data), false, 'Quota fixtures must not initialize application storage');
  console.log('Quota HTTP fixtures passed without initializing application storage');
} finally { rmSync(root, {recursive: true, force: true}); }
