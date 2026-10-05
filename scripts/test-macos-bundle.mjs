// Real, disposable Mach-O fixtures; no app is executed, no signing identity is used.
import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, cpSync, renameSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyMacosBundle } from './verify-macos-bundle.mjs';

const mac = process.platform === 'darwin';
const run = (command, args) => execFileSync(command, args, { stdio: 'pipe' });
function fixture(fn) {
  const root = mkdtempSync(join(tmpdir(), 'agy-macos-signature-test-'));
  try {
    const app = join(root, 'Antigravity Tools Lite.app');
    mkdirSync(join(app, 'Contents/MacOS'), { recursive: true });
    mkdirSync(join(app, 'Contents/Resources'));
    writeFileSync(join(root, 'fixture.c'), 'int main(void) { return 0; }\n');
    run('/usr/bin/clang', ['-target', 'arm64-apple-macos11', join(root, 'fixture.c'), '-o', join(app, 'Contents/MacOS/antigravity-tools')]);
    const plist = '<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict>' +
      '<key>CFBundleIdentifier</key><string>com.lbjlaq.antigravity-tools-lite</string><key>CFBundleExecutable</key><string>antigravity-tools</string>' +
      '<key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>4.7.7</string></dict></plist>';
    writeFileSync(join(app, 'Contents/Info.plist'), plist);
    cpSync(new URL('../src-tauri/icons/icon.icns', import.meta.url), join(app, 'Contents/Resources/icon.icns'));
    const sign = (entitlements = fileURLToPath(new URL('../src-tauri/Entitlements.plist', import.meta.url))) =>
      run('/usr/bin/codesign', ['--force', '--sign', '-', '--timestamp=none', '--options', 'runtime', '--entitlements', entitlements, app]);
    return fn({ root, app, sign });
  } finally { rmSync(root, { recursive: true, force: true }); }
}

test('linker-only signature is rejected; fully sealed ad-hoc bundle passes without executing it', { skip: !mac }, () => fixture(({ app, sign }) => {
  assert.throws(() => verifyMacosBundle(app, '4.7.7'));
  sign();
  const result = verifyMacosBundle(app, '4.7.7');
  assert.equal(result.bundle_signature_integrity, 'verified');
  assert.equal(result.entitlements, 'empty');
  assert.equal(result.gatekeeper_acceptance, 'not_tested');
  assert.equal(result.notarization, 'not_tested');
}));

test('changed or missing resources and changed bound metadata fail before staging', { skip: !mac }, () => {
  for (const kind of ['resource-changed', 'resource-missing', 'plist-changed', 'binary-changed']) fixture(({ app, sign }) => {
    sign();
    if (kind === 'resource-changed') writeFileSync(join(app, 'Contents/Resources/icon.icns'), 'tampered');
    if (kind === 'resource-missing') rmSync(join(app, 'Contents/Resources/icon.icns'));
    if (kind === 'plist-changed') writeFileSync(join(app, 'Contents/Info.plist'), readFileSync(join(app, 'Contents/Info.plist'), 'utf8').replace('</dict>', '<key>UntrustedChange</key><true/></dict>'));
    if (kind === 'binary-changed') {
      const file = join(app, 'Contents/MacOS/antigravity-tools'), bytes = readFileSync(file); bytes[4096] ^= 1; writeFileSync(file, bytes);
    }
    assert.throws(() => verifyMacosBundle(app, '4.7.7'), kind);
  });
});

test('wrong version and Intel payload are rejected even with a complete signature', { skip: !mac }, () => fixture(({ root, app, sign }) => {
  sign(); assert.throws(() => verifyMacosBundle(app, '4.7.8'), /version mismatch/);
  run('/usr/bin/clang', ['-target', 'x86_64-apple-macos11', join(root, 'fixture.c'), '-o', join(app, 'Contents/MacOS/antigravity-tools')]);
  sign(); assert.throws(() => verifyMacosBundle(app, '4.7.7'), /arm64/);
}));

test('published ZIP round trip preserves the complete resource seal', { skip: !mac }, () => fixture(({ root, app, sign }) => {
  sign();
  const archive = join(root, 'package.zip'), extracted = join(root, 'extracted');
  run('/usr/bin/ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', app, archive]);
  run('/usr/bin/ditto', ['-x', '-k', archive, extracted]);
  assert.equal(verifyMacosBundle(join(extracted, 'Antigravity Tools Lite.app'), '4.7.7').bundle_signature_integrity, 'verified');
}));


test('a complete signature carrying App Sandbox or unexpected permissions is rejected', { skip: !mac }, () => {
  for (const key of ['com.apple.security.app-sandbox', 'com.apple.security.cs.disable-library-validation']) fixture(({ root, app, sign }) => {
    const permissions = join(root, 'unexpected-entitlements.plist');
    writeFileSync(permissions, '<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>' + key + '</key><true/></dict></plist>');
    sign(permissions);
    run('/usr/bin/codesign', ['--verify', '--deep', '--strict', app]);
    assert.throws(() => verifyMacosBundle(app, '4.7.7'), /Sandbox|Unexpected signed entitlements/);
  });
});

test('4.9.1 requires both full display names even when the bundle signature is valid', { skip: !mac }, () => fixture(({ app, sign }) => {
  renameSync(join(app, 'Contents/MacOS/antigravity-tools'), join(app, 'Contents/MacOS/agy-switch-desktop'));
  const plist = join(app, 'Contents/Info.plist');
  const xml = readFileSync(plist, 'utf8').replace('<string>antigravity-tools</string>', '<string>agy-switch-desktop</string>')
    .replace('<string>4.7.7</string>', '<string>4.9.1</string>')
    .replace('</dict>', '<key>CFBundleName</key><string>AntiGravity Switch</string><key>CFBundleDisplayName</key><string>AntiGravity Switch</string></dict>');
  writeFileSync(plist, xml); sign();
  assert.equal(verifyMacosBundle(app, '4.9.1').bundle_signature_integrity, 'verified');
  for (const key of ['CFBundleName', 'CFBundleDisplayName']) {
    writeFileSync(plist, xml.replace(`<key>${key}</key><string>AntiGravity Switch</string>`, `<key>${key}</key><string>agy-switch</string>`));
    sign(); assert.throws(() => verifyMacosBundle(app, '4.9.1'), /display name mismatch/);
  }
}));
