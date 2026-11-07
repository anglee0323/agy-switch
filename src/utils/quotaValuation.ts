import { formatCurrencyCompact, formatCurrencyAmount, CurrencyMode } from './currency';

export interface BucketValuation {
    bucket_id: string;
    window: string;
    pool: 'gemini' | 'third_party' | 'argon' | 'unknown';
    remaining_fraction: number;
    reset_time: string;
    provenance: 'observed' | 'exhausted' | 'used_only' | 'manual_reference' | 'calibrated' | 'estimated_reference' | 'none';
    used_usd: number;
    used_tokens: number;
    estimated_total_usd?: number;
    estimated_remaining_usd?: number;
    estimated_total_tokens?: number;
    estimated_remaining_tokens?: number;
    used_cny: number;
    estimated_total_cny?: number;
    estimated_remaining_cny?: number;
    last_delta_fraction?: number;
    consumed_fraction?: number;
    delta_total_usd?: number;
    delta_total_pct?: number;
    warning?: string;
}

export interface AccountQuotaValuation {
    account_id: string;
    email: string;
    subscription_tier?: string;
    is_active: boolean;
    buckets: BucketValuation[];
    unpriced_events_count: number;
}

export interface QuotaValuationResponse {
    accounts: AccountQuotaValuation[];
    currency: 'usd' | 'cny' | 'both';
    display_mode: 'remaining' | 'used_and_total';
    usd_to_cny_rate: number;
    rate_stale: boolean;
    calculated_at: number;
}

/**
 * 紧凑格式化 Token 数值 (如 0, 850, 34.0K, 1.77M, 1.2B)
 */
export function formatCompactTokens(tokens: number | undefined | null): string {
    if (tokens == null || !Number.isFinite(tokens) || tokens <= 0) {
        return '0';
    }
    if (tokens < 1000) {
        return tokens.toString();
    }
    if (tokens < 1_000_000) {
        const k = tokens / 1000;
        return `${k.toFixed(k < 10 ? 1 : 0)}K`;
    }
    if (tokens < 1_000_000_000) {
        const m = tokens / 1_000_000;
        return `${m.toFixed(2).replace(/\.?0+$/, '')}M`;
    }
    const b = tokens / 1_000_000_000;
    return `${b.toFixed(2).replace(/\.?0+$/, '')}B`;
}

/**
 * 周视图匹配桶：精确按 bucket_id
 */
export function findBucketForWeekly(
    buckets: BucketValuation[] | undefined,
    bucketId: string
): BucketValuation | undefined {
    if (!buckets || !bucketId) return undefined;
    const lowerId = bucketId.toLowerCase();
    return buckets.find(b => b.bucket_id.toLowerCase() === lowerId);
}

/**
 * 5h 视图匹配桶：按 pool 与 5h 窗口 (Argon 可回退 weekly)
 */
export function findBucketFor5h(
    buckets: BucketValuation[] | undefined,
    pool: 'gemini' | 'third_party' | 'argon',
    windowHint?: '5h' | 'weekly'
): BucketValuation | undefined {
    if (!buckets) return undefined;
    if (windowHint === 'weekly') {
        const weekly = buckets.find(b => b.pool === pool && b.window.toLowerCase().includes('week'));
        if (weekly) return weekly;
    }
    // 优先匹配 5h 窗口
    const exact = buckets.find(b => b.pool === pool && b.window.toLowerCase().includes('5h'));
    if (exact) return exact;
    // Argon 或未分 5h 的池回退首个匹配池的桶
    return buckets.find(b => b.pool === pool);
}

export interface ValuationDisplayResult {
    line: string;
    title: string;
}

/**
 * 生成紧凑折算展示文案与提示
 */
