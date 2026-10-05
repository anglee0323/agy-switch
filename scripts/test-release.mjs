import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, unlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, basename } from 'node:path';
import { spawnSync } from 'node:child_process';
import { packageNames, validateSource, verifyAssets, writeChecksum, writeManifest } from './release-assets.mjs';
import { writeUpdateFeed } from './update-assets.mjs';
import { renderCask } from './generate-homebrew.mjs';
import { publishRelease, releaseNotes } from './publish-release.mjs';
import { desktopBrand } from './release-brand.mjs';

// Entirely synthetic packages and GitHub responses; no network or real release.
const context = { tag: 'v4.7.7', commit: 'a'.repeat(40), repository: 'fixture/repository' };
const template = readFileSync(new URL('../packaging/homebrew/agy-switch.rb.in', import.meta.url), 'utf8');
function fixture(fn) {
  const root = mkdtempSync(join(tmpdir(), 'agy-release-test-'));
  try {
    const directory = join(root, 'assets'); mkdirSync(directory);
    const source = join(root, 'source'); const bin = join(source, 'Antigravity Tools Lite.app/Contents/MacOS');
    mkdirSync(bin, { recursive: true }); writeFileSync(join(bin, 'antigravity-tools'), 'synthetic app');
    const names = packageNames('4.7.7');
    const archive = join(directory, names[0]);
    assert.equal(spawnSync('zip', ['-qr', archive, 'Antigravity Tools Lite.app'], { cwd: source }).status, 0);
    for (const name of names.slice(1)) writeFileSync(join(directory, name), `synthetic ${name}`);
    for (const name of names) writeChecksum(join(directory, name));
    writeFileSync(join(directory, 'agy-switch.rb'), renderCask({ archive, version: '4.7.7', template,
      url: `https://github.com/${context.repository}/releases/download/${context.tag}/${names[0]}` }));
    return fn(directory, names, root);
  } finally { rmSync(root, { recursive: true, force: true }); }
}

function fakeGithub({ existing, failUpload = false, corruptDownload = false, remoteCommit = context.commit, newer = false, changeNotesOnRefresh = false, hideCreatedDraftFromList = false } = {}) {
  let release = existing ? { id: 123, body: releaseNotes(context), ...existing, assets: [] } : null;
  const files = new Map(existing?.files ?? []);
  const calls = [];
  const snapshot = () => ({ ...release, assets: [...files.keys()].map(name => ({ name })) });
  const run = args => {
    calls.push(args);
    if (args[0] === 'api') {
      if (args.includes('POST')) {
        const request = JSON.parse(readFileSync(args[args.indexOf('--input') + 1], 'utf8'));
        assert.equal(request.tag_name, context.tag); assert.equal(request.target_commitish, context.commit);
        assert.equal(request.draft, true); assert.equal(request.body, releaseNotes(context));
        release = { id: 123, ...request }; return JSON.stringify(snapshot());
      }
      if (args[1].includes('/commits/')) return JSON.stringify({ sha: remoteCommit });
      if (args[1].endsWith('/releases')) return JSON.stringify([[...(release && !hideCreatedDraftFromList ? [snapshot()] : []), ...(newer ? [{ tag_name: 'v4.7.8', draft: false }] : [])]]);
      if (args[1].includes('/releases/tags/')) {
        if (!release || release.draft) throw new Error('HTTP 404: tag lookup only returns a published release');
        return JSON.stringify(snapshot());
      }
      if (args[1].endsWith('/releases/123')) return JSON.stringify({ ...snapshot(), ...(changeNotesOnRefresh ? { body: 'All platforms fully tested' } : {}) });
    }
    if (args[0] === 'release') {
      if (args[1] === 'upload') {
        assert.ok(release.draft); assert.ok(!args.includes('--clobber'));
        for (const file of args.slice(3, args.indexOf('--repo'))) { files.set(basename(file), readFileSync(file)); if (failUpload) throw new Error('Upload interrupted'); }
        return '';
      }
      if (args[1] === 'download') {
        const directory = args[args.indexOf('--dir') + 1]; mkdirSync(directory, { recursive: true });
        for (const [name, bytes] of files) writeFileSync(join(directory, name), corruptDownload ? 'wrong bytes' : bytes);
        return '';
      }
      if (args[1] === 'edit') { assert.ok(args.includes('--draft=false')); assert.equal(release.body, releaseNotes(context)); release.draft = false; return ''; }
    }
    throw new Error(`Unexpected mock invocation: ${args.join(' ')}`);
  };
  return { run, calls, files, isPublic: () => release?.draft === false };
}

