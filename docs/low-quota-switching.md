# Smart account switching

This opt-in feature selects a permitted backup when monitored quota reaches the reserve threshold. Its activity observations, client-close attempts and credential checks are separate steps: a live idle observation is not an atomic lock against new work starting. Depending on the target and mode, it may close a client or attempt to interrupt a VS Code task. It does not migrate running work, replay tools or submit continuation messages.

## Setup and the two modes

Open **Settings → Auto Switch Policy**. Choose a model or family scope, the reserve threshold (default 10%), the minimum backup quota (default 30%) and allowed candidate accounts. The settings page separates **Switch timing** (when to switch) from **Account selection order** (which eligible backup to choose). Choose priority order or round robin; drag selected accounts or use their keyboard sorting controls to change the order. Settings save automatically. The feature is off by default; the original quota-protection setting is separate.

- **Wait for verified task inactivity:** Read live trajectory and executor status for every observed App conversation, across all model families. A running executor, background work or an unknown response delays switching, even when a different model family triggered the quota threshold. Confirm idle again immediately before automatically closing the affected client and updating credentials. The macOS App-data Hub is observed separately; an idle Hub no longer blocks the switch merely because its `agy` process exists. Transcript timestamps are not used.
- **Switch at the threshold:** The tool attempts to close the affected client or interrupt a VS Code task before updating credentials. Running work may be interrupted.

The tool records which affected clients were open before closing them, then reopens those clients after updating credentials. On macOS it also restores a previously running App-data Hub using its original launch arguments, retained only in memory. A temporary process observation failure does not erase the relaunch plan. Status distinguishes automatic closing, close failure and successful account switching with a relaunch failure. After completion, verify the signed-in account, open the original conversation from history and continue manually. There is no claim that a running command or model generation has migrated to another account. A ten-percent reserve is a trigger, not a guarantee that a long task can finish within that balance.

**Cancel this switch** suppresses another attempt for that source account until its quota recovers to the configured backup minimum. Saving the settings clears the cancellation. Cancellations survive application restart. A failed or uncertain credential commit pauses automatic attempts until the settings are saved again; inspect both clients first.

The current-source CLI also edits this policy and candidate order through `agy-switch policy` and its Settings & Order menu. It does not run the scheduler itself. See [CLI coverage and package availability](cli.md#policy-ordering-and-updates).

## Eligibility and safety

- Only explicitly selected candidates are considered. Priority picks the first eligible candidate; round robin starts after the current account in that same list, wraps and skips the current account. Disabled, forbidden and validation-blocked accounts are excluded.
- Known provider bucket IDs are used, not translated display labels. The current policy uses the five-hour bucket together with the model percentage; weekly-only FREE accounts use their weekly data. Bucket metadata and reset times must still be valid. Unsupported or missing information blocks switching instead of inventing quota.
- Quota data older than three minutes, invalid/reset-expired data, refresh failures, disabled accounts, and validation-blocked accounts are not eligible. A reset deadline alone is not treated as quota recovery.
- The Rust coordinator runs independently of the visible settings page. Routine per-account refreshes are limited to once a minute; explicit checks retain a ten-second floor. Backup quotas are refreshed only when the source reaches the threshold.
- Config, cancellation, source identity, target eligibility, quota freshness and process observations are revalidated. The credential commit uses the same in-process and cross-process switch locks as the agy-switch CLI.
- An external client can still launch between process observations and the credential write. Process inspection is not a global execution lock. Do not open clients while the status says it is switching. Unknown process/configuration state blocks the operation.
- The default APP credential store and an initialized native agy session are updated together. No generic Gemini CLI files are changed. A missing native session file inside an existing agy directory can be created; an existing file associated with a different account blocks the switch.
- An installed modern APP and a matching current system-keyring identity are required. There is no file-only CLI switch option. On Linux, version discovery reads the installed `resources/app/package.json`; it never launches the APP with `--version` as a fallback. An installation without readable version metadata is reported as unsupported.

## Storage and compatibility

`~/.antigravity_tools/auto_switch.json` stores this feature's settings separately from general UI preferences. `auto_switch_state.json` stores only cancellation/failure state, with no tokens, email addresses, prompts or conversation content. An in-progress credential commit is journaled before writing so that a crash cannot cause an unattended retry. Pending process observations are never trusted across restart.

No task-observation Hooks are installed and no permission settings are changed. Observation reuses existing verified loopback listeners. Live task confirmation currently supports the macOS standalone App and the native `agy --hub --app_data_dir antigravity` server at the canonical CLI installation. The Hub is checked for live activity before it is closed and restored; interactive agy CLI sessions, other Hub data roots, additional IDE/language-server processes, Windows/Linux or unavailable state remain unknown and keep wait mode pending. Threshold mode retains its explicit interruption behavior. Process checks cannot prevent new work from starting immediately after a successful idle observation. Most targets require the detected clients to be closed before credential commit; the VS Code target has a separate interruption/in-place path. Conversation preservation and uninterrupted continuation are not guaranteed.

## Verification

- Rust tests use synthetic quota windows and an isolated subprocess account store. Both modes run the production coordinator and `account::switch_account`, including the actual shared switch lock, injected native-identity check and credential boundary, and real index/last-used updates. Failure cases cover A-cancel → healthy B → still-low A, quota persistence failure during throttling, identity mismatch and a partial native write journal. Smaller guard/file fixtures are labeled separately. No real account or keyring is used.
- UI acceptance tests use synthetic Tauri IPC in Chromium. They cover default-off setup, validation, cancellation, truthful stop guidance, unknown-process status, next-launch completion wording and a narrow viewport. CI uploads screenshots and traces. These are interface tests, not native authentication tests.
- The same Rust feature tests run in macOS, Windows and Linux CI. Full authenticated behavior still depends on the installed Antigravity version and platform credential store; passing build/unit/UI tests is not evidence of every native account-switch scenario.
- Separate manual checks on Antigravity APP 2.19.1 established native Stop persistence and reopening the same conversation, and account B continuing an existing conversation. They did not establish a public programmatic Stop endpoint or an atomic, uninterrupted migration of a running task. Native CLI cancellation/resume remains unverified.

Reference: [Antigravity Hooks](https://www.antigravity.google/docs/hooks/), [CLI cancellation controls](https://www.antigravity.google/docs/cli/prompting/), [CLI conversation resume](https://www.antigravity.google/docs/cli/commands/resume/).
