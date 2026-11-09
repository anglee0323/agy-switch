export type CurrencyMode = 'usd' | 'cny' | 'both';

/**
 * 格式化单币种数值
 */
export function formatSingleCurrency(amount: number, symbol: '$' | '¥'): string {
    if (!Number.isFinite(amount)) return '-';
    if (amount <= 0) return `${symbol}0.00`;
    if (amount < 0.01) return `<${symbol}0.01`;
    return `${symbol}${amount.toFixed(2)}`;
}

/**
 * 根据币种配置（USD / CNY / Both）格式化金额
 */
export function formatCurrencyAmount(
    usd: number,
    cny: number,
    mode: CurrencyMode = 'usd'
): string {
    const usdStr = formatSingleCurrency(usd, '$');
    const cnyStr = formatSingleCurrency(cny, '¥');

    if (mode === 'cny') return cnyStr;
    if (mode === 'both') return `${usdStr} / ${cnyStr}`;
    return usdStr;
}

/**
 * 格式化双币种紧凑带括号（如 `$1.16 (¥8.41)` 或单独 `$1.16` / `¥8.41`）
 */
export function formatCurrencyCompact(
    usd: number,
    cny: number,
    mode: CurrencyMode = 'usd'
): string {
    const usdStr = formatSingleCurrency(usd, '$');
    const cnyStr = formatSingleCurrency(cny, '¥');

    if (mode === 'cny') return cnyStr;
    if (mode === 'both') return `${usdStr} (${cnyStr})`;
    return usdStr;
}

/**
 * 方便根据当前汇率与币种偏好格式化金额
 */
export function formatUsdWithRate(
    usd: number,
    mode: CurrencyMode = 'usd',
    usdToCnyRate = 7.25,
    compact = false
): string {
    const cny = usd * usdToCnyRate;
    return compact
        ? formatCurrencyCompact(usd, cny, mode)
        : formatCurrencyAmount(usd, cny, mode);
}

