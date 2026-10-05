import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { renderCask } from './generate-homebrew.mjs';

const template = readFileSync(new URL('../packaging/homebrew/agy-switch.rb.in', import.meta.url), 'utf8');
const version = '4.9.0';
const filename = `agy-switch-${version}-macos-arm64.zip`;
// Fixture URL only; the generator never contacts or publishes it.
const url = `https://example.invalid/releases/${filename}`;
function fixture(fn) {
  const root = mkdtempSync(join(tmpdir(), 'agy-lite-brew-'));
  try {
    const bin = join(root, 'agy-switch.app/Contents/MacOS');
    mkdirSync(bin, { recursive: true }); writeFileSync(join(bin, 'agy-switch-desktop'), 'fixture');
    const archive = join(root, filename);
    const zip = spawnSync('zip', ['-qr', archive, 'agy-switch.app'], { cwd: root });
    assert.equal(zip.status, 0, 'zip must be installed for generator tests');
    fn(archive);
  } finally { rmSync(root, { recursive: true, force: true }); }
}
test('cask pins exact local archive hash, arm64 and bundled management CLI', () => fixture(archive => {
  const cask = renderCask({ archive, url, version, template });
  assert.ok(cask.includes(createHash('sha256').update(readFileSync(archive)).digest('hex')));
  assert.ok(cask.includes(`url "${url}"`));
  assert.ok(cask.includes('depends_on arch: :arm64'));
  assert.ok(cask.includes('target: "agy-switch"'));
  assert.ok(cask.includes('app "agy-switch.app"'));
  assert.ok(cask.includes('Contents/MacOS/agy-switch-desktop'));
  assert.ok(cask.includes('cask "agy-switch"'));
  assert.ok(!cask.includes('@APP_NAME@') && !cask.includes('@EXECUTABLE@'));
  assert.ok(!cask.includes('sha256 :no_check')); assert.ok(!cask.includes('@VERSION@'));
  assert.equal(cask, renderCask({ archive, url, version, template }));
}));
test('rejects mismatched versions, insecure/injected URLs and wrong archive content', () => fixture(archive => {
  for (const bad of [url.replace('https:', 'http:'), `${url}#fragment`, `${url}?token=x`, `${url}#{system("bad")}`]) assert.throws(() => renderCask({ archive, url: bad, version, template }));
  assert.throws(() => renderCask({ archive, url, version: '5.0.0', template }));
  writeFileSync(archive, 'not a ZIP');
  assert.throws(() => renderCask({ archive, url, version, template }));
}));
