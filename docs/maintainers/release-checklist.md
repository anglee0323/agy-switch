# Release and Homebrew checklist

This is a release procedure, not evidence that a new version has been published.

Ordinary commits and merges into `main` do not publish updates. Accumulate
maintenance changes, then prepare one release when requested. A pushed `v*` tag
starts packaging and automatic publication after verification; it is a publication
action. Manual dispatch of an existing tag can also publish. Neither route waits
for an additional human approval inside the workflow. Follow the
[maintenance guide](maintenance-guide.md) before triggering either route.

Each version must have reviewed acceptance notes in `docs/release-notes/VERSION.md` before tagging. The publisher includes them when creating the draft and reads back the exact body before any asset upload, immediately before publication, and after publication. A resumed draft with missing or changed notes fails without becoming public; review its body before resuming. Keep passed, blocked and untested native behavior distinct from package/CLI checks. Never publish first and add these limits afterward.

1. **Freeze the source.** Merge all intended feature and acceptance PRs into `main`. Recheck remote tags/releases and choose an unused version. Keep `package.json`, both root versions in `package-lock.json`, `src-tauri/Cargo.toml`, the root package in `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json` identical. Run all applicable CI against the final version commit. Record that exact SHA; do not release from a previous feature branch.
2. **Create the new tag at that SHA.** A `v*` tag push starts the Release workflow for all three platforms. Manual dispatch accepts only an existing version tag, with no feature-branch override or partial-platform switches. The workflow validates every version field, requires the tag commit to be in `main`, and pins all jobs to that SHA. It publishes only after all three package jobs pass, the complete asset set is staged as a draft, and downloaded asset bytes match the originals. Do not reuse an existing public version or overwrite its assets.
3. **Verify the actual release packages.** Download each asset from the newly published release, record its SHA-256 and inspect its architecture/version. macOS must contain an arm64 `.app` in the ZIP; run the bundled CLI and inspect the deployment target. Windows must contain the x64 release executable/NSIS installer; test installation, explicit CLI waiting and JSON exit codes. Linux must be an amd64 `.deb`; inspect package architecture/dependencies and install on the supported distribution. GUI launch and account-data preservation need native package acceptance; CI synthetic-data checks alone do not prove them.
4. **Verify Homebrew on an Apple Silicon Mac.** Generate the cask from the downloaded public ZIP and verify its SHA-256 against `release-manifest.json`. The `Homebrew recipe` workflow loads the proposed recipe, fetches the actual public ZIP and compares a newly generated recipe. Follow the [Homebrew installation guide](../homebrew.md) for native acceptance. Check `agy-switch --version`, `--help`, JSON reads with an isolated synthetic data directory, the `bin` symlink, a custom `--appdir`, and uninstall/upgrade preservation of saved accounts. Do not change real credentials to test packaging. Record trust/Gatekeeper behavior separately; recipe loading/fetching alone does not prove installation or launch.
5. **Publish the tap entry and instructions.** Open a small PR with the verified generated file at `Casks/agy-switch.rb` and the English/Chinese README installation commands. Keep the source ZIP URL and checksum unchanged. Merge after review, and state explicitly if native installation acceptance is still pending. Verify the two-argument tap and fully qualified install from a fresh tap checkout before claiming successful native installation, upgrade or uninstall tests.

## Expected release assets

| Platform | Current workflow output | Architecture evidence |
| --- | --- | --- |
| macOS | `agy-switch-VERSION-macos-arm64.zip`, `.sha256` and generated `agy-switch.rb` | Explicit `aarch64-apple-darwin` build and `lipo -archs` assertion |
| Windows | `agy-switch-VERSION-windows-x64-setup.exe` and `.sha256` | x64 APP payload PE assertion; NSIS installer may itself be a 32-bit bootstrapper |
| Linux | `agy-switch-VERSION-linux-amd64.deb` and `.sha256` | x64 Ubuntu runner; verify `dpkg-deb -f PACKAGE Architecture` |

There is no DMG, Intel macOS Homebrew cask, Linux Homebrew formula or standalone server CLI in this workflow. A ZIP is a supported Homebrew cask source and includes the app's own `agy-switch` entry point.

The release also includes `release-manifest.json` recording the source commit and hashes of all public payloads and the update feed. All platform builds pass `--locked`. Package generation and synthetic CLI checks occur before the publishing job receives repository write permission.