export function formatValuationDisplay(
    valuation: BucketValuation | undefined,
    currencyMode: CurrencyMode = 'usd',
    displayMode: 'remaining' | 'used_and_total' = 'remaining',
    isZh = true
): ValuationDisplayResult | null {
    if (!valuation || valuation.provenance === 'none') {
        return null;
    }

    const {
        provenance,
        used_usd,
        used_cny,
        used_tokens,
        estimated_total_usd,
        estimated_total_cny,
        estimated_total_tokens,
        estimated_remaining_usd,
        estimated_remaining_cny,
        estimated_remaining_tokens,
        warning,
    } = valuation;

    const usedCost = formatCurrencyCompact(used_usd, used_cny, currencyMode);
    const usedTok = formatCompactTokens(used_tokens);

    if (displayMode === 'remaining') {
        if (provenance === 'observed') {
            const remTok = formatCompactTokens(estimated_remaining_tokens ?? 0);
            const totalTok = formatCompactTokens(estimated_total_tokens ?? 0);

            if (estimated_remaining_usd != null) {
                const remCost = formatCurrencyCompact(estimated_remaining_usd, estimated_remaining_cny ?? 0, currencyMode);
                const totalCost = formatCurrencyCompact(estimated_total_usd ?? 0, estimated_total_cny ?? 0, currencyMode);
                const title = `${isZh ? '本周期预估总额' : 'Est. Total'}: ${totalCost} (${totalTok})\n${isZh ? '已消耗' : 'Used'}: ${usedCost} (${usedTok})\n${isZh ? '剩余' : 'Remaining'}: ${remCost} (${remTok})${warning ? `\n⚠️ ${warning}` : ''}`;
                return {
                    line: `≈ ${remCost} · ${remTok}`,
                    title,
                };
            } else {
                const title = `${isZh ? '本周期预估总额' : 'Est. Total'}: ${totalTok}\n${isZh ? '已消耗' : 'Used'}: ${usedTok}\n${isZh ? '剩余' : 'Remaining'}: ${remTok}${warning ? `\n⚠️ ${warning}` : ''}`;
                return {
                    line: `≈ ${remTok} tokens`,
                    title,
                };
            }
        }

        if (provenance === 'calibrated' || provenance === 'manual_reference' || provenance === 'estimated_reference') {
            const remTok = formatCompactTokens(estimated_remaining_tokens ?? 0);
            const totalTok = formatCompactTokens(estimated_total_tokens ?? 0);
            const prefix = provenance === 'estimated_reference'
                ? (isZh ? '预估' : 'Est')
                : (isZh ? '参考' : 'Ref');
            const sourceLabel = provenance === 'manual_reference'
                ? (isZh ? '自定义参考基准' : 'Manual Reference')
                : provenance === 'calibrated'
                ? (isZh ? '同档位历史样本校准' : 'Calibrated Benchmark')
                : (isZh ? '档位经验基准（随本地实测自动校准）' : 'Tier Benchmark (auto-calibrated locally)');

            if (estimated_remaining_usd != null) {
                const remCost = formatCurrencyCompact(estimated_remaining_usd, estimated_remaining_cny ?? 0, currencyMode);
                const totalCost = formatCurrencyCompact(estimated_total_usd ?? 0, estimated_total_cny ?? 0, currencyMode);
                const title = `[${sourceLabel}]\n${isZh ? '预估基准总额' : 'Benchmark Total'}: ${totalCost} (${totalTok})\n${isZh ? '等效剩余' : 'Equiv. Remaining'}: ${remCost} (${remTok})${warning ? `\n⚠️ ${warning}` : ''}`;
                return {
                    line: `${prefix} ≈ ${remCost} · ${remTok}`,
                    title,
                };
            } else {
                const title = `[${sourceLabel}]\n${isZh ? '预估基准总额' : 'Benchmark Total'}: ${totalTok}\n${isZh ? '等效剩余' : 'Equiv. Remaining'}: ${remTok}${warning ? `\n⚠️ ${warning}` : ''}`;
                return {
                    line: `${prefix} ≈ ${remTok} tokens`,
                    title,
                };
            }
        }

        if (provenance === 'exhausted') {
            const zeroCost = formatCurrencyCompact(0, 0, currencyMode);
            const usedLabel = isZh ? '已用' : 'Used';
            const title = `${isZh ? '本周期额度已耗尽' : 'Quota exhausted'}\n${isZh ? '实际消耗' : 'Actual consumption'}: ${usedCost} (${usedTok})`;
            return {
                line: `${zeroCost} · 0 (${usedLabel} ${usedCost} · ${usedTok})`,
                title,
            };
        }

        if (provenance === 'used_only') {
            const usedLabel = isZh ? '已用' : 'Used';
            const title = `${isZh ? '实际消耗' : 'Actual consumption'}: ${usedCost} (${usedTok})\n${isZh ? '消耗量极小（<3%），为防误差未做总额推算' : 'Extrapolation skipped (<3% usage)'}`;
            return {
                line: `${usedLabel} ${usedCost} · ${usedTok}`,
                title,
            };
        }
    } else {
        // used_and_total 模式
        if (provenance === 'observed' || provenance === 'exhausted') {
            const totalTok = formatCompactTokens(estimated_total_tokens ?? 0);
            if (estimated_total_usd != null) {
                const totalCost = formatCurrencyAmount(estimated_total_usd, estimated_total_cny ?? 0, currencyMode);
                const title = `${isZh ? '本周期消耗' : 'Used'}: ${usedCost} (${usedTok})\n${isZh ? '预估总额' : 'Est. Total'}: ${totalCost} (${totalTok})`;
                return {
                    line: `↓ ${usedCost} / ${usedTok} · ↑ ${totalCost} / ${totalTok}`,
                    title,
                };
            } else {
                const title = `${isZh ? '本周期消耗' : 'Used'}: ${usedTok}\n${isZh ? '预估总额' : 'Est. Total'}: ${totalTok}`;
                return {
                    line: `↓ ${usedTok} · ↑ ${totalTok}`,
                    title,
                };
            }
        }

        if (provenance === 'calibrated' || provenance === 'manual_reference' || provenance === 'estimated_reference') {
            const totalTok = formatCompactTokens(estimated_total_tokens ?? 0);
            const sourceLabel = provenance === 'manual_reference'
                ? (isZh ? '自定义参考基准' : 'Manual Reference')
                : provenance === 'calibrated'
                ? (isZh ? '同档位历史样本校准' : 'Calibrated Benchmark')
                : (isZh ? '档位经验基准（随本地实测自动校准）' : 'Tier Benchmark (auto-calibrated locally)');
            if (estimated_total_usd != null) {
                const totalCost = formatCurrencyAmount(estimated_total_usd, estimated_total_cny ?? 0, currencyMode);
                const title = `[${sourceLabel}]\n${isZh ? '已用' : 'Used'}: ${usedCost} (${usedTok})\n${isZh ? '基准总额' : 'Benchmark Total'}: ${totalCost} (${totalTok})`;
                return {
                    line: `↓ ${usedCost} / ${usedTok} · ≈ ${totalCost} / ${totalTok}`,
                    title,
                };
            } else {
                const title = `[${sourceLabel}]\n${isZh ? '已用' : 'Used'}: ${usedTok}\n${isZh ? '基准总额' : 'Benchmark Total'}: ${totalTok}`;
                return {
                    line: `↓ ${usedTok} · ≈ ${totalTok}`,
                    title,
                };
            }
        }

        if (provenance === 'used_only') {
            const title = `${isZh ? '实际消耗' : 'Actual consumption'}: ${usedCost} (${usedTok})\n${isZh ? '消耗量极小（<3%），为防误差未做总额推算' : 'Extrapolation skipped (<3% usage)'}`;
            return {
                line: `↓ ${usedCost} / ${usedTok}`,
                title,
            };
        }
    }

    return null;
}