test('source validation requires the tag to match all six version fields', () => fixture((_directory, _names, root) => {
  mkdirSync(join(root, 'src-tauri'));
  const write = () => {
    writeFileSync(join(root, 'package.json'), '{"version":"4.7.7"}');
    writeFileSync(join(root, 'package-lock.json'), '{"version":"4.7.7","packages":{"":{"version":"4.7.7"}}}');
    writeFileSync(join(root, 'src-tauri/tauri.conf.json'), '{"version":"4.7.7"}');
    writeFileSync(join(root, 'src-tauri/Cargo.toml'), '[package]\nname = "agy-switch"\nversion = "4.7.7"\n');
    writeFileSync(join(root, 'src-tauri/Cargo.lock'), '[[package]]\nname = "agy-switch"\nversion = "4.7.7"\n');
  };
  write(); assert.equal(validateSource(root, context.tag), '4.7.7');
  assert.throws(() => validateSource(root, 'v4.7.6'));
  assert.throws(() => validateSource(root, 'main'));
  for (const file of ['package.json', 'package-lock.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock']) {
    write(); const path = join(root, file); writeFileSync(path, readFileSync(path, 'utf8').replace('4.7.7', '4.7.6'));
    assert.throws(() => validateSource(root, context.tag));
  }
  write(); writeFileSync(join(root, 'package-lock.json'), '{"version":"4.7.7","packages":{"":{"version":"4.7.6"}}}');
  assert.throws(() => validateSource(root, context.tag));
}));

test('all packages, exact checksums and exact generated cask are required before network calls', () => {
  for (const change of ['missing', 'checksum', 'cask', 'extra']) fixture((directory, names) => {
    if (change === 'missing') unlinkSync(join(directory, names[2]));
    if (change === 'checksum') writeFileSync(join(directory, names[1]), 'changed');
    if (change === 'cask') writeFileSync(join(directory, 'agy-switch.rb'), 'sha256 :no_check');
    if (change === 'extra') writeFileSync(join(directory, 'old-package.zip'), 'old');
    const api = fakeGithub(); assert.throws(() => publishRelease(directory, context, api.run)); assert.equal(api.calls.length, 0);
  });
});

test('success stages a draft, checks downloaded bytes, and only then publishes', () => fixture(directory => {
  assert.equal(verifyAssets(directory, context).length, 11);
  const api = fakeGithub(); publishRelease(directory, context, api.run);
  assert.ok(api.isPublic()); assert.equal(api.files.size, 12);
  const manifest = JSON.parse(readFileSync(join(directory, 'release-manifest.json'), 'utf8'));
  assert.equal(manifest.source_commit, context.commit); assert.equal(manifest.files.length, 11);
  const edit = api.calls.findIndex(a => a[1] === 'edit');
  assert.ok(edit > api.calls.findIndex(a => a[1] === 'download'));
  assert.ok(api.calls.some(a => a[1]?.endsWith('/releases/123')));
  assert.ok(!api.calls.some(a => a[1]?.includes('/releases/tags/')));
}));

test('public releases, moved tags and obsolete versions cannot be published', () => fixture(directory => {
  for (const options of [{ existing: { tag_name: context.tag, draft: false } }, { remoteCommit: 'b'.repeat(40) }, { newer: true }]) {
    const api = fakeGithub(options); assert.throws(() => publishRelease(directory, context, api.run));
    assert.ok(!api.calls.some(a => a[0] === 'release' || a.includes('POST')));
  }
}));

test('new drafts publish through their creation response even when release listing stays stale', () => fixture(directory => {
  const api = fakeGithub({ hideCreatedDraftFromList: true });
  publishRelease(directory, context, api.run);
  assert.equal(api.isPublic(), true);
  assert.equal(api.calls.filter(a => a[1]?.endsWith('/releases')).length, 1);
  assert.equal(api.calls.filter(a => a.includes('POST')).length, 1);
}));

test('reviewed release notes disclose native acceptance limits and cannot be silently reused for a new version', () => {
  const notes = releaseNotes(context);
  assert.match(notes, /Linux native Tauri\/WebKitGTK acceptance passed/);
  assert.match(notes, /Windows native GUI acceptance is \*\*blocked\*\*/);
  assert.match(notes, /Final native window.*still need acceptance on a Mac/);
  assert.match(notes, /off by default/);
  assert.doesNotMatch(notes, /App localization is experimental/);
  assert.match(notes, /Build and CLI success must not be interpreted as complete native acceptance/);
  assert.throws(() => releaseNotes({ ...context, tag: 'v999.999.999' }), /ENOENT/);
});

