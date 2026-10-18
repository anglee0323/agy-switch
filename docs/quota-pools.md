# Shared and model-specific quota pools

The quota summary can contain the shared Gemini and Claude/GPT pools and separate
model pools. Gemini 4 Argon's reported `gemini-argon-wtu-weekly` bucket is a
weekly-only pool. It must not be averaged into shared Gemini, borrowed for other
Gemini models, or assigned an invented five-hour window.

Account weekly cards/tables and CLI quota details retain its name and reset time.
The menu adds a dedicated weekly row when reported and weekly Gemini display is
enabled. The dashboard shows a separate Argon weekly mean for accounts reporting
that pool, respecting the Gemini/all aggregate scope. Shared summaries and account
availability retain their shared-pool meaning. Missing or invalid observations
remain unknown; a missing pool is not evidence of either zero or full allowance.

Model-to-pool association is an inference, not a server-provided relationship.
Canonical bucket IDs take precedence over display labels. Unknown model-specific
Gemini IDs cannot fall back to the shared pool. The shared TS/Rust classification
cases are in `tests/fixtures/quota-pool-cases.json`.

## Data source and verification limits

The account refresh uses `retrieveUserQuotaSummary` with the account's own OAuth
token and project. Production (`cloudcode-pa.googleapis.com`) is queried before
daily and sandbox endpoints: the latter can return HTTP 200 containing only the
Claude/GPT group. Omitted groups stay omitted; results from different endpoint
environments are not combined into a fabricated complete observation.

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
