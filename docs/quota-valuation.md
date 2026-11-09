# Quota value and token equivalents

Account cards, the account table and the cycle-change dialog can show what a
reported quota is worth at API prices: the value already consumed, the value that
remains, and the token volume those values correspond to. The terminal dashboard
keeps reporting percentages; valuation is a desktop display aid, not a policy input.

## What the numbers mean

A quota fraction is relative headroom, not a purchased balance. The valuation is a
model of commercial value built from three facts the app actually has: the reported
fraction, the local generation records attributed to that account, and the matched
model prices used by the dashboard. It is an estimate, never a bill.

- **Prices** come from the same snapshot the dashboard uses. A model without a
  single unambiguous rate stays unpriced; its requests are counted and excluded
  rather than guessed.
- **Attribution** uses recorded active spans. A machine that switched accounts
  splits its local records across those spans instead of crediting one account with
  everything.
- **Pools stay separate.** Gemini, Claude/GPT and Gemini 4 Argon are valued on their
  own buckets. A shared pool is never blended into a dedicated one.
- **Small consumption is not extrapolated.** Below 3% of a window the total is not
  inferred from usage, so a single request cannot inflate the estimate.
- **Full windows need a baseline.** When nothing has been consumed, the 100% value
  comes from a user-defined reference cap, a calibrated median from earlier cycles,
  or a built-in per-plan reference; the row badge says which one was used.

Each bucket reports its provenance: `observed` (measured consumption),
`used_only` (consumption without an extrapolated total), `calibrated`,
`manual_reference`, `exhausted`, or `none`. A disabled, forbidden or stale account
shows no valuation, and a missing observation stays unknown.

## Settings

Settings → Quota & Models → model display offers:

| Setting | Effect |
| --- | --- |
| Enable quota value | Shows or hides the valuation line; the percentage display is unchanged |
| Currency | USD, CNY, or both side by side |
| Display mode | Remaining value, or consumed value against the estimated total |
| USD → CNY rate | Auto-refresh once per day from a public rate feed, or a fixed user value |
| Reference caps | A 100% baseline per pool and window (Gemini, Claude/GPT, Argon), replacing the calibrated median or the built-in per-plan reference |

Changing these settings only changes presentation and the reference baseline; it
never changes switching policy, thresholds or saved account data.

## Cycle changes

The **Cycle delta** action opens per-account cycle records: the consumption measured
in each completed or active window, the change against the previous cycle of the same
bucket, and the checkpoints recorded while the window was open. Records are capped at
24 cycles per bucket with up to 20 checkpoints each, so the file stays small.

## Local files

Valuation state lives next to the account data (`~/.antigravity_tools`, or
`ABV_DATA_DIR`):

| File | Contents | Retention |
| --- | --- | --- |
| `account_usage_spans.json` | Which account was active, and when | 35 days |
| `quota_capacity_calibration.json` | Measured value per bucket used as a baseline | 9 samples per bucket |
| `quota_cycle_history.json` | Per-cycle consumption and checkpoints | 24 cycles per bucket |

Deleting these files resets the estimates; accounts, quotas and switching policy are
unaffected. No prompt, conversation or credential content is stored in them.
