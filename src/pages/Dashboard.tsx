import { Fragment, type ReactElement, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Activity, BarChart3, CalendarDays, Clock3, Cpu, Database, DollarSign, Gauge, LayoutDashboard, MessageSquare, PieChart, RefreshCw, Timer, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { request as invoke } from '../utils/request';
import { showToast } from '../components/common/ToastContainer';
import { estimateApiCost, findModelPricing } from '../utils/modelPricing';
import { useConfigStore } from '../stores/useConfigStore';
import { dashboardCards, dashboardGridClass, DEFAULT_MENU_BAR_PREFERENCES, type DashboardCardId } from '../types/config';
import { dashboardOverview, resetCountdown } from '../utils/dashboardOverview';
import { quotaDisplay } from '../utils/menuBarOverview';
import { useDashboardOverview } from '../hooks/useDashboardOverview';

interface LocalTokenTotals {
    input_tokens: number;
    output_tokens: number;
    cached_tokens: number;
    total_tokens: number;
    request_count: number;
}

interface LocalTokenDaily extends LocalTokenTotals {
    date: string;
}

interface LocalTokenHourly extends LocalTokenTotals {
    hour: string;
}

interface LocalTokenModel extends LocalTokenTotals {
    model: string;
}

interface LocalTokenUsageSummary {
    recent_performance?: {
        model_count: number;
        source_count: number;
        sample_count: number;
        first_text_seconds: number;
        body_tokens_per_second: number;
        last_activity: number;
    } | null;
    today: LocalTokenTotals;
    yesterday: LocalTokenTotals;
    last_3_days: LocalTokenTotals;
    last_7_days: LocalTokenTotals;
    last_30_days: LocalTokenTotals;
    daily: LocalTokenDaily[];
    hourly: LocalTokenHourly[];
    by_model_today: LocalTokenModel[];
    by_model_yesterday: LocalTokenModel[];
    by_model_3_days: LocalTokenModel[];
    by_model_7_days: LocalTokenModel[];
    by_model: LocalTokenModel[];
    databases_scanned: number;
    generations_scanned: number;
    skipped_large_records: number;
    unreadable_databases: number;
    last_activity?: number;
    generated_at: number;
}

interface ModelPricing {
    input: number;
    output: number;
    cached: number;
}

interface ApiPricingEntry extends ModelPricing {
    model: string;
}

interface ApiPricingSnapshot {
    prices: ApiPricingEntry[];
    fetched_at: number;
    stale: boolean;
    source: string;
    warning?: string;
}

type RangeKey = 'today' | 'yesterday' | '3d' | '7d' | '30d';

const formatTokens = (value: number, locale: string) => value.toLocaleString(locale);

const formatUsd = (value: number) => {
    if (value === 0) return '$0.00';
    return value < 0.01 ? `$${value.toFixed(4)}` : `$${value.toFixed(2)}`;
};

const compactTokens = (value: number, locale: string) => {
    if (value >= 1_000_000) return `${(value / 1_000_000).toLocaleString(locale, { maximumFractionDigits: 1 })}M`;
    if (value >= 1_000) return `${(value / 1_000).toLocaleString(locale, { maximumFractionDigits: 1 })}K`;
    return value.toLocaleString(locale);
};

const dateKey = (date: Date) => {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    return `${year}-${month}-${day}`;
};

const shortDate = (key: string, locale: string) => new Date(`${key}T00:00:00`).toLocaleDateString(locale, {
    month: 'numeric',
    day: 'numeric',
});

const formatTime = (timestamp: number | null | undefined, locale: string) => {
    if (!timestamp) return '';
    return new Date(timestamp).toLocaleTimeString(locale, {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
    });
};

const rangeDays: Record<RangeKey, number> = {
    today: 1,
    yesterday: 1,
    '3d': 3,
    '7d': 7,
    '30d': 30,
};

const chartBarMaxWidth: Record<RangeKey, number> = {
    today: 12,
    yesterday: 12,
    '3d': 32,
    '7d': 32,
    '30d': 8,
};

export interface ModelCostBreakdown extends LocalTokenModel {
    costUsd: number;
    costPercent: number;
    isPriced: boolean;
    color: string;
}

const MODEL_COLORS = [
    '#5b9fd6', // sky blue
    '#65b99f', // mint
    '#d6b45f', // soft gold
    '#8cacca', // slate blue
    '#87bec5', // sea glass
    '#bf969d', // muted rose
    '#a6b57d', // sage
    '#7893ad', // blue gray
    '#a9a1be', // muted lavender
    '#bea98b', // sand
];

type TokenChartPoint = LocalTokenTotals & {
    key: string;
    label: string;
};

const emptyTokenTotals = (): LocalTokenTotals => ({
    input_tokens: 0,
    output_tokens: 0,
    cached_tokens: 0,
    total_tokens: 0,
    request_count: 0,
});

function ModelCostDonut({
    data,
    totalCost,
    unpricedModels,
    hoveredModel,
    onHover,
}: {
    data: ModelCostBreakdown[];
    totalCost: number;
    unpricedModels: number;
    hoveredModel: string | null;
    onHover: (model: string | null) => void;
}) {
    const { t } = useTranslation();
    const radius = 38;
    const circumference = 2 * Math.PI * radius; // ~238.761

    const activeItem = data.find((d) => d.model === hoveredModel);

    if (totalCost === 0 || data.length === 0) {
        return (
            <div className="relative flex h-[100px] w-[100px] shrink-0 items-center justify-center">
                <svg viewBox="0 0 100 100" className="h-full w-full -rotate-90">
                    <circle
                        cx="50"
                        cy="50"
                        r={radius}
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="10"
                        className="text-gray-100 dark:text-base-200"
                    />
                </svg>
                <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
                    <span className="font-mono text-xs font-bold text-gray-400">{unpricedModels ? t('local_dashboard.unpriced') : '$0.00'}</span>
                    <span className="text-[9px] text-gray-400">{unpricedModels ? t('local_dashboard.pricing_pending') : t('local_dashboard.no_cost_in_range')}</span>
                </div>
            </div>
        );
    }

    let accumulatedPercent = 0;

    return (
        <div className="relative flex h-[100px] w-[100px] shrink-0 items-center justify-center">
            <svg viewBox="0 0 100 100" className="h-full w-full -rotate-90">
                <circle
                    cx="50"
                    cy="50"
                    r={radius}
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="11"
                    className="text-gray-100 dark:text-base-200"
                />
                {data.map((slice) => {
                    const percent = slice.costPercent / 100;
                    const strokeDasharray = `${percent * circumference} ${circumference * (1 - percent)}`;
                    const strokeDashoffset = -accumulatedPercent * circumference;
                    accumulatedPercent += percent;
                    const isHovered = hoveredModel === slice.model;

                    return (
                        <circle
                            key={slice.model}
                            cx="50"
                            cy="50"
                            r={radius}
                            fill="none"
                            stroke={slice.color}
                            strokeWidth={isHovered ? 13 : 11}
                            strokeDasharray={strokeDasharray}
                            strokeDashoffset={strokeDashoffset}
                            className="cursor-pointer transition-all duration-150 hover:opacity-90"
                            onMouseEnter={() => onHover(slice.model)}
                            onMouseLeave={() => onHover(null)}
                        />
                    );
                })}
            </svg>
            <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center p-1">
                <span className="max-w-[70px] truncate font-mono text-xs font-bold tracking-tight text-gray-900 dark:text-base-content">
                    {formatUsd(activeItem ? activeItem.costUsd : totalCost)}
                </span>
                <span className="max-w-[65px] truncate text-[9px] font-medium text-gray-400 dark:text-gray-500">
                    {activeItem ? `${activeItem.costPercent.toFixed(1)}%` : t(unpricedModels ? 'local_dashboard.priced_subtotal' : 'local_dashboard.total_cost')}
                </span>
            </div>
        </div>
    );
}

function TokenCard({
    cardId,
    label,
    value,
    color,
    icon: Icon,
    displayValue,
    unit,
    detail,
    hint,
    locale,
    children,
}: {
    cardId: DashboardCardId;
    label: string;
    value: number;
    color: string;
    icon: typeof Cpu;
    displayValue?: string;
    unit?: string;
    detail?: string;
    hint?: string;
    locale: string;
    children?: ReactNode;
}) {
    return (
        <div data-dashboard-card={cardId} className="relative min-w-0 rounded-2xl border border-gray-100 bg-white p-3 shadow-sm transition-all hover:border-gray-200 hover:shadow-md dark:border-base-200 dark:bg-base-100">
            <div className="mb-2 flex items-center justify-between">
                <div className="flex min-w-0 items-center gap-1.5 text-xs font-medium leading-4 text-gray-600 dark:text-gray-300">
                    <span className={`shrink-0 rounded-lg p-1.5 ${color}`}>
                        <Icon className="h-3.5 w-3.5" />
                    </span>
                    {label}
                </div>
            </div>
            {children ?? <div className="break-words text-xl font-bold tracking-tight text-gray-900 dark:text-base-content" title={`${displayValue || formatTokens(value, locale)}${unit ? ` ${unit}` : ''}`}>
                {displayValue || compactTokens(value, locale)}
                {unit && <> <span className="inline-block text-sm font-medium text-gray-500 dark:text-gray-400">{unit}</span></>}
            </div>}
            <div
                data-card-detail
                className="mt-1 break-words text-xs leading-5 text-gray-500 dark:text-gray-400"
                title={hint || detail || `${formatTokens(value, locale)} Token`}
            >
                {detail || `${formatTokens(value, locale)} Token`}
            </div>
        </div>
    );
}

function Dashboard() {
    const config = useConfigStore(state => state.config);
    const visibleCards = dashboardCards(config?.dashboard?.cards);
    const accountData = useDashboardOverview(Boolean(config) && visibleCards.some(id => ['account_status', 'aggregate_quota', 'quota_reset'].includes(id)));
    const { t, i18n } = useTranslation();
    const locale = i18n.resolvedLanguage === 'zh' ? 'zh-CN' : 'en-US';
    const quotaPreferences = useMemo(() => ({ ...DEFAULT_MENU_BAR_PREFERENCES, ...config?.menu_bar }), [config?.menu_bar]);
    const overview = useMemo(() => accountData.snapshot ? dashboardOverview(accountData.snapshot, quotaPreferences, accountData.now,
        config?.refresh_interval, accountData.reserve ?? config?.quota_protection.threshold_percentage ?? 10) : null,
    [accountData.snapshot, accountData.now, accountData.reserve, quotaPreferences, config?.refresh_interval, config?.quota_protection.threshold_percentage]);
    const quotaScope = t(`local_dashboard.quota_scope_${quotaPreferences.quota_scope}`);
    const overviewUnavailable = t(accountData.failed ? 'local_dashboard.overview_failed' : accountData.loading ? 'local_dashboard.overview_loading' : 'local_dashboard.overview_unknown');
    const nextReset = overview?.reset;
    const countdown = nextReset ? resetCountdown(nextReset.at, accountData.now) : null;
    const rangeLabels: Record<RangeKey, string> = {
        today: t('local_dashboard.today'),
        yesterday: t('local_dashboard.yesterday'),
        '3d': t('local_dashboard.last_3_days'),
        '7d': t('local_dashboard.last_7_days'),
        '30d': t('local_dashboard.last_30_days'),
    };
    const [usage, setUsage] = useState<LocalTokenUsageSummary | null>(null);
    const [pricing, setPricing] = useState<ApiPricingSnapshot | null>(null);
    const [range, setRange] = useState<RangeKey>('today');
    const [modelViewMode, setModelViewMode] = useState<'tokens' | 'cost'>('cost');
    const [hoveredDonutModel, setHoveredDonutModel] = useState<string | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [hoveredPoint, setHoveredPoint] = useState<TokenChartPoint | null>(null);
    const [lastUpdatedAt, setLastUpdatedAt] = useState<number | null>(null);
    const fetchInFlight = useRef(false);

    const fetchUsage = useCallback(async (notify = false) => {
        if (fetchInFlight.current) return;
        fetchInFlight.current = true;
        setLoading(true);
        setError(null);
        try {
            const result = await invoke<LocalTokenUsageSummary>('get_local_token_usage');
            setUsage(result);
            setLastUpdatedAt(Date.now());
            if (notify) {
                const dataTime = result.last_activity
                    ? t('local_dashboard.toast_data_time', { time: formatTime(result.last_activity * 1000, locale) })
                    : '';
                showToast(t('local_dashboard.toast_updated', { dataTime }), 'success');
            }
        } catch (fetchError) {
            const message = String(fetchError);
            setError(message);
            showToast(t('local_dashboard.toast_scan_failed', { message }), 'error');
        } finally {
            fetchInFlight.current = false;
            setLoading(false);
        }
    }, [locale, t]);

    const fetchPricing = useCallback(async () => {
        try {
            const result = await invoke<ApiPricingSnapshot>('get_api_pricing');
            setPricing(result);
        } catch (fetchError) {
            console.warn('API pricing unavailable:', fetchError);
            setPricing(null);
        }
    }, []);

    useEffect(() => {
        fetchUsage();
        fetchPricing();
        const usageInterval = window.setInterval(fetchUsage, 60_000);
        const pricingInterval = window.setInterval(fetchPricing, 24 * 60 * 60 * 1000);
        return () => {
            window.clearInterval(usageInterval);
            window.clearInterval(pricingInterval);
        };
    }, [fetchPricing, fetchUsage]);

    const totals = useMemo<LocalTokenTotals>(() => {
        if (!usage) {
            return { input_tokens: 0, output_tokens: 0, cached_tokens: 0, total_tokens: 0, request_count: 0 };
        }
        if (range === 'yesterday') return usage.yesterday;
        if (range === '3d') return usage.last_3_days;
        if (range === '7d') return usage.last_7_days;
        if (range === '30d') return usage.last_30_days;
        return usage.today;
    }, [range, usage]);

    useEffect(() => {
        setHoveredPoint(null);
    }, [range]);

    const chartPoints = useMemo<TokenChartPoint[]>(() => {
        const now = new Date();
        if (range === 'today' || range === 'yesterday') {
            const chartDate = new Date(now);
            if (range === 'yesterday') chartDate.setDate(chartDate.getDate() - 1);
            const dayKey = dateKey(chartDate);
            const byHour = new Map((usage?.hourly || []).map((item) => [item.hour, item]));
            return Array.from({ length: 24 }, (_, hour) => {
                const label = `${String(hour).padStart(2, '0')}:00`;
                const key = `${dayKey} ${label}`;
                const item = byHour.get(key);
                return {
                    ...(item || emptyTokenTotals()),
                    key,
                    label,
                };
            });
        }

        const byDate = new Map((usage?.daily || []).map((item) => [item.date, item]));
        const days: TokenChartPoint[] = [];
        for (let offset = rangeDays[range] - 1; offset >= 0; offset -= 1) {
            const date = new Date(now);
            date.setHours(0, 0, 0, 0);
            date.setDate(date.getDate() - offset);
            const key = dateKey(date);
            const item = byDate.get(key);
            days.push({
                ...(item || emptyTokenTotals()),
                key,
                label: shortDate(key, locale),
            });
        }
        return days;
    }, [locale, range, usage]);

    const maxChartTokens = Math.max(...chartPoints.map((point) => point.total_tokens), 1);

    const modelsForRange = useMemo(() => {
        if (!usage) return [];
        if (range === 'today') return usage.by_model_today;
        if (range === 'yesterday') return usage.by_model_yesterday;
        if (range === '3d') return usage.by_model_3_days;
        if (range === '7d') return usage.by_model_7_days;
        return usage.by_model;
    }, [range, usage]);

    const cacheHitRate = useMemo(() => {
        const denominator = totals.input_tokens + totals.cached_tokens;
        return denominator > 0 ? (totals.cached_tokens / denominator) * 100 : 0;
    }, [totals]);

    const apiCost = useMemo(() => estimateApiCost(modelsForRange, pricing), [modelsForRange, pricing]);

    const modelCostList = useMemo<ModelCostBreakdown[]>(() => {
        const list = modelsForRange.map((m) => {
            const p = findModelPricing(m.model, pricing);
            const costUsd = p
                ? (m.input_tokens * p.input + m.output_tokens * p.output + m.cached_tokens * p.cached) / 1_000_000
                : 0;
            return {
                ...m,
                costUsd,
                costPercent: 0,
                isPriced: !!p,
                color: '#64748b',
            };
        });

        list.sort((a, b) => {
            if (b.costUsd !== a.costUsd) return b.costUsd - a.costUsd;
            return b.total_tokens - a.total_tokens;
        });

        const totalCost = list.reduce((acc, cur) => acc + cur.costUsd, 0);

        return list.map((item, idx) => ({
            ...item,
            costPercent: totalCost > 0 ? (item.costUsd / totalCost) * 100 : 0,
            color: MODEL_COLORS[idx % MODEL_COLORS.length],
        }));
    }, [modelsForRange, pricing]);

    // Blended price per token type for the current range, so the chart can show the cost of a
    // single bar. Models without a known price contribute nothing (same caveat as the KPI card).
    const costPerPoint = useMemo(() => {
        const rates = modelsForRange.reduce(
            (acc, model) => {
                const price = findModelPricing(model.model, pricing);
                if (!price) return acc;
                acc.input += model.input_tokens * price.input;
                acc.output += model.output_tokens * price.output;
                acc.cached += model.cached_tokens * price.cached;
                return acc;
            },
            { input: 0, output: 0, cached: 0 },
        );
        return {
            input: totals.input_tokens ? rates.input / totals.input_tokens / 1_000_000 : 0,
            output: totals.output_tokens ? rates.output / totals.output_tokens / 1_000_000 : 0,
            cached: totals.cached_tokens ? rates.cached / totals.cached_tokens / 1_000_000 : 0,
        };
    }, [modelsForRange, pricing, totals]);

    const costForPoint = useCallback(
        (point: TokenChartPoint) =>
            point.input_tokens * costPerPoint.input
            + point.output_tokens * costPerPoint.output
            + point.cached_tokens * costPerPoint.cached,
        [costPerPoint],
    );
    const pricingLabel = pricing
        ? pricing.stale
            ? t('local_dashboard.pricing_local_cache')
            : t('local_dashboard.pricing_google')
        : t('local_dashboard.pricing_fallback');

    const performance = usage?.recent_performance;
    const performanceDetail = performance
        ? t('local_dashboard.recent_performance', {
            count: performance.sample_count,
        })
        : t('local_dashboard.performance_empty');
    const performanceScope = performance ? t('local_dashboard.performance_scope', {
        models: performance.model_count, sources: performance.source_count,
    }) : '';

    // Scan freshness is intentionally not rendered inline: the toolbar row only fits next to
    // the page title when this string stays out of the layout. It is exposed on the refresh button.
    const scanStatus = loading
        ? t('local_dashboard.scanning')
        : lastUpdatedAt
            ? `${t('local_dashboard.scanned_at', { time: formatTime(lastUpdatedAt, locale) })}${usage?.last_activity ? t('local_dashboard.data_through', { time: formatTime(usage.last_activity * 1000, locale) }) : ''}`
            : t('local_dashboard.waiting_scan');

    const cards: Record<DashboardCardId, ReactElement> = {
        total_tokens: (<TokenCard cardId="total_tokens" label={t('local_dashboard.total_tokens', { range: rangeLabels[range] })} value={totals.total_tokens} hint={t('local_dashboard.total_tokens_hint')} color="bg-blue-50 text-blue-600 dark:bg-blue-900/20 dark:text-blue-300" icon={BarChart3} locale={locale} />),
        input_tokens: (<TokenCard cardId="input_tokens" label={t('local_dashboard.input_tokens')} value={totals.input_tokens} hint={t('local_dashboard.input_tokens_hint')} color="bg-indigo-50 text-indigo-600 dark:bg-indigo-900/20 dark:text-indigo-300" icon={MessageSquare} locale={locale} />),
        output_tokens: (<TokenCard cardId="output_tokens" label={t('local_dashboard.output_tokens')} value={totals.output_tokens} hint={t('local_dashboard.output_tokens_hint')} color="bg-purple-50 text-purple-600 dark:bg-purple-900/20 dark:text-purple-300" icon={Cpu} locale={locale} />),
        cache_hit_rate: (<TokenCard cardId="cache_hit_rate"
            label={t('local_dashboard.cache_hit_rate')}
            value={cacheHitRate}
            displayValue={`${cacheHitRate.toFixed(1)}%`}
            detail={t('local_dashboard.cache_hit_rate_detail')}
            hint={t('local_dashboard.cache_hit_rate_hint')}
            color="bg-emerald-50 text-emerald-600 dark:bg-emerald-900/20 dark:text-emerald-300"
            icon={Database}
            locale={locale}
        />),
        api_cost: (<TokenCard cardId="api_cost"
            label={t('local_dashboard.api_cost')}
            value={apiCost.usd}
            displayValue={apiCost.unpricedModels && !apiCost.pricedModels ? t('local_dashboard.unpriced') : formatUsd(apiCost.usd)}
            detail={t('local_dashboard.api_requests', {
                requestCount: formatTokens(totals.request_count, locale),
                pricing: pricingLabel,
                unpriced: apiCost.unpricedModels ? t('local_dashboard.pricing_unavailable') : '',
            })}
            hint={t('local_dashboard.api_cost_hint')}
            color="bg-amber-50 text-amber-600 dark:bg-amber-900/20 dark:text-amber-300"
            icon={DollarSign}
            locale={locale}
        />),
        first_text_latency: (<TokenCard cardId="first_text_latency"
            label={t('local_dashboard.first_text_latency')}
            value={performance?.first_text_seconds ?? 0}
            displayValue={performance ? t('local_dashboard.seconds_value', { value: performance.first_text_seconds.toLocaleString(locale, { maximumFractionDigits: 2 }) }) : '—'}
            detail={performanceDetail}
            hint={`${performanceDetail}\n${performanceScope}\n${t('local_dashboard.first_text_hint')}`}
            color="bg-cyan-50 text-cyan-600 dark:bg-cyan-900/20 dark:text-cyan-300"
            icon={Timer}
            locale={locale}
        />),
        body_speed: (<TokenCard cardId="body_speed"
            label={t('local_dashboard.body_speed')}
            value={performance?.body_tokens_per_second ?? 0}
            displayValue={performance ? performance.body_tokens_per_second.toLocaleString(locale, { maximumFractionDigits: 1 }) : '—'}
            unit={performance ? 'token/s' : undefined}
            detail={performanceDetail}
            hint={`${performanceDetail}\n${performanceScope}\n${t('local_dashboard.body_speed_hint')}`}
            color="bg-rose-50 text-rose-600 dark:bg-rose-900/20 dark:text-rose-300"
            icon={Gauge}
            locale={locale}
        />),
        account_status: (<TokenCard cardId="account_status"
            label={t('local_dashboard.account_status')}
            value={overview?.accounts.total ?? 0}
            displayValue={overview ? formatTokens(overview.accounts.total, locale) : '—'}
            unit={overview ? t('local_dashboard.accounts_unit') : undefined}
            detail={overview ? t('local_dashboard.account_status_detail', overview.accounts) : overviewUnavailable}
            hint={t('local_dashboard.account_status_hint', { scope: quotaScope, threshold: overview?.threshold ?? 10 })}
            color="bg-teal-50 text-teal-600 dark:bg-teal-900/20 dark:text-teal-300"
            icon={Users}
            locale={locale}
        />),
        aggregate_quota: (<TokenCard cardId="aggregate_quota"
            label={t('local_dashboard.aggregate_quota')}
            value={0}
            detail={overview ? t('local_dashboard.quota_average_detail', { scope: quotaScope }) : overviewUnavailable}
            hint={`${t('local_dashboard.quota_used_hint')}\n${overview ? t('local_dashboard.quota_coverage', {
                session: overview.windows['5h'].covered, weekly: overview.windows.weekly.covered, total: overview.windows['5h'].total,
            }) : overviewUnavailable}`}
            color="bg-orange-50 text-orange-600 dark:bg-orange-900/20 dark:text-orange-300"
            icon={PieChart}
            locale={locale}
        >
            <div className="space-y-1">
                {(['5h', 'weekly'] as const).map(window => <div key={window} data-quota-window={window} className="flex flex-wrap items-baseline justify-between gap-x-2">
                    <span className="text-xs leading-5 text-gray-600 dark:text-gray-300">{t(`local_dashboard.quota_used_${window}`)}</span>
                    <span className="text-lg font-bold tabular-nums tracking-tight text-gray-900 dark:text-base-content">{quotaDisplay(overview?.windows[window].used ?? null)}</span>
                </div>)}
            </div>
        </TokenCard>),
        quota_reset: (<TokenCard cardId="quota_reset"
            label={t('local_dashboard.quota_reset')}
            value={0}
            displayValue={countdown ? t(`local_dashboard.${countdown.key}`, countdown) : '—'}
            detail={nextReset ? t('local_dashboard.quota_reset_detail', { window: t(`local_dashboard.quota_reset_${nextReset.window}`), count: nextReset.accounts })
                : overview ? t('local_dashboard.quota_reset_empty') : overviewUnavailable}
            hint={`${t('local_dashboard.quota_reset_hint', { scope: quotaScope })}${nextReset ? `\n${new Date(nextReset.at).toLocaleString(locale)}` : ''}`}
            color="bg-violet-50 text-violet-600 dark:bg-violet-900/20 dark:text-violet-300"
            icon={Clock3}
            locale={locale}
        />),
    };

    return (
        <div className="h-full w-full overflow-y-auto">
            <div className="mx-auto flex min-h-full max-w-7xl flex-col gap-2 p-3 lg:h-full lg:min-h-0 lg:p-4">
                <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="min-w-0">
                        <h1 className="flex items-center gap-2 text-xl font-bold text-gray-900 dark:text-base-content">
                            <LayoutDashboard className="h-5 w-5 text-blue-500" />
                            {t('local_dashboard.title')}
                        </h1>
                        <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">
                            {t('local_dashboard.subtitle')}
                        </p>
                    </div>
                    <div className="flex flex-wrap items-center justify-end gap-2">
                        <div className="flex items-center gap-1 rounded-xl border border-gray-100 bg-white p-1 shadow-sm dark:border-base-200 dark:bg-base-100">
                            <div className="flex items-center gap-1 px-1 text-[11px] text-gray-500 dark:text-gray-400">
                                <CalendarDays className="h-3.5 w-3.5" />
                                <span>{t('local_dashboard.range_label')}</span>
                            </div>
                            {(Object.keys(rangeLabels) as RangeKey[]).map((key) => (
                                <button
                                    key={key}
                                    onClick={() => setRange(key)}
                                    className={`rounded-lg px-2 py-1 text-[11px] font-medium transition-colors ${range === key
                                        ? 'bg-blue-50 text-blue-600 shadow-sm dark:bg-blue-900/20 dark:text-blue-400'
                                        : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
                                        }`}
                                >
                                    {rangeLabels[key]}
                                </button>
                            ))}
                        </div>
                        <button
                            onClick={() => { void fetchUsage(true); void accountData.refresh(); }}
                            disabled={loading}
                            title={scanStatus}
                            className="flex items-center gap-1.5 rounded-xl bg-blue-500 px-2.5 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-600 disabled:cursor-not-allowed disabled:opacity-60"
                        >
                            <RefreshCw className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`} />
                            {loading ? t('local_dashboard.refreshing') : t('local_dashboard.refresh')}
                        </button>
                    </div>
                </div>

                {error && (
                    <div className="rounded-xl border border-red-100 bg-red-50 px-3 py-2 text-xs text-red-600 dark:border-red-900/30 dark:bg-red-900/10 dark:text-red-300">
                        {error}
                    </div>
                )}

                {!!usage?.unreadable_databases && (
                    <div className="rounded-xl border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-700 dark:border-amber-900/30 dark:bg-amber-900/10 dark:text-amber-300">
                        {t('local_dashboard.unreadable_databases', { count: usage.unreadable_databases })}
                    </div>
                )}

                {visibleCards.length > 0 && <div className={`grid shrink-0 gap-2 ${dashboardGridClass(visibleCards.length)}`} data-dashboard-cards>
                    {visibleCards.map(id => <Fragment key={id}>{cards[id]}</Fragment>)}
                </div>}

                <div className="grid shrink-0 gap-2 lg:h-[176px] lg:grid-cols-[1.35fr_1fr]">
                    <section className="rounded-2xl border border-gray-100 bg-white p-3 shadow-sm dark:border-base-200 dark:bg-base-100 lg:flex lg:min-h-0 lg:flex-col">
                        <div className="mb-1.5 flex flex-col gap-1.5 sm:flex-row sm:items-start sm:justify-between">
                            <div>
                                <h2 className="flex items-center gap-2 text-sm font-semibold text-gray-900 dark:text-base-content">
                                    <Activity className="h-4 w-4 text-blue-500" />
                                    {rangeLabels[range]}
                                </h2>
                                <p className="mt-0.5 text-[11px] text-gray-400 dark:text-gray-500">{t('local_dashboard.chart_subtitle')}</p>
                            </div>
                            <div className="pointer-events-none h-[60px] w-full shrink-0 text-right sm:w-[236px]" aria-live="polite">
                                {hoveredPoint ? (
                                    <div className="flex h-full flex-col justify-center gap-1">
                                        <div className="flex items-baseline justify-end gap-1.5">
                                            <span className="text-[10px] font-medium text-blue-600 dark:text-blue-400">{hoveredPoint.label}</span>
                                            <span className="whitespace-nowrap text-sm font-bold leading-4 text-gray-900 dark:text-base-content">
                                                {formatTokens(hoveredPoint.total_tokens, locale)} {t('local_dashboard.token')}
                                            </span>
                                        </div>
                                        <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-[10px] leading-3">
                                            <div className="flex items-baseline justify-end gap-1">
                                                <span className="text-gray-400 dark:text-gray-500">{t('local_dashboard.input_short')}</span>
                                                <span className="font-mono text-gray-600 dark:text-gray-300">{compactTokens(hoveredPoint.input_tokens, locale)}</span>
                                            </div>
                                            <div className="flex items-baseline justify-end gap-1">
                                                <span className="text-gray-400 dark:text-gray-500">{t('local_dashboard.output_short')}</span>
                                                <span className="font-mono text-gray-600 dark:text-gray-300">{compactTokens(hoveredPoint.output_tokens, locale)}</span>
                                            </div>
                                            <div className="flex items-baseline justify-end gap-1">
                                                <span className="text-gray-400 dark:text-gray-500">{t('local_dashboard.cached_tokens')}</span>
                                                <span className="font-mono text-gray-600 dark:text-gray-300">{compactTokens(hoveredPoint.cached_tokens, locale)}</span>
                                            </div>
                                            <div className="flex items-baseline justify-end gap-1">
                                                <span className="text-gray-400 dark:text-gray-500">{t('local_dashboard.request_count')}</span>
                                                <span className="font-mono text-gray-600 dark:text-gray-300">{formatTokens(hoveredPoint.request_count, locale)}</span>
                                            </div>
                                            <div className="col-span-2 flex items-baseline justify-end gap-1">
                                                <span className="text-gray-400 dark:text-gray-500">{t('local_dashboard.estimated_cost')}</span>
                                                <span className="font-mono font-medium text-amber-600 dark:text-amber-400">{apiCost.unpricedModels && !apiCost.pricedModels ? t('local_dashboard.unpriced') : formatUsd(costForPoint(hoveredPoint))}</span>
                                            </div>
                                        </div>
                                    </div>
                                ) : (
                                    <div className="flex h-full items-center justify-end text-[11px] text-gray-400 dark:text-gray-500">
                                        {t('local_dashboard.hover_for_usage')}
                                    </div>
                                )}
                            </div>
                        </div>
                        <div
                            className="flex h-24 items-end gap-1 border-b border-gray-100 pb-1 dark:border-base-200 lg:min-h-0 lg:flex-1"
                            onMouseLeave={() => setHoveredPoint(null)}
                        >
                            {chartPoints.map((point, index) => {
                                const isEmpty = point.total_tokens === 0;
                                const height = Math.max((point.total_tokens / maxChartTokens) * 100, 6);
                                const isHourlyRange = range === 'today' || range === 'yesterday';
                                const showPointLabel = isHourlyRange
                                    ? index % 3 === 0 || index === chartPoints.length - 1
                                    : range !== '30d' || index % 5 === 0 || index === chartPoints.length - 1;
                                return (
                                    <div
                                        key={point.key}
                                        className="group flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-2"
                                        onMouseEnter={() => setHoveredPoint(point)}
                                    >
                                        <div className="relative flex w-full min-w-0 flex-1 items-end justify-center">
                                            <div
                                                className={`shrink-0 bg-gradient-to-t from-blue-500 to-cyan-400 transition-[filter,opacity] duration-150 group-hover:brightness-95 ${isEmpty ? 'rounded-none opacity-20' : 'rounded-t-[3px]'} ${hoveredPoint?.key === point.key ? 'brightness-95' : ''}`}
                                                style={{
                                                    width: '70%',
                                                    maxWidth: `${chartBarMaxWidth[range]}px`,
                                                    height: isEmpty ? '2px' : `${height}%`,
                                                }}
                                            />
                                        </div>
                                        <span className="w-full whitespace-nowrap text-center text-[10px] text-gray-400 dark:text-gray-500">{showPointLabel ? point.label : '\u00a0'}</span>
                                    </div>
                                );
                            })}
                        </div>
                    </section>

                    <section className="rounded-2xl border border-gray-100 bg-white p-3 shadow-sm dark:border-base-200 dark:bg-base-100 lg:flex lg:min-h-0 lg:flex-col">
                        <div className="mb-1.5 flex items-center justify-between">
                            <div>
                                <h2 className="flex items-center gap-2 text-sm font-semibold text-gray-900 dark:text-base-content">
                                    <PieChart className="h-4 w-4 text-blue-500" />
                                    {t('local_dashboard.model_breakdown')}
                                </h2>
                                <p className="mt-0.5 text-[11px] text-gray-400 dark:text-gray-500">
                                    {modelViewMode === 'cost' && apiCost.unpricedModels ? t('local_dashboard.unpriced_models_hint', { count: apiCost.unpricedModels }) : t('local_dashboard.local_records', { range: rangeLabels[range] })}
                                </p>
                            </div>
                            <div className="flex items-center gap-1 rounded-lg bg-gray-100 p-0.5 dark:bg-base-200">
                                <button
                                    type="button"
                                    onClick={() => setModelViewMode('tokens')}
                                    className={`px-2 py-0.5 text-[10px] font-medium rounded-md transition-all ${
                                        modelViewMode === 'tokens'
                                            ? 'bg-white text-blue-600 shadow-sm dark:bg-base-100 dark:text-blue-400'
                                            : 'text-gray-500 hover:text-gray-700 dark:text-gray-400'
                                    }`}
                                >
                                    {t('local_dashboard.view_tokens')}
                                </button>
                                <button
                                    type="button"
                                    onClick={() => setModelViewMode('cost')}
                                    className={`flex items-center gap-1 px-2 py-0.5 text-[10px] font-medium rounded-md transition-all ${
                                        modelViewMode === 'cost'
                                            ? 'bg-white text-emerald-600 shadow-sm dark:bg-base-100 dark:text-emerald-400'
                                            : 'text-gray-500 hover:text-gray-700 dark:text-gray-400'
                                    }`}
                                >
                                    <PieChart className="h-3 w-3" />
                                    <span>{t('local_dashboard.view_estimated_cost')}</span>
                                </button>
                            </div>
                        </div>

                        {modelViewMode === 'tokens' ? (
                            <div className="max-h-36 space-y-1.5 overflow-y-auto pr-1 lg:min-h-0 lg:flex-1 lg:max-h-none">
                                {modelsForRange.slice(0, 8).map((model) => {
                                    const width = totals.total_tokens
                                        ? Math.max((model.total_tokens / totals.total_tokens) * 100, 2)
                                        : 0;
                                    return (
                                        <div key={model.model}>
                                            <div className="mb-0.5 flex items-center justify-between gap-3 text-[11px]">
                                                <span className="truncate text-gray-600 dark:text-gray-300" title={model.model}>{model.model}</span>
                                                <span className="shrink-0 font-mono text-gray-500 dark:text-gray-400">{compactTokens(model.total_tokens, locale)}</span>
                                            </div>
                                            <div className="h-1 overflow-hidden rounded-full bg-gray-100 dark:bg-base-200">
                                                <div className="h-full rounded-full bg-purple-400" style={{ width: `${width}%` }} />
                                            </div>
                                        </div>
                                    );
                                })}
                                {!loading && !modelsForRange.length && (
                                    <div className="py-6 text-center text-xs text-gray-400 dark:text-gray-500">{t('local_dashboard.no_model_usage')}</div>
                                )}
                            </div>
                        ) : (
                            <div className="flex h-full items-center gap-3 overflow-hidden py-1 lg:min-h-0 lg:flex-1">
                                <ModelCostDonut
                                    data={modelCostList.filter((m) => m.costUsd > 0)}
                                    totalCost={apiCost.usd}
                                    unpricedModels={apiCost.unpricedModels}
                                    hoveredModel={hoveredDonutModel}
                                    onHover={setHoveredDonutModel}
                                />
                                <div className="flex-1 min-w-0 max-h-32 overflow-y-auto pr-1 space-y-1 lg:max-h-none">
                                    {modelCostList.filter((m) => m.costUsd > 0).map((item) => (
                                        <div
                                            key={item.model}
                                            onMouseEnter={() => setHoveredDonutModel(item.model)}
                                            onMouseLeave={() => setHoveredDonutModel(null)}
                                            className={`flex items-center justify-between text-[11px] p-1 rounded-lg transition-colors cursor-pointer ${
                                                hoveredDonutModel === item.model
                                                    ? 'bg-amber-50 dark:bg-amber-900/20'
                                                    : 'hover:bg-gray-50 dark:hover:bg-base-200/50'
                                            }`}
                                        >
                                            <div className="flex items-center gap-1.5 min-w-0 mr-2">
                                                <span className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: item.color }} />
                                                <span className="truncate font-medium text-gray-700 dark:text-gray-300" title={item.model}>
                                                    {item.model}
                                                </span>
                                            </div>
                                            <div className="text-right shrink-0 font-mono">
                                                <span className="font-semibold text-gray-900 dark:text-gray-100">{formatUsd(item.costUsd)}</span>
                                                <span className="text-[10px] text-gray-400 ml-1">({item.costPercent.toFixed(1)}%)</span>
                                            </div>
                                        </div>
                                    ))}
                                    {!loading && !modelCostList.filter((m) => m.costUsd > 0).length && (
                                        <div className="py-6 text-center text-xs text-gray-400 dark:text-gray-500">
                                            {t(apiCost.unpricedModels ? 'local_dashboard.pricing_pending' : 'local_dashboard.no_cost_in_range')}
                                        </div>
                                    )}
                                </div>
                            </div>
                        )}
                    </section>
                </div>

                <section className="overflow-hidden rounded-2xl border border-gray-100 bg-white shadow-sm dark:border-base-200 dark:bg-base-100 lg:flex lg:min-h-[160px] lg:flex-1 lg:flex-col">
                    <div className="flex shrink-0 items-center justify-between border-b border-gray-100 px-4 py-2 dark:border-base-200">
                        <div>
                            <h2 className="flex items-center gap-2 text-sm font-semibold text-gray-900 dark:text-base-content">
                                <Cpu className="h-4 w-4 text-blue-500" />
                                {t('local_dashboard.model_details')}
                            </h2>
                            <p className="mt-0.5 text-[11px] text-gray-400 dark:text-gray-500">{t('local_dashboard.model_details_desc', { range: rangeLabels[range] })}</p>
                        </div>
                        <span className="text-[11px] text-gray-400 dark:text-gray-500">{t('local_dashboard.model_count', { count: modelsForRange.length })}</span>
                    </div>
                    <div className="max-h-[142px] overflow-y-auto lg:min-h-0 lg:flex-1 lg:max-h-none">
                        <div className="overflow-x-auto">
                            <table className="w-full min-w-[560px] text-left text-xs">
                                <thead className="bg-gray-50 text-[10px] uppercase text-gray-500 dark:bg-slate-800 dark:text-gray-300">
                                    <tr>
                                        <th className="px-4 py-1.5 font-medium">{t('local_dashboard.model')}</th>
                                        <th className="px-4 py-1.5 text-right font-medium">{t('local_dashboard.estimated_cost')}</th>
                                        <th className="px-4 py-1.5 text-right font-medium">{t('local_dashboard.total_tokens_column')}</th>
                                        <th className="px-4 py-1.5 text-right font-medium">{t('local_dashboard.input_short')}</th>
                                        <th className="px-4 py-1.5 text-right font-medium">{t('local_dashboard.output_short')}</th>
                                        <th className="px-4 py-1.5 text-right font-medium">{t('local_dashboard.request_count')}</th>
                                    </tr>
                                </thead>
                                <tbody className="divide-y divide-gray-100 dark:divide-base-200">
                                    {modelCostList.map((model) => (
                                        <tr key={model.model} className="text-gray-700 dark:text-gray-300">
                                            <td className="max-w-[300px] truncate px-4 py-1.5 font-medium" title={model.model}>
                                                <div className="flex items-center gap-1.5">
                                                    <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: model.color }} />
                                                    <span className="truncate">{model.model}</span>
                                                </div>
                                            </td>
                                            <td className="px-4 py-1.5 text-right font-mono font-medium text-amber-600 dark:text-amber-400">
                                                {model.isPriced ? (
                                                    formatUsd(model.costUsd)
                                                ) : (
                                                    <span className="text-gray-400 dark:text-gray-500" title={t('local_dashboard.pricing_unavailable')}>
                                                        {t('local_dashboard.unpriced')}
                                                    </span>
                                                )}
                                            </td>
                                            <td className="px-4 py-1.5 text-right font-mono">{formatTokens(model.total_tokens, locale)}</td>
                                            <td className="px-4 py-1.5 text-right font-mono text-indigo-500">{formatTokens(model.input_tokens, locale)}</td>
                                            <td className="px-4 py-1.5 text-right font-mono text-purple-500">{formatTokens(model.output_tokens, locale)}</td>
                                            <td className="px-4 py-1.5 text-right font-mono">{formatTokens(model.request_count, locale)}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                            {!loading && !modelsForRange.length && (
                                <div className="px-4 py-8 text-center text-xs text-gray-400 dark:text-gray-500">
                                    {t('local_dashboard.no_token_records')}
                                </div>
                            )}
                        </div>
                    </div>
                </section>
            </div>
        </div>
    );
}

export default Dashboard;
