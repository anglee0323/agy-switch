// Historical asset names remain immutable. Releases from 4.9.0 use the new brand.
export function desktopBrand(version) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error('Expected a release version');
  const [major, minor, patch] = version.split('.').map(Number);
  return major < 4 || (major === 4 && minor < 9)
    ? { prefix: 'Antigravity-Tools-Lite', app: 'Antigravity Tools Lite', executable: 'antigravity-tools' }
    : { prefix: 'agy-switch', app: major > 4 || minor > 9 || patch >= 1 ? 'AntiGravity Switch' : 'agy-switch', executable: 'agy-switch-desktop' };
}
