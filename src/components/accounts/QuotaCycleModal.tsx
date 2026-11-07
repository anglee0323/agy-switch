import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { createPortal } from 'react-dom';
import {
    Activity,
    AlertCircle,
    BarChart3,
    Calendar,
    ChevronDown,
    ChevronRight,
    Clock,
    DollarSign,
    Filter,
    Layers,
    RefreshCw,
    TrendingDown,
    TrendingUp,
    X,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { request } from '../../utils/request';
import { cn } from '../../utils/cn';
import { formatCurrencyCompact } from '../../utils/currency';
import { useConfigStore } from '../../stores/useConfigStore';

export interface CycleCheckpoint {
    timestamp: number;
    remaining_fraction: number;
    delta_fraction: number;
}

export interface QuotaCycleRecord {
    cycle_id: string;
    account_id: string;
    pool: 'gemini' | 'third_party' | 'argon' | 'unknown';
    window: string;
    start_time: string;
    reset_time: string;
    first_observed_remaining: number;
    latest_remaining: number;
    min_remaining: number;
    consumed_fraction: number;
    last_delta_fraction: number;
    sample_count: number;
    status: 'active' | 'completed';
    used_usd?: number;
    estimated_total_usd?: number;
    prev_cycle_total_usd?: number;
    delta_total_usd?: number;
    delta_total_pct?: number;
    checkpoints: CycleCheckpoint[];
    email?: string;
    subscription_tier?: string;
}

export interface QuotaCycleAccountSummary {
    account_id: string;
    email: string;
    total_cycles: number;
    active_cycles: number;
    total_consumed_usd: number;
    latest_reset_time?: string;
}

export interface QuotaCycleHistoryResponse {
    cycles: QuotaCycleRecord[];
    accounts_summary: QuotaCycleAccountSummary[];
    currency: 'usd' | 'cny' | 'both';
    usd_to_cny_rate: number;
    queried_at: number;
}

interface QuotaCycleModalProps {
    isOpen: boolean;
    onClose: () => void;
    initialAccountId?: string;
}

export default function QuotaCycleModal({
    isOpen,
    onClose,
    initialAccountId,
}: QuotaCycleModalProps) {
    const { i18n } = useTranslation();
    const isZh = i18n.language.startsWith('zh');
    const { config } = useConfigStore();
    const currencyMode = config?.quota_valuation?.currency ?? 'usd';

    const [loading, setLoading] = useState(false);
    const [data, setData] = useState<QuotaCycleHistoryResponse | null>(null);
    const [error, setError] = useState<string | null>(null);

    // 过滤条件
    const [selectedAccount, setSelectedAccount] = useState<string>(initialAccountId || 'all');
    const [selectedPool, setSelectedPool] = useState<string>('all');
    const [selectedWindow, setSelectedWindow] = useState<string>('all');
    const [selectedStatus, setSelectedStatus] = useState<string>('all');

    // 展开检查点详情的行
    const [expandedCycleId, setExpandedCycleId] = useState<string | null>(null);

    const loadData = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const resp = await request<QuotaCycleHistoryResponse>('get_quota_cycle_history', {
                accountId: selectedAccount === 'all' ? undefined : selectedAccount,
            });
            setData(resp);
        } catch (err: any) {
            setError(err?.toString() || 'Failed to fetch cycle history');
        } finally {
            setLoading(false);
        }
    }, [selectedAccount]);

    useEffect(() => {
        if (isOpen) {
            if (initialAccountId) {
                setSelectedAccount(initialAccountId);
            }
            loadData();
        }
    }, [isOpen, initialAccountId, loadData]);

    // 过滤后的周期列表
    const filteredCycles = useMemo(() => {
        if (!data?.cycles) return [];
        return data.cycles.filter(c => {
            if (selectedAccount !== 'all' && c.account_id !== selectedAccount) return false;
            if (selectedPool !== 'all' && c.pool !== selectedPool) return false;
            if (selectedWindow !== 'all') {
                const isWeekly = c.window.toLowerCase().includes('week');
                if (selectedWindow === 'weekly' && !isWeekly) return false;
                if (selectedWindow === '5h' && isWeekly) return false;
            }
            if (selectedStatus !== 'all' && c.status !== selectedStatus) return false;
            return true;
        });
    }, [data?.cycles, selectedAccount, selectedPool, selectedWindow, selectedStatus]);

    // 汇总指标计算
    const metrics = useMemo(() => {
        if (!data?.cycles || data.cycles.length === 0) {
            return {
                totalCycles: 0,
                activeCycles: 0,
                totalConsumedUsd: 0,
                totalConsumedCny: 0,
                totalCheckpoints: 0,
                avgDeltaPct: 0,
            };
        }
        const active = filteredCycles.filter(c => c.status === 'active').length;
        const totalUsedUsd = filteredCycles.reduce((sum, c) => sum + (c.used_usd ?? 0), 0);
        const rate = data.usd_to_cny_rate || 7.2;
        const totalCheckpoints = filteredCycles.reduce((sum, c) => sum + (c.checkpoints?.length || 0), 0);

        const cyclesWithDelta = filteredCycles.filter(c => c.delta_total_pct != null);
        const avgDelta = cyclesWithDelta.length > 0
            ? cyclesWithDelta.reduce((sum, c) => sum + (c.delta_total_pct ?? 0), 0) / cyclesWithDelta.length
            : 0;

        return {
            totalCycles: filteredCycles.length,
            activeCycles: active,
            totalConsumedUsd: totalUsedUsd,
            totalConsumedCny: totalUsedUsd * rate,
            totalCheckpoints,
            avgDeltaPct: avgDelta,
        };
    }, [filteredCycles, data?.usd_to_cny_rate]);

    // 趋势图展示的最近 10 个周期
    const chartCycles = useMemo(() => {
        return [...filteredCycles].slice(0, 10).reverse();
    }, [filteredCycles]);

    // 格式化时间辅助函数
    const formatTime = (isoString?: string) => {
        if (!isoString) return '—';
        try {
            const d = new Date(isoString);
            return d.toLocaleString(isZh ? 'zh-CN' : 'en-US', {
                month: 'numeric',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit',
            });
        } catch {
            return isoString;
        }
    };

    if (!isOpen) return null;

    return createPortal(
        <div
            className="modal modal-open z-[9999] flex items-center justify-center bg-black/60 backdrop-blur-sm"
            style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0 }}
        >
            {/* Draggable Top Region */}
            <div data-tauri-drag-region className="fixed top-0 left-0 right-0 h-8 z-[1]" />

            {/* Click outside to close */}
            <div className="absolute inset-0 z-[0]" onClick={onClose} />

            <div className="bg-white dark:bg-base-100 text-gray-900 dark:text-base-content rounded-2xl shadow-2xl w-full max-w-5xl p-6 relative z-[10] m-4 max-h-[94vh] flex flex-col border border-gray-200 dark:border-white/10">
                {/* 1. Header */}
                <div className="flex items-center justify-between pb-3.5 border-b border-gray-100 dark:border-white/10 shrink-0">
                    <div>
                        <div className="flex items-center gap-2">
                            <h3 className="font-bold text-lg text-gray-900 dark:text-white flex items-center gap-2">
                                <Activity className="w-5 h-5 text-indigo-500" />
                                {isZh ? '周期额度变化与消耗统计' : 'Quota Cycle History & Analytics'}
                            </h3>
                            <span className="text-[11px] px-2 py-0.5 rounded-full bg-indigo-50 dark:bg-indigo-950/40 text-indigo-600 dark:text-indigo-400 font-mono font-medium border border-indigo-200/50 dark:border-indigo-800/50">
                                5h / Weekly
                            </span>
                        </div>
                        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                            {isZh
                                ? '自动记录账号在各重置周期内的起始余量、实际消耗幅度、检查点采样与环比变化'
                                : 'Tracks initial quota, real usage deltas, checkpoints, and cycle-over-cycle changes'}
                        </p>
                    </div>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={loadData}
                            disabled={loading}
                            className="p-2 rounded-xl text-gray-500 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-white/10 transition-colors"
                            title={isZh ? '刷新历史数据' : 'Refresh'}
                        >
                            <RefreshCw className={cn('w-4 h-4', loading && 'animate-spin')} />
                        </button>
                        <button
                            onClick={onClose}
                            className="p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-white/10 transition-colors"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>
                </div>

                {/* 2. 4 张统计汇总卡 */}
                <div className="grid grid-cols-2 md:grid-cols-4 gap-3 my-3.5 shrink-0">
                    {/* 卡 1：监控周期数 */}
                    <div className="p-3 rounded-xl bg-gray-50/80 dark:bg-white/[0.03] border border-gray-200/70 dark:border-white/[0.08]">
                        <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
                            <span>{isZh ? '监控周期数' : 'Monitored Cycles'}</span>
                            <Calendar className="w-4 h-4 text-sky-500" />
                        </div>
                        <div className="mt-1.5 flex items-baseline gap-1.5">
                            <span className="text-xl font-bold font-mono text-gray-900 dark:text-white">
                                {metrics.totalCycles}
                            </span>
                            <span className="text-xs text-gray-400">
                                ({isZh ? `活跃 ${metrics.activeCycles}` : `Active ${metrics.activeCycles}`})
                            </span>
                        </div>
                    </div>

                    {/* 卡 2：累计消耗费用 */}
                    <div className="p-3 rounded-xl bg-gray-50/80 dark:bg-white/[0.03] border border-gray-200/70 dark:border-white/[0.08]">
                        <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
                            <span>{isZh ? '累计周期消耗' : 'Total Consumed'}</span>
                            <DollarSign className="w-4 h-4 text-emerald-500" />
                        </div>
                        <div className="mt-1.5 flex items-baseline gap-1.5">
                            <span className="text-xl font-bold font-mono text-gray-900 dark:text-white">
                                {formatCurrencyCompact(metrics.totalConsumedUsd, metrics.totalConsumedCny, currencyMode)}
                            </span>
                        </div>
                    </div>

                    {/* 卡 3：平均环比变化 */}
                    <div className="p-3 rounded-xl bg-gray-50/80 dark:bg-white/[0.03] border border-gray-200/70 dark:border-white/[0.08]">
                        <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
                            <span>{isZh ? '平均环比变动' : 'Avg. CoC Delta'}</span>
                            {metrics.avgDeltaPct >= 0 ? (
                                <TrendingUp className="w-4 h-4 text-amber-500" />
                            ) : (
                                <TrendingDown className="w-4 h-4 text-emerald-500" />
                            )}
                        </div>
                        <div className="mt-1.5 flex items-baseline gap-1.5">
                            <span
                                className={cn(
                                    'text-xl font-bold font-mono',
                                    metrics.avgDeltaPct > 0
                                        ? 'text-rose-600 dark:text-rose-400'
                                        : metrics.avgDeltaPct < 0
                                        ? 'text-emerald-600 dark:text-emerald-400'
                                        : 'text-gray-600 dark:text-gray-300'
                                )}
                            >
                                {metrics.avgDeltaPct > 0 ? `+${metrics.avgDeltaPct.toFixed(1)}%` : `${metrics.avgDeltaPct.toFixed(1)}%`}
                            </span>
                            <span className="text-xs text-gray-400">
                                {isZh ? '跨周期基准' : 'vs prev'}
                            </span>
                        </div>
                    </div>

                    {/* 卡 4：采样检查点 */}
                    <div className="p-3 rounded-xl bg-gray-50/80 dark:bg-white/[0.03] border border-gray-200/70 dark:border-white/[0.08]">
                        <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
                            <span>{isZh ? '有效检查点' : 'Checkpoints'}</span>
                            <Layers className="w-4 h-4 text-violet-500" />
                        </div>
                        <div className="mt-1.5 flex items-baseline gap-1.5">
                            <span className="text-xl font-bold font-mono text-gray-900 dark:text-white">
                                {metrics.totalCheckpoints}
                            </span>
                            <span className="text-xs text-gray-400">
                                {isZh ? '防刷屏去重' : 'Filtered'}
                            </span>
                        </div>
                    </div>
                </div>

                {/* 3. 周期额度变化与消耗趋势可视化柱状图（纯 Tailwind 样式） */}
                {chartCycles.length > 0 && (
                    <div className="p-3 mb-3 rounded-xl bg-gray-50/60 dark:bg-white/[0.02] border border-gray-200/60 dark:border-white/[0.06] shrink-0">
                        <div className="flex items-center justify-between text-xs font-semibold text-gray-700 dark:text-gray-300 mb-2">
                            <span className="flex items-center gap-1.5">
                                <BarChart3 className="w-3.5 h-3.5 text-indigo-500" />
                                {isZh ? '近期周期消耗深度对比图' : 'Recent Cycle Consumption Trend'}
                            </span>
                            <span className="text-[10px] text-gray-400 font-normal">
                                {isZh ? '柱高代表周期内已消耗额度占比 (%)' : 'Bar height shows consumed fraction (%)'}
                            </span>
                        </div>
                        <div className="h-20 flex items-end gap-2 pt-2 px-1">
                            {chartCycles.map((cycle) => {
                                const consumedPct = Math.round(cycle.consumed_fraction * 100);
                                const barHeight = Math.max(8, Math.min(100, consumedPct));
                                const rate = data?.usd_to_cny_rate || 7.2;
                                const costStr = cycle.used_usd != null && cycle.used_usd > 0
                                    ? formatCurrencyCompact(cycle.used_usd, cycle.used_usd * rate, currencyMode)
                                    : '0';

                                return (
                                    <div
                                        key={cycle.cycle_id}
                                        className="flex-1 flex flex-col items-center gap-1 group relative h-full justify-end"
                                        title={`${cycle.pool} (${cycle.window})\n${isZh ? '已消耗' : 'Consumed'}: ${consumedPct}% (${costStr})\n${formatTime(cycle.reset_time)}`}
                                    >
                                        <div className="text-[9px] font-mono text-gray-500 dark:text-gray-400 opacity-80 group-hover:opacity-100 font-semibold">
                                            {consumedPct}%
                                        </div>
                                        <div className="w-full bg-gray-200 dark:bg-white/10 rounded-t-md overflow-hidden flex flex-col justify-end" style={{ height: '52px' }}>
                                            <div
                                                className={cn(
                                                    'w-full rounded-t-md transition-all duration-300',
                                                    consumedPct > 60
                                                        ? 'bg-rose-500 dark:bg-rose-600'
                                                        : consumedPct > 20
                                                        ? 'bg-amber-500 dark:bg-amber-600'
                                                        : 'bg-emerald-500 dark:bg-emerald-600'
                                                )}
                                                style={{ height: `${barHeight}%` }}
                                            />
                                        </div>
                                        <div className="text-[8px] font-mono text-gray-400 truncate w-full text-center">
                                            {cycle.pool.slice(0, 3)}·{cycle.window}
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    </div>
                )}

                {/* 4. 过滤器工具栏（包含窗口过滤：全部 / 5H / 周配额） */}
                <div className="flex flex-wrap items-center justify-between gap-2.5 pb-2.5 shrink-0">
                    <div className="flex flex-wrap items-center gap-2">
                        {/* 账号下拉 */}
                        <div className="flex items-center gap-1.5 text-xs text-gray-500">
                            <Filter className="w-3.5 h-3.5" />
                            <select
                                value={selectedAccount}
                                onChange={e => setSelectedAccount(e.target.value)}
                                className="px-2.5 py-1 rounded-lg text-xs bg-gray-100 dark:bg-base-200 border-none font-medium text-gray-800 dark:text-gray-200 focus:ring-1 focus:ring-indigo-500"
                            >
                                <option value="all">{isZh ? '全部账号' : 'All Accounts'}</option>
                                {data?.accounts_summary?.map(a => (
                                    <option key={a.account_id} value={a.account_id}>
                                        {a.email} ({a.total_cycles})
                                    </option>
                                ))}
                            </select>
                        </div>

                        {/* 窗口类型过滤（5H / 周配额） */}
                        <div className="flex items-center gap-1 bg-gray-100 dark:bg-base-200 p-0.5 rounded-lg text-xs">
                            {[
                                { id: 'all', label: isZh ? '全部窗口' : 'All Windows' },
                                { id: '5h', label: '5H' },
                                { id: 'weekly', label: isZh ? '周配额' : 'Weekly' },
                            ].map(item => (
                                <button
                                    key={item.id}
                                    onClick={() => setSelectedWindow(item.id)}
                                    className={cn(
                                        'px-2 py-1 rounded-md transition-colors font-medium',
                                        selectedWindow === item.id
                                            ? 'bg-white dark:bg-base-100 text-indigo-600 dark:text-indigo-400 shadow-sm'
                                            : 'text-gray-500 hover:text-gray-800 dark:hover:text-gray-200'
                                    )}
                                >
                                    {item.label}
                                </button>
                            ))}
                        </div>

                        {/* 配额池类型过滤 */}
                        <div className="flex items-center gap-1 bg-gray-100 dark:bg-base-200 p-0.5 rounded-lg text-xs">
                            {[
                                { id: 'all', label: isZh ? '全部池' : 'All Pools' },
                                { id: 'gemini', label: 'Gemini' },
                                { id: 'third_party', label: isZh ? '第三方' : '3rd Party' },
                                { id: 'argon', label: 'Argon' },
                            ].map(item => (
                                <button
                                    key={item.id}
                                    onClick={() => setSelectedPool(item.id)}
                                    className={cn(
                                        'px-2 py-1 rounded-md transition-colors font-medium',
                                        selectedPool === item.id
                                            ? 'bg-white dark:bg-base-100 text-indigo-600 dark:text-indigo-400 shadow-sm'
                                            : 'text-gray-500 hover:text-gray-800 dark:hover:text-gray-200'
                                    )}
                                >
                                    {item.label}
                                </button>
                            ))}
                        </div>

                        {/* 状态过滤 */}
                        <div className="flex items-center gap-1 bg-gray-100 dark:bg-base-200 p-0.5 rounded-lg text-xs">
                            {[
                                { id: 'all', label: isZh ? '全部状态' : 'All' },
                                { id: 'active', label: isZh ? '进行中' : 'Active' },
                                { id: 'completed', label: isZh ? '已归档' : 'Archived' },
                            ].map(item => (
                                <button
                                    key={item.id}
                                    onClick={() => setSelectedStatus(item.id)}
                                    className={cn(
                                        'px-2 py-1 rounded-md transition-colors font-medium',
                                        selectedStatus === item.id
                                            ? 'bg-white dark:bg-base-100 text-indigo-600 dark:text-indigo-400 shadow-sm'
                                            : 'text-gray-500 hover:text-gray-800 dark:hover:text-gray-200'
                                    )}
                                >
                                    {item.label}
                                </button>
                            ))}
                        </div>
                    </div>
                </div>

                {/* 5. 周期流水表格 */}
                <div className="flex-1 overflow-y-auto min-h-0 border border-gray-200 dark:border-white/10 rounded-xl">
                    {loading && filteredCycles.length === 0 ? (
                        <div className="flex items-center justify-center p-10 text-sm text-gray-500">
                            <RefreshCw className="w-5 h-5 animate-spin mr-2 text-indigo-500" />
                            {isZh ? '正在载入周期流水…' : 'Loading cycle ledger…'}
                        </div>
                    ) : error ? (
                        <div className="flex items-center justify-center p-10 text-sm text-rose-500 gap-2">
                            <AlertCircle className="w-5 h-5" />
                            <span>{error}</span>
                        </div>
                    ) : filteredCycles.length === 0 ? (
                        <div className="flex flex-col items-center justify-center p-10 text-center text-gray-400">
                            <Activity className="w-10 h-10 stroke-1 mb-2 opacity-50" />
                            <p className="text-sm font-medium text-gray-600 dark:text-gray-300">
                                {isZh ? '暂无周期历史记录' : 'No cycle history yet'}
                            </p>
                            <p className="text-xs text-gray-400 mt-1 max-w-sm">
                                {isZh
                                    ? '系统已自动建立周期账本，刷新配额或产生模型调用后即可看到完整变动。'
                                    : 'The ledger will automatically capture quota changes after refreshing or consuming quota.'}
                            </p>
                        </div>
                    ) : (
                        <table className="w-full text-left text-xs border-collapse">
                            <thead className="bg-gray-50/90 dark:bg-white/[0.04] text-gray-500 dark:text-gray-400 sticky top-0 backdrop-blur-sm z-10 border-b border-gray-200 dark:border-white/10 font-semibold">
                                <tr>
                                    <th className="py-2.5 px-3 w-8"></th>
                                    <th className="py-2.5 px-3">{isZh ? '账号' : 'Account'}</th>
                                    <th className="py-2.5 px-3">{isZh ? '配额池 / 窗口' : 'Pool / Window'}</th>
                                    <th className="py-2.5 px-3">{isZh ? '周期时段' : 'Cycle Time'}</th>
                                    <th className="py-2.5 px-3">{isZh ? '余量起始 → 最新' : 'Quota Range'}</th>
                                    <th className="py-2.5 px-3">{isZh ? '累计消耗' : 'Consumed'}</th>
                                    <th className="py-2.5 px-3">{isZh ? '估值费用' : 'Valuation'}</th>
                                    <th className="py-2.5 px-3">{isZh ? '环比上期' : 'vs Previous'}</th>
                                    <th className="py-2.5 px-3">{isZh ? '状态' : 'Status'}</th>
                                </tr>
                            </thead>
                            <tbody className="divide-y divide-gray-100 dark:divide-white/5 font-mono text-[11px]">
                                {filteredCycles.map(cycle => {
                                    const isExpanded = expandedCycleId === cycle.cycle_id;
                                    const startPct = Math.round(cycle.first_observed_remaining * 100);
                                    const latestPct = Math.round(cycle.latest_remaining * 100);
                                    const consumedPct = Math.round(cycle.consumed_fraction * 100);
                                    const deltaPct = cycle.delta_total_pct != null ? Math.round(cycle.delta_total_pct) : null;
                                    const rate = data?.usd_to_cny_rate || 7.2;
                                    const usedCny = (cycle.used_usd ?? 0) * rate;

                                    return (
                                        <React.Fragment key={cycle.cycle_id}>
                                            <tr
                                                onClick={() => setExpandedCycleId(isExpanded ? null : cycle.cycle_id)}
                                                className="hover:bg-gray-50/70 dark:hover:bg-white/[0.02] cursor-pointer transition-colors"
                                            >
                                                <td className="py-2 px-3 text-gray-400">
                                                    {cycle.checkpoints?.length > 0 ? (
                                                        isExpanded ? (
                                                            <ChevronDown className="w-3.5 h-3.5" />
                                                        ) : (
                                                            <ChevronRight className="w-3.5 h-3.5" />
                                                        )
                                                    ) : null}
                                                </td>
                                                <td className="py-2 px-3 truncate max-w-[140px] font-sans font-medium text-gray-800 dark:text-gray-200">
                                                    <span title={cycle.email || cycle.account_id}>
                                                        {cycle.email || cycle.account_id.slice(0, 8)}
                                                    </span>
                                                    {cycle.subscription_tier && (
                                                        <span className="ml-1 text-[9px] px-1 py-0.2 rounded bg-gray-200/60 dark:bg-white/10 text-gray-500 uppercase">
                                                            {cycle.subscription_tier}
                                                        </span>
                                                    )}
                                                </td>
                                                <td className="py-2 px-3">
                                                    <span className="font-semibold text-gray-700 dark:text-gray-300 capitalize">
                                                        {cycle.pool}
                                                    </span>
                                                    <span className="ml-1.5 text-[10px] text-gray-400">
                                                        {cycle.window}
                                                    </span>
                                                </td>
                                                <td className="py-2 px-3 text-gray-500 dark:text-gray-400">
                                                    <span>{formatTime(cycle.start_time)}</span>
                                                    <span className="mx-1 text-gray-300 dark:text-gray-600">→</span>
                                                    <span>{formatTime(cycle.reset_time)}</span>
                                                </td>
                                                <td className="py-2 px-3">
                                                    <span className="text-gray-500">{startPct}%</span>
                                                    <span className="mx-1 text-gray-300 dark:text-gray-600">→</span>
                                                    <span className="font-bold text-gray-900 dark:text-white">
                                                        {latestPct}%
                                                    </span>
                                                </td>
                                                <td className="py-2 px-3 font-semibold text-rose-600 dark:text-rose-400">
                                                    {consumedPct > 0 ? `-${consumedPct}%` : '0%'}
                                                    {cycle.last_delta_fraction !== 0 && (
                                                        <span className="text-[10px] font-normal opacity-75 ml-1">
                                                            ({cycle.last_delta_fraction < 0 ? `↓${Math.round(Math.abs(cycle.last_delta_fraction) * 100)}%` : `↑${Math.round(cycle.last_delta_fraction * 100)}%`})
                                                        </span>
                                                    )}
                                                </td>
                                                <td className="py-2 px-3 text-gray-800 dark:text-gray-200">
                                                    {cycle.used_usd != null && cycle.used_usd > 0
                                                        ? formatCurrencyCompact(cycle.used_usd, usedCny, currencyMode)
                                                        : '—'}
                                                </td>
                                                <td className="py-2 px-3">
                                                    {deltaPct != null ? (
                                                        <span
                                                            className={cn(
                                                                'px-1.5 py-0.5 rounded text-[10px] font-semibold border',
                                                                deltaPct > 0
                                                                    ? 'text-rose-600 dark:text-rose-400 bg-rose-500/10 border-rose-500/20'
                                                                    : deltaPct < 0
                                                                    ? 'text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 border-emerald-500/20'
                                                                    : 'text-gray-500 dark:text-gray-400 bg-gray-500/10 border-gray-500/20'
                                                            )}
                                                        >
                                                            {deltaPct > 0 ? `+${deltaPct}%` : `${deltaPct}%`}
                                                        </span>
                                                    ) : (
                                                        <span className="text-gray-400">—</span>
                                                    )}
                                                </td>
                                                <td className="py-2 px-3">
                                                    {cycle.status === 'active' ? (
                                                        <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-emerald-50 dark:bg-emerald-950/40 text-emerald-600 dark:text-emerald-400 border border-emerald-200/50 dark:border-emerald-800/50">
                                                            {isZh ? '进行中' : 'Active'}
                                                        </span>
                                                    ) : (
                                                        <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-gray-100 dark:bg-white/10 text-gray-500 dark:text-gray-400">
                                                            {isZh ? '已归档' : 'Archived'}
                                                        </span>
                                                    )}
                                                </td>
                                            </tr>

                                            {/* 展开的检查点详细轨迹 */}
                                            {isExpanded && cycle.checkpoints?.length > 0 && (
                                                <tr className="bg-gray-50/50 dark:bg-white/[0.01]">
                                                    <td colSpan={9} className="p-3 pl-8">
                                                        <div className="text-[11px] font-sans font-medium text-gray-600 dark:text-gray-400 mb-1.5 flex items-center gap-1.5">
                                                            <Clock className="w-3.5 h-3.5 text-indigo-500" />
                                                            <span>{isZh ? '本周期采样检查点轨迹 (最新在前)' : 'Cycle Checkpoint Timeline'}</span>
                                                        </div>
                                                        <div className="flex flex-wrap gap-2">
                                                            {cycle.checkpoints.map((cp, idx) => {
                                                                const timeStr = new Date(cp.timestamp * 1000).toLocaleTimeString([], {
                                                                    hour: '2-digit',
                                                                    minute: '2-digit',
                                                                    second: '2-digit',
                                                                });
                                                                const remPct = Math.round(cp.remaining_fraction * 100);
                                                                const deltaStr = cp.delta_fraction < 0
                                                                    ? `↓${Math.round(Math.abs(cp.delta_fraction) * 100)}%`
                                                                    : `↑${Math.round(cp.delta_fraction * 100)}%`;
                                                                return (
                                                                    <div
                                                                        key={idx}
                                                                        className="px-2 py-1 rounded bg-white dark:bg-base-200 border border-gray-200/60 dark:border-white/10 text-[10px] font-mono flex items-center gap-1.5 shadow-2xs"
                                                                    >
                                                                        <span className="text-gray-400">{timeStr}</span>
                                                                        <span className="font-bold text-gray-800 dark:text-gray-200">{remPct}%</span>
                                                                        {cp.delta_fraction !== 0 && (
                                                                            <span
                                                                                className={cn(
                                                                                    'font-semibold',
                                                                                    cp.delta_fraction < 0
                                                                                        ? 'text-rose-500'
                                                                                        : 'text-emerald-500'
                                                                                )}
                                                                            >
                                                                                {deltaStr}
                                                                            </span>
                                                                        )}
                                                                    </div>
                                                                );
                                                            })}
                                                        </div>
                                                    </td>
                                                </tr>
                                            )}
                                        </React.Fragment>
                                    );
                                })}
                            </tbody>
                        </table>
                    )}
                </div>

                {/* 6. Footer */}
                <div className="flex items-center justify-between pt-3 mt-2 border-t border-gray-100 dark:border-white/10 shrink-0 text-xs text-gray-400">
                    <div>
                        {isZh
                            ? `共计 ${filteredCycles.length} 条周期记录 · 纯本地私密存储`
                            : `${filteredCycles.length} records · Private local storage`}
                    </div>
                    <button
                        onClick={onClose}
                        className="px-4 py-2 bg-gray-100 dark:bg-base-200 text-gray-700 dark:text-gray-300 font-medium rounded-xl hover:bg-gray-200 dark:hover:bg-base-300 transition-colors"
                    >
                        {isZh ? '关闭' : 'Close'}
                    </button>
                </div>
            </div>
        </div>,
        document.body
    );
}
