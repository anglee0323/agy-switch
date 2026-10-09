# agy-switch CLI (`agy-switch`)

`agy-switch` manages accounts saved by agy-switch. It is separate from Google's `agy`: it does not run an AI session. The same interactive workflow works on macOS, Windows and Linux, including masked refresh-token entry.

## Install and run

- **macOS:** the [Homebrew cask](homebrew.md) installs `agy-switch`. A manual installation includes the console executable at `"/Applications/AntiGravity Switch.app/Contents/MacOS/agy-switch"`.
- **Windows:** the installer includes console `agy-switch.exe` beside the desktop executable; the release also offers a standalone console ZIP. Open PowerShell in that directory and run `.\agy-switch.exe`. The console executable preserves normal shell waiting, stdout/stderr and `$LASTEXITCODE`; use it instead of scripting the GUI-subsystem executable. [Windows guide](windows.md)
- **Linux:** the deb installs `/usr/bin/agy-switch`; a console tarball is also available. Cached reads and terminal interaction do not require a display, but the executable still needs GTK/WebKitGTK runtime libraries. [Linux guide](linux.md)

For a local build:

```sh
npm ci
npm run build
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin agy-switch
./src-tauri/target/debug/agy-switch
```

Use ↑/↓ to select, Enter/→ to enter and Esc/← to return. Number shortcuts, j/k and q remain supported. Terminal input is restored on exit. Secret entry refuses to proceed if the terminal cannot disable echo. In a pipe, bare `agy-switch` prints help. Launching the original desktop executable without arguments preserves GUI startup.

Policy editing, ordering and update checks described below are available from **4.9.0**. Older v4.8.1 binaries do not include them. Check `agy-switch --version` before using these commands.

## Commands

```sh
agy-switch                      # interactive dashboard on all three platforms
agy-switch stats                # local usage and cached-price estimates
agy-switch stats --json
agy-switch refresh              # refresh all saved accounts over the network
agy-switch refresh user@example.com
agy-switch accounts list
agy-switch current --json
agy-switch quota                       # current account's cached quota
agy-switch quota user@example.com --json
agy-switch switch ACCOUNT_ID
agy-switch switch user@example.com --target app --json
agy-switch switch ACCOUNT_ID --target ide
agy-switch policy show --json
agy-switch policy set --mode wait --strategy priority --reserve 10 --minimum 30 --model gemini
agy-switch policy set --enabled true --candidates ACCOUNT_B ACCOUNT_C
agy-switch policy order ACCOUNT_C ACCOUNT_B --json
agy-switch accounts order ACCOUNT_B ACCOUNT_A ACCOUNT_C --json
agy-switch update check --json
agy-switch experiments show --json
agy-switch experiments translation on
agy-switch experiments translation off
agy-switch experiments run             # keep App translation active in foreground
```

`accounts current`, `accounts quota` and `accounts switch` are also accepted. Selectors are exact account IDs or case-insensitive exact emails; duplicate emails require an ID. There is no fuzzy selection. Bare `agy-switch` opens the dashboard in an interactive terminal on all three platforms.

- `accounts list`, `current`, and `quota` only read local files. They do not initialize the GUI, refresh tokens, query Google, create directories/logs, or repair corrupt indexes
- `current` is agy-switch's recorded selection, not a live check of the APP keyring or `agy` session. Changes made outside agy-switch can make it stale
- `quota` reports cached data and `last_updated` (Unix seconds). Use `agy-switch refresh` when fresh quota is needed. A cache can be stale even when the command succeeds
- `--json` may appear before or after a command. Success goes to stdout; errors go to stderr. All JSON has `schema_version: 1`
- Output uses an explicit field allow-list: no access/refresh/ID tokens, raw OAuth responses, validation URLs, or stored error strings. Treat emails, account IDs, names and quota as personal data when sharing output
- `ABV_DATA_DIR` selects the account-data directory, matching the GUI. If unset, it is `~/.antigravity_tools`. Set it identically for GUI and CLI if you use a custom directory

