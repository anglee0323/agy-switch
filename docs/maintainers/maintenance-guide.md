# Maintenance guide

This guide applies to human maintainers and coding agents. The root [AGENTS.md](../../AGENTS.md)
is the short entry point; this document explains which project surfaces need to stay
aligned. Release execution is covered by the [release checklist](release-checklist.md).

## Where instructions belong

Public project rules belong in the repository: product contracts, source locations,
verification commands and release boundaries help every contributor. They contain
no personal machine details or credentials. Keep them focused; do not turn them into
an agent's conversation history or a collection of abandoned design drafts.

Optional `AGENTS.local.md` at the repository root is ignored by Git. Use it for local
installation paths, preview conventions or personal working preferences. The root
instructions tell agents to read it when present; do not assume every agent discovers
arbitrarily named files automatically. Secrets still belong in OS credential stores
or the appropriate CI secret store, never in either instruction document.

## Development and releases

Several maintenance PRs can be merged before one release. The number of commits is
not a release trigger. Batch related fixes and improvements; publish sooner for a
fix users urgently need. Documentation-only maintenance usually needs no app release.

| Action | Repository effect | Existing users |
| --- | --- | --- |
| Commit or push a feature branch | Saves work and runs applicable CI | No update notice |
| Merge a PR into `main` | Advances the development source and runs CI | No update notice |
| Build a PR package | Produces validation artifacts; its updater feed is an unsigned candidate | No stable update notice |
| Push a new `vMAJOR.MINOR.PATCH` tag | Starts three-platform packaging and verified publication | No notice until a complete stable release is public |
| Publish the verified stable release as latest | Makes its version and signed feed available | Eligible apps discover it on their next update check |

The tag is a publication trigger, not a harmless backup marker. The workflow
publishes after all package jobs and complete-set verification pass; it does not
pause for a second human approval before publication. Manual dispatch can rebuild
an existing version tag and also publish, subject to the same checks. Public
versions and assets must never be overwritten.

The app checks after startup when enabled, or when the user selects the manual
check. It does not receive a server push or install merely because `main` changed.
A dismissed version is suppressed for startup checks but remains available through
manual checks. The next newer stable version can prompt again. CLI update checks
report availability without downloading or installing.

## Change impact table

Use the relevant rows, not the whole table for every small patch.

| Change | Source surfaces to review | Documentation and verification |
| --- | --- | --- |
| Account identity, switching or quotas | Rust account/service/integration modules, CLI projection, account store, menu projection | Account data contract; isolated account/CLI checks; native client checks when possible |
| Smart switching or candidate order | `src-tauri/src/modules/auto_switch.rs`, Rust config models, `src-tauri/src/cli/`, Settings and both locales | GUI/CLI options, saved values and ordering must agree; policy and keyboard checks; README/CLI guide when behavior changes |
| Model pricing or usage statistics | Rust pricing/statistics modules, `src/utils/modelPricing.ts`, dashboard, menu and CLI output | Shared `tests/fixtures/model-pricing-aliases.json` cases in both TS/Rust; catalog precedence, version/variant boundaries and unpriced behavior; local-record/scan times; usage guide when semantics change |
| Desktop or tray UI | `src/pages/`, `src/components/`, shared quota helpers; `native_menu.rs` for macOS and `MenuBarDashboard.tsx` for the web dashboard | Both locales, hover/keyboard/layout checks; native platform limits; replace README screenshots only when materially outdated |
| CLI commands or terminal controls | `src-tauri/src/cli/`, common Rust services, console packaging | `--help`, both terminal languages, JSON/exit codes, `docs/cli.md`; CLI and PTY/ConPTY checks |
| App name, logo or identity | `branding/`, `scripts/release-brand.mjs`, asset generator, Tauri metadata, `Info.plist`, tray and installer assets | GUI/Dock/tray names, both READMEs and install paths; branding/bundle tests; review data/autostart migration before identity changes |
| Installation, updating or packaging | Platform scripts, `.github/workflows/`, updater module/store/dialog, Homebrew generator/template | Platform/update guides, release notes, real archive hashes/signatures and relevant installer checks |
| Public release | Six version fields, versioned release notes, frozen source tag; verified assets; then `Casks/agy-switch.rb` | Both README current-version/install references, platform guides and current acceptance index; record the exact release source and actual package results |
| Instructions or documentation | `AGENTS.md`, maintainer guides or affected user guides | Verify referenced paths/commands and rendered screenshot layout; no version bump or release solely for these changes |
| Removing legacy code | Active Rust/TS consumers, registered commands, migration entry points and platform-only modules | Confirm the replacement path still supports saved data; run frontend build, Rust checks/tests and relevant CLI/platform checks |

