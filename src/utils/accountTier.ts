import type { Account } from '../types/account';

export type AccountTier = 'pro' | 'ultra' | 'free' | 'unknown';

/** Missing subscription observations are not evidence of a Free plan. */
export function getAccountTier(account: Pick<Account, 'quota'>): AccountTier {
    const tier = account.quota?.subscription_tier?.trim().toLowerCase();
    if (!tier) return 'unknown';
    if (tier.includes('ultra')) return 'ultra';
    if (tier.includes('pro')) return 'pro';
    return 'free';
}
