# Screenshot sources

These images come from the integrated **CI test build**, not an already published release. Source and checkout commit: [`6896ce61ea25ac402a12db0098e631b0fe45462d`](https://github.com/anglee0323/agy-switch/commit/6896ce61ea25ac402a12db0098e631b0fe45462d), package version 4.7.7. Later documentation-only changes do not change this capture provenance.

- `linux-*.png`: real Tauri/WebKitGTK viewports from the [native Linux artifact](https://github.com/anglee0323/agy-switch/actions/runs/36930452189/artifacts/11195728431), debug executable, English UI, synthetic accounts and empty local usage. The system window frame is excluded. Settings images show a 1024 × 700 viewport and require scrolling for content below it; they are not full-page captures. All six native captures in the source artifact passed real IPC, theme, viewport and cleanup checks; four are selected here.
- `menu-*-preview-*.png`: 380 × 480 Chromium component previews from the [browser artifact](https://github.com/anglee0323/agy-switch/actions/runs/36930452189/artifacts/11195373720), using synthetic IPC and account/quota data. These are **not native macOS/Windows menu screenshots** and do not verify desktop blur, Dock, positioning, focus or login startup.
- Windows native GUI acceptance at this head was blocked before app launch and produced no screenshots. No new native macOS GUI result is claimed. See [native acceptance](../../native-gui-acceptance.md) and [menu-bar limits](../../menu-bar-dashboard.md).

Every selected PNG was visually inspected and copied unchanged. Dimensions and SHA-256 values are recorded in [provenance.json](provenance.json). No screenshot logs in, imports credentials, or changes a live account.
