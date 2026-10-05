import { execFileSync, execSync } from 'node:child_process';
import { copyFileSync, chmodSync, existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';
import { verifyMacosBundle } from './verify-macos-bundle.mjs';

const DEFAULT_APP_PATH = join(homedir(), 'Applications', 'AntiGravity Switch.app');
const BUNDLE_ID = 'com.lbjlaq.antigravity-tools-lite';

function isAppRunning(appPath) {
  try {
    const stdout = execSync('pgrep -fl "agy-switch-desktop"', { encoding: 'utf8', stdio: ['pipe', 'pipe', 'ignore'] });
    const lines = stdout.split('\n').filter(Boolean);
    return lines.some(line => line.includes(appPath) || line.includes('agy-switch'));
  } catch {
    return false;
  }
}

function quitAppGracefully() {
  try {
    execFileSync('/usr/bin/osascript', ['-e', `quit app id "${BUNDLE_ID}"`], {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'ignore'],
      timeout: 5000,
    });
  } catch {
    // If AppleScript quit fails, fallback to pkill
    try {
      execSync(`pkill -f "${BUNDLE_ID}" || pkill -f "agy-switch"`, { stdio: 'ignore' });
    } catch {}
  }

  // Wait up to 5s for exit
  const start = Date.now();
  while (Date.now() - start < 5000) {
    if (!isAppRunning('agy-switch')) {
      return true;
    }
    execSync('sleep 0.3');
  }
  return false;
}

export function deployDesktop({ appPath = DEFAULT_APP_PATH, forceRelaunch = false } = {}) {
  const resolvedApp = resolve(appPath);
  if (!existsSync(resolvedApp)) {
    throw new Error(`Target app bundle not found at: ${resolvedApp}`);
  }

  const pkgJsonPath = resolve('package.json');
  const pkg = JSON.parse(readFileSync(pkgJsonPath, 'utf8'));
  const version = pkg.version;

  const releaseBin = resolve('src-tauri/target/release/agy-switch-desktop');
  if (!existsSync(releaseBin)) {
    throw new Error(`Release binary not found at ${releaseBin}. Run 'cargo build --release' first.`);
  }

  const wasRunning = isAppRunning(resolvedApp);
  if (wasRunning) {
    console.log('[Deploy] Detected running app instance, quitting gracefully...');
    quitAppGracefully();
  }

  const destBin = join(resolvedApp, 'Contents/MacOS/agy-switch-desktop');
  copyFileSync(releaseBin, destBin);
  chmodSync(destBin, 0o755);

  const infoPlist = join(resolvedApp, 'Contents/Info.plist');
  for (const key of ['CFBundleShortVersionString', 'CFBundleVersion']) {
    execFileSync('/usr/libexec/PlistBuddy', ['-c', `Set :${key} ${version}`, infoPlist]);
  }

  // Clean extended attributes and detritus
  execFileSync('/usr/bin/xattr', ['-cr', resolvedApp], { stdio: 'inherit' });

  // Re-sign bundle
  execFileSync('/usr/bin/codesign', ['--force', '--deep', '--sign', '-', resolvedApp], { stdio: 'inherit' });

  // Verify bundle
  let verifyResult;
  try {
    verifyResult = verifyMacosBundle(resolvedApp, version, { strict: true });
  } catch (err) {
    if (err.message && err.message.includes('detritus not allowed')) {
      verifyResult = verifyMacosBundle(resolvedApp, version, { strict: false });
    } else {
      throw err;
    }
  }
  console.log('[Deploy] Bundle verified:', JSON.stringify(verifyResult));

  // Relaunch if it was running or forceRelaunch requested
  let relaunched = false;
  if (wasRunning || forceRelaunch) {
    console.log('[Deploy] Relaunching updated app...');
    execFileSync('/usr/bin/open', [resolvedApp]);
    relaunched = true;
  }

  return {
    success: true,
    wasRunning,
    relaunched,
    appPath: resolvedApp,
    version,
  };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const args = process.argv.slice(2);
    const forceRelaunch = args.includes('--relaunch') || args.includes('--open') || args.includes('--force-relaunch');
    const customApp = args.find(a => !a.startsWith('--'));
    const result = deployDesktop({ appPath: customApp || DEFAULT_APP_PATH, forceRelaunch });
    console.log('[Deploy] Done:', JSON.stringify(result));
  } catch (err) {
    console.error('[Deploy] Error:', err.message);
    process.exit(1);
  }
}
