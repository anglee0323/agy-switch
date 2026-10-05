import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { packageNames, validateSource } from './release-assets.mjs';
import { updatePackages } from './update-assets.mjs';
import { desktopBrand } from './release-brand.mjs';

const root = new URL('../', import.meta.url);
const read = file => readFileSync(new URL(file, root));
const json = file => JSON.parse(read(file));
test('source app, npm, Rust and both languages agree on the new identity', () => {
  const version = json('package.json').version;
  assert.equal(validateSource(fileURLToPath(root), `v${version}`), version);
  assert.equal(json('package.json').name, 'agy-switch');
  for (const lang of ['en', 'zh']) assert.equal(json(`src/locales/${lang}.json`).common.app_name, 'AntiGravity Switch');
  const conf = json('src-tauri/tauri.conf.json');
  assert.equal(conf.productName, 'agy-switch');
  assert.equal(conf.mainBinaryName, 'agy-switch-desktop');
  // Keep OS-managed settings identity and the existing account store stable.
  assert.equal(conf.identifier, 'com.lbjlaq.antigravity-tools-lite');
  assert.match(read('src-tauri/src/modules/account.rs').toString(), /const DATA_DIR: &str = "\.antigravity_tools"/);
  for (const platform of ['linux', 'windows']) assert.equal(json(`src-tauri/tauri.${platform}.conf.json`).app.windows[0].title, 'agy-switch');
  assert.deepEqual(conf.plugins.updater.endpoints, ['https://github.com/anglee0323/agy-switch/releases/latest/download/latest.json']);
});
test('new desktop packages and signed updater formats use the same brand', () => {
  const version = '4.9.0';
  assert.deepEqual(desktopBrand(version), { prefix: 'agy-switch', app: 'agy-switch', executable: 'agy-switch-desktop' });
  assert.deepEqual(packageNames(version), ['agy-switch-4.9.0-macos-arm64.zip', 'agy-switch-4.9.0-windows-x64-setup.exe',
    'agy-switch-4.9.0-linux-amd64.deb', 'agy-switch-4.9.0-windows-x64.zip', 'agy-switch-4.9.0-linux-amd64.tar.gz']);
  assert.deepEqual(updatePackages(version), { 'darwin-aarch64': 'agy-switch-4.9.0-macos-arm64.app.tar.gz',
    'windows-x86_64': 'agy-switch-4.9.0-windows-x64-setup.exe', 'linux-x86_64-deb': 'agy-switch-4.9.0-linux-amd64.deb' });
  assert.equal(packageNames('4.8.1')[0], 'Antigravity-Tools-Lite-4.8.1-macos-arm64.zip');
  assert.equal(updatePackages('4.8.1')['darwin-aarch64'], 'Antigravity-Tools-Lite-4.8.1-macos-arm64.app.tar.gz');
  assert.throws(() => desktopBrand('4.9.0/path'));
});
test('frontend and OS icons have the expected formats, dimensions and matching source', () => {
  const png = (file, size) => {
    const data = read(file);
    assert.equal(data.subarray(0, 8).toString('hex'), '89504e470d0a1a0a', file);
    assert.equal(data.readUInt32BE(16), size, file);
    assert.equal(data.readUInt32BE(20), size, file);
    assert.equal(data[25], 6, `${file} should preserve RGBA transparency`);
  };
  for (const [file, size] of [['32x32.png',32], ['128x128.png',128], ['128x128@2x.png',256],
    ['icon.png',512], ['icon_master_squircle.png',1024], ['tray-icon.png',44]]) png(`src-tauri/icons/${file}`,size);
  for (const file of ['public/logo.png','public/icon.png','src/assets/logo.png']) {
    png(file,256); assert.deepEqual(read(file),read('src-tauri/icons/128x128@2x.png'));
  }
  assert.equal(read('src-tauri/icons/icon.icns').subarray(0,4).toString(),'icns');
  assert.equal(read('src-tauri/icons/icon.ico').readUInt16LE(2),1);
  for (const size of [16,32,128,256,512]) {
    png(`src-tauri/icons/icon.iconset/icon_${size}x${size}.png`,size);
    png(`src-tauri/icons/icon.iconset/icon_${size}x${size}@2x.png`,size*2);
  }
});
