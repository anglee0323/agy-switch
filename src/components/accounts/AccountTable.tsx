import { weeklyQuotaItems } from '../../utils/menuBarQuota';
/**
 * 账号表格组件
 * 支持拖拽排序功能，用户可以通过拖拽行来调整账号顺序
 */
import { useMemo, useState } from 'react';
import {
    DndContext,
    closestCenter,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    DragEndEvent,
    DragStartEvent,
    DragOverlay,
} from '@dnd-kit/core';
import {
    arrayMove,
    SortableContext,
    sortableKeyboardCoordinates,
    useSortable,
    verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import {
    GripVertical,
    Download, ArrowRightLeft,
    RefreshCw,
    Trash2,
    Lock,
    Ban,
    Diamond,
    Gem,
    Circle,
    Sparkles,
    Tag,
    Clock,
    Bot,
    BrainCircuit,
    ArrowUpDown,
    ArrowUp,
    ArrowDown,
    CheckCircle2,
} from 'lucide-react';
import type { Account } from '../../types/account';
import { useTranslation } from 'react-i18next';
import { cn } from '../../utils/cn';

import { useConfigStore } from '../../stores/useConfigStore';
import { QuotaItem } from './QuotaItem';
import { getDisplayQuotaModels } from '../../config/modelConfig';
import { categorizeModel, getModelProtectionKey } from '../../utils/modelCategory';
import { getValidationBlockedStatusLabel } from './accountValidationStatus';
import AccountSelection from './AccountSelection';
import { getAccountTier } from '../../utils/accountTier';

// ============================================================================
// 类型定义
// ============================================================================

interface AccountTableProps {
    accounts: Account[];
    selectedIds: Set<string>;
    refreshingIds: Set<string>;
    onToggleSelect: (id: string) => void;
    onToggleAll: () => void;
    currentAccountId: string | null;
    switchingAccountId: string | null;
    onSwitch: (accountId: string, targetIde?: string) => void;
    onRefresh: (accountId: string) => void;
    onEditLabel: (accountId: string) => void;
    onDelete: (accountId: string) => void;
    onExport?: (accountId: string) => void;
    /** 拖拽排序回调，当用户完成拖拽时触发 */
    onReorder?: (accountIds: string[]) => void;
    quotaWindow?: '5h' | 'weekly';
}

interface SortableRowProps {
    account: Account;
    selected: boolean;
    isRefreshing: boolean;
    isCurrent: boolean;
    isSwitching: boolean;
    isDragging?: boolean;
    onSelect: () => void;
    onSwitch: (targetIde?: string) => void;
    onRefresh: () => void;
    onEditLabel: () => void;
    onDelete: () => void;
    onExport?: () => void;
    quotaWindow?: '5h' | 'weekly';
    isDragDisabled?: boolean;
}

interface AccountRowContentProps {
    account: Account;
    isCurrent: boolean;
    isRefreshing: boolean;
    isSwitching: boolean;
    isDisabled: boolean;
    onSwitch: (targetIde?: string) => void;
    onRefresh: () => void;
    onEditLabel: () => void;
    onDelete: () => void;
    onExport?: () => void;
    quotaWindow?: '5h' | 'weekly';
}

// ============================================================================
// 辅助函数
// ============================================================================



function isModelProtected(protectedModels: string[] | undefined, modelName: string): boolean {
    if (!protectedModels || protectedModels.length === 0) return false;
    const lowerName = modelName.toLowerCase();

    if (lowerName === 'gemini-pro') {
        return protectedModels.some((model) =>
            categorizeModel(model) === 'gemini-pro' && getModelProtectionKey(model) === 'gemini-3-pro-high',
        );
    }
    if (lowerName === 'gemini-flash') {
        return protectedModels.some((model) =>
            categorizeModel(model) === 'gemini-flash' && getModelProtectionKey(model) === 'gemini-3-flash',
        );
    }
    if (lowerName === 'claude-sonnet') {
        return protectedModels.some((model) =>
            categorizeModel(model) === 'claude' && getModelProtectionKey(model) === 'claude',
        );
    }

    const protectionKey = getModelProtectionKey(lowerName);
    return protectionKey ? protectedModels.includes(protectionKey) : false;
}

/**
 * 提取账号的最快配额重置时间（毫秒时间戳）
 * 用于表格排序
 */
function extractAccountResetTime(account: Account, quotaWindow?: '5h' | 'weekly'): number | null {
    let earliestTime: number | null = null;

    if (quotaWindow === 'weekly') {
        const groups = account.quota?.quota_groups || [];
        for (const group of groups) {
            for (const bucket of group.buckets || []) {
                const isWeekly = bucket.window.toLowerCase().includes('week') || bucket.bucket_id.toLowerCase().includes('week');
                if (isWeekly && bucket.reset_time) {
                    const t = new Date(bucket.reset_time).getTime();
                    if (!isNaN(t) && (earliestTime === null || t < earliestTime)) {
                        earliestTime = t;
                    }
                }
            }
        }
    } else {
        // 5h 或常规视图下，从 models 中获取最近的 reset_time
        const models = account.quota?.models || [];
        for (const model of models) {
            if (model.reset_time) {
                const t = new Date(model.reset_time).getTime();
                if (!isNaN(t) && (earliestTime === null || t < earliestTime)) {
                    earliestTime = t;
                }
            }
        }
    }

    return earliestTime;
}

// ============================================================================
// 子组件
// ============================================================================

/**
 * 可拖拽的表格行组件
 * 使用 @dnd-kit/sortable 实现拖拽功能
 */
function SortableAccountRow({
    account,
    selected,
    isRefreshing,
    isCurrent,
    isSwitching,
    isDragging,
    onSelect,
    onSwitch,
    onRefresh,
    onEditLabel,
    onDelete,
    onExport,
    quotaWindow,
    isDragDisabled = false,
}: SortableRowProps) {
    const { t } = useTranslation();
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
        isDragging: isSortableDragging,
    } = useSortable({ id: account.id, disabled: isDragDisabled });

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
        opacity: isSortableDragging ? 0.5 : 1,
        zIndex: isSortableDragging ? 1000 : 'auto',
    };

    return (
        <tr
            ref={setNodeRef}
            style={style as React.CSSProperties}
            className={cn(
                "group transition-colors border-b",
                isCurrent
                    ? "bg-blue-100/70 dark:bg-blue-950/60 border-t border-t-blue-500/80 border-b-blue-500/80 dark:border-t-blue-400/80 dark:border-b-blue-400/80"
                    : "border-gray-100 dark:border-base-200",
                isDragging && "bg-blue-100 dark:bg-blue-900/30 shadow-lg",
                !isDragging && (isCurrent ? "hover:bg-blue-100 dark:hover:bg-blue-900/50" : "hover:bg-gray-50 dark:hover:bg-base-200")
            )}
        >
            {/* 拖拽手柄 */}
            <td className={cn(
                "pl-2 py-1 w-8 align-middle transition-colors",
                isCurrent && "border-l border-l-blue-500/80 dark:border-l-blue-400/80"
            )}>
                <div
                    {...(!isDragDisabled ? attributes : {})}
                    {...(!isDragDisabled ? listeners : {})}
                    className={cn(
                        "flex items-center justify-center w-6 h-6 rounded transition-colors",
                        isDragDisabled
                            ? "text-gray-200 dark:text-gray-700 cursor-not-allowed opacity-40"
                            : (isCurrent
                                ? "cursor-grab active:cursor-grabbing text-blue-600 dark:text-blue-400 hover:bg-blue-200/50 dark:hover:bg-blue-900/40"
                                : "cursor-grab active:cursor-grabbing text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700")
                    )}
                    title={isDragDisabled ? t('accounts.drag_disabled_during_sort', '已激活列排序，拖拽排序已暂停') : t('accounts.drag_to_reorder')}
                >
                    <GripVertical className="w-4 h-4" />
                </div>
            </td>
            {/* 复选框 */}
            <td className="px-2 py-1 w-10 align-middle">
                <AccountSelection
                    checked={selected}
                    onChange={onSelect}
                    disabled={isRefreshing}
                    label={t('accounts.select_account', { email: account.email })}
                />
            </td>
            <AccountRowContent
                account={account}
                isCurrent={isCurrent}
                isRefreshing={isRefreshing}
                isSwitching={isSwitching}
                isDisabled={Boolean(account.disabled)}
                onSwitch={onSwitch}
                onRefresh={onRefresh}
                onEditLabel={onEditLabel}
                onDelete={onDelete}
                onExport={onExport}
                quotaWindow={quotaWindow}
            />
        </tr>
    );
}

