
import { Clock, Lock, TrendingDown, TrendingUp } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '../../utils/cn';
import { getQuotaColor, formatTimeRemaining } from '../../utils/format';
import type { ValuationStructuredResult } from '../../utils/quotaValuation';

interface QuotaItemProps {
    label: React.ReactNode;
    title?: string;
    percentage: number | null;
    resetTime?: string;
    windowLabel?: string;
    isProtected?: boolean;
    className?: string;
    Icon?: React.ComponentType<{ size?: number; className?: string }>;
    valuationStructured?: ValuationStructuredResult | null;
    valuationLine?: React.ReactNode;
    valuationTitle?: string;
}

export function QuotaItem({
    label,
    title,
    percentage,
    resetTime,
    windowLabel,
    isProtected,
    className,
    Icon,
    valuationStructured,
    valuationLine,
    valuationTitle,
}: QuotaItemProps) {
    const { t, i18n } = useTranslation();

    const getBgColorClass = (p: number) => {
        const color = getQuotaColor(p);
        switch (color) {
            case 'success': return 'bg-emerald-500';
            case 'warning': return 'bg-amber-500';
            case 'error': return 'bg-rose-500';
            default: return 'bg-gray-500';
        }
    };

    const getTextColorClass = (p: number) => {
        const color = getQuotaColor(p);
        switch (color) {
            case 'success': return 'text-emerald-600 dark:text-emerald-400';
            case 'warning': return 'text-amber-600 dark:text-amber-400';
            case 'error': return 'text-rose-600 dark:text-rose-400';
            default: return 'text-gray-500';
        }
    };

    // 1. 当 percentage === null 时（如未知/失效/未激活桶），保持极简紧凑单行灰显
    if (percentage === null) {
        return (
            <div
                className={cn(
                    "h-[24px] px-2 rounded-md flex items-center justify-between text-gray-400 dark:text-gray-500 text-[10px] font-mono opacity-60 border border-gray-100/50 dark:border-white/5 bg-gray-50/20 dark:bg-white/5",
                    className
                )}
                title={title || (typeof label === 'string' ? label : undefined)}
            >
                <span className="flex items-center gap-1 truncate font-medium">
                    {Icon && <Icon size={12} className="shrink-0" />}
                    {label}
                </span>
                <div className="flex items-center gap-1.5 shrink-0">
                    {windowLabel && <span className="text-[9px] px-1 py-0.2 rounded bg-gray-200/50 dark:bg-white/5">{windowLabel}</span>}
                    <span className="italic">—</span>
                </div>
            </div>
        );
    }

    // 2. 当 percentage !== null 时，采用图 1 同款清晰直观三段式布局
    const isZh = i18n.language.startsWith('zh');
    const roundedPercent = Math.round(percentage);

    return (
        <div
            className={cn(
                "relative w-full rounded-lg px-2.5 py-1.5 flex flex-col gap-1 transition-all",
                "bg-gray-50/80 dark:bg-white/[0.04] border border-gray-200/70 dark:border-white/[0.08] shadow-sm group/quota",
                className
            )}
            title={valuationStructured?.title || valuationTitle || title || (typeof label === 'string' ? label : undefined)}
        >
            {/* 第 1 行：名称 + 醒目剩余百分比 */}
            <div className="w-full flex items-center justify-between text-[11px] leading-tight">
                <div className="flex items-center gap-1.5 min-w-0 truncate font-semibold text-gray-800 dark:text-gray-100">
                    {Icon && <Icon size={13} className="shrink-0 text-gray-500 dark:text-gray-400" />}
                    <span className="truncate">{label}</span>
                    {windowLabel && (
                        <span className="text-[10px] px-1 py-0.2 rounded bg-gray-200/70 dark:bg-white/10 text-gray-500 dark:text-gray-400 font-normal shrink-0">
                            {windowLabel}
                        </span>
                    )}
                </div>
                <div className="flex items-center gap-1 shrink-0 font-bold font-mono">
                    {isProtected && (
                        <span title={t('accounts.quota_protected')}><Lock className="w-2.5 h-2.5 text-amber-500" /></span>
                    )}
                    <span className={cn(getTextColorClass(percentage), "inline-flex items-center gap-1")}>
                        <span className="text-[10px] font-normal opacity-80">{isZh ? '剩余' : 'Left'}</span>
                        <span>{roundedPercent}%</span>
                    </span>
                    {valuationStructured?.lastDeltaText && (
                        <span
                            className={cn(
                                "text-[10px] font-mono px-1 py-0.2 rounded font-semibold",
                                valuationStructured.lastDeltaText.startsWith('↓')
                                    ? "text-rose-600 dark:text-rose-400 bg-rose-500/10"
                                    : "text-emerald-600 dark:text-emerald-400 bg-emerald-500/10"
                            )}
                            title={isZh ? `最近单次刷新变化: ${valuationStructured.lastDeltaText}` : `Recent delta: ${valuationStructured.lastDeltaText}`}
                        >
                            {valuationStructured.lastDeltaText}
                        </span>
                    )}
                </div>
            </div>

            {/* 第 2 行：独立实心细圆角进度条 (100% 复刻图 1) */}
            <div className="w-full h-1.5 rounded-full bg-gray-200 dark:bg-white/10 overflow-hidden">
                <div
                    className={cn(
                        "h-full rounded-full transition-all duration-500 ease-out",
                        getBgColorClass(percentage)
                    )}
                    style={{ width: `${Math.max(0, Math.min(100, percentage))}%` }}
                />
            </div>

            {/* 第 3 行：左侧双图标美刀/人民币/Token + 右侧重置倒计时 (100% 复刻图 1) */}
            <div className="w-full flex items-center justify-between text-[10px] font-mono leading-none pt-0.5">
                <div className="flex items-center gap-2 min-w-0 truncate">
                    {valuationStructured ? (
                        <>
                            {/* 已用指标 */}
                            <span className="flex items-center gap-0.5 text-gray-600 dark:text-gray-300 font-medium shrink-0">
                                <TrendingDown className="w-3 h-3 text-sky-500 shrink-0" />
                                <span>{valuationStructured.usedText}</span>
                            </span>
                            {/* 剩余/总额上限指标 */}
                            {valuationStructured.quotaText && (
                                <span className="flex items-center gap-1 text-gray-900 dark:text-white font-bold shrink-0">
                                    <TrendingUp className="w-3 h-3 text-violet-500 dark:text-indigo-400 shrink-0" />
                                    <span>{valuationStructured.quotaText}</span>
                                    {valuationStructured.badge && (
                                        <span className="text-[9px] px-1 py-0.2 rounded bg-gray-200/80 dark:bg-white/10 text-gray-500 dark:text-gray-300 font-normal">
                                            {valuationStructured.badge}
                                        </span>
                                    )}
                                </span>
                            )}
                            {/* 周期环比变动徽标 */}
                            {valuationStructured.cycleCompareText && (
                                <span
                                    className={cn(
                                        "text-[9px] px-1 py-0.2 rounded font-normal shrink-0 border",
                                        valuationStructured.cycleCompareTone === 'increase'
                                            ? "text-rose-600 dark:text-rose-400 bg-rose-500/10 border-rose-500/20"
                                            : valuationStructured.cycleCompareTone === 'decrease'
                                            ? "text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 border-emerald-500/20"
                                            : "text-gray-500 dark:text-gray-400 bg-gray-500/10 border-gray-500/20"
                                    )}
                                    title={isZh ? `上个周期总消耗环比: ${valuationStructured.cycleCompareText}` : `Vs previous cycle: ${valuationStructured.cycleCompareText}`}
                                >
                                    {valuationStructured.cycleCompareText}
                                </span>
                            )}
                        </>
                    ) : valuationLine ? (
                        <span className="truncate text-gray-600 dark:text-gray-300 font-medium">
                            {valuationLine}
                        </span>
                    ) : null}
                </div>

                {/* 右侧：重置倒计时 */}
                <div className="shrink-0 ml-auto pl-1">
                    {resetTime ? (
                        <span className="flex items-center gap-0.5 font-medium text-amber-600 dark:text-amber-400">
                            <Clock className="w-2.5 h-2.5 shrink-0" />
                            <span>{formatTimeRemaining(resetTime)}</span>
                        </span>
                    ) : (
                        <span className="text-gray-300 dark:text-gray-600 italic scale-90">N/A</span>
                    )}
                </div>
            </div>
        </div>
    );
}