PRs that change release inputs run the same three package jobs and local asset verification without creating a tag or release. The publish job is explicitly disabled for PR events. Linux smoke runs the executable extracted from the actual `.deb`; the macOS job checks `CFBundleExecutable` against the cask path before packaging. It explicitly asks Tauri for a complete ad-hoc bundle signature (`APPLE_SIGNING_IDENTITY=-`). Before executing CLI smoke tests, and again after extracting the final ZIP, `scripts/verify-macos-bundle.mjs` requires the expected bundle ID/version/executable, arm64 architecture, nonempty icon and resource seal, and successful `codesign --verify --deep --strict`. Any failure stops the package job before upload/publication. The Mac regression tests create disposable Mach-O fixtures and never launch them.

## Signing and source checks

- No Developer ID/notarization credentials or Windows signing configuration are referenced by the current workflow. This does not establish whether unrelated repository secrets exist. Report the actual package signature/trust result; do not describe a successful build or an ad-hoc signature as a notarized release.
- Homebrew does not solve missing platform trust. Do not add quarantine-removal commands, change security settings or create signing credentials as part of package testing. See [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/) and [Windows signing](https://v2.tauri.app/distribute/sign/windows/).
- Public releases are never overwritten. A matching partial draft can resume by adding missing assets; existing assets must first download and match byte-for-byte. Conflicting drafts fail without being published. Package artifacts expire after seven days, so a later retry may require a complete rebuild and a new version if the rebuilt bytes differ.
- A failed final confirmation can occur after GitHub accepted publication. Read the release state before retrying; do not assume a reported failure means nothing was published. The publisher refuses a retry against an already public release.

## README text after publication and installation acceptance

English:

> On Apple Silicon macOS, Homebrew installs both AntiGravity Switch and its `agy-switch` management command. It does not install Google's `agy`. The current packages are not Developer ID signed/notarized; see the release's platform-trust notes.

简体中文：

> Apple Silicon Mac 可以通过 Homebrew 同时安装 AntiGravity Switch 和管理命令 `agy-switch`，不会安装或替代 Google 的 `agy`。当前安装包没有 Developer ID 签名或公证，请查看该版本的系统信任说明。

```sh
brew tap anglee0323/agy-switch https://github.com/anglee0323/agy-switch.git
brew install --cask anglee0323/agy-switch/agy-switch
agy-switch --version
agy-switch --help
```

Publish these commands together with the real root-level cask and its current acceptance limits. Do not describe pending native installation as tested. Update the signing wording only if the actual final release is signed/notarized.

## macOS signature integrity is separate from platform trust

The 4.7.7 Mac ZIP shipped only a linker-generated ad-hoc executable signature without a complete bundle resource seal. Its checksum can match while macOS rejects the bundle as damaged. Do not repair an installed/downloaded copy or remove quarantine as a release fix. Build a new patch release with the corrected signing step and verify the actual ZIP after extraction. Preserve existing public tags/assets and regenerate the new checksum, manifest and cask through the release workflow.

A complete ad-hoc signature verifies bundle integrity but does not identify a developer, provide Apple notarization, or establish Gatekeeper acceptance. Normal trusted distribution requires an authorized Developer ID Application signing identity, the associated private key made available securely to the signing runner, and Apple notarization/stapling. Those credentials are not created or configured by this fix. See [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/).

The direct ZIP/Homebrew distribution uses an empty entitlements dictionary. App Sandbox is incompatible with its existing configuration/session files, LaunchAgent registration and external-client process integration; the signed payload must not enable it. Regression fixtures sign with the actual production entitlements, and the final bundle verifier rejects sandbox or any unexpected signed entitlement. This build configuration change does not modify macOS Gatekeeper or grant OS privacy permissions.

## Windows and Linux console packaging

From 4.9.1, the release exposes five user payloads (Mac ZIP, Windows installer, Linux deb, Windows console ZIP and Linux console tarball), the Mac updater tarball, `latest.json` and `release-manifest.json`. Checksums, detached signatures and the generated cask are still validated in CI, but are not separate public attachments. Windows compiles both executables before applying the bundle-only resource config; the installer fixture checks the installed console program. Linux checks both executables extracted from the deb and packages that same console payload. Actual terminal tests run through PTY/ConPTY. Native GUI acceptance uses its opt-in debug feature only; the release workflow never enables it.
