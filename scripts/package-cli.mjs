// Package an already tested console executable. Never downloads or publishes.
import { copyFileSync, mkdirSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { releaseVersion, writeChecksum } from './release-assets.mjs';
const [binary, directory, tag] = process.argv.slice(2);
const version = releaseVersion(tag);
const windows = process.platform === 'win32';
if (!['linux', 'win32'].includes(process.platform)) throw new Error('Build CLI packages on their native Windows/Linux runner');
const root = mkdtempSync(join(tmpdir(), 'agy-cli-package-'));
const output = resolve(directory); mkdirSync(output, { recursive: true });
const executable = windows ? 'agy-switch.exe' : 'agy-switch';
const archive = join(output, `agy-switch-${version}-${windows ? 'windows-x64.zip' : 'linux-amd64.tar.gz'}`);
try {
  copyFileSync(resolve(binary), join(root, executable));
  copyFileSync('LICENSE', join(root, 'LICENSE'));
  copyFileSync('docs/cli.md', join(root, 'CLI.md'));
  writeFileSync(join(root, 'README.txt'), `agy-switch ${version}\n\nRun ${windows ? '.\\agy-switch.exe' : './agy-switch'} for the interactive dashboard.\nRun ${windows ? '.\\agy-switch.exe' : './agy-switch'} --help for script commands.\nAccount data is shared with the desktop app at ~/.antigravity_tools.\n${windows ? 'Windows x64 console executable; no desktop window is started.' : 'Linux x64, Ubuntu 22.04 baseline. Requires GTK3, WebKitGTK 4.1 and libayatana-appindicator3 runtime libraries even without a display. This is not a static/musl binary.'}\nThe CLI does not run the background smart-switch scheduler.\nDocumentation: https://github.com/anglee0323/agy-switch/blob/main/docs/cli.md\n`);
  if (windows) {
    // Paths originate from the controlled runner, quoted as PowerShell literals.
    const quote = s => "'" + s.replaceAll("'", "''") + "'";
    execFileSync('pwsh', ['-NoProfile', '-NonInteractive', '-Command', `Compress-Archive -Path ${quote(join(root, '*'))} -DestinationPath ${quote(archive)}`]);
  } else execFileSync('tar', ['-czf', archive, '-C', root, executable, 'LICENSE', 'CLI.md', 'README.txt']);
  writeChecksum(archive);
  console.log(archive);
} finally { rmSync(root, { recursive: true, force: true }); }
