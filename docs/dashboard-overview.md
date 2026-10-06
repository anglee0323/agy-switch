# Account and quota cards

The Dashboard offers ten optional cards: total, input and output tokens, cache
hit rate, estimated API cost, first-text latency, body output speed, account
status, aggregate quota and quota reset. Settings uses the same saved selection
and drag/keyboard ordering for all ten. Existing selections remain unchanged;
new cards can be enabled individually. Eight or ten selected cards use two rows
on wide screens; nine use three rows. Narrow and short windows wrap and scroll.

The three account cards read the credential-free `get_account_dashboard_snapshot`
and saved reserve policy on opening, every 60 seconds, relevant account events
and manual dashboard refresh. They never refresh remote quotas, authenticate,
switch accounts or inspect the running client's credentials. Countdown updates
every 15 seconds. The usage date selector does not change these observations.

**Account status** counts every indexed account. Available requires fresh,
unambiguous observations for both 5-hour and weekly windows of all families in
the menu's quota scope, with every value above the reserve threshold. Disabled
or restricted accounts, or any known window at or below reserve, are unavailable.
Missing, unreadable, stale, protected or incomplete observations remain unknown
unless an explicit restriction or a known low window already establishes
unavailability. These are quota readiness counts, not live login verification.

**Aggregate quota** reuses the menu's projection, selected family scope,
visibility preference, freshness interval and equal-weight account/family mean.
Each window shows used percentage as 100 minus mean remaining quota. Missing or
stale observations are excluded, while known zero quota is included. Coverage
counts are in the tooltip. The percentages never add up account allowances or
estimate token capacity. Weekly refers to the server-reported reset window, not
a calendar-week token total. No eligible observations produce `—`.

**Quota reset** finds the earliest future reset among valid, partially or fully
used pools in that same scope. Full pools and invalid observations are excluded.
The card shows a countdown, the affected window and distinct account count at
that reset instant. The tooltip includes local date/time. A reset is not a claim
that every account and window recovers simultaneously. Passed reset times become
unknown until a newer observation exists; no recovery time is extrapolated.

Verification uses synthetic accounts and mocked desktop IPC only:

```sh
node scripts/test-dashboard-overview.mjs
node scripts/test-menubar-overview.mjs
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib config::tests
npx playwright test tests/ui/dashboard-overview.spec.ts tests/ui/dashboard-cards.spec.ts
```
