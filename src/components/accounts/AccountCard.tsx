import { weeklyQuotaItems, modelQuotaForWindow } from '../../utils/menuBarQuota';
import { useMemo } from 'react';
import { Download, ArrowRightLeft, RefreshCw, Trash2, Lock, Ban, Diamond, Gem, Circle, Sparkles, Tag, Clock, GripVertical, BrainCircuit, CheckCircle2 } from 'lucide-react';
import { Account } from '../../types/account';
import { cn } from '../../utils/cn';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { QuotaItem } from './QuotaItem';
import { getDisplayQuotaModels } from '../../config/modelConfig';
import { getValidationBlockedStatusLabel } from './accountValidationStatus';
import AccountSelection from './AccountSelection';
import { getAccountTier } from '../../utils/accountTier';

interface AccountCardProps {
    account: Account;
    selected: boolean;
    onSelect: () => void;
    isCurrent: boolean;
    isRefreshing: boolean;
    isSwitching?: boolean;
    onSwitch: (targetIde?: string) => void;
    onRefresh: () => void;
    onEditLabel: () => void;
    onDelete: () => void;
    onExport?: () => void;
    quotaWindow?: '5h' | 'weekly';
    dragHandleProps?: {
        attributes?: any;
        listeners?: any;
    };
    isDragging?: boolean;
}

