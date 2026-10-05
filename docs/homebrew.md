# Homebrew distribution

## Status

The repository and cask token are now `agy-switch`. Brand assets and app name in 4.9.0 are source-only until the next release. The renamed recipe on this development branch still installs the immutable 4.8.1 bundle; it does not claim to install new artwork. Do not install it alongside an existing old-name cask: both own the same app and CLI path. Updating main and the public recipe waits for the next authorized delivery.

This repository contains the **macOS Apple Silicon cask** at `Casks/agy-switch.rb`, backed by the public [v4.8.1 release](https://github.com/anglee0323/agy-switch/releases/tag/v4.8.1). It uses this repository as an explicit-URL tap; no separate tap repository is required.

All seventeen v4.8.1 public assets were downloaded and checked against the source manifest; the three updater signatures also passed cryptographic verification. The Mac ZIP contains an ARM64 executable and bundle version 4.8.1. The renamed root cask keeps the same archive and ZIP SHA-256 `e36a70b759c2ecbff6d3863dcc7a4a5aa4673295c7e92313b3538f196e141dc0`. See [4.8.1 verification](4.8.1-public-acceptance.md).

**Historical v4.8.0 native acceptance:** Homebrew upgraded v4.7.9 to v4.8.0 in `/Applications`, correctly linked `agy-switch`, and preserved twelve existing account/configuration JSON files byte-for-byte. The installed bundle passed strict signature verification. Gatekeeper rejected the app (exit 3); the quarantined Brew CLI timed out after ten seconds. This is an installation/upgrade pass, **not a launch pass**. The separately downloaded public ZIP passed 14 isolated CLI read checks and 13 real PTY checks through its `agy-switch` entry point. Uninstall and authenticated switching remain separate. No trust checks or quarantine attributes were removed. See [public-package acceptance](4.8.0-public-acceptance.md). The earlier custom-app-directory check remains recorded in [4.7.9 acceptance](release-notes/4.7.9-public-acceptance.md).

The cask installs both `Antigravity Tools Lite.app` and the management command `agy-switch`. It uses Homebrew's documented [`app` and `binary` artifacts](https://docs.brew.sh/Cask-Cookbook#stanza-binary). A separate formula would still carry the current desktop-linked executable, so no lightweight CLI-only formula is claimed here. Linux and Intel macOS Homebrew packages are not provided; use the existing Linux packages or build from source.

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

The v4.8.0 release uses complete ad-hoc bundle signing. The public ZIP extracted into a fresh OS temporary directory and the Brew app installed in `/Applications` pass strict signature verification with empty entitlements. Neither has been repaired or re-signed during acceptance. The release workflow does not configure Developer ID signing/notarization and does not disable quarantine/Gatekeeper. Homebrew installation does not remove that limitation. Trusted distribution needs appropriate signing/notarization or explicit user review of the ad-hoc signed app; do not add quarantine-removal commands to the cask. The actual Brew launch remains blocked on the tested Mac.

## Publish the cask in this repository

A separate `homebrew-*` repository is optional. Homebrew's [two-argument tap form](https://docs.brew.sh/Taps) supports this existing Git repository. The root `Casks/agy-switch.rb` uses the new cask token while pinning the existing v4.8.1 ZIP and its real SHA-256. The archive still contains the old app name. A release attachment alone is not a tap entry.

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
