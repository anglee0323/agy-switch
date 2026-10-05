# Homebrew distribution

## Status

The Apple Silicon cask at `Casks/agy-switch.rb` installs **AntiGravity Switch 4.9.0** from the public [v4.9.0 release](https://github.com/anglee0323/agy-switch/releases/tag/v4.9.0). It uses this repository as an explicit-URL tap; no separate tap repository is required. The archive contains `agy-switch.app`, and the bundled CLI entry is linked as `agy-switch`.

All seventeen public attachments were downloaded and checked against the source manifest. The three updater signatures passed cryptographic verification. The cask matches the actual Mac ZIP, version and URL; its SHA-256 is `b8799230d6a378a447dbeee654aaf108eff33e696e1378987bd9af243f22b02f`. See [4.9.0 verification](4.9.0-public-acceptance.md).

The Mac package passes strict signature integrity checks but is ad-hoc signed, without Developer ID or notarization. Gatekeeper rejected it with exit 3. Homebrew does not bypass that restriction. A fresh Homebrew recipe load/fetch is checked separately from native installation, launch and upgrade acceptance.

The cask installs the app and management command through Homebrew's documented [`app` and `binary` artifacts](https://docs.brew.sh/Cask-Cookbook#stanza-binary). A separate formula would still carry the current desktop-linked executable, so no lightweight CLI-only formula is claimed. Linux and Intel macOS Homebrew packages are not provided.

Do not install this alongside an old Tools Lite cask: both own the `agy-switch` CLI link and share application data. The old app may still be installed under its original name. Back it up and review conflicts before migration; this guide does not force adoption or overwrite. Historical native Brew installation and launch results remain in [4.8.0 acceptance](4.8.0-public-acceptance.md) and [4.8.1 verification](4.8.1-public-acceptance.md).

## Release inputs and generation

The macOS release job explicitly builds `aarch64-apple-darwin`, verifies the Mach-O architecture, and produces:

- `agy-switch-VERSION-macos-arm64.zip`
- `agy-switch.rb`, generated from the exact ZIP with a real SHA-256 checksum

The ZIP and cask are uploaded together by the existing release workflow. Editing this workflow does not itself trigger a release or create a tap. The workflow refuses a tag/version mismatch. Never replace a published ZIP or its checksum; publish a new version and update the cask through a PR.

For manual generation, set these to a real archive and its intended immutable release URL; no sample release URL is assumed to exist:

```sh
node scripts/generate-homebrew.mjs \
  --archive "$RELEASE_ZIP" \
  --url "$RELEASE_ARCHIVE_URL" \
  --version "$RELEASE_VERSION" \
  --output artifacts/homebrew/Casks/agy-switch.rb
```

The generator validates the filename/version pair, HTTPS URL and bundled executable path. It computes SHA-256 from the file; it does not use `:no_check`, upload anything, fetch the URL, or claim the release is live. The release job separately verifies that the bundled executable is ARM64. The recipe requires Apple Silicon macOS; older OS versions still need separate release acceptance.

## Test a generated cask locally on a Mac

After the exact ZIP is publicly available at the cask URL:

```sh
# A local-only developer tap. This command does not create a GitHub repository.
brew tap-new local/agy-switch
mkdir -p "$(brew --repository local/agy-switch)/Casks"
cp artifacts/homebrew/Casks/agy-switch.rb \
  "$(brew --repository local/agy-switch)/Casks/agy-switch.rb"
brew style --cask local/agy-switch/agy-switch
brew audit --cask local/agy-switch/agy-switch
brew install --cask local/agy-switch/agy-switch
agy-switch --version
agy-switch accounts list --json
brew uninstall --cask local/agy-switch/agy-switch
```

Review any Homebrew trust prompt yourself. `brew uninstall` retains saved accounts and OS credentials. No `zap` stanza deletes them. Test that upgrades preserve account files and that `agy-switch` follows a custom `--appdir`, then publish the verified cask under `Casks/` in an authorized tap. Only after publication should end-user installation instructions name that tap. See Homebrew's [tap maintenance guide](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap).

## Signing and Gatekeeper

The v4.9.0 release uses complete ad-hoc bundle signing. Its ZIP, extracted into a fresh OS temporary directory, passes strict signature verification with empty entitlements. The package was not repaired or re-signed during acceptance. The release workflow does not configure Developer ID signing/notarization and does not disable quarantine/Gatekeeper. Homebrew installation does not remove that limitation. Trusted distribution needs appropriate signing/notarization or explicit user review of the ad-hoc signed app; do not add quarantine-removal commands to the cask. The package remains rejected by Gatekeeper on the tested Mac; a successful archive fetch does not establish a successful Brew launch.

## Publish the cask in this repository

A separate `homebrew-*` repository is optional. Homebrew's [two-argument tap form](https://docs.brew.sh/Taps) supports this existing Git repository. The root `Casks/agy-switch.rb` pins the public v4.9.0 ZIP and its verified SHA-256. The archive contains `agy-switch.app`. A release attachment alone is not a tap entry.

Install from the repository with:

```sh
brew tap anglee0323/agy-switch https://github.com/anglee0323/agy-switch.git
brew install --cask anglee0323/agy-switch/agy-switch
agy-switch --version
agy-switch --help
```

The explicit Git URL matters: the one-argument `brew tap` form would look for a different, `homebrew-`-prefixed repository. The cask's `app` artifact installs the app, and its `binary` artifact links the bundled executable as `$(brew --prefix)/bin/agy-switch`, following a custom `--appdir`. It neither installs nor replaces Google's `agy`.

Installation does not launch the app. If a manually installed app already occupies the destination, keep a backup and review Homebrew's conflict message before proceeding. These commands do not request forced overwrite, app adoption, account-data deletion or removal of platform trust checks.

For later versions, publish the new ZIP first, generate and test the matching cask, then update the same root-level cask through a PR. Users can then run:

```sh
brew update
brew upgrade --cask anglee0323/agy-switch/agy-switch
```

Use the [release checklist](release-checklist.md) to keep the source commit, version, assets and installation evidence aligned.
