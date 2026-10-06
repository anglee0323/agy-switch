# macOS installation and upgrades

AntiGravity Switch supports Apple Silicon Macs. The Homebrew cask installs the app and the `agy-switch` management command from the verified [4.9.2 release](https://github.com/anglee0323/agy-switch/releases/tag/v4.9.2). It does not install Google's `agy` client. Intel Mac and Linux Homebrew packages are not provided.

## Install with Homebrew

```sh
brew tap anglee0323/agy-switch https://github.com/anglee0323/agy-switch.git
brew install --cask anglee0323/agy-switch/agy-switch
agy-switch --version
```

The explicit repository URL is required because this project also serves as the tap. Homebrew installs `AntiGravity Switch.app` and links its management entry as `agy-switch`, including when a custom `--appdir` is used. The application is not launched during installation.

If a manually installed copy already occupies the destination, keep a backup and review Homebrew's conflict message before proceeding. The cask does not force adoption or overwrite another installation.

## Install from the ZIP

Download `agy-switch-4.9.2-macos-arm64.zip` from the release page, unzip it, and move **AntiGravity Switch.app** into Applications. The archive also includes a console executable:

```sh
"/Applications/AntiGravity Switch.app/Contents/MacOS/agy-switch"
```

This opens the terminal dashboard. Homebrew makes the same operations available through the shorter `agy-switch` command. See the [CLI guide](cli.md).

## Upgrade

```sh
brew update
brew upgrade --cask anglee0323/agy-switch/agy-switch
```

For a manual installation, check for updates in the app or download the new ZIP. From 4.9.1, the app opens the release page before downloading when the current Mac installation fails system trust checks. Install the new ZIP manually in that case. Developer ID signing and notarization are not configured, so fully unattended installation is unavailable; the installed copy is preserved. Homebrew users should use `brew upgrade` to keep the receipt aligned with the installed version.

Saved accounts and preferences live outside the application bundle in `~/.antigravity_tools`. Ordinary upgrades and cask removal retain them; the cask has no `zap` rule that deletes account data.

## Verification and system trust

The root recipe at `Casks/agy-switch.rb` pins the exact public ZIP and its SHA-256. Package hashes are listed in `release-manifest.json`; the update feed includes cryptographic package signatures. The Mac app has a complete ad-hoc resource signature but no Apple Developer ID or notarization. Gatekeeper can reject launch, and Homebrew does not bypass these checks.

See [4.9.2 package verification](maintainers/4.9.2-public-acceptance.md) and [in-app updates](software-updates.md). Generating and publishing the cask is covered by the [maintainer release checklist](maintainers/release-checklist.md).
