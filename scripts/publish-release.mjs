// All packages must validate before any write; publication is the last operation.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { releaseVersion, sha256, writeManifest, packageNames, focusedDownloads } from './release-assets.mjs';
import { desktopBrand } from './release-brand.mjs';

const gh = args => execFileSync('gh', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 16 * 1024 * 1024 });
const newer = (a, b) => { const aa = a.split('.').map(Number), bb = b.split('.').map(Number); for (let i = 0; i < 3; i++) { if (aa[i] !== bb[i]) return aa[i] > bb[i]; } return false; };

export function releaseNotes({ tag, commit, repository }) {
  const version = releaseVersion(tag);
  // Each version needs reviewed notes; do not carry old acceptance claims forward.
  const acceptance = readFileSync(new URL(`../docs/release-notes/${version}.md`, import.meta.url), 'utf8').trim()
    // GitHub Release bodies do not share the Markdown source file's directory.
    .replace(/\]\(([^)\s]+)\)/g, (link, target) => {
      if (/^(?:[a-z][a-z\d+.-]*:|\/\/|#)/i.test(target)) return link;
      const base = `https://github.com/${repository}/blob/${commit}/`;
      return `](${new URL(target.startsWith('/') ? target.slice(1) : target, target.startsWith('/') ? base : `${base}docs/release-notes/`).href})`;
    });
  if (!acceptance) throw new Error('Release acceptance notes are empty');
  const names = packageNames(version);
  const asset = (label, name) => `[${label}](https://github.com/${repository}/releases/download/${tag}/${name})`;
  const compact = focusedDownloads(version);
  const downloads = compact
    ? `| macOS · Apple Silicon | Windows · x64 | Linux · x64 |\n| --- | --- | --- |\n| ${asset('Download ZIP', names[0])} | ${asset('Download installer', names[1])} | ${asset('Download deb', names[2])} |\n\nTerminal packages: ${asset('Windows console ZIP', names[3])} · ${asset('Linux console tarball', names[4])}.\n\nSHA-256 values and source commit are in ${asset('release-manifest.json', 'release-manifest.json')}. The app reads the update feed automatically; the Mac updater tarball is not needed for a manual install.`
    : 'Packages: macOS Apple Silicon ZIP, Windows x64 NSIS installer, Linux amd64 deb, Windows console ZIP and Linux console tarball. Checksums and source manifest are attached.';
  return `${desktopBrand(version).app} ${version}\n\n${downloads}\n\n${acceptance}\n\nSource commit: ${commit}\n\n[Platform verification](https://github.com/${repository}/blob/${commit}/docs/maintainers/native-gui-acceptance.md)\n`;
}

export function publishRelease(directory, context, runGh = gh) {
  const { tag, repository, commit } = context;
  const version = releaseVersion(tag);
  const expected = writeManifest(directory, { ...context, candidate: false }).sort();
  const expectedNotes = releaseNotes(context);
  const assertNotes = release => { if (release.body !== expectedNotes) throw new Error('Release notes differ from the reviewed acceptance disclosure; review the draft before resuming'); };
  const json = args => JSON.parse(runGh(args));
  const assertTag = () => { if (json(['api', `repos/${repository}/commits/${tag}`]).sha !== commit) throw new Error('Remote tag does not match the built source commit'); };
  assertTag();
  const listReleases = () => json(['api', `repos/${repository}/releases`, '--paginate', '--slurp']).flat();
  const findRelease = releases => {
    const matches = releases.filter(r => r.tag_name === tag);
    if (matches.length > 1) throw new Error('Multiple releases use this tag; resolve the ambiguity first');
    return matches[0];
  };
  const releases = listReleases();
  let release = findRelease(releases);
  if (release && !release.draft) throw new Error('Refusing to overwrite an already public release');
  for (const r of releases.filter(r => !r.draft && /^v\d+\.\d+\.\d+$/.test(r.tag_name))) {
    if (newer(releaseVersion(r.tag_name), version)) throw new Error('A newer stable release is already public');
  }
  const scratch = mkdtempSync(join(tmpdir(), 'agy-release-'));
  const verifyDownloaded = (destination, names) => {
    const actual = readdirSync(destination).sort();
    if (JSON.stringify(actual) !== JSON.stringify([...names].sort())) throw new Error('Downloaded release asset set differs');
    for (const name of names) if (sha256(join(destination, name)) !== sha256(join(directory, name))) throw new Error(`Published bytes differ: ${name}`);
  };
  try {
    if (!release) {
      const request = join(scratch, 'release.json');
      writeFileSync(request, JSON.stringify({ tag_name: tag, target_commitish: commit, draft: true,
        name: `${desktopBrand(version).app} ${version}`, body: expectedNotes }));
      // Use the creation response's ID; the release listing can lag behind.
      release = json(['api', '--method', 'POST', `repos/${repository}/releases`, '--input', request]);
    }
    if (!release?.draft) throw new Error('Could not confirm the draft release');
    if (!Number.isSafeInteger(release.id) || release.id <= 0) throw new Error('Draft has no valid release ID');
    assertNotes(release);
    const present = release.assets.map(a => a.name);
    if (new Set(present).size !== present.length || present.some(name => !expected.includes(name))) throw new Error('Draft contains unexpected assets');
    if (present.length) {
      const previous = join(scratch, 'previous');
      runGh(['release', 'download', tag, '--repo', repository, '--dir', previous]);
      verifyDownloaded(previous, present);
    }
    const missing = expected.filter(name => !present.includes(name));
    if (missing.length) runGh(['release', 'upload', tag, ...missing.map(name => join(directory, name)), '--repo', repository]);
    const downloaded = join(scratch, 'downloaded');
    runGh(['release', 'download', tag, '--repo', repository, '--dir', downloaded]);
    verifyDownloaded(downloaded, expected);
    assertTag();
    const latest = json(['api', `repos/${repository}/releases/${release.id}`]);
    if (!latest.draft || latest.tag_name !== tag) throw new Error('Release was published or retagged during verification');
    assertNotes(latest);
    if (JSON.stringify(latest.assets.map(a => a.name).sort()) !== JSON.stringify(expected)) throw new Error('Release asset metadata changed during verification');
    runGh(['release', 'edit', tag, '--repo', repository, '--draft=false', '--latest']);
    const published = json(['api', `repos/${repository}/releases/${release.id}`]);
    if (published.draft || published.tag_name !== tag || JSON.stringify(published.assets.map(a => a.name).sort()) !== JSON.stringify(expected)) throw new Error('Could not confirm the complete public release');
    assertNotes(published);
  } finally { rmSync(scratch, { recursive: true, force: true }); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [directory, tag, commit, repository, ...extra] = process.argv.slice(2);
    if (!directory || !tag || !commit || !repository || extra.length) throw new Error('Usage: publish-release.mjs DIRECTORY TAG COMMIT OWNER/REPOSITORY');
    publishRelease(resolve(directory), { tag, commit, repository });
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
