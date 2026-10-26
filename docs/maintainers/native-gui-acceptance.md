# Native Windows and Linux UI acceptance

This lane runs the existing CI debug executable, with its embedded frontend and real
Rust IPC, in a fresh GitHub-hosted VM. It does not build another release, add test
plugins to the application, mock IPC, log in, import accounts, or connect to a user
computer. `scripts/test-native-gui.mjs --self-test` validates the fixture, child
process environment allowlist, and blank-image rejection without starting a GUI.
Actual GUI execution is intentionally refused outside a GitHub-hosted runner.

## Evidence and scope

The separate `desktop-lifecycle-{OS}-{source SHA}` lane uses the production
desktop runtime, an owned tray and the actual secondary WebView window. On
Windows/Linux, `scripts/test-desktop-lifecycle.mjs` requires a fresh hosted runner
and isolated empty account/config files. It checks background startup, prewarm,
window reuse, monitor bounds, click-toggle semantics, close-to-hide, focus-loss
dismissal, main-window recovery and exit without a tray. Linux runs with Openbox
inside Xvfb and a private D-Bus session. Reports require all 16 stages across the
with-tray and without-tray runs; incomplete or failed runs cannot pass. It does
not assert physical icon clicks, taskbar presentation, Wayland behavior or
authenticated switching. The fixture access point is excluded from release
builds and requires the existing `native-gui-test` feature.

Tray opening runs in a blocking task because Windows WebView creation must not
block the event loop. Monitor lookup and native-handle conversion both run on
the UI thread before returning a plain snapshot. Application-level getters
and window-getter conversion can call GDK on their caller, so neither belongs
in that worker. Repeated native opens cover this boundary. Failed Linux fixtures retain the original
report and capture a separate native exit stack for diagnosis, never a substitute
pass. Windows fixture cleanup waits for the WebView2 processes associated with
its unique data directory to exit, then retries file removal briefly. The host's
exit alone does not release that directory; see Microsoft's
[user-data folder guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder).
Startup checkpoints and elapsed time are retained on failure. A fresh Windows
profile has a 60-second process budget for WebView2 initialization and the entire
lifecycle sequence (Linux uses 30 seconds); this is not a startup-performance
benchmark. Every native assertion and clean exit remains required.

macOS runs its separate AppKit fixture with 16 native Dock/window stages,
including process exit after closing the main window without a usable tray.
It also verifies that the main frame is opaque, page requests remain hidden until
acknowledged, and stale replies or replies after closing cannot reveal it. These
fixture acknowledgements are synthetic; React navigation ordering is checked by
`tests/ui/window-presentation.spec.ts`. Neither suite measures time to first pixels.
Its isolated configuration, native policy observations and full output are
uploaded alongside the platform reports; installed-package verification remains
separate.

The `native-gui-{OS}-{source SHA}` artifact contains `acceptance.json`, bounded
driver diagnostics, and these screenshots:

- `dashboard-light.png`: real Dashboard and empty local usage state
- `accounts-light.png`: two synthetic accounts loaded through the actual backend
- `settings-light.png` / `settings-dark.png`: low-quota Settings controls and themes
- `settings-light-760.png` / `settings-dark-760.png`: exactly 760 CSS-pixel viewport
- `quick-dashboard-light.png`: 424 CSS-pixel compact dashboard in the native WebView

Each image is a capture of the app's actual native WebView viewport, not the system
window frame or the runner desktop. Linux uses WebKitGTK in Xvfb with a new D-Bus
session. Windows uses the installed Microsoft WebView2 Runtime and a matching
Microsoft Edge WebDriver. WebDriver starts the exact app executable; its unique PID
and executable path are checked before screenshots. There is no browser-mode
fallback in this lane. Missing drivers, failed launch, blank/transparent images,
missing controls, layout overflow, or failure to exit/clean up fail acceptance. A narrowly detected incompatible Windows
environment is explicitly reported as `status: blocked`, `passed: false`, with no
app launch or screenshots; its compatibility step emits a warning and lets build
and unit tests continue. This exception is not a native GUI pass. Unexpected
errors still fail CI.

The report records the source and checked-out SHA, executable SHA-256, OS, driver
versions, session capabilities, app PID, exact viewport, PNG SHA-256 and pixel
variation, each assertion, and cleanup result. `passed: true` is required. A green
build alone, a created PNG alone, or renderer-only Playwright tests do not establish
native UI acceptance. Inspect the pixels before selecting README artwork.

Suggested README caption, including under each platform's section:

> Windows (or Linux) native WebView capture, CI test build at `<checkout SHA>`.
> Synthetic example data; system window frame excluded. This is not a login,
> real-account switch, installer, tray, signing, or native authentication test.

Do not use an image from another platform under that platform's heading. If native
capture is unavailable, say so; any separately generated component preview must
be visibly labelled as an illustrative preview, not a desktop test result.

## Safety boundary

- Only a fresh `github-hosted` VM is supported. No self-hosted runner or user desktop
- Accounts have `.invalid` addresses and **empty** access/refresh tokens
- `current_account_id` is null; auto-switch, auto-refresh, auto-sync, and quota
  protection are explicitly off. No test turns them on or calls login, switch,
  refresh, import, process-stop, or autostart commands