## Release preparation and follow-up

1. Consolidate the selected changes in a new version's notes. Choose an unused
   patch version for compatible fixes, or a minor version for compatible features;
   review breaking changes separately. Do not invent a release number before
   checking existing tags/releases.
2. Align the versions in `package.json`, both root entries in `package-lock.json`,
   `src-tauri/Cargo.toml`, the root package in `src-tauri/Cargo.lock`, and
   `src-tauri/tauri.conf.json`. Prepare both-language user-facing changes and
   installation instructions. Current-release references and the Brew checksum
   must continue to name the actually published package until the new one is verified.
3. Finish the applicable checks and merge the release preparation into `main`.
   Record the exact commit. Publish only when the current task authorizes that
   version, using the existing workflow and checklist.
4. Download the public assets and verify the manifest, update signatures, platform
   architecture and installed-entry behavior. Do not claim a fresh native test by
   copying an earlier version's acceptance result.
5. Generate the Brew cask from the actual public Mac ZIP. Verify its checksum,
   app/CLI paths and real archive fetch. Update both README version references,
   platform guides and the maintainer acceptance index in a small follow-up PR.
   Keep the verified published source tag immutable.

Current releases have no Apple Developer ID/notarization or Windows Authenticode
certificate. A complete resource signature or updater signature does not establish
OS trust. Keep those limitations visible until the actual final package passes the
appropriate trust checks. Never fix distribution by removing quarantine, weakening
OS protections or re-signing a downloaded package locally.

## Verification and handoff

For a requested local replacement, record the source commit and any uncommitted
changes, build the final package and verify its platform identity, integrity and
CLI entry before replacing it. Close Switch, retain its external saved data and
compare preference/account hashes without copying credentials into test artifacts.
Reopen the installed app, verify its actual running path, visible behavior, CLI links
and existing autostart executable path. Multiple registered copies can launch an
older bundle; preserve the startup identity, arguments and enabled preference when
correcting a stale path. Distinguish
pre-launch account preservation from ordinary background updates after launch.
Use synthetic storage for test operations; do not stop another client's tasks to
test a Switch update. Follow local notes for installation paths and backup preferences.

Development builds can share the latest release's version metadata. Record and
compare the actual source/package payload; a matching `--version` string alone
does not prove that replacement occurred. Local replacement is separate from
publishing an update and does not change the pinned public package or checksum.

A local replacement can use an exact-source CI package after its checks pass;
record the workflow, source commit, archive hash and installed payload identity.
Do not describe a downloaded CI package as a local source build or a public release.

When cleanup is authorized, finish checks and installation before removing owned
temporary downloads and project-local outputs. `cargo clean --manifest-path
src-tauri/Cargo.toml` removes the repository's Rust build outputs; `node_modules/`,
`dist/`, `.vite/`, `src-tauri/gen/`, `test-results/` and `playwright-report/` are
regenerable local dependencies or outputs. Check for another contributor's active
build before cleaning them. Retain source and lockfiles, installed applications,
external account/config storage and shared global Cargo/npm caches. The next
build restores frontend dependencies with `npm ci` and regenerates its outputs.

Read `.github/workflows/ci.yml` for current check names and commands. Use Node.js 22+
and Rust stable. Common entry points are:

```sh
npm run build
cargo check --locked --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib modules::config::tests
node scripts/test-cli.mjs PATH_TO_BUILT_CONSOLE
node scripts/test-cli-terminal.mjs PATH_TO_BUILT_CONSOLE
node scripts/test-model-pricing.mjs
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib menu_bar_usage::tests
npx playwright test tests/ui/updater.spec.ts
node --test scripts/test-release.mjs scripts/test-update-assets.mjs scripts/test-homebrew-generator.mjs
```

Choose commands matching the changed behavior; the console path depends on the
platform and build profile. Tests must use isolated synthetic data and keep real
credential stores and clients untouched. Passing mocked UI or a hosted debug window
does not prove live authorization, installed-package trust or physical tray behavior.

For handoff, report the branch/PR and tested commit, actual checks and limitations,
whether anything was published, and any remaining work. Keep local logs and preview
files out of commits. Save durable acceptance evidence under `docs/maintainers/`
and user-facing release notes under `docs/release-notes/`.
