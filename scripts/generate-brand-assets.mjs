// Render the SVG sources with the existing Tauri CLI. Generates artwork only, never an app bundle.
import { execFileSync } from 'node:child_process';
import { copyFileSync, cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const scratch = mkdtempSync(join(tmpdir(), 'agy-brand-'));
const tauri = resolve(root, 'node_modules/@tauri-apps/cli/tauri.js');
const render = (source, output, sizes = []) => execFileSync(process.execPath,
  [tauri, 'icon', join(root, source), '--output', output, ...sizes.flatMap(size => ['--png', String(size)])],
  { cwd: root, stdio: 'pipe' });
const copy = (from, to) => { mkdirSync(resolve(to, '..'), { recursive: true }); copyFileSync(from, to); };
// The renderer writes ICNS entries in hash-map order. Canonicalize that order
// without altering the pixel payloads so regeneration yields identical artwork.
const canonicalIcns = file => {
  const bytes = readFileSync(file), entries = [];
  if (bytes.subarray(0,4).toString() !== 'icns' || bytes.readUInt32BE(4) !== bytes.length) throw new Error('Invalid generated ICNS');
  for (let offset = 8; offset < bytes.length;) {
    const length = bytes.readUInt32BE(offset + 4);
    if (length < 8 || offset + length > bytes.length) throw new Error('Invalid ICNS entry');
    entries.push(bytes.subarray(offset,offset + length)); offset += length;
  }
  entries.sort((a,b) => Buffer.compare(a.subarray(0,4),b.subarray(0,4)));
  writeFileSync(file,Buffer.concat([bytes.subarray(0,8),...entries]));
};
try {
  const desktop = join(scratch, 'desktop'), sizes = join(scratch, 'sizes'), tray = join(scratch, 'tray');
  render('branding/app-icon.svg', desktop);
  canonicalIcns(join(desktop,'icon.icns'));
  render('branding/app-icon.svg', sizes, [16, 32, 64, 128, 256, 512, 1024]);
  cpSync(desktop,join(root,'src-tauri/icons'),{ recursive: true });
  copy(join(sizes,'64x64.png'),join(root,'src-tauri/icons/64x64.png'));
  copy(join(sizes, '1024x1024.png'), join(root, 'src-tauri/icons/icon_master_squircle.png'));
  for (const size of [16, 32, 128, 256, 512]) {
    copy(join(sizes, `${size}x${size}.png`), join(root, `src-tauri/icons/icon.iconset/icon_${size}x${size}.png`));
    copy(join(sizes, `${size * 2}x${size * 2}.png`), join(root, `src-tauri/icons/icon.iconset/icon_${size}x${size}@2x.png`));
  }
  for (const file of ['src/assets/logo.png', 'public/logo.png', 'public/icon.png'])
    copy(join(sizes, '256x256.png'), join(root, file));
  render('branding/tray.svg', tray, [44]);
  copy(join(tray, '44x44.png'), join(root, 'src-tauri/icons/tray-icon.png'));
  console.log('Updated app, web and menu bar artwork from branding SVG sources. No app was packaged.');
} finally { rmSync(scratch, { recursive: true, force: true }); }