/**
 * 账号行内容组件
 * 渲染邮箱、配额、最后使用时间和操作按钮等列
 */
function AccountRowContent({
    account,
    isCurrent,
    isRefreshing,
    isSwitching,
    isDisabled,
    onSwitch,
    onRefresh,
    onEditLabel,
    onDelete,
    onExport,
    quotaWindow,
}: AccountRowContentProps) {
    const { t } = useTranslation();
    const { config } = useConfigStore();
    const validationBlockedLabel = getValidationBlockedStatusLabel(account.validation_blocked_reason, t);

    // 解析周配额项 (当处于 weekly 视图时)
    const weeklyItems = useMemo(() => {
        if (quotaWindow !== 'weekly') return [];
        return weeklyQuotaItems(account.quota).map(item => ({ ...item,
            poolBadge: item.shared ? t('accounts.shared_pool') : t('accounts.dedicated_pool'),
            Icon: item.isClaude ? BrainCircuit : Sparkles,
        }));
    }, [quotaWindow, account.quota?.quota_groups, t]);

    // 获取统一解析后的展示模型列表 (严格对齐用户自定义勾选)
    const displayModels = useMemo(() => {
        return getDisplayQuotaModels(account.quota?.models, config?.pinned_quota_models?.models);
    }, [config?.pinned_quota_models?.models, account.quota?.models]);

    return (
        <>
            {/* 邮箱列 */}
            <td className="px-2 py-1 align-middle">
                <div className="flex flex-col justify-center gap-1">
                    <div className="flex items-center gap-1.5 min-w-0">
                        <span className={cn(
                            "text-sm break-all transition-colors leading-tight",
                            isCurrent ? "font-bold text-blue-800 dark:text-blue-300" : "font-medium text-gray-900 dark:text-base-content"
                        )} title={account.email}>
                            {account.email}
                        </span>
                        {isCurrent && (
                            <CheckCircle2 className="w-4 h-4 text-blue-600 dark:text-blue-400 shrink-0" />
                        )}
                    </div>

                    <div className="flex items-center gap-1.5 shrink-0 flex-wrap">
                        {/* 1. 订阅类型徽章 (始终置前，上下对齐) */}
                        {(() => {
                            const tier = getAccountTier(account);
                            if (tier === 'ultra') {
                                return (
                                    <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-gradient-to-r from-purple-600 to-pink-600 text-white text-[10px] font-bold shadow-sm hover:scale-105 transition-transform cursor-default">
                                        <Gem className="w-2.5 h-2.5 fill-current" />
                                        {t('accounts.ultra')}
                                    </span>
                                );
                            } else if (tier === 'pro') {
                                return (
                                    <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-gradient-to-r from-blue-600 to-indigo-600 text-white text-[10px] font-bold shadow-sm hover:scale-105 transition-transform cursor-default">
                                        <Diamond className="w-2.5 h-2.5 fill-current" />
                                        {t('accounts.pro')}
                                    </span>
                                );
                            } else {
                                return (
                                    <span title={tier === 'unknown' ? t('accounts.unknown_hint') : undefined} className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-gray-100 dark:bg-white/10 text-gray-600 dark:text-gray-400 text-[10px] font-bold shadow-sm border border-gray-200 dark:border-white/10 hover:bg-gray-200 transition-colors cursor-default">
                                        <Circle className="w-2.5 h-2.5" />
                                        {t(`accounts.${tier}`)}
                                    </span>
                                );
                            }
                        })()}

                        {/* 2. 自定义标签 */}
                        {account.custom_label && (
                            <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-300 text-[10px] font-bold shadow-sm border border-orange-200/50 dark:border-orange-800/50">
                                <Tag className="w-2.5 h-2.5" />
                                {account.custom_label}
                            </span>
                        )}

                        {/* 3. 额外状态标签 (全部后置，不挤占 Pro 对齐位置) */}
                        {isCurrent && (
                            <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 text-[10px] font-bold shadow-sm border border-blue-200/50 dark:border-blue-700/50">
                                <CheckCircle2 className="w-3 h-3" />
                                {t('accounts.current_active', '当前选择')}
                            </span>
                        )}
                        {isDisabled && (
                            <span
                                className="px-2 py-0.5 rounded-md bg-rose-100 dark:bg-rose-900/50 text-rose-700 dark:text-rose-300 text-[10px] font-bold flex items-center gap-1 shadow-sm border border-rose-200/50"
                            >
                                <Ban className="w-2.5 h-2.5" />
                                <span>{t('accounts.disabled')}</span>
                            </span>
                        )}

                        {account.quota?.is_forbidden && (
                            <span className="px-2 py-0.5 rounded-md bg-red-100 dark:bg-red-900/50 text-red-600 dark:text-red-400 text-[10px] font-bold flex items-center gap-1 shadow-sm border border-red-200/50">
                                <Lock className="w-2.5 h-2.5" />
                                <span>{t('accounts.forbidden')}</span>
                            </span>
                        )}
                        {account.validation_blocked && (
                            <span className="px-2 py-0.5 rounded-md bg-amber-100 dark:bg-amber-900/50 text-amber-700 dark:text-amber-400 text-[10px] font-bold flex items-center gap-1 shadow-sm border border-amber-200/50">
                                <Clock className="w-2.5 h-2.5" />
                                <span>{validationBlockedLabel}</span>
                            </span>
                        )}
                    </div>

                </div>
            </td>

            {/* 模型配额列 */}
            <td className="px-2 py-1 align-middle">
                {isDisabled || account.quota?.is_forbidden || account.validation_blocked ? (
                    <div className={cn(
                        "flex items-center justify-center gap-3 py-1.5 px-4 rounded-xl border group/error",
                        account.validation_blocked ? "bg-amber-50/50 dark:bg-amber-900/10 border-amber-100/50 dark:border-amber-900/20" : "bg-red-50/50 dark:bg-red-900/10 border-red-100/50 dark:border-red-900/20"
                    )}>
                        <div className={cn(
                            "flex items-center gap-1.5",
                            account.validation_blocked ? "text-amber-600 dark:text-amber-400" : "text-red-600 dark:text-red-400"
                        )}>
                            {account.validation_blocked ? <Clock className="w-3.5 h-3.5" /> : (account.quota?.is_forbidden ? <Lock className="w-3.5 h-3.5" /> : <Ban className="w-3.5 h-3.5" />)}
                            <span className={cn(
                                "text-[11px] font-bold",
                                account.validation_blocked ? "text-amber-700/80 dark:text-amber-400" : "text-red-700/80 dark:text-red-400"
                            )}>
                                {account.validation_blocked ? validationBlockedLabel : (isDisabled ? t('accounts.status.disabled') : t('accounts.forbidden_msg'))}
                            </span>
                        </div>
                    </div>
                ) : (
                    <div className="flex flex-col gap-1.5 py-0.5 w-full">
                        {quotaWindow === 'weekly' && weeklyItems.length > 0 ? (
                            weeklyItems.map((item) => (
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
                            ))
                        ) : (
                            displayModels.map((model) => {
                                const modelData = model.data;

                                return (
                                    <QuotaItem
                                        key={model.id}
                                        label={model.label}
                                        percentage={modelData?.percentage || 0}
                                        resetTime={modelData?.reset_time}
                                        isProtected={Boolean(config?.quota_protection?.enabled && isModelProtected(account.protected_models, model.protectedKey))}
                                        Icon={model.Icon || Bot}
                                    />
                                );
                            })
                        )}
                    </div>
                )}
            </td>

            {/* 最后使用时间列 */}
            <td className="px-2 py-1 align-middle">
                <div className="flex flex-col">
                    <span className="text-xs font-medium text-gray-600 dark:text-gray-400 font-mono whitespace-nowrap">
                        {new Date(account.last_used * 1000).toLocaleDateString()}
                    </span>
                    <span className="text-[10px] text-gray-400 dark:text-gray-500 font-mono whitespace-nowrap leading-tight">
                        {new Date(account.last_used * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    </span>
                </div>
            </td>

            {/* 操作列 */}
            <td className={cn(
                "px-1 py-1 sticky right-0 z-10 shadow-[-12px_0_12px_-12px_rgba(0,0,0,0.1)] dark:shadow-[-12px_0_12px_-12px_rgba(255,255,255,0.05)] text-center align-middle transition-colors",
                // 动态背景色处理
                isCurrent
                    ? "bg-[#dbeafe] dark:bg-[#15243e] group-hover:bg-[#bfdbfe] dark:group-hover:bg-[#1e345b]"
                    : "bg-white dark:bg-base-100 group-hover:bg-gray-50 dark:group-hover:bg-base-200"
            )}>
                <div className="flex items-center justify-center gap-1 opacity-60 group-hover:opacity-100 transition-opacity max-w-[150px] mx-auto">
                    <button
                        className={cn(
                            "p-1.5 rounded-lg transition-all text-gray-500 dark:text-gray-400",
                            isSwitching
                                ? "bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 cursor-not-allowed"
                                : "hover:text-blue-600 dark:hover:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-gray-500 dark:disabled:hover:text-gray-400"
                        )}
                        onClick={(e) => { e.stopPropagation(); onSwitch(); }}
                        title={isDisabled ? t('accounts.disabled_tooltip') : (isSwitching ? t('common.loading') : t('accounts.switch_account'))}
                        disabled={isSwitching || isDisabled}
                    >
                        <ArrowRightLeft className={cn("w-3.5 h-3.5", isSwitching && "animate-spin")} />
                    </button>
                    <button
                        className={cn(
                            "p-1.5 rounded-lg transition-all text-gray-500 dark:text-gray-400",
                            isRefreshing
                                ? "bg-green-50 dark:bg-green-900/20 text-green-600 dark:text-green-400 cursor-not-allowed"
                                : "hover:text-green-600 dark:hover:text-green-400 hover:bg-green-50 dark:hover:bg-green-900/30 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-gray-500 dark:disabled:hover:text-gray-400"
                        )}
                        onClick={(e) => { e.stopPropagation(); onRefresh(); }}
                        title={isDisabled ? t('accounts.disabled_tooltip') : (isRefreshing ? t('common.refreshing') : t('accounts.refresh_quota'))}
                        disabled={isRefreshing || isDisabled}
                    >
                        <RefreshCw className={cn("w-3.5 h-3.5", isRefreshing && "animate-spin")} />
                    </button>
                    <button
                        className="p-1.5 text-gray-500 dark:text-gray-400 rounded-lg transition-all hover:text-orange-600 dark:hover:text-orange-400 hover:bg-orange-50 dark:hover:bg-orange-900/30 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-gray-500"
                        onClick={(e) => { e.stopPropagation(); onEditLabel(); }}
                        title={t('accounts.edit_remark', '编辑备注')}
                        aria-label={`${t('accounts.edit_remark', '编辑备注')} ${account.email}`}
                        disabled={isRefreshing || isSwitching}
                    >
                        <Tag className="w-3.5 h-3.5" />
                    </button>
                    <button
                        className="p-1.5 text-gray-500 dark:text-gray-400 rounded-lg transition-all hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/30 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-gray-500"
                        onClick={(e) => { e.stopPropagation(); onDelete(); }}
                        title={t('accounts.delete_account')}
                        aria-label={`${t('accounts.delete_account')} ${account.email}`}
                        disabled={isRefreshing || isSwitching}
                    >
                        <Trash2 className="w-3.5 h-3.5" />
                    </button>
                    {onExport && (
                        <button
                            className="p-1.5 rounded-lg transition-all text-gray-500 dark:text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30"
                            title={t('accounts.export_account')}
                            aria-label={t('accounts.export_account')}
                            onClick={event => { event.stopPropagation(); onExport(); }}
                        >
                            <Download className="w-3.5 h-3.5" />
                        </button>
                    )}
                </div>
            </td>
        </>
    );
}

// ============================================================================
// 主组件
// ============================================================================

/**
 * 账号表格组件
 * 支持拖拽排序、多选、批量操作等功能
 */
function AccountTable({
    accounts,
    selectedIds,
    refreshingIds,
    onToggleSelect,
    onToggleAll,
    currentAccountId,
    switchingAccountId,
    onSwitch,
    onRefresh,
    onEditLabel,
    onDelete,
    onExport,
    onReorder,
    quotaWindow,
}: AccountTableProps) {
    const { t } = useTranslation();

    const [activeId, setActiveId] = useState<string | null>(null);
    // 排序状态配置: 支持按配额重置时间 (reset_time) 或最后使用时间 (last_used) 排序
    const [sortConfig, setSortConfig] = useState<{
        key: 'reset_time' | 'last_used' | null;
        direction: 'asc' | 'desc' | null;
    }>({
        key: null,
        direction: null,
    });

    const isSortingActive = sortConfig.key !== null && sortConfig.direction !== null;

    const handleSortToggle = (key: 'reset_time' | 'last_used') => {
        setSortConfig(prev => {
            if (prev.key !== key) {
                return { key, direction: 'asc' };
            }
            if (prev.direction === 'asc') {
                return { key, direction: 'desc' };
            }
            return { key: null, direction: null };
        });
    };

    // 根据排序状态对 accounts 进行拦截排序
    const sortedAccounts = useMemo(() => {
        if (!isSortingActive) return accounts;

        return [...accounts].sort((a, b) => {
            if (sortConfig.key === 'reset_time') {
                const timeA = extractAccountResetTime(a, quotaWindow);
                const timeB = extractAccountResetTime(b, quotaWindow);

                // 没有 reset_time 的排到后面
                if (timeA === null && timeB === null) return 0;
                if (timeA === null) return 1;
                if (timeB === null) return -1;

                return sortConfig.direction === 'asc' ? timeA - timeB : timeB - timeA;
            }

            if (sortConfig.key === 'last_used') {
                const timeA = a.last_used || 0;
                const timeB = b.last_used || 0;

                return sortConfig.direction === 'asc' ? timeA - timeB : timeB - timeA;
            }

            return 0;
        });
    }, [accounts, sortConfig, quotaWindow, isSortingActive]);

    // 配置拖拽传感器
    const sensors = useSensors(
        useSensor(PointerSensor, {
            activationConstraint: { distance: 8 }, // 需要移动 8px 才触发拖拽
        }),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates,
        })
    );

    const accountIds = useMemo(() => sortedAccounts.map(a => a.id), [sortedAccounts]);
    const activeAccount = useMemo(() => sortedAccounts.find(a => a.id === activeId), [sortedAccounts, activeId]);

    const handleDragStart = (event: DragStartEvent) => {
        if (isSortingActive) return;
        setActiveId(event.active.id as string);
    };

    const handleDragEnd = (event: DragEndEvent) => {
        const { active, over } = event;
        setActiveId(null);

        if (isSortingActive) return;

        if (over && active.id !== over.id) {
            const oldIndex = accountIds.indexOf(active.id as string);
            const newIndex = accountIds.indexOf(over.id as string);

            if (oldIndex !== -1 && newIndex !== -1 && onReorder) {
                onReorder(arrayMove(accountIds, oldIndex, newIndex));
            }
        }
    };

    if (accounts.length === 0) {
        return (
            <div className="bg-white dark:bg-base-100 rounded-2xl p-12 shadow-sm border border-gray-100 dark:border-base-200 text-center">
                <p className="text-gray-400 mb-2">{t('accounts.empty.title')}</p>
                <p className="text-sm text-gray-400">{t('accounts.empty.desc')}</p>
            </div>
        );
    }

    return (
        <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragStart={handleDragStart}
            onDragEnd={handleDragEnd}
        >
            <div className="overflow-x-auto">
                <table className="w-full">
                    <thead>
                        <tr className="border-b border-gray-100 dark:border-base-200 bg-gray-50 dark:bg-base-200">
                            <th className="pl-2 py-2 text-left w-8">
                                <span className="sr-only">{t('accounts.drag_to_reorder')}</span>
                            </th>
                            <th className="px-2 py-2 text-left w-10">
                                <AccountSelection
                                    checked={accounts.every(account => selectedIds.has(account.id))}
                                    indeterminate={accounts.some(account => selectedIds.has(account.id)) && !accounts.every(account => selectedIds.has(account.id))}
                                    onChange={onToggleAll}
                                    label={t('accounts.select_page')}
                                />
                            </th>
                            <th className="px-2 py-1 text-left rtl:text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider w-[300px] whitespace-nowrap">{t('accounts.table.email')}</th>
                            <th className="px-2 py-1 text-left rtl:text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider min-w-[340px] whitespace-nowrap">
                                <button
                                    type="button"
                                    onClick={() => handleSortToggle('reset_time')}
                                    className={cn(
                                        "inline-flex items-center gap-1 hover:text-blue-600 dark:hover:text-blue-400 transition-colors uppercase font-medium",
                                        sortConfig.key === 'reset_time' && "text-blue-600 dark:text-blue-400 font-semibold"
                                    )}
                                    title={t('accounts.table.sort_by_reset_time', '点击按配额重置时间排序')}
                                >
                                    <span>{quotaWindow === 'weekly' ? t('accounts.table.weekly_quota', '周配额') : t('accounts.table.quota')}</span>
                                    {sortConfig.key === 'reset_time' ? (
                                        sortConfig.direction === 'asc' ? <ArrowUp className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" /> : <ArrowDown className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />
                                    ) : (
                                        <ArrowUpDown className="w-3 h-3 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 opacity-60 hover:opacity-100" />
                                    )}
                                </button>
                            </th>
                            <th className="px-2 py-1 text-left rtl:text-right text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider w-[90px] whitespace-nowrap">
                                <button
                                    type="button"
                                    onClick={() => handleSortToggle('last_used')}
                                    className={cn(
                                        "inline-flex items-center gap-1 hover:text-blue-600 dark:hover:text-blue-400 transition-colors uppercase font-medium",
                                        sortConfig.key === 'last_used' && "text-blue-600 dark:text-blue-400 font-semibold"
                                    )}
                                    title={t('accounts.table.sort_by_last_used', '点击按最后使用时间排序')}
                                >
                                    <span>{t('accounts.table.last_used')}</span>
                                    {sortConfig.key === 'last_used' ? (
                                        sortConfig.direction === 'asc' ? <ArrowUp className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" /> : <ArrowDown className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />
                                    ) : (
                                        <ArrowUpDown className="w-3 h-3 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 opacity-60 hover:opacity-100" />
                                    )}
                                </button>
                            </th>
                            <th className="px-2 py-1 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider whitespace-nowrap sticky right-0 w-[120px] bg-gray-50 dark:bg-base-200 z-20 shadow-[-12px_0_12px_-12px_rgba(0,0,0,0.1)] dark:shadow-[-12px_0_12px_-12px_rgba(255,255,255,0.05)] text-center">{t('accounts.table.actions')}</th>
                        </tr >
                    </thead >
                    <SortableContext items={accountIds} strategy={verticalListSortingStrategy}>
                        <tbody className="divide-y divide-gray-100 dark:divide-base-200">
                            {sortedAccounts.map((account) => (
                                <SortableAccountRow
                                    key={account.id}
                                    account={account}
                                    selected={selectedIds.has(account.id)}
                                    isRefreshing={refreshingIds.has(account.id)}
                                    isCurrent={account.id === currentAccountId}
                                    isSwitching={account.id === switchingAccountId}
                                    isDragging={account.id === activeId}
                                    onSelect={() => onToggleSelect(account.id)}
                                    onSwitch={(targetIde?: string) => onSwitch(account.id, targetIde)}
                                    onRefresh={() => onRefresh(account.id)}
                                    onEditLabel={() => onEditLabel(account.id)}
                                    onDelete={() => onDelete(account.id)}
                            onExport={() => onExport?.(account.id)}
                                    quotaWindow={quotaWindow}
                                    isDragDisabled={isSortingActive}
                                />
                            ))}
                        </tbody>
                    </SortableContext>
                </table >
            </div >

            {/* 拖拽悬浮预览层 */}
            <DragOverlay>
                {
                    activeAccount ? (
                        <table className="w-full bg-white dark:bg-base-100 shadow-2xl rounded-lg border border-blue-200 dark:border-blue-800">
                            <tbody>
                                <tr className={cn(
                                    "border-b border-gray-100 dark:border-base-200",
                                    activeAccount.id === currentAccountId ? "bg-blue-100/70 dark:bg-blue-950/60" : "bg-blue-50 dark:bg-blue-900/30"
                                )}>
                                    <td className={cn(
                                        "pl-2 py-1 w-8",
                                        activeAccount.id === currentAccountId && "border-l border-l-blue-500/80 dark:border-l-blue-400/80"
                                    )}>
                                        <div className="flex items-center justify-center w-6 h-6 text-blue-500">
                                            <GripVertical className="w-4 h-4" />
                                        </div>
                                    </td>
                                    <td className="px-2 py-1 w-10">
                                        <input
                                            type="checkbox"
                                            className="checkbox checkbox-xs rounded border-2"
                                            checked={selectedIds.has(activeAccount.id)}
                                            readOnly
                                        />
                                    </td>
                                    <AccountRowContent
                                        account={activeAccount}
                                        isCurrent={activeAccount.id === currentAccountId}
                                        isRefreshing={refreshingIds.has(activeAccount.id)}
                                        isSwitching={activeAccount.id === switchingAccountId}
                                        onSwitch={() => { }}
                                        onRefresh={() => { }}
                                        onEditLabel={() => { }}
                                        onDelete={() => { }}
                                        isDisabled={Boolean(activeAccount.disabled)}
                                        quotaWindow={quotaWindow}
                                    />
                                </tr>
                            </tbody>
                        </table>
                    ) : null
                }
            </DragOverlay>
        </DndContext>
    );
}

export default AccountTable;
