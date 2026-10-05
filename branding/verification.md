# Brand change verification

Source version: **4.9.0**, development branch `codex/cli-policy-workflows`. The repository was renamed to `anglee0323/agy-switch`; its repository ID remained `1371076232`. No release, tag, app installation or production bundle was created for this change.

Verified on macOS on 2026-10-05:

- `npm run build` passed. Vite still reports its existing chunk-size warning.
- `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib`: **136 passed, 2 intentionally ignored** credential fixtures. Those isolated Linux credentials fixtures were not run on this Mac.
- `cargo build --locked --manifest-path src-tauri/Cargo.toml --bins` passed in the debug profile. Cargo exposes `agy-switch` and `agy-switch-desktop`, retaining the internal library name.
- `npx playwright test`: **31 passed**, with synthetic IPC. The menu/header and logo were inspected in the resulting browser images; these are not native installed-app screenshots.
- Both actual Mac debug executables passed **27 CLI smoke checks each**. The console executable passed **51 real PTY checks** including both UI languages, policy editing, ordering, masking, resize and cancellation. Tests use synthetic data; they do not switch real credentials.
- An actual CLI update check contacted the new GitHub repository, returned public latest `v4.8.1` for source version `4.9.0`, and created no account directory or download. The JSON payload is under `update`; `check_only` is top-level.
- Branding, cask generation, release inputs, update signatures and disposable macOS signature fixtures: **25 tests passed**. The new candidate set uses the new app/executable/package paths. Unsigned candidates cannot be published.
- All **67 existing icon resources** regenerate byte-for-byte from the SVG master. ICNS entry order is canonicalized; macOS `sips` successfully decoded the resulting icon for visual inspection. PNG dimensions, matching frontend artwork, ICO and ICNS headers passed.
- The renamed cask was regenerated from the real public 4.8.1 ZIP and matched `Casks/agy-switch.rb`. It keeps the original bundle/executable paths and exact ZIP checksum until a new release.
- All **17 public 4.8.1 assets** still match their recorded GitHub digests and local source manifest after the repository rename.
- Existing quota, dashboard, pricing, settings, desktop preference and CLI launch-isolation checks passed. `git diff --check`, shell syntax checks and source version validation passed.

Windows/Linux package names, icons, window titles and test/build paths are updated in source. This brand revision has not been packaged or run natively on Windows/Linux. OS signing, Gatekeeper, installer upgrade behavior and actual account switching still require their documented delivery acceptance. Historical release acceptance remains historical.

The former updater URL was pinned in versions through 4.8.1. Those versions may reject GitHub's canonical renamed release URL; the first branded upgrade requires a manual installation or the new Homebrew recipe after publication. Runtime URL/signature validation is not relaxed. Account/configuration directories and bundle identity remain stable.
