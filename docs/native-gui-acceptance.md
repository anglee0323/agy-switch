# Native Windows and Linux UI acceptance

This lane runs the existing CI debug executable, with its embedded frontend and real
Rust IPC, in a fresh GitHub-hosted VM. It does not build another release, add test
plugins to the application, mock IPC, log in, import accounts, or connect to a user
computer. `scripts/test-native-gui.mjs --self-test` validates the fixture, child
process environment allowlist, and blank-image rejection without starting a GUI.
Actual GUI execution is intentionally refused outside a GitHub-hosted runner.

## Evidence and scope

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

## First hosted run: known limitations (2026-10-01)

The initial Windows run used matching WebView2/EdgeDriver 153.0.4234.48 and completed
isolation/cleanup, but native session creation timed out. WebView2 150+ deliberately
ignores environment-based debugging overrides for elevated host processes; see
[Microsoft's explanation](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5645)
and the [upstream testing issue](https://github.com/webdriverio/desktop-mobile/issues/542).
The locked Wry 0.54.1/hosted administrator combination remains blocked. A read-only
elevation/runtime/version preflight now records that exact combination as blocked
before creating any Windows application directories or starting the app. This is not a
Windows GUI pass. This lane does not change registry policies, weaken security, or
modify the production dependency to force a pass.

The initial Linux run reached a real app PID and real IPC, but rejected a two-color
first capture. Screenshots now wait for fonts and painted frames, then validate
pixels for a bounded rendering window. An image still rejected is retained with a
`.rejected.png` suffix for diagnosis only; pixel thresholds remain unchanged.

The [second hosted run](https://github.com/anglee0323/agy-switch/actions/runs/36922441718)
at `d0003780894a75abf3fee83f5d46af5808ed9e6e` passed all six Linux native captures,
real IPC, theme persistence, 760px layout checks, app exit and cleanup. All six images
were visually reviewed. Waiting for real paint resolved the capture race; no
software-renderer workaround or production graphics setting change was needed.
Windows native GUI remains blocked, separately from passing release CLI checks.

## Integrated 4.7.7 test build

The [integrated Build run](https://github.com/anglee0323/agy-switch/actions/runs/36930452189) checked out `6896ce61ea25ac402a12db0098e631b0fe45462d`, including all product changes and version 4.7.7. Its Linux report records `passed: true`, six visually reviewed native Tauri/WebKitGTK captures, successful real IPC/theme/viewport checks, and completed app exit/cleanup. Normal viewports are 1024 × 700; narrow Settings viewports are 760 × 900, not full-page images.

The Windows report at the same source/checkout SHA records `status: blocked`, `passed: false`, no app launch and no screenshots. macOS package/CLI checks passed, but no native Mac GUI acceptance is implied. Selected unchanged images and browser-preview labels are preserved with hashes in the [screenshot sources](screenshots/4.7.7/README.md).

## Integrated 4.7.9 test build

The [final Build run](https://github.com/anglee0323/agy-switch/actions/runs/37233892799) checked out `5be961d3149be5f80bdd4b39b51b4a4020dea4cd`. Linux passed all six native Tauri/WebKitGTK viewport captures, real IPC, persisted themes, narrow layouts and cleanup. A route-mount race was fixed by waiting for a visible, enabled element before native clicks, retaining the original assertions and failure timeout. Four unchanged, visually inspected captures are in [4.7.9 screenshot sources](screenshots/4.7.9/README.md).

Windows still records `status: blocked`, no app launch and no screenshots, independently of passing package and one-line CLI checks. Installed-package, real-account and Mac login/multiple-display acceptance remain separate.

## Integrated 4.8.0 test build

The [4.8.0 Build run](https://github.com/anglee0323/agy-switch/actions/runs/37240883998) checked out `8f14803e9a7ceeb8293b3a9f75739eb3e8ed7de2`. Both Windows and Linux reports now record `status: passed`, `passed: true`, seven actual native captures, real Rust IPC, persisted themes, 760px Settings layouts and completed app/fixture cleanup. The compact route also checks all three dashboard sections, English copy, synthetic per-family quota percentages, single-line window labels and action/percentage alignment. Its viewport is 424 × 720; this is not an actual tray-window placement or opening-speed test. Three unchanged, visually reviewed captures are in [4.8.0 screenshot sources](screenshots/4.8.0/README.md).

Windows CI explicitly builds the opt-in `native-gui-test` feature. Only Windows debug builds with that feature and the fixture marker forward the driver's validated numeric port through Tauri's `additional_browser_args` WebView2 API. Release builds cannot enable this path. This resolves the hosted elevated-process automation limitation without registry changes, runtime downgrades or security-policy changes. The older blocked reports remain valid for their recorded source; they are not current Windows UI results.

Native GUI acceptance remains independent of package installation, real Google authorization, authenticated switching, login startup, physical monitors, Wayland/KDE and platform signing. Console acceptance separately uses real PTY/ConPTY, direct PowerShell invocations and JSON/exit-code checks.

## Integrated 4.9.0 test build

The [final main Build run](https://github.com/anglee0323/agy-switch/actions/runs/37320154989) checked out `e1c3b80de1ed3e7043c3dc73a7d1e56259e8a5bd`, the v4.9.0 release source. Both Windows and Linux reports record `status: passed`, `passed: true`, seven native captures, real Rust IPC, persisted themes, 760px Settings layouts, a 424px quick dashboard and completed cleanup. The selected twin-wave app artwork, full navigation name and separate switch timing/account ordering groups are present. Three unchanged, visually reviewed images are in [4.9.0 screenshot sources](screenshots/4.9.0/README.md).

The [release CLI run](https://github.com/anglee0323/agy-switch/actions/runs/37320155019) also passed Windows/Linux console, PowerShell and real PTY/ConPTY checks, including policy editing and ordering. These results use isolated synthetic data and do not establish live Google authorization or authenticated account switching. Public package verification is recorded separately after publication.