export interface ValuationStructuredResult {
    usedText: string;
    quotaText?: string;
    badge?: string;
    lastDeltaText?: string;
    cycleCompareText?: string;
    cycleCompareTone?: 'increase' | 'decrease' | 'flat';
    title: string;
}

/**
 * 结构化格式化折算数据，供图 1 同款三段式 UI 高清渲染
 */
export function formatValuationStructured(
    valuation: BucketValuation | undefined,
    currencyMode: CurrencyMode = 'usd',
    displayMode: 'remaining' | 'used_and_total' = 'remaining',
    isZh = true
): ValuationStructuredResult | null {
    if (!valuation || valuation.provenance === 'none') {
        return null;
    }

    const {
        provenance,
        used_usd,
        used_cny,
        used_tokens,
        estimated_total_usd,
        estimated_total_cny,
        estimated_total_tokens,
        estimated_remaining_usd,
        estimated_remaining_cny,
        estimated_remaining_tokens,
        last_delta_fraction,
        delta_total_pct,
        warning,
    } = valuation;

    const usedCost = formatCurrencyCompact(used_usd, used_cny, currencyMode);
    const usedTok = formatCompactTokens(used_tokens);
    const usedText = `${usedCost} / ${usedTok}`;

    let quotaText: string | undefined;
    let badge: string | undefined;

    const isRemaining = displayMode === 'remaining';
    const targetUsd = isRemaining ? estimated_remaining_usd : estimated_total_usd;
    const targetCny = isRemaining ? estimated_remaining_cny : estimated_total_cny;
    const targetTok = formatCompactTokens(isRemaining ? estimated_remaining_tokens : estimated_total_tokens);
    const labelPrefix = isZh
        ? (isRemaining ? '剩余' : '总额')
        : (isRemaining ? 'Left' : 'Cap');

    if (targetUsd != null) {
        const costStr = formatCurrencyCompact(targetUsd, targetCny ?? 0, currencyMode);
        quotaText = `${labelPrefix} ${costStr} / ${targetTok}`;
    } else if (targetTok !== '0') {
        quotaText = `${labelPrefix} ${targetTok}`;
    }

    if (provenance === 'calibrated') {
        badge = isZh ? '校准' : 'Cal';
    } else if (provenance === 'manual_reference') {
        badge = isZh ? '自定义' : 'Ref';
    } else if (provenance === 'estimated_reference') {
        badge = isZh ? '预估' : 'Est';
    }

    let lastDeltaText: string | undefined;
    if (last_delta_fraction != null && Math.abs(last_delta_fraction) >= 0.005) {
        const pct = Math.round(Math.abs(last_delta_fraction) * 100);
        lastDeltaText = last_delta_fraction < 0 ? `↓${pct}%` : `↑${pct}%`;
    }

    let cycleCompareText: string | undefined;
    let cycleCompareTone: 'increase' | 'decrease' | 'flat' | undefined;
    if (delta_total_pct != null) {
        const pct = Math.round(delta_total_pct);
        if (pct > 0) {
            cycleCompareText = isZh ? `环比 +${pct}%` : `+${pct}% vs last`;
            cycleCompareTone = 'increase';
        } else if (pct < 0) {
            cycleCompareText = isZh ? `环比 ${pct}%` : `${pct}% vs last`;
            cycleCompareTone = 'decrease';
        } else {
            cycleCompareText = isZh ? '环比持平' : 'flat vs last';
            cycleCompareTone = 'flat';
        }
    }

    const totalTok = formatCompactTokens(estimated_total_tokens ?? 0);
    const remTok = formatCompactTokens(estimated_remaining_tokens ?? 0);
    const totalCost = estimated_total_usd != null ? formatCurrencyCompact(estimated_total_usd, estimated_total_cny ?? 0, currencyMode) : undefined;
    const remCost = estimated_remaining_usd != null ? formatCurrencyCompact(estimated_remaining_usd, estimated_remaining_cny ?? 0, currencyMode) : undefined;

    const title = [
        totalCost ? `${isZh ? '预估总额' : 'Est. Total'}: ${totalCost} (${totalTok})` : undefined,
        `${isZh ? '已消耗' : 'Used'}: ${usedCost} (${usedTok})`,
        remCost ? `${isZh ? '剩余' : 'Remaining'}: ${remCost} (${remTok})` : undefined,
        lastDeltaText ? `${isZh ? '最近单次变化' : 'Last delta'}: ${lastDeltaText}` : undefined,
        cycleCompareText ? `${isZh ? '周期环比变化' : 'Cycle delta'}: ${cycleCompareText}` : undefined,
        warning ? `⚠️ ${warning}` : undefined,
    ].filter(Boolean).join('\n');

    return {
        usedText,
        quotaText,
        badge,
        lastDeltaText,
        cycleCompareText,
        cycleCompareTone,
        title,
    };
}
