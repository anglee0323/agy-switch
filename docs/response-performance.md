# Recent response performance

The Dashboard's **First-text latency** and **Body output speed** cards summarize
existing local Antigravity records. They never send benchmark prompts, refresh
credentials, switch accounts or contact an inference endpoint. They update with
the existing usage scan (on opening, every 60 seconds, and manual refresh).

The cards use the median of up to 10 eligible text generations from the last
seven days across all models and local desktop / CLI stores. Each eligible
generation contributes equally, so five Gemini and five Claude responses produce
one ten-response summary. Both cards use the same samples. The actual sample count
appears in the card detail; model and source counts are available in its tooltip.
This is a summary of recent user experience, not a comparison of individual
model speeds. These cards are independent of the
usage date-range selector. One available sample can be shown; no eligible samples
produce an unknown value (`—`), never a zero-speed claim.

In **Settings > General > Dashboard cards**, select any of the ten summary
cards and drag their handles to set the display order. The compact two-column
list shows order numbers and becomes a single column in narrow windows.
Keyboard users can press Space, move with the arrow
keys and confirm with Space. Changes save automatically and persist across app
restarts. The dashboard adapts its columns to the selection; an empty selection
hides the summary cards while retaining charts. New configurations without a
saved selection default to all ten cards. Explicit selections, including empty
selections and older five- or seven-card layouts, retain their order and visibility.
Appearance changes cannot overwrite a newer card selection. See
[account and quota cards](dashboard-overview.md) for the three additional metrics.

Native `ChatModelMetadata` supplies `time_to_first_token` (field 11),
`streaming_duration` (12), and usage `response_output_tokens` (10).
`thinking_output_tokens` (9) is excluded from body speed. The estimate is response
tokens divided by streaming duration; it is a chunk-boundary average, not exact
per-token timing. Latency runs from the model request to first response text; it
does not include application startup or screen rendering, and is not the total
duration of a conversation or agent task.

Eligibility requires a completed type-15 step with a text planner response,
`STOP_PATTERN`, no tool calls, valid positive protobuf durations, at least 100
response tokens and at least one second of streaming. This avoids misleading
speeds from buffered tool calls and very short replies. Unknown schemas, canceled
or partial steps, missing timestamps, future records and records older than seven
days are excluded. Step payload reads are capped at 1 MiB; generation metadata
retains the existing 64 MiB limit. Text is checked locally, never returned in the
usage IPC or logged. Archived token totals without timing cannot supply these
cards; timing can still be read from matching live rows without double-counting
their archived token usage.

Verification uses synthetic SQLite fixtures for parsing, exclusions, median / age
selection, mixed model / source coverage and archive deduplication, plus mocked
dashboard UI checks for both locales, unknown data and unclipped descriptions in
narrower windows:

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib native_token_stats::
npm run build
npx playwright test tests/ui/dashboard-performance.spec.ts tests/ui/dashboard-cost.spec.ts
npx playwright test tests/ui/dashboard-cards.spec.ts
```
