# Read-only account dashboard data contract

This batch adds `get_account_dashboard_snapshot` without changing the menu UI, release version, OAuth, keychain access, account activation or automatic switching/protection policy. The existing `list_accounts` remains compatible.

## Counts and identity

`indexed_total` is the account-index row count, `loaded_count` counts successfully parsed matching account files, and `failed_count` counts retained failed rows. These counts partition the index, rather than silently skipping unreadable files. A missing/corrupt index is an error, not a fabricated empty account list. Each failed row retains its index identity and a bounded error code. The reader does not create directories, recover indexes, heal JSON, write files, refresh credentials or invoke the account service. Its DTO does not deserialize or return tokens, validation URLs or raw error contents.

`current_account_id` is explicitly sourced from `tools_record`. It does not identify the live client session. A recorded ID can refer to an unreadable or removed account and remains a record, not evidence of activation.

The separate current-account synchronization command checks the running official macOS App through its existing `GetUserStatus` RPC before consulting shared credentials. It validates the standalone package identity and matching package/plist versions, then verifies the App executable, language-server child and loopback listener ownership. It does not pin identity reads to a specific release number: compatibility is established by the verified `GetUserStatus` response. It keeps the process CSRF value in memory and returns only the status email. An older agy process can refresh a different account into the shared keyring while the App retains its own session; this must not overwrite the App's recorded identity. A running App whose identity cannot be verified is an error, never a keyring fallback. With the App closed, synchronization matches the exact observed credential against saved accounts and imports only that state when necessary. Bulk local import also selects current identity by observation instead of the first imported account. Both operations share the account-switch lock. They do not switch or restart the App, rewrite its credentials, or turn this read-only snapshot into a live query. Other platforms retain credential-based synchronization.

Automatic switching also checks the running App identity against the recorded source before attempting to close the client. It revalidates the source credentials, index, configuration and pending request under the shared switch lock at that boundary. A mismatched source blocks the request without closing the App or writing credentials. Isolated coordinator fixtures cover this behavior in both wait and stop modes; they never operate on native client processes.

## Compatibility and unknown quota

Existing policy-facing `percentage: i32` and bucket fraction fields retain their types and numeric behavior. New optional `percentage_known` / `remaining_fraction_known` markers distinguish valid explicit API fractions (including zero) from missing/null/out-of-range values. Models also retain the valid original `observed_remaining_fraction` for read-only projection, including the raw group bucket fraction after existing fusion. Policy truncation/rounding stays unchanged; the DTO derives precise percentages from that fraction rather than the policy integer. A zero integer without the raw observation remains unknown even if an earlier draft marker says known, since it may have lost positive sub-one-percent precision. These markers are written only as part of the existing refresh/cache lifecycle; this change performs no migration or immediate production write. The read-only DTO outputs absent/invalid observations as JSON null. Legacy fields remain for old consumers, so legacy views/policies are not retroactively rewritten.

An old cache lacks provenance: earlier parsers substituted zero for missing data. Its zero is conservatively unknown until a subsequent successful refresh explicitly reports a value; positive in-range legacy observations remain visible with `legacy_cache` provenance. This may temporarily classify a genuinely exhausted legacy account as unknown. There is no safe way to recover lost provenance without refreshing. Old binaries may drop new optional metadata on save; the reader then reverts to this conservative legacy behavior.

## Pools and status

Group buckets are authoritative observations. Models associated by the existing fusion rule retain the actual selected bucket ID; this association is explicitly **inferred**, not an API-provided pool mapping. No pool identity is constructed from a group name or its reported window set. Model names/display names do not prove pool identity. Models with an unambiguous inferred bucket owner become labels on that reported group. Bucket/window observations deduplicate across all groups in one account, including full/partial group records; matching duplicates occur once and value/reset conflicts are explicitly marked unknown. Presentation row keys are local response indexes, not stable API pool IDs. Equal group names with distinct buckets are never merged. Ambiguous or missing bucket associations remain unmapped; independent windows stay separate. Unmapped models remain visible separately, and mixed/incomplete group coverage is partial. The current summary schema is weekly + 5h; either missing window or any extra unrecognized window label has unverified coverage and is partial. This is conservative coverage validation, not a claim that other API window types cannot exist. No summed percentage, absolute quota, total token usage or all-account usage percent is exposed.

Authentication (`disabled`, `verification_required`, `not_verified`, `unknown`), quota (`reported`, `exhausted`, `partial`, `unknown`, `forbidden`) and freshness are independent observations. A successful saved read/old quota response does not authenticate the account now. `reported` does not imply switchability; consumers must also check fresh data, reset expiry, local protection and authentication restrictions. Local token statistics have no account attribution and are absent from this contract.

## Background quota freshness

Automatic full-account quota refresh runs on the native Rust runtime, independently
of main-window visibility or WebView timers. Hidden WebKit pages can throttle or
suspend JavaScript work ([WebKit power behavior](https://webkit.org/blog/8970/how-web-content-can-affect-power-usage/));
the frontend only saves refresh preferences and observes the existing refresh event.
The native loop reads saved settings every five seconds. Enabling refresh or changing
its interval starts a new round; disabling it or setting a nonpositive interval stops
new rounds. A started request may finish after the preference changes.

Each period reserves 10% of the configured interval, capped at 30 seconds, for API
latency before the presentation freshness cutoff. Resume from sleep starts at most
one round, never a catch-up burst. Overlapping automatic/manual full-account requests
join the same in-flight batch; a completed result is not reused for a later request.
Partial/network failures retain their original observation timestamps and remain
unknown when stale. Display and account-switch policy freshness rules are unchanged;
menu opening reads local observations without triggering credential refresh.

Synthetic regression checks cover one hour of native refresh with all five account
rows and aggregate coverage, preference changes/opt-out/resume, overlapping manual
and automatic requests, retry after failure, and absence of frontend quota timers.
These checks perform no real account authorization, switching or quota requests.

## Validation boundaries

Synthetic fixtures cover missing/null values versus real zero, legacy cache provenance, shared pool mappings, conflicting duplicates, partial group coverage, separate status axes, missing/corrupt account files, an unreadable recorded-current entry and a missing/corrupt index. Fixtures never load the user's saved accounts. Rust tests also verify that trailing JSON and indexes remain byte-identical, and serialized responses contain no token fields. GUI and real-account refresh acceptance belong to later batches.
