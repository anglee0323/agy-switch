export function makeDashboardSnapshot(now: number) {
    const account = (id: string, gemini = [.8, .6], other = [.4, .2]) => ({
        id, email: `${id}@example.invalid`, name: null, custom_label: null, read_status: 'loaded' as 'loaded' | 'failed', read_error: null,
        disabled: false, validation_blocked: false, validation_blocked_until: null as number | null, protected_models: [] as string[],
        quota: { last_updated: now / 1000 - 30, is_forbidden: false, subscription_tier: null, provenance: 'observed' as const, models: [],
            groups: [['Gemini', gemini], ['Claude/GPT', other]].map(([display_name, values], family) => ({ display_name: display_name as string,
                buckets: (values as number[]).map((remaining_fraction, index) => ({ bucket_id: `pool-${family}-${index}`, window: index ? 'weekly' : '5h',
                    remaining_fraction: remaining_fraction as number | null, reset_time: new Date(now + (index ? 2 * 86400000 : 3 * 3600000)).toISOString() })) })) },
    });
    const accounts = [account('normal'), account('secondary', [1, .8], [.6, .4]), account('disabled'), account('stale'), account('zero', [0, 0], [0, 0])];
    accounts[2].disabled = true;
    accounts[3].quota.last_updated -= 3600;
    return { indexed_total: accounts.length, loaded_count: accounts.length, failed_count: 0, current_account_id: accounts[0].id,
        current_identity_source: 'tools_record' as const, accounts };
}
