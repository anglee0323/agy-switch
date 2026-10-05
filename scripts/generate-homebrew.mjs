#!/usr/bin/env node
// Generate only from a real, immutable release archive and an explicit HTTPS URL.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { desktopBrand } from './release-brand.mjs';
import { spawnSync } from 'node:child_process';

export function renderCask({ archive, url, version, template }) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version ?? '')) throw new Error('Provide a release version such as 4.7.6');
  const parsed = new URL(url);
  if (parsed.protocol !== 'https:' || parsed.username || parsed.password || parsed.hash || parsed.search || /["\\\r\n#]/.test(url)) throw new Error('Provide a plain HTTPS release archive URL');
  const brand = desktopBrand(version);
  const expected = `${brand.prefix}-${version}-macos-arm64.zip`;
  if (basename(archive) !== expected || decodeURIComponent(parsed.pathname.split('/').at(-1)) !== expected) throw new Error(`Archive and URL must both name ${expected}`);
  const bytes = readFileSync(archive);
  if (bytes.length < 4 || bytes.readUInt32LE(0) !== 0x04034b50) throw new Error('The release archive is not a ZIP');
  const listing = spawnSync('unzip', ['-Z1', archive], { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
  if (listing.error || listing.status !== 0) throw new Error('Cannot inspect the archive. Install unzip and supply a valid ZIP');
  if (!listing.stdout.split('\n').includes(`${brand.app}.app/Contents/MacOS/${brand.executable}`)) throw new Error('ZIP does not contain the bundled desktop executable');
  const sha256 = createHash('sha256').update(bytes).digest('hex');
  return template.replaceAll('@VERSION@', version).replaceAll('@SHA256@', sha256).replaceAll('@URL@', url).replaceAll('@APP_NAME@', brand.app).replaceAll('@EXECUTABLE@', brand.executable);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const options = {};
    const accepted = new Set(['--archive', '--url', '--version', '--output']);
    for (let i = 2; i < process.argv.length; i += 2) {
      const key = process.argv[i]; const value = process.argv[i + 1];
      if (!accepted.has(key) || !value || options[key.slice(2)]) throw new Error('Usage: node scripts/generate-homebrew.mjs --archive ZIP --url HTTPS_URL --version VERSION --output CASK_RB');
      options[key.slice(2)] = value;
    }
    if (Object.keys(options).length !== 4) throw new Error('All four arguments are required: --archive, --url, --version, --output');
    const template = readFileSync(new URL('../packaging/homebrew/agy-switch.rb.in', import.meta.url), 'utf8');
    const content = renderCask({ ...options, template });
    mkdirSync(dirname(options.output), { recursive: true });
    writeFileSync(options.output, content);
    console.log(`Generated ${options.output} with the archive's SHA-256. Publish that exact ZIP before installing this cask.`);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