- A fresh synthetic local pricing cache avoids the Dashboard's public price fetch
- `ABV_DATA_DIR`, HOME, USERPROFILE, APPDATA, LOCALAPPDATA, XDG directories, WebView
  storage, and temporary files point to test directories. Child environments use
  an allowlist and never inherit runner secrets or cloud credentials
- The app tray is disabled for this test, so closing its WebDriver session exits it
- The Tauri/native drivers listen only on loopback. Xvfb disables TCP listening
- Only the exact test-created process tree is terminated; screenshots never
  capture the whole desktop. Cleanup is checked and included in the report

### Windows Known Folders limitation

The production dependency `dirs` 5 uses `SHGetKnownFolderPath` on Windows. Changing
HOME/USERPROFILE/APPDATA does **not** redirect these paths. Therefore the isolated
fresh CI VM is an additional required boundary. Before launching, the test asserts
that these exact Known Folder application directories do not exist, and rejects
symlinks/junctions in their ancestor paths:

- UserProfile: `.gemini`, `.antigravity_tools`
- ApplicationData and LocalApplicationData: `com.lbjlaq.antigravity-tools-lite`

If any exists, the test fails without reading or deleting it. Only after all checks
pass does the test create and record these exact directories. The pricing cache is
synthetic; `.gemini` remains empty. Cleanup removes only directories created by this
test. There is no registry modification, user-account creation, security override,
or production path-resolution change. This approach must never be used on a user
computer. It is not a claim that environment variables sandbox Windows Known Folders.

## Official references

- [Tauri native WebDriver CI](https://v2.tauri.app/develop/tests/webdriver/ci/)
- [Tauri manual native driver setup](https://v2.tauri.app/develop/tests/webdriver/manual-setup/)
- [Microsoft WebView2 WebDriver testing](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webdriver)
- [GitHub Windows runner image](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-Readme.md)

## Current test setup

Windows CI builds the opt-in `native-gui-test` feature. Only debug builds with that feature and the synthetic fixture marker pass the validated driver port through the WebView2 API. Release builds cannot enable this path. Both Windows and Linux now pass native-window acceptance on hosted runners. Package installation and real-account switching are separate checks.

## Integrated 4.9.0 test build

The [final main Build run](https://github.com/anglee0323/agy-switch/actions/runs/37320154989) checked out `e1c3b80de1ed3e7043c3dc73a7d1e56259e8a5bd`, the v4.9.0 release source. Both Windows and Linux reports record `status: passed`, `passed: true`, seven native captures, real Rust IPC, persisted themes, 760px Settings layouts, a 424px quick dashboard and completed cleanup. The selected twin-wave app artwork, full navigation name and separate switch timing/account ordering groups are present. Three unchanged, visually reviewed images are in [4.9.0 screenshot sources](../screenshots/4.9.0/README.md).

The [release CLI run](https://github.com/anglee0323/agy-switch/actions/runs/37320155019) also passed Windows/Linux console, PowerShell and real PTY/ConPTY checks, including policy editing and ordering. These results use isolated synthetic data and do not establish live Google authorization or authenticated account switching. Public package verification is recorded separately after publication.

## Integrated 4.9.1 test build

The [final main Build](https://github.com/anglee0323/agy-switch/actions/runs/37356704254) checked out `a9b5bee75f4d0feaa4a181dc8b4a7ba73710cc0f`, the v4.9.1 release source. Windows and Linux each record `status: passed`, `passed: true`, seven native captures with no overflow, real Rust IPC and completed cleanup. The compact dashboard uses the full AntiGravity Switch name. These are hosted debug viewport tests, separate from actual package installation and live account switching. Its public-package results are in [4.9.1 verification](4.9.1-public-acceptance.md).

## Integrated 4.10.9 release

The final main Build and optimized CLI checks passed at `a4b521dc4dcfdd3d8d384dc79146b7a7fd568d75`.
Windows and Linux native reports each contain seven SHA-verified captures, real
Rust IPC and completed cleanup; representative final Dashboard/narrow Settings
captures were inspected. The macOS AppKit lifecycle lane passed.
See [4.10.9 public verification](4.10.9-public-acceptance.md) for the
exact workflows, public byte/signature checks and live-account limits.

## Integrated 4.10.10 release

Source `11d1b1d0e4be2ddf59ea57d8bf42213386e14c67` passed the three-platform source, native lifecycle
and package checks. Both Windows/Linux reports have seven hash-verified captures
and completed cleanup. The public Mac package was independently verified and
installed with saved-data preservation. See [4.10.10 public verification](4.10.10-public-acceptance.md)
for exact hashes, workflows, window regressions and remaining native trust/account limits.

## Integrated 4.10.11 release

Source `f8e084f9c480ede711d57b52c63f1b9b745c5c56` passed three-platform source/lifecycle checks and optimized CLI checks. Windows and Linux native reports each contain seven hash-verified captures and completed fixture cleanup; representative layouts were inspected. The actual public Mac ZIP was verified and installed with preserved account index, preferences and startup identity. See [4.10.11 public verification](4.10.11-public-acceptance.md) for exact workflows, package hashes and account/trust limits.
