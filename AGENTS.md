# Project instructions for maintainers and agents

AntiGravity Switch is the application name; `agy-switch` is the repository and
management command. It manages Antigravity accounts through a Tauri desktop app,
macOS native menu, Windows/Linux tray dashboard and a terminal interface.

## Before changing anything

- Read the relevant source, tests and [maintenance guide](docs/maintainers/maintenance-guide.md).
- Read `AGENTS.local.md` if present for local workspace notes. It is ignored by Git.
  Keep machine paths and personal preferences there; `AGENTS.md` contains shared
  project rules only. Credentials do not belong in either file.
- Check the working tree and active PRs. Preserve other contributors' changes.
  Use a focused `codex/` branch or an equivalent contributor branch and a PR to `main`.
- Investigate and implement within the requested scope without repeatedly asking
  for permission. Keep diffs focused and use existing services and conventions.

## Product contracts

- Desktop, CLI and tray must share account data, policy semantics and ordering.
  The CLI edits the desktop scheduler's policy; it does not run a scheduler daemon.
- Keep the application identifier, saved-data location, autostart identity and
  command names stable during routine updates. Identity changes need a migration plan.
- Saved data is outside the app in `~/.antigravity_tools`; `ABV_DATA_DIR` overrides it.
  Use an isolated temporary directory and synthetic accounts for verification.
  Never switch, refresh, delete or export a maintainer's real accounts to test a change.
- A recorded selected account is not proof of the client's live credentials.
  Missing or stale quotas remain unknown; disabled accounts use the shared disabled
  presentation. Aggregated quota is an average, not an additive token balance.
- API costs are estimates from matched prices. Keep GUI, menu and CLI matching
  consistent; unknown model rates stay unpriced.
- User-facing Chinese and English must be complete, natural and concise. Update
  `src/locales/zh.json` and `src/locales/en.json` together; remarks are user input.
- Use shared presentation helpers. Preserve native styling, alignment, readable
  numbers and cached, responsive menu opening. Check desktop and tray variants.
- Never put real credentials, personal test data or screenshot demonstration
  overrides in production artifacts. Do not dump account/config contents into logs.

## Verification and Git

- Follow the impact table in the maintenance guide. Update affected documentation
  and tests together; unchanged sections do not need cosmetic edits.
- Run checks appropriate to the change. For frontend code, build with
  `npm run build`; for Rust, use `cargo check --locked --manifest-path src-tauri/Cargo.toml`
  and scoped tests. Use the existing CLI, UI and package checks where relevant.
- Distinguish source builds, mocked UI, native windows, installed packages and live
  account operations. Report failures and untested behavior accurately.
- Commit coherent steps and push when authorized. Merge only after required checks
  pass. Do not bypass branch protection, force-push, rewrite public tags/assets or
  delete unmerged work to make the repository look tidy.
- Keep temporary previews, test logs and local handoff notes in ignored directories.
  Public maintainer docs should describe durable contracts and reproducible procedures.

## Development is separate from publication

- Ordinary commits, PR merges and CI artifacts do not publish user updates.
  Accumulate maintenance changes on `main`; prepare a release when requested.
- Version bumps belong to release preparation. Keep all six version fields aligned
  and follow the [release checklist](docs/maintainers/release-checklist.md).
- **Pushing a `v*` tag can publish a stable release automatically.** Tag pushes and
  manual release dispatch require an explicit request to publish that version.
  A request to fix, commit or push code alone is not permission to release.
- Before tagging, finish versioned release notes, applicable platform checks and
  the source freeze. Update the pinned Homebrew recipe and current-version guide
  references only after the public archive is available and verified.
- Startup/manual checks read the latest stable GitHub release. Installation needs
  user action; current Mac trust limitations still apply. Do not promise forced,
  real-time or fully unattended updates. See [update behavior](docs/software-updates.md).