function AccountCard({
    account,
    selected,
    onSelect,
    isCurrent: propIsCurrent,
    isRefreshing,
    isSwitching = false,
    onSwitch,
    onRefresh,
    onEditLabel,
    onDelete,
    onExport,
    quotaWindow,
    dragHandleProps,
    isDragging = false,
}: AccountCardProps) {
    const { t } = useTranslation();
    const { config } = useConfigStore();
    const isDisabled = Boolean(account.disabled);
    const validationBlockedLabel = getValidationBlockedStatusLabel(account.validation_blocked_reason, t);

    // Use the prop directly from parent component
    const isCurrent = propIsCurrent;

    // 获取统一解析后的展示模型列表 (严格对齐用户自定义勾选)
    const displayModels = useMemo(() => {
        return getDisplayQuotaModels(account.quota?.models, config?.pinned_quota_models?.models);
    }, [config?.pinned_quota_models?.models, account.quota?.models]);

    // 解析周配额项 (当处于 weekly 视图时)
    const weeklyItems = useMemo(() => {
        if (quotaWindow !== 'weekly') return [];
        return weeklyQuotaItems(account.quota).map(item => ({ ...item,
            poolBadge: item.shared ? t('accounts.shared_pool') : t('accounts.dedicated_pool'),
            Icon: item.isClaude ? BrainCircuit : Sparkles,
        }));
    }, [quotaWindow, account.quota?.quota_groups, t]);

    const isModelProtected = (key?: string) => {
        if (!config?.quota_protection?.enabled) return false;
        if (!key) return false;
        return account.protected_models?.includes(key);
    };

    return (
        <div className={cn(
            "h-full w-full flex flex-col p-3 rounded-xl border transition-all hover:shadow-md relative overflow-hidden",
            isCurrent
                ? "bg-blue-50/80 border-blue-500/80 dark:bg-blue-950/40 dark:border-blue-400/80"
                : "bg-white dark:bg-base-100 border-gray-200 dark:border-base-300",
            (isRefreshing || isDisabled) && "opacity-70",
            isDragging && "shadow-xl ring-2 ring-blue-500/30"
        )}>
            {/* Header: Grip Handle (left) + Column (Row 1: Selection + Full Email, Row 2: Badges + Date) */}
            <div className="flex-none flex items-start gap-1.5 mb-2.5">
                {dragHandleProps && (
                    <div
                        {...dragHandleProps.attributes}
                        {...dragHandleProps.listeners}
                        className="flex items-center justify-center w-5 h-5 rounded cursor-grab active:cursor-grabbing text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-base-200 transition-colors shrink-0"
                        title={t('accounts.drag_to_reorder', '拖拽调整账号顺序')}
                    >
                        <GripVertical className="w-3.5 h-3.5" />
                    </div>
                )}
                <div className="min-w-0 flex-1 flex flex-col gap-1.5">
                    {/* Row 1: Selection + Email (占据整行，不再被时间戳挤压截断) */}
                    <div className="flex items-center gap-1.5 min-w-0 w-full">
                        <AccountSelection
                            checked={selected}
                            onChange={onSelect}
                            disabled={isRefreshing}
                            label={t('accounts.select_account', { email: account.email })}
                        />
                        <h3 className={cn(
                            "text-sm truncate flex-1 min-w-0",
                            isCurrent ? "font-bold text-blue-800 dark:text-blue-300" : "font-semibold text-gray-900 dark:text-base-content"
                        )} title={account.email}>
                            {account.email}
                        </h3>
                        {isCurrent && (
                            <CheckCircle2 className="w-4 h-4 text-blue-600 dark:text-blue-400 shrink-0" />
                        )}
                    </div>

                    {/* Row 2: Badges + Last Used Date (靠右) */}
                    <div className="flex items-center justify-between w-full gap-2">
                        <div className="flex items-center gap-1.5 flex-wrap">
                            {/* 1. 订阅类型徽章 (始终置前，上下对齐) */}
                            {(() => {
                                const tier = getAccountTier(account);
                                if (tier === 'ultra') {
                                    return (
                                        <span className="flex items-center gap-1 px-1.5 py-0.5 rounded-md bg-gradient-to-r from-purple-600 to-pink-600 text-white text-[9px] font-bold shadow-sm">
                                            <Gem className="w-2.5 h-2.5 fill-current" />
                                            ULTRA
                                        </span>
                                    );
                                } else if (tier === 'pro') {
                                    return (
                                        <span className="flex items-center gap-1 px-1.5 py-0.5 rounded-md bg-gradient-to-r from-blue-600 to-indigo-600 text-white text-[9px] font-bold shadow-sm">
                                            <Diamond className="w-2.5 h-2.5 fill-current" />
                                            PRO
                                        </span>
                                    );
                                } else {
                                    return (
                                        <span title={tier === 'unknown' ? t('accounts.unknown_hint') : undefined} className="flex items-center gap-1 px-1.5 py-0.5 rounded-md bg-gray-100 dark:bg-white/10 text-gray-500 dark:text-gray-400 text-[9px] font-bold shadow-sm border border-gray-200 dark:border-white/10">
                                            <Circle className="w-2.5 h-2.5" />
                                            {t(`accounts.${tier}`)}
                                        </span>
                                    );
                                }
                            })()}

                            {/* 2. 自定义标签 */}
                            {account.custom_label && (
                                <span className="flex items-center gap-1 px-1.5 py-0.5 rounded-md bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-300 text-[9px] font-bold shadow-sm border border-orange-200/50">
                                    <Tag className="w-2.5 h-2.5" />
                                    {account.custom_label}
                                </span>
                            )}

                            {/* 3. 额外状态标签 (全部后置，不挤占 Pro 对齐位置) */}
                            {isCurrent && (
                                <span className="flex items-center gap-1 px-1.5 py-0.5 rounded-md bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 text-[9px] font-bold shadow-sm border border-blue-200/50 dark:border-blue-700/50">
                                    <CheckCircle2 className="w-2.5 h-2.5" />
                                    {t('accounts.current_active', '当前选择')}
                                </span>
                            )}
                            {isDisabled && (
                                <span
                                    className="px-1.5 py-0.5 rounded-md bg-rose-100 dark:bg-rose-900/40 text-rose-700 dark:text-rose-300 text-[9px] font-bold flex items-center gap-1 shadow-sm border border-rose-200/50"
                                >
                                    <Ban className="w-2.5 h-2.5" />
                                    {t('accounts.disabled').toUpperCase()}
                                </span>
                            )}
                            {account.quota?.is_forbidden && (
                                <span className="px-1.5 py-0.5 rounded-md bg-red-100 dark:bg-red-900/40 text-red-600 dark:text-red-400 text-[9px] font-bold flex items-center gap-1 shadow-sm border border-red-200/50">
                                    <Lock className="w-2.5 h-2.5" />
                                    {t('accounts.forbidden').toUpperCase()}
                                </span>
                            )}
                            {account.validation_blocked && (
                                <span className="px-1.5 py-0.5 rounded-md bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-400 text-[9px] font-bold flex items-center gap-1 shadow-sm border border-amber-200/50">
                                    <Clock className="w-2.5 h-2.5" />
                                    {validationBlockedLabel.toUpperCase()}
                                </span>
                            )}
                        </div>
                        <span className="text-[10px] text-gray-400 dark:text-gray-500 font-mono shrink-0 whitespace-nowrap">
                            {new Date(account.last_used * 1000).toLocaleString([], { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })}
                        </span>
                    </div>
                </div>
            </div>

            {/* 配额展示 */}
            <div className="flex-1 px-2 mb-2 flex flex-col justify-center overflow-y-auto scrollbar-none">
                {isDisabled || account.quota?.is_forbidden || account.validation_blocked ? (
                    <div className="flex flex-col items-center justify-center gap-1.5 h-full py-6 text-center my-auto">
                        <div className={cn(
                            "flex items-center gap-1.5",
                            account.validation_blocked ? "text-amber-600 dark:text-amber-400" : "text-red-600 dark:text-red-400"
                        )}>
                            {account.validation_blocked ? <Clock className="w-4 h-4" /> : (isDisabled ? <Ban className="w-4 h-4" /> : <Lock className="w-4 h-4" />)}
                            <span className="text-[11px] font-bold">
                                {account.validation_blocked ? validationBlockedLabel : (isDisabled ? t('accounts.status.disabled') : t('accounts.forbidden_msg'))}
                            </span>
                        </div>
                    </div>
                ) : quotaWindow === 'weekly' && weeklyItems.length > 0 ? (
                    <div className="grid grid-cols-1 gap-2 content-start my-auto">
                        {weeklyItems.map((item) => (
                            <QuotaItem
                                key={item.id}
                                label={
                                    <span className="flex items-center gap-1.5 truncate">
                                        <span>{item.title}</span>
                                        <span className="shrink-0">
                                            {item.poolBadge}
                                        </span>
                                    </span>
                                }
                                title={`${item.title} (${item.poolBadge})`}
                                percentage={item.percentage}
                                resetTime={item.resetTime}
                                Icon={item.Icon}
                            />
                        ))}
                    </div>
                ) : (
                    <div className="grid grid-cols-1 gap-2 content-start my-auto">
                        {displayModels.map((model) => {
                            const resolved = modelQuotaForWindow(model.data, account.quota, quotaWindow);
                            return (
                                <QuotaItem
                                    key={model.id}
                                    label={model.label}
                                    percentage={resolved.percentage}
                                    resetTime={resolved.resetTime}
                                    windowLabel={resolved.window === 'weekly' ? t('accounts.quota_window_weekly_short') : undefined}
                                    isProtected={isModelProtected(model.protectedKey)}
                                    Icon={model.Icon}
                                />
                            );
                        })}
                    </div>
                )}
            </div>

            {/* Footer: Actions Only */}
            <div className="flex-none flex items-center justify-center pt-2 pb-1 border-t border-gray-100 dark:border-base-200 mt-auto">
                <div className="flex items-center justify-center gap-2 w-full">
                    <button
                        className={cn(
                            "p-1.5 rounded-lg transition-all text-gray-400 dark:text-gray-500",
                            isSwitching
                                ? "text-blue-600 bg-blue-50 dark:text-blue-400 dark:bg-blue-900/20 cursor-not-allowed"
                                : "hover:text-blue-600 dark:hover:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
                        )}
                        onClick={(e) => { e.stopPropagation(); onSwitch(); }}
                        title={isDisabled ? t('accounts.disabled_tooltip') : (isSwitching ? t('common.loading') : t('accounts.switch_account'))}
                        disabled={isSwitching || isDisabled}
                    >
                        <ArrowRightLeft className={cn("w-3.5 h-3.5", isSwitching && "animate-spin")} />
                    </button>
                    <button
                        className={cn(
                            "p-1.5 rounded-lg transition-all text-gray-400 dark:text-gray-500",
                            isRefreshing
                                ? "text-green-600 bg-green-50 dark:text-green-400 dark:bg-green-900/20 cursor-not-allowed"
                                : "hover:text-green-600 dark:hover:text-green-400 hover:bg-green-50 dark:hover:bg-green-900/30 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
                        )}
                        onClick={(e) => { e.stopPropagation(); onRefresh(); }}
                        disabled={isRefreshing || isDisabled}
                        title={isDisabled ? t('accounts.disabled_tooltip') : t('accounts.refresh_quota')}
                    >
                        <RefreshCw className={cn("w-3.5 h-3.5", isRefreshing && "animate-spin")} />
                    </button>
                    <button
                        className="p-1.5 rounded-lg transition-all text-gray-400 hover:text-orange-600 hover:bg-orange-50 dark:hover:text-orange-400 dark:hover:bg-orange-900/30 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
                        onClick={(e) => { e.stopPropagation(); onEditLabel(); }}
                        disabled={isRefreshing || isSwitching}
                        title={t('accounts.edit_remark', '编辑备注')}
                        aria-label={`${t('accounts.edit_remark', '编辑备注')} ${account.email}`}
                    >
                        <Tag className="w-3.5 h-3.5" />
                    </button>
                    <button
                        className="p-1.5 rounded-lg transition-all text-gray-400 hover:text-red-600 hover:bg-red-50 dark:hover:text-red-400 dark:hover:bg-red-900/30 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
                        onClick={(e) => { e.stopPropagation(); onDelete(); }}
                        disabled={isRefreshing || isSwitching}
                        title={t('accounts.delete_account')}
                        aria-label={`${t('accounts.delete_account')} ${account.email}`}
                    >
                        <Trash2 className="w-3.5 h-3.5" />
                    </button>
                    {onExport && (
                        <button
                            className="p-1.5 rounded-lg transition-all text-gray-400 dark:text-gray-500 hover:text-blue-600 dark:hover:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30"
                            title={t('accounts.export_account')}
                            aria-label={t('accounts.export_account')}
                            onClick={event => { event.stopPropagation(); onExport(); }}
                        >
                            <Download className="w-3.5 h-3.5" />
                        </button>
                    )}
                </div>
            </div>
        </div >
    );
}

export default AccountCard;
