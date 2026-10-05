// Signed updater feeds are separate from OS code signing and never accept caller-provided URLs.
import { execFileSync } from 'node:child_process';
import { createHash, createPublicKey, verify } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { desktopBrand } from './release-brand.mjs';

export function updatePackages(version) {
  const { prefix } = desktopBrand(version);
  return {
    'darwin-aarch64': `${prefix}-${version}-macos-arm64.app.tar.gz`,
    'windows-x86_64': `${prefix}-${version}-windows-x64-setup.exe`,
    'linux-x86_64-deb': `${prefix}-${version}-linux-amd64.deb`,
  };
}
export function verifyUpdateSignature(bytes, encodedSignature, encodedPublicKey) {
  const keyLines = Buffer.from(encodedPublicKey, 'base64').toString('utf8').trim().split('\n');
  const key = Buffer.from(keyLines[1] || '', 'base64');
  const lines = Buffer.from(encodedSignature, 'base64').toString('utf8').trim().split('\n');
  const signature = Buffer.from(lines[1] || '', 'base64'), global = Buffer.from(lines[3] || '', 'base64');
  if (key.length !== 42 || signature.length !== 74 || global.length !== 64 || !lines[2]?.startsWith('trusted comment: ') || !signature.subarray(2, 10).equals(key.subarray(2, 10))) throw new Error('Invalid updater signature or key');
  const algorithm = signature.subarray(0, 2).toString();
  if (!['Ed', 'ED'].includes(algorithm)) throw new Error('Invalid updater signature algorithm');
  const publicKey = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), key.subarray(10)]), format: 'der', type: 'spki' });
  const payload = algorithm === 'ED' ? createHash('blake2b512').update(bytes).digest() : bytes;
  if (!verify(null, payload, publicKey, signature.subarray(10)) || !verify(null, Buffer.concat([signature.subarray(10), Buffer.from(lines[2].slice('trusted comment: '.length))]), publicKey, global)) throw new Error('Updater signature verification failed');
}
export function writeUpdateFeed(directory, version, repository, candidate = false) {
  const platforms = Object.fromEntries(Object.entries(updatePackages(version)).map(([target, name]) => [target, {
    url: `https://github.com/${repository}/releases/download/v${version}/${name}`,
    signature: candidate ? 'UNSIGNED-CANDIDATE-NOT-FOR-PUBLICATION' : readFileSync(join(directory, `${name}.sig`), 'utf8').trim(),
  }]));
  writeFileSync(join(directory, 'latest.json'), JSON.stringify({ version, notes: `${desktopBrand(version).app} ${version}`, pub_date: execFileSync('git', ['show', '-s', '--format=%cI', 'HEAD'], { encoding: 'utf8' }).trim(), platforms }, null, 2) + '\n');
}
export function verifyUpdateFeed(directory, version, repository, candidate = false) {
  const feed = JSON.parse(readFileSync(join(directory, 'latest.json'), 'utf8'));
  const names = updatePackages(version);
  if (feed.version !== version || !Number.isFinite(Date.parse(feed.pub_date)) || JSON.stringify(Object.keys(feed.platforms).sort()) !== JSON.stringify(Object.keys(names).sort())) throw new Error('Invalid updater feed');
  const publicKey = JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8')).plugins.updater.pubkey;
  for (const [target, name] of Object.entries(names)) {
    const entry = feed.platforms[target];
    if (entry.url !== `https://github.com/${repository}/releases/download/v${version}/${name}`) throw new Error('Untrusted updater URL');
    if (!candidate) {
      if (entry.signature !== readFileSync(join(directory, `${name}.sig`), 'utf8').trim()) throw new Error('Updater signature differs from feed');
      verifyUpdateSignature(readFileSync(join(directory, name)), entry.signature, publicKey);
      const comment = Buffer.from(entry.signature, 'base64').toString('utf8').split('\n')[2].slice('trusted comment: '.length);
      const filenames = comment.split('\t').filter(field => field.startsWith('file:'));
      if (filenames.length !== 1 || filenames[0] !== `file:${name}`) throw new Error('Signed filename does not match the announced release');
    } else if (entry.signature !== 'UNSIGNED-CANDIDATE-NOT-FOR-PUBLICATION') throw new Error('Unexpected candidate signature');
  }
  return [...Object.values(names).filter(name => name.endsWith('.app.tar.gz')), ...(candidate ? [] : Object.values(names).map(name => `${name}.sig`)), 'latest.json'];
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { const [directory, version, repository, mode] = process.argv.slice(2); if (!directory || !/^\d+\.\d+\.\d+$/.test(version) || !/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error('Usage: update-assets.mjs DIRECTORY VERSION OWNER/REPO [candidate]'); writeUpdateFeed(directory, version, repository, mode === 'candidate'); verifyUpdateFeed(directory, version, repository, mode === 'candidate'); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
