# Shared and model-specific quota pools

The quota summary can contain the shared Gemini and Claude/GPT pools and separate
model pools. Gemini 4 Argon's reported `gemini-argon-wtu-weekly` bucket is a
weekly-only pool. Gemini 4 Carbon can report its own `gemini-carbon-5h` and
`gemini-carbon-wtu-weekly` buckets. Neither dedicated pool may be averaged into
shared Gemini, borrowed for other Gemini models, or assigned an invented window.
When the provider does not report a Carbon bucket, Carbon keeps its per-model
observation instead of borrowing shared Gemini. Gemini 4 Hydrogen and the bare
Orchestrator (Carbon) and Orchestrator (Argon) identities follow their planner
pool; they are not treated as separate shared allowances.

Account weekly cards/tables and CLI quota details retain its name and reset time.
Accounts reporting the pool also get a green dot beside their name in the menu and
tray panel, independent of display settings.
The menu adds a dedicated Argon weekly row when reported and weekly Gemini display
is enabled. The dashboard shows separate Argon and Carbon weekly means for accounts
reporting those pools, respecting the Gemini/all aggregate scope. Carbon buckets
also appear in account cards, the account table and cycle details when reported. Shared summaries and account
availability retain their shared-pool meaning. Missing or invalid observations
remain unknown; a missing pool is not evidence of either zero or full allowance.

The account 5H view shows the five-hour allowance and its reset time; the weekly
view shows the weekly allowance and its own reset time. An effective weekly value
cached by 4.10.9 must not replace the five-hour value or countdown. A missing
five-hour observation stays unknown instead of borrowing the week. Argon keeps
its dedicated weekly observation and is explicitly labelled Weekly in model rows.
The original model-endpoint observation is retained separately so a lower primary
allowance is not increased by a shared five-hour summary.

Switching decisions and CLI effective model values still use the more restrictive
reported five-hour and weekly allowances. A depleted week cannot trigger switching
to an account just because its short window has recovered. Grouped menu/tray rows
continue to show their individual source windows and reset times.

Gemini 4 Argon, Carbon and Hydrogen are available as display selections even
before the catalog reports them. A reported dedicated pool can populate that selection without a catalog model;
otherwise it shows unknown. Selecting or refreshing the row does not grant model
access. Catalog models without a quota observation retain their names and metadata
without inventing a percentage.

Model-to-pool association is an inference, not a server-provided relationship.
Canonical bucket IDs take precedence over display labels. Unknown model-specific
Gemini IDs cannot fall back to the shared pool. The shared TS/Rust classification
cases are in `tests/fixtures/quota-pool-cases.json`.

## Data source and verification limits

The account refresh uses `fetchAvailableModels` and `retrieveUserQuotaSummary`
with the account's own OAuth token and project. Daily
(`daily-cloudcode-pa.googleapis.com`) is queried first to match the live
Antigravity desktop client, falling back to `daily-cloudcode-pa.sandbox` and
production (`cloudcode-pa.googleapis.com`). If an endpoint returns HTTP 200
containing only the Claude/GPT group (for example under an unversioned user
agent), it is held only as a partial fallback while later endpoints are checked;
omitted groups stay omitted and results from different endpoint environments are
not combined into a fabricated complete observation.
Summary permission failures retry the native user agent and an empty project
payload before falling back. Malformed or empty responses also try the next
endpoint. Refresh queries the subscription tier again while retaining a cached
project as a fallback; offered paid tiers alone never prove a paid subscription.

A read-only probe on 2026-10-18 confirmed that difference. Available Pro accounts
returned both shared pools on production, but no Argon pool or Argon model through
`fetchAvailableModels`. The running App 2.24.0's `RetrieveUserQuotaSummary` RPC also
returned only the shared pools. These results do **not** establish that every
eligible Pro, Ultra or enterprise account receives Argon through this remote API.
The dedicated-pool compatibility is verified with synthetic observations, not a
live entitled Argon account. It cannot make the upstream service expose a pool it
does not return.

To finish live acceptance, compare an entitled account's native quota popup with
the production summary, including group name, bucket ID, fraction and reset time.
Use a narrowly scoped, read-only request; keep credentials in memory and retain
only a sanitized schema summary. Do not refresh saved account credentials or save
raw account responses as fixtures. A local App RPC describes its signed-in
account and must never be attributed to every account in Switch.

The reported Argon ID and weekly-only shape are also documented in the
[ai-usagebar implementation fixture](https://github.com/akitaonrails/ai-usagebar/blob/v1.40.1/src/antigravity/agy.rs).