## Switching and safety

There is no `--target cli` mode. The APP and Google’s `agy` may share a system credential store; writing only a session file cannot guarantee that a new `agy` process uses that account. Use the normal APP+agy synchronization path and verify the active identity in the client. This CLI does not install `agy` or create a missing native CLI data directory. If that directory already exists, normal synchronization can create its first native token file. A valid system token profile may also let `agy` sign in locally without a browser, as described in the [official authentication guide](https://antigravity.google/docs/cli/install/#local-silent-keyring-sign-in).

Switching is an explicit mutating command. It reuses the GUI's token validation/refresh, credential synchronization, process handling and account-index update. The default `--target app` follows the GUI behavior: it may close and restart Antigravity and synchronize an initialized native `agy` session. The CLI requires a discoverable APP for this target and rejects a file-only fallback; the GUI’s existing fallback behavior is unchanged. `--target ide` retains the independent IDE database path.

When the CLI relaunches an APP or IDE, child stdin/stdout/stderr are detached so the command returns promptly and JSON output stays machine-readable. GUI launches preserve their existing I/O behavior.

Save work in Antigravity before switching. Start a new `agy` invocation afterward; a running invocation may retain the previous token. A failure can follow a partial external credential change, so inspect both clients before retrying. CLI errors intentionally omit raw server details; use the GUI for detailed troubleshooting.

GUI and CLI switches in this version share an OS-level lock (`account-switch.lock` inside the data directory). A competing switch fails with exit code 5; the OS releases the lock when the owning process exits. Do not remove the lock file while a switch is running. GUI quota refresh/add/delete operations are not coordinated by this switch-only lock; avoid editing or deleting accounts while a CLI switch is in progress. A CLI switch does not directly refresh an already-open GUI's tray; reopen the account page to read the saved state.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Data, runtime or switch error (switch may be partially applied) |
| 2 | Invalid arguments or ambiguous email |
| 3 | No matching/current account |
| 4 | No cached quota |
| 5 | Another switch is in progress |

Settings/order commands also use code 5 when another client has changed an editor's snapshot. Reload before retrying.

## Policy, ordering and updates

`policy show` reads the same `auto_switch.json` as the desktop app without creating files. `policy set` patches only the supplied fields. Options are `--enabled true|false`, `--mode wait|stop`, `--strategy priority|round-robin`, `--reserve 1..98`, `--minimum` (above reserve, up to 100), `--model all|gemini|claude|MODEL_ID`, `--target app|app-cli|ide|vscode`, `--candidates ID|EMAIL...` and `--clear-candidates`. `claude` selects the Claude/GPT family. Policy target `app` is global sync, `app-cli` is APP plus native agy, and `ide`/`vscode` select their independent backends.

Changing the policy clears its cancellation/failure state. The desktop coordinator from this source revision reloads changed settings on its next five-second tick and invalidates pending selections. A settings write cannot run during a credential commit. The CLI configures this coordinator; it does not start a background scheduler or switch any account when saving settings. Without the desktop app, the saved policy takes effect on the next desktop launch.

`accounts order` without selectors shows the local list order; with selectors it requires every saved account exactly once. `policy order` instead requires every selected candidate exactly once. To change the candidate selection, use `policy set --candidates`. Ordering leaves the current selection, account credentials and other index metadata intact. Duplicate, unknown, ambiguous or stale selections fail instead of being guessed. Keep GUI/CLI account additions and deletions separate from ordering; the switch lock does not serialize every account-management operation across processes.

`update check` contacts this project's public GitHub release metadata with a 12-second timeout. It reports the compiled CLI version, latest stable version, update availability and release link; `--json` adds `schema_version: 1` and `check_only: true`. It does not download, run an installer or open a browser. Use the desktop updater or the original installation method to upgrade.

## Experimental Features

The experimental commands above are available in development builds after 4.10.0. **Experimental Features** is a primary terminal menu; its secondary **Chinese interface** switch shares the desktop setting. `show` reads local saved state without connecting to the App; `on|off` saves only Switch's own flag. Keep Switch desktop running, or use the explicit foreground `experiments run` command for CLI-only use on Windows/macOS. Ctrl+C stops that runner without starting a background service or changing the saved switch. Without another runner, text restores when the last 15-second lease expires.

Translation applies only to the standalone Antigravity App. Runtime injection replaces known UI labels without modifying installation/source files, client permissions or accounts, or uploading conversations; executing a script in App pages is not risk-free. It does not translate `agy`, IDE or other agent tools. See [App translation](app-experiments.md) for compatibility and scope.

## Verification

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib cli::
cargo build --locked --manifest-path src-tauri/Cargo.toml
node scripts/test-cli.mjs ./src-tauri/target/debug/agy-switch
node scripts/test-cli-terminal.mjs ./src-tauri/target/debug/agy-switch
node scripts/test-cli-launch.mjs
node --test scripts/test-homebrew-generator.mjs
```

The launch regression test compiles the production process-launch functions with a synthetic configuration and harmless child executable; it covers manual/auto-detected launches, JSON output and timely pipe EOF without credentials.

The separate `Release CLI` workflow builds actual optimized Windows and Linux executables and runs the same synthetic-data smoke test. Windows also runs `scripts/test-windows-cli.ps1`, which checks the GUI PE subsystem, `Start-Process -Wait` and `WaitForExit()` completion/exit codes, and redirected JSON success/errors. Inherited-I/O checks establish completion and exit codes; console text visibility and a real account switch still need interactive acceptance. Debug smoke alone does not establish release shell behavior.

The smoke test uses a temporary data directory and synthetic tokens. It does not call `switch`, log in, contact Google or modify real accounts. Real credential-store switches and Homebrew installation require platform testing before a release is advertised as verified.

## Interactive workflow and coverage

The main menu groups Accounts & Quotas, Statistics, Refresh, Add Account, Status, Settings & Order, Check for Updates, and Experimental Features. Use Up/Down or Tab/Shift+Tab to select, Enter/Right to open or confirm, and Esc/Left/0 to return or cancel. Home/End jump to the first/last item; numbers and j/k remain shortcuts. Enter on an account opens its action menu; `S` explicitly switches it, while `V`, `R`, `T` and `X` retain their detail/label/toggle/delete shortcuts. Text fields support Left/Right, Home/End, Delete and Backspace; Esc cancels. Secret fields stay masked while editing.

Settings separate **Switch timing** from **Account selection order**. Use Space to select backup candidates and Shift+Up/Down (or U/D) to reorder them. Enter applies the list to the policy draft; **Save changes** persists that draft. Back discards it, and **Reload settings** reads another client's changes. Account-list sorting has the same keys, with Enter saving and Esc discarding. Refresh and authorization use the network; switching can change credentials and restart clients. Read-only JSON commands are suitable for scripts.

Cost estimates share model matching with the dashboard and native menu. Native `-n` aliases are supported; `gemini-3.8-flash-exp-a` uses `gemini-3.8-flash` prices for estimation. Other versions and variants require their own matching rates. Unknown model prices display `Unpriced`; aggregate amounts contain only priced models and have no parenthetical suffix. The desktop dashboard fetches public pricing when its 24-hour cache expires or the parser revision changes. CLI reads use that cache without network requests. Missing quota windows are unknown, never inferred as 100%. API-equivalent costs are estimates, not the subscription bill.

Policy editing, candidate ordering, account ordering and update checks share the same operations between terminal menus and one-line commands. Visual themes, menu layout and interactive charts remain GUI features.

## CLI scope

Account management, cached quotas, refresh, switching, local usage, policy configuration, ordering and update checks are terminal workflows. Appearance, desktop startup, update installation and the background smart-switch scheduler remain desktop features. Linux terminal-only users can inspect/add/refresh accounts without a display; APP switching still requires an installed APP and its credential backend. No file-only agy switch or background CLI daemon is provided.
