/** Request input includes system prompts and the separately reported cache input.
 * Use pooled totals so each request has equal weight across models. */
export function averageRequestInput(totals: { input_tokens: number; cached_tokens: number; request_count: number }): number | null {
    const { input_tokens, cached_tokens, request_count } = totals;
    if (request_count <= 0 || ![input_tokens, cached_tokens, request_count].every(value => Number.isFinite(value) && value >= 0)) return null;
    return Math.round((input_tokens + cached_tokens) / request_count);
}
