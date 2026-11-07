import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

// Transpile currency.ts and quotaValuation.ts
const transpile = (path) => {
    const code = readFileSync(new URL(path, import.meta.url), 'utf8');
    return ts.transpileModule(code, { compilerOptions: { module: ts.ModuleKind.ES2022 } }).outputText;
};

const currencyCode = transpile('../src/utils/currency.ts');
const currencyModule = await import(`data:text/javascript;base64,${Buffer.from(currencyCode).toString('base64')}`);
const { formatSingleCurrency, formatCurrencyAmount, formatCurrencyCompact, formatUsdWithRate } = currencyModule;

// Transpile quotaValuation.ts with mock of currency
const qvCode = transpile('../src/utils/quotaValuation.ts')
    .replace(/from\s+['"]\.\/currency['"]/g, `from "data:text/javascript;base64,${Buffer.from(currencyCode).toString('base64')}"`);
const qvModule = await import(`data:text/javascript;base64,${Buffer.from(qvCode).toString('base64')}`);
const { formatCompactTokens, findBucketForWeekly, findBucketFor5h, formatValuationDisplay, formatValuationStructured } = qvModule;

test('currency formatting: single, amount, compact and withRate', () => {
    // formatSingleCurrency
    assert.equal(formatSingleCurrency(0, '$'), '$0.00');
    assert.equal(formatSingleCurrency(-5, '$'), '$0.00');
    assert.equal(formatSingleCurrency(0.004, '$'), '<$0.01');
    assert.equal(formatSingleCurrency(1.234, '$'), '$1.23');
    assert.equal(formatSingleCurrency(NaN, '$'), '-');

    // formatCurrencyAmount
    assert.equal(formatCurrencyAmount(1.5, 10.5, 'usd'), '$1.50');
    assert.equal(formatCurrencyAmount(1.5, 10.5, 'cny'), '¥10.50');
    assert.equal(formatCurrencyAmount(1.5, 10.5, 'both'), '$1.50 / ¥10.50');

    // formatCurrencyCompact
    assert.equal(formatCurrencyCompact(1.5, 10.5, 'usd'), '$1.50');
    assert.equal(formatCurrencyCompact(1.5, 10.5, 'cny'), '¥10.50');
    assert.equal(formatCurrencyCompact(1.5, 10.5, 'both'), '$1.50 (¥10.50)');

    // formatUsdWithRate
    assert.equal(formatUsdWithRate(2.0, 'usd', 7.25), '$2.00');
    assert.equal(formatUsdWithRate(2.0, 'cny', 7.25), '¥14.50');
    assert.equal(formatUsdWithRate(2.0, 'both', 7.25, false), '$2.00 / ¥14.50');
    assert.equal(formatUsdWithRate(2.0, 'both', 7.25, true), '$2.00 (¥14.50)');
});

test('compact token formatting handles 0, K, M and B boundaries', () => {
    assert.equal(formatCompactTokens(0), '0');
    assert.equal(formatCompactTokens(-10), '0');
    assert.equal(formatCompactTokens(null), '0');
    assert.equal(formatCompactTokens(850), '850');
    assert.equal(formatCompactTokens(999), '999');
    assert.equal(formatCompactTokens(1_000), '1.0K');
    assert.equal(formatCompactTokens(34_200), '34K');
    assert.equal(formatCompactTokens(1_770_000), '1.77M');
    assert.equal(formatCompactTokens(1_000_000), '1M');
    assert.equal(formatCompactTokens(2_500_000_000), '2.5B');
});

test('bucket finder matches weekly bucket_id and 5h pool', () => {
    const buckets = [
        { bucket_id: 'gemini-5h', window: '5h', pool: 'gemini', remaining_fraction: 0.8, reset_time: '2026-11-07T12:00:00Z', provenance: 'observed', used_usd: 1.0, used_tokens: 1000, used_cny: 7.25 },
        { bucket_id: 'third_party-5h', window: '5h', pool: 'third_party', remaining_fraction: 0.5, reset_time: '2026-11-07T12:00:00Z', provenance: 'observed', used_usd: 2.0, used_tokens: 2000, used_cny: 14.5 },
        { bucket_id: 'argon-weekly', window: 'weekly', pool: 'argon', remaining_fraction: 0.9, reset_time: '2026-11-14T00:00:00Z', provenance: 'calibrated', used_usd: 0, used_tokens: 0, used_cny: 0 },
    ];

    assert.equal(findBucketForWeekly(buckets, 'argon-weekly')?.pool, 'argon');
    assert.equal(findBucketForWeekly(buckets, 'ARGON-WEEKLY')?.pool, 'argon');
    assert.equal(findBucketForWeekly(buckets, 'non-existent'), undefined);

    assert.equal(findBucketFor5h(buckets, 'gemini')?.bucket_id, 'gemini-5h');
    assert.equal(findBucketFor5h(buckets, 'third_party')?.bucket_id, 'third_party-5h');
    assert.equal(findBucketFor5h(buckets, 'argon')?.bucket_id, 'argon-weekly');
});

test('formatValuationDisplay renders observed provenance with remaining mode', () => {
    const b = {
        bucket_id: 'gemini-5h',
        window: '5h',
        pool: 'gemini',
        remaining_fraction: 0.75,
        reset_time: '2026-11-07T12:00:00Z',
        provenance: 'observed',
        used_usd: 1.25,
        used_cny: 9.0625,
        used_tokens: 250_000,
        estimated_total_usd: 5.0,
        estimated_total_cny: 36.25,
        estimated_total_tokens: 1_000_000,
        estimated_remaining_usd: 3.75,
        estimated_remaining_cny: 27.1875,
        estimated_remaining_tokens: 750_000,
    };

    const zhRes = formatValuationDisplay(b, 'usd', 'remaining', true);
    assert.ok(zhRes);
    assert.equal(zhRes.line, '≈ $3.75 · 750K');
    assert.ok(zhRes.title.includes('本周期预估总额: $5.00 (1M)'));
    assert.ok(zhRes.title.includes('已消耗: $1.25 (250K)'));
    assert.ok(zhRes.title.includes('剩余: $3.75 (750K)'));

    const enBothRes = formatValuationDisplay(b, 'both', 'remaining', false);
    assert.ok(enBothRes);
    assert.equal(enBothRes.line, '≈ $3.75 (¥27.19) · 750K');
    assert.ok(enBothRes.title.includes('Est. Total: $5.00 (¥36.25) (1M)'));
});

test('formatValuationDisplay renders observed provenance with used_and_total mode', () => {
    const b = {
        bucket_id: 'gemini-5h',
        window: '5h',
        pool: 'gemini',
        remaining_fraction: 0.75,
        reset_time: '2026-11-07T12:00:00Z',
        provenance: 'observed',
        used_usd: 1.25,
        used_cny: 9.0625,
        used_tokens: 250_000,
        estimated_total_usd: 5.0,
        estimated_total_cny: 36.25,
        estimated_total_tokens: 1_000_000,
        estimated_remaining_usd: 3.75,
        estimated_remaining_cny: 27.1875,
        estimated_remaining_tokens: 750_000,
    };

    const res = formatValuationDisplay(b, 'usd', 'used_and_total', true);
    assert.ok(res);
    assert.equal(res.line, '↓ $1.25 / 250K · ↑ $5.00 / 1M');
    assert.ok(res.title.includes('本周期消耗: $1.25 (250K)'));
    assert.ok(res.title.includes('预估总额: $5.00 (1M)'));
});

test('formatValuationDisplay handles calibrated and manual_reference benchmarks', () => {
    const b = {
        bucket_id: 'gemini-5h',
        window: '5h',
        pool: 'gemini',
        remaining_fraction: 1.0,
        reset_time: '2026-11-07T12:00:00Z',
        provenance: 'calibrated',
        used_usd: 0,
        used_cny: 0,
        used_tokens: 0,
        estimated_total_usd: 10.0,
        estimated_total_cny: 72.5,
        estimated_total_tokens: 2_000_000,
        estimated_remaining_usd: 10.0,
        estimated_remaining_cny: 72.5,
        estimated_remaining_tokens: 2_000_000,
    };

    const zhRes = formatValuationDisplay(b, 'usd', 'remaining', true);
    assert.ok(zhRes);
    assert.equal(zhRes.line, '参考 ≈ $10.00 · 2M');
    assert.ok(zhRes.title.includes('[同档位历史样本校准]'));

    const enRes = formatValuationDisplay(b, 'usd', 'remaining', false);
    assert.ok(enRes);
    assert.equal(enRes.line, 'Ref ≈ $10.00 · 2M');
    assert.ok(enRes.title.includes('[Calibrated Benchmark]'));
});

test('formatValuationDisplay handles estimated_reference benchmarks', () => {
    const b = {
        bucket_id: 'gemini-5h',
        window: '5h',
        pool: 'gemini',
        remaining_fraction: 1.0,
        reset_time: '2026-11-07T12:00:00Z',
        provenance: 'estimated_reference',
        used_usd: 0,
        used_cny: 0,
        used_tokens: 0,
        estimated_total_usd: 5.0,
        estimated_total_cny: 36.25,
        estimated_total_tokens: 1_000_000,
        estimated_remaining_usd: 5.0,
        estimated_remaining_cny: 36.25,
        estimated_remaining_tokens: 1_000_000,
    };

    const zhRes = formatValuationDisplay(b, 'usd', 'remaining', true);
    assert.ok(zhRes);
    assert.equal(zhRes.line, '预估 ≈ $5.00 · 1M');
    assert.ok(zhRes.title.includes('[档位经验基准（随本地实测自动校准）]'));

    const enRes = formatValuationDisplay(b, 'usd', 'remaining', false);
    assert.ok(enRes);
    assert.equal(enRes.line, 'Est ≈ $5.00 · 1M');
    assert.ok(enRes.title.includes('[Tier Benchmark (auto-calibrated locally)]'));
});

test('formatValuationDisplay handles exhausted, used_only (<3%) and none', () => {
    // none returns null
    assert.equal(formatValuationDisplay({ provenance: 'none' }), null);
    assert.equal(formatValuationDisplay(undefined), null);

    // exhausted
    const ex = {
        bucket_id: 'gemini-5h',
        window: '5h',
        pool: 'gemini',
        remaining_fraction: 0.0,
        reset_time: '2026-11-07T12:00:00Z',
        provenance: 'exhausted',
        used_usd: 3.5,
        used_cny: 25.375,
        used_tokens: 700_000,
        estimated_total_usd: 3.5,
        estimated_total_cny: 25.375,
        estimated_total_tokens: 700_000,
        estimated_remaining_usd: 0,
        estimated_remaining_cny: 0,
        estimated_remaining_tokens: 0,
    };
    const exRes = formatValuationDisplay(ex, 'usd', 'remaining', true);
    assert.ok(exRes);
    assert.equal(exRes.line, '$0.00 · 0 (已用 $3.50 · 700K)');
    assert.ok(exRes.title.includes('本周期额度已耗尽'));

    // used_only (<3%)
    const uo = {
        bucket_id: 'gemini-5h',
        window: '5h',
        pool: 'gemini',
        remaining_fraction: 0.99,
        reset_time: '2026-11-07T12:00:00Z',
        provenance: 'used_only',
        used_usd: 0.05,
        used_cny: 0.3625,
        used_tokens: 10_000,
    };
    const uoRes = formatValuationDisplay(uo, 'usd', 'remaining', true);
    assert.ok(uoRes);
    assert.equal(uoRes.line, '已用 $0.05 · 10K');
    assert.ok(uoRes.title.includes('消耗量极小（<3%）'));
});

test('formatValuationDisplay renders tokens-only display when unpriced models lack USD', () => {
    const unpricedBucket = {
        bucket_id: 'gemini-5h',
        window: '5h',
        pool: 'gemini',
        remaining_fraction: 0.75,
        reset_time: '2026-11-07T12:00:00Z',
        provenance: 'observed',
        used_usd: 0,
        used_tokens: 250_000,
        estimated_total_tokens: 1_000_000,
        estimated_remaining_tokens: 750_000,
        // estimated_remaining_usd and estimated_total_usd are undefined!
        warning: 'Contains unpriced models',
    };

    const remRes = formatValuationDisplay(unpricedBucket, 'usd', 'remaining', true);
    assert.ok(remRes);
    assert.equal(remRes.line, '≈ 750K tokens');
    assert.ok(!remRes.line.includes('$0.00'), 'Must not display fake $0.00');
    assert.ok(remRes.title.includes('⚠️ Contains unpriced models'));

    const usedTotalRes = formatValuationDisplay(unpricedBucket, 'usd', 'used_and_total', true);
    assert.ok(usedTotalRes);
    assert.equal(usedTotalRes.line, '↓ 250K · ↑ 1M');
    assert.ok(!usedTotalRes.line.includes('$0.00'), 'Must not display fake $0.00');
});

test('formatValuationStructured outputs structured fields for 3-tier card UI', () => {
    // 1. none / undefined returns null
    assert.equal(formatValuationStructured(undefined), null);
    assert.equal(formatValuationStructured({ provenance: 'none' }), null);

    // 2. observed standard valuation (remaining mode)
    const observedBucket = {
        bucket_id: 'gemini-5h',
        window: '5h',
        pool: 'gemini',
        remaining_fraction: 0.98,
        reset_time: '2026-11-07T15:00:00Z',
        provenance: 'observed',
        used_usd: 0.02,
        used_cny: 0.145,
        used_tokens: 34_000,
        estimated_total_usd: 1.18,
        estimated_total_cny: 8.555,
        estimated_total_tokens: 1_800_000,
        estimated_remaining_usd: 1.16,
        estimated_remaining_cny: 8.41,
        estimated_remaining_tokens: 1_766_000,
    };

    const resZh = formatValuationStructured(observedBucket, 'usd', 'remaining', true);
    assert.ok(resZh);
    assert.equal(resZh.usedText, '$0.02 / 34K');
    assert.equal(resZh.quotaText, '剩余 $1.16 / 1.77M');
    assert.equal(resZh.badge, undefined);
    assert.ok(resZh.title.includes('已消耗: $0.02 (34K)'));
    assert.ok(resZh.title.includes('预估总额: $1.18 (1.8M)'));

    // 3. used_and_total display mode
    const resTotal = formatValuationStructured(observedBucket, 'usd', 'used_and_total', true);
    assert.ok(resTotal);
    assert.equal(resTotal.usedText, '$0.02 / 34K');
    assert.equal(resTotal.quotaText, '总额 $1.18 / 1.8M');

    // 4. calibrated with badge
    const calBucket = {
        ...observedBucket,
        provenance: 'calibrated',
    };
    const resCal = formatValuationStructured(calBucket, 'usd', 'remaining', true);
    assert.ok(resCal);
    assert.equal(resCal.badge, '校准');

    // 5. estimated_reference with badge
    const estBucket = {
        ...observedBucket,
        provenance: 'estimated_reference',
    };
    const resEst = formatValuationStructured(estBucket, 'usd', 'remaining', true);
    assert.ok(resEst);
    assert.equal(resEst.badge, '预估');

    // 6. English mode
    const resEn = formatValuationStructured(estBucket, 'usd', 'remaining', false);
    assert.ok(resEn);
    assert.equal(resEn.quotaText, 'Left $1.16 / 1.77M');
    assert.equal(resEst.badge, '预估');
    assert.equal(resEn.badge, 'Est');

    // 7. Cycle delta and CoC percentage badges
    const cycleBucket = {
        ...observedBucket,
        last_delta_fraction: -0.07,
        delta_total_pct: 8.5,
    };
    const resCycle = formatValuationStructured(cycleBucket, 'usd', 'remaining', true);
    assert.ok(resCycle);
    assert.equal(resCycle.lastDeltaText, '↓7%');
    assert.equal(resCycle.cycleCompareText, '环比 +9%');
    assert.equal(resCycle.cycleCompareTone, 'increase');
    assert.ok(resCycle.title.includes('最近单次变化: ↓7%'));
    assert.ok(resCycle.title.includes('周期环比变化: 环比 +9%'));
});


