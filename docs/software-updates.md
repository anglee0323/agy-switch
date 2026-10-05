# In-app updates

The app checks the latest stable GitHub release and shows the version of the running executable separately from the available version. Checking for updates never installs anything. Select **Download and install** to download the package, verify its update signature, install it and restart where supported.

Repository commits, PR merges and CI artifacts do not notify installed users.
Several maintenance changes can be combined into one release. Pushing an authorized
new `v*` tag starts packaging; only the verified public stable release becomes
eligible for update checks. Drafts and prereleases are excluded from the stable
check. The current workflow publishes stable releases, not a beta channel.

With startup checks enabled, the main app schedules one check four seconds after
its configuration becomes available. There is no server push or continuous
background update poll. A running app can use the manual check in Settings; a
later startup can discover the new release automatically. Dismissing a version
suppresses that same version on startup, while manual checks can show it again and
a newer version can prompt normally. The CLI's `update check` only reports the
available version. Homebrew checks for new recipes when the user runs `brew update`.

The backend chooses the platform package from the project's release feed. The frontend cannot supply a download URL or installation path. A second update cannot run concurrently. A changed release, invalid signature or failed download stops before installation. Account data remains outside the application bundle.

Update signatures use a project key pinned in the application. The encrypted private key and its password are GitHub Actions secrets; neither is checked into the repository. Pull-request builds receive no signing secrets and their updater feed is explicitly an unsigned candidate. Public release verification checks both the package signature and its authenticated comment before publishing.

## Platform behavior

- **macOS:** the downloaded bundle must pass both strict code-signature integrity and Gatekeeper assessment before replacing the running app. Replacement uses a staging directory on the same volume, with restoration of the previous bundle if the replacement fails. No quarantine attributes or system security settings are changed. Current releases have no Developer ID signing or notarization and therefore cannot pass unattended installation. From 4.9.1, an installed Mac bundle that fails Gatekeeper assessment opens the release page before any package download and explains that manual installation is required. Opening the browser must succeed before the app reports that it opened; a failure is shown separately. The installed version is preserved. When a future trusted release ships, an existing ad-hoc installation needs one final manual upgrade; trusted installations keep the automated path. The downloaded candidate must still pass both integrity and Gatekeeper checks.
- **Windows:** the signed NSIS update package runs in passive mode and relaunches the app. Windows may still display publisher or security prompts because the installer has no Authenticode certificate.
- **Linux deb:** the updater uses the native package installer. Installing a system package may require authentication; it is not a silent privilege escalation. A standalone CLI installation continues to be updated by replacing its console package.

The updater signature is separate from Apple Developer ID or Windows Authenticode signing. A successful download verification does not imply operating-system trust.

Older versions that only open the release page require a one-time installation of a version containing the new updater. They cannot acquire this behavior merely by checking for updates. Homebrew-managed installations may continue using `brew upgrade --cask anglee0323/agy-switch/agy-switch` to keep the Homebrew receipt aligned.

## Repository rename

The 4.9.0 source checks only `anglee0323/agy-switch` and accepts signed packages named `agy-switch-VERSION-PLATFORM`. Repository and filename checks remain strict. Versions through 4.8.1 pin the former repository URL; after the GitHub rename, their version check can reject the canonical new URL. Install the next branded release manually (or through the new Homebrew cask) once it is published. Saved accounts and configuration stay in the existing local directory. No old-name runtime alias is provided.
