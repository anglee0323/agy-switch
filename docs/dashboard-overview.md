# Account and quota cards

The Dashboard offers ten optional cards: total, input and output tokens, cache
hit rate, estimated API cost, first-text latency, body output speed, account
status, aggregate quota and mean request input. Settings uses the same saved selection
and drag/keyboard ordering for all ten. Existing selections remain unchanged;
new cards can be enabled individually. Eight or ten selected cards use two rows
on wide screens; nine use three rows. All cards share equal widths and heights
within the grid, including across rows. The former quota-reset card maps to mean
request input in its saved position; hidden and empty selections stay hidden.
Narrow and short windows wrap and scroll.
The model-detail section retains enough height for its table instead of
collapsing when the card grid occupies additional rows.

The two account cards read the credential-free `get_account_dashboard_snapshot`
and saved reserve policy on opening, every 60 seconds, relevant account events
and manual dashboard refresh. They never refresh remote quotas, authenticate,
switch accounts or inspect the running client's credentials. The local clock
updates every 15 seconds to reevaluate freshness. The usage date selector does
not change these observations.

**Account status** counts every indexed account. Available requires fresh,
unambiguous observations for both 5-hour and weekly windows of all families in
the menu's quota scope, with every value above the reserve threshold. Disabled
or restricted accounts, or any known window at or below reserve, are unavailable.
Missing, unreadable, stale, protected or incomplete observations remain unknown
unless an explicit restriction or a known low window already establishes
unavailability. These are quota readiness counts, not live login verification.

**Aggregate quota** reuses the menu's projection, selected family scope,
visibility preference, freshness interval and equal-weight account/family mean.
Each window shows used percentage and a progress bar filled to 100 minus mean
remaining quota. Bar colors follow the menu's remaining-quota thresholds. Missing
or stale observations are excluded, while known zero quota is included. Coverage
counts and family scope are in the tooltip; the card contains only the two quota
windows, without an additional scope footer. The percentages never add up account
allowances or estimate token capacity. Weekly refers to the server-reported reset window, not
a calendar-week token total. No eligible observations produce `—` and a neutral
hatched track, rather than a zero-valued meter.

**Mean request input** divides pooled input and cached input tokens by request
count in the selected usage date range, including system prompts and all models.
It rounds to the nearest token and includes cache because those tokens still form
part of request input. It does not average per-model averages. No requests or
invalid totals produce `—`; a valid zero-token request produces zero. This is an
average request size, not an individual context length, model context limit or
proof of the cause of response latency. It uses the existing local usage scan and
does not require an account snapshot or a new benchmark request.

Verification uses synthetic accounts and mocked desktop IPC only:

```sh
node scripts/test-dashboard-overview.mjs
node scripts/test-menubar-overview.mjs
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib config::tests
npx playwright test tests/ui/dashboard-overview.spec.ts tests/ui/dashboard-cards.spec.ts
```