test('release page links resolve from the reviewed source commit rather than the releases URL', () => {
  const notes = releaseNotes({ ...context, tag: 'v4.8.0' });
  assert.ok(notes.includes(`https://github.com/${context.repository}/blob/${context.commit}/docs/screenshots/4.8.0/README.md`));
  assert.ok(notes.includes('https://github.com/anglee0323/agy-switch/actions/runs/37240883985'));
  assert.doesNotMatch(notes, /\]\(\.\.?\//);
});

test('missing or changed acceptance notes leave fresh and resumed drafts unpublished', () => fixture(directory => {
  const missing = fakeGithub({ existing: { tag_name: context.tag, draft: true, body: 'Old incomplete notes' } });
  assert.throws(() => publishRelease(directory, context, missing.run), /acceptance disclosure/);
  assert.ok(!missing.calls.some(a => a[0] === 'release'));
  for (const existing of [undefined, { tag_name: context.tag, draft: true }]) {
    const changed = fakeGithub({ existing, changeNotesOnRefresh: true });
    assert.throws(() => publishRelease(directory, context, changed.run), /acceptance disclosure/);
    assert.equal(changed.isPublic(), false);
    assert.ok(!changed.calls.some(a => a[1] === 'edit'));
  }
}));

test('failed upload or mismatching remote bytes leave the release unpublished', () => fixture(directory => {
  for (const options of [{ failUpload: true }, { corruptDownload: true }]) {
    const api = fakeGithub(options); assert.throws(() => publishRelease(directory, context, api.run));
    assert.equal(api.isPublic(), false); assert.ok(!api.calls.some(a => a[1] === 'edit'));
  }
}));

test('matching partial drafts resume without overwriting assets; conflicting drafts fail', () => fixture((directory, names) => {
  writeManifest(directory, context);
  const name = names[0];
  const matching = fakeGithub({ existing: { tag_name: context.tag, draft: true, files: [[name, readFileSync(join(directory, name))]] } });
  publishRelease(directory, context, matching.run); assert.ok(matching.isPublic());
  assert.ok(!matching.calls.find(a => a[1] === 'upload').includes(join(directory, name)));
  const conflicting = fakeGithub({ existing: { tag_name: context.tag, draft: true, files: [[name, Buffer.from('different')]] } });
  assert.throws(() => publishRelease(directory, context, conflicting.run));
  assert.ok(!conflicting.calls.some(a => ['upload', 'edit'].includes(a[1])));
}));


test('unsigned updater candidates validate for CI but cannot be published', () => fixture(directory => {
  writeFileSync(join(directory, 'Antigravity-Tools-Lite-4.7.7-macos-arm64.app.tar.gz'), 'synthetic updater');
  writeUpdateFeed(directory, '4.7.7', context.repository, true);
  assert.equal(verifyAssets(directory, { ...context, candidate: true }).length, 13);
  const api = fakeGithub();
  assert.throws(() => publishRelease(directory, { ...context, candidate: true }, api.run));
  assert.equal(api.calls.length, 0);
}));

test('the branded candidate validates the complete new package, cask and updater set', () => {
  const root = mkdtempSync(join(tmpdir(), 'agy-brand-release-test-'));
  try {
    const version = '4.9.0', brand = desktopBrand(version), directory = join(root, 'assets'), source = join(root, 'source');
    mkdirSync(directory);
    const bin = join(source, `${brand.app}.app/Contents/MacOS`);
    mkdirSync(bin, { recursive: true }); writeFileSync(join(bin, brand.executable), 'synthetic app');
    const names = packageNames(version), archive = join(directory, names[0]);
    assert.equal(spawnSync('zip', ['-qr', archive, `${brand.app}.app`], { cwd: source }).status, 0);
    for (const name of names.slice(1)) writeFileSync(join(directory, name), 'synthetic package');
    for (const name of names) writeChecksum(join(directory, name));
    writeFileSync(join(directory, 'agy-switch.rb'), renderCask({ archive, version, template,
      url: `https://github.com/${context.repository}/releases/download/v${version}/${names[0]}` }));
    writeFileSync(join(directory, `agy-switch-${version}-macos-arm64.app.tar.gz`), 'synthetic updater');
    writeUpdateFeed(directory, version, context.repository, true);
    const verified = verifyAssets(directory, { ...context, tag: `v${version}`, candidate: true });
    assert.equal(verified.length, 13);
    assert.ok(verified.every(name => name === 'latest.json' || name.startsWith('agy-switch')));
    assert.throws(() => verifyAssets(directory, { ...context, tag: `v${version}`, candidate: false }));
  } finally { rmSync(root, { recursive: true, force: true }); }
});
