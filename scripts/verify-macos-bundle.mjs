// Read-only validation of the final macOS bundle; never launches or signs it.
import { execFileSync } from 'node:child_process';
import { statSync, lstatSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { desktopBrand } from './release-brand.mjs';

export function verifyMacosBundle(app, version, { strict = true } = {}) {
  if (process.platform !== 'darwin') throw new Error('macOS bundle verification requires macOS');
  if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) throw new Error('Expected a release version');
  app = resolve(app);
  const brand = desktopBrand(version);
  const run = (file, args, input) => {
    try { return execFileSync(file, args, { encoding: 'utf8', input, stdio: ['pipe', 'pipe', 'pipe'], timeout: 30000 }).trim(); }
    catch (error) { throw new Error(`${file} verification failed: ${error.stderr?.toString().trim() || error.message}`); }
  };
  const regular = file => {
    if (!lstatSync(file).isFile() || statSync(file).size === 0) throw new Error(`Missing/non-regular/empty bundle resource: ${file}`);
  };
  const plist = join(app, 'Contents/Info.plist');
  regular(plist);
  const field = key => run('/usr/libexec/PlistBuddy', ['-c', 'Print :' + key, plist]);
  if (field('CFBundleIdentifier') !== 'com.lbjlaq.antigravity-tools-lite') throw new Error('Unexpected bundle identifier');
  if (field('CFBundleExecutable') !== brand.executable) throw new Error('Unexpected bundle executable');
  if (field('CFBundleShortVersionString') !== version) throw new Error('Bundle version mismatch');
  if (brand.app === 'AntiGravity Switch' &&
      (field('CFBundleName') !== brand.app || field('CFBundleDisplayName') !== brand.app)) throw new Error('Bundle display name mismatch');
  const executable = join(app, `Contents/MacOS/${brand.executable}`);
  regular(executable);
  if (!(statSync(executable).mode & 0o111)) throw new Error('Bundle executable is not executable');
  if (run('/usr/bin/lipo', ['-archs', executable]) !== 'arm64') throw new Error('Expected arm64-only executable');
  regular(join(app, 'Contents/Resources/icon.icns'));
  // A linker-signed Mach-O alone does not seal the .app Info.plist/resources.
  regular(join(app, 'Contents/_CodeSignature/CodeResources'));
  const codesignArgs = ['--verify', '--deep'];
  if (strict) codesignArgs.push('--strict');
  codesignArgs.push('--verbose=2', app);
  run('/usr/bin/codesign', codesignArgs);
  const xml = run('/usr/bin/codesign', ['--display', '--entitlements', ':-', app]);
  const entitlements = xml ? JSON.parse(run('/usr/bin/plutil', ['-convert', 'json', '-o', '-', '-'], xml)) : {};
  if (entitlements['com.apple.security.app-sandbox'] === true) throw new Error('App Sandbox is not supported by the direct ZIP/Homebrew distribution');
  if (Object.keys(entitlements).length !== 0) throw new Error('Unexpected signed entitlements: this distribution requires an empty dictionary');
  return { version, architecture: 'arm64', bundle_signature_integrity: 'verified', entitlements: 'empty',
    gatekeeper_acceptance: 'not_tested', notarization: 'not_tested' };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [app, version, ...extra] = process.argv.slice(2);
    if (!app || !version || extra.length) throw new Error('Usage: verify-macos-bundle.mjs APP VERSION');
    console.log(JSON.stringify(verifyMacosBundle(app, version)));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
