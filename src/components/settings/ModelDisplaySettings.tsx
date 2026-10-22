import { useMemo, useState } from 'react';
import { Bot, BrainCircuit, Check, RefreshCw, RotateCcw, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { useAccountStore } from '../../stores/useAccountStore';
import {
    DEFAULT_PINNED_MODELS,
    getModelSortWeight,
    getCanonicalModelKey,
    getCanonicalModelDisplayName,
    getCanonicalModelSublabel,
    getCanonicalModelTag,
} from '../../config/modelConfig';
import { showToast } from '../common/ToastContainer';

interface ModelOption {
    id: string;
    label: string;
    group: 'gemini' | 'claude' | 'other';
    iconType: 'gemini' | 'claude' | 'bot';
    tag?: string;
}

// 兜底基准主模型（已预先归并多档位）
const BASELINE_CANONICAL_MODELS: ModelOption[] = [
    { id: 'gemini-4-argon', label: 'Gemini 4 Argon', group: 'gemini', iconType: 'gemini' },
    { id: 'gemini-3.1-pro', label: 'Gemini 3.1 Pro', group: 'gemini', iconType: 'gemini', tag: 'PRO' },
    { id: 'gemini-3.8-flash', label: 'Gemini 3.8 Flash', group: 'gemini', iconType: 'gemini', tag: 'FLASH' },
    { id: 'gemini-3.5-flash', label: 'Gemini 3.5 Flash', group: 'gemini', iconType: 'gemini', tag: 'FLASH' },
    { id: 'gemini-3-flash', label: 'Gemini 3 Flash', group: 'gemini', iconType: 'gemini', tag: 'FLASH' },
    { id: 'gemini-3.1-flash-lite', label: 'Gemini 3.1 Flash Lite', group: 'gemini', iconType: 'gemini', tag: 'LITE' },
    { id: 'gemini-2.5-pro', label: 'Gemini 2.5 Pro', group: 'gemini', iconType: 'gemini', tag: 'PRO' },
    { id: 'gemini-2.5-flash', label: 'Gemini 2.5 Flash', group: 'gemini', iconType: 'gemini', tag: 'FLASH' },
    { id: 'gemini-3.1-flash-image', label: 'Gemini 3.1 Flash Image', group: 'gemini', iconType: 'gemini', tag: 'IMAGE' },
    { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6 (Thinking)', group: 'claude', iconType: 'claude', tag: 'SONNET' },
    { id: 'claude-opus-4-6-thinking', label: 'Claude Opus 4.6 (Thinking)', group: 'claude', iconType: 'claude', tag: 'OPUS' },
    { id: 'gpt-oss-120b', label: 'GPT-OSS 120B', group: 'other', iconType: 'bot', tag: 'OPENAI' },
];

function detectModelGroup(id: string): 'gemini' | 'claude' | 'other' {
    const lower = id.toLowerCase();
    if (lower.includes('claude') || lower.includes('opus') || lower.includes('sonnet')) return 'claude';
    if (lower.includes('gemini') || lower.includes('imagen')) return 'gemini';
    return 'other';
}

interface ModelDisplaySettingsProps {
    onClose?: () => void;
    embedded?: boolean;
}

export default function ModelDisplaySettings({ onClose, embedded = false }: ModelDisplaySettingsProps) {
    const { t } = useTranslation();
    const { config, saveConfig } = useConfigStore();
    const accounts = useAccountStore(state => state.accounts);
    const refreshAllQuotas = useAccountStore(state => state.refreshAllQuotas);
    const [isRefreshing, setIsRefreshing] = useState(false);

    // 当前用户勾选的主模型列表（标准化去重）
    const currentPinned = useMemo(() => {
        const configured = config?.pinned_quota_models?.models;
        const source = (configured && configured.length > 0) ? configured : DEFAULT_PINNED_MODELS;
        return Array.from(new Set(source.map(getCanonicalModelKey)));
    }, [config?.pinned_quota_models?.models]);

    // 实时从所有账号中提取真实活跃的主模型（自动去重归并同系列 High/Medium/Low 档位）
    const allModels = useMemo(() => {
        const map = new Map<string, ModelOption>();

        // 1. 优先提取当前所有账号真实返回的活跃主模型
        for (const account of accounts) {
            for (const m of account.quota?.models || []) {
                if (!m.name) continue;
                const canonKey = getCanonicalModelKey(m.name);
                if (!map.has(canonKey)) {
                    const label = getCanonicalModelDisplayName(canonKey);
                    const group = detectModelGroup(canonKey);
                    const iconType: ModelOption['iconType'] = group === 'claude' ? 'claude' : group === 'gemini' ? 'gemini' : 'bot';
                    map.set(canonKey, {
                        id: canonKey,
                        label,
                        group,
                        iconType,
                        tag: getCanonicalModelTag(canonKey),
                    });
                }
            }
        }

        // 2. 兜底补齐常用基准主模型
        for (const bm of BASELINE_CANONICAL_MODELS) {
            if (!map.has(bm.id)) {
                map.set(bm.id, bm);
            }
        }

        // 3. 动态排序
        return Array.from(map.values()).sort((a, b) => {
            const weightA = getModelSortWeight(a.id);
            const weightB = getModelSortWeight(b.id);
            if (weightA !== weightB) return weightA - weightB;
            return a.label.localeCompare(b.label);
        });
    }, [accounts]);

    const handleToggleModel = async (canonKey: string) => {
        if (!config) return;
        const exists = currentPinned.includes(canonKey);
        let next: string[];
        if (exists) {
            next = currentPinned.filter(id => id !== canonKey);
            if (next.length === 0) {
                showToast(t('model_display.empty_tip', '请至少保留一个模型'), 'warning');
                return;
            }
        } else {
            next = [...currentPinned, canonKey];
        }

        try {
            await saveConfig({
                ...config,
                pinned_quota_models: { models: next }
            }, true);
            showToast(t('model_display.saved', '已更新卡片展示模型'), 'success');
        } catch (err) {
            showToast(String(err), 'error');
        }
    };

    const handleResetDefault = async () => {
        if (!config) return;
        try {
            await saveConfig({
                ...config,
                pinned_quota_models: { models: DEFAULT_PINNED_MODELS }
            }, true);
            showToast(t('model_display.reset_success', '已恢复默认推荐模型'), 'success');
        } catch (err) {
            showToast(String(err), 'error');
        }
    };

    const handleSelectAll = async () => {
        if (!config) return;
        const allIds = allModels.map(m => m.id);
        const isAllSelected = allIds.every(id => currentPinned.includes(id));
        const next = isAllSelected ? [DEFAULT_PINNED_MODELS[0]] : allIds;
        try {
            await saveConfig({
                ...config,
                pinned_quota_models: { models: next }
            }, true);
            showToast(t('model_display.saved', '已更新卡片展示模型'), 'success');
        } catch (err) {
            showToast(String(err), 'error');
        }
    };

    const handleRefreshLiveModels = async () => {
        if (isRefreshing) return;
        setIsRefreshing(true);
        try {
            await refreshAllQuotas();
            showToast(t('model_display.models_refreshed', '模型列表已更新'), 'success');
        } catch (err) {
            showToast(String(err), 'error');
        } finally {
            setIsRefreshing(false);
        }
    };

    const renderIcon = (type: ModelOption['iconType']) => {
        switch (type) {
            case 'claude':
                return <BrainCircuit className="w-4 h-4 text-purple-600 dark:text-purple-400" />;
            case 'gemini':
                return <Sparkles className="w-4 h-4 text-amber-500 dark:text-amber-400" />;
            case 'bot':
            default:
                return <Bot className="w-4 h-4 text-blue-500 dark:text-blue-400" />;
        }
    };

    const groups: { key: ModelOption['group']; title: string }[] = [
        { key: 'gemini', title: t('model_display.group_gemini', 'Gemini 系列模型 (Google 原生)') },
        { key: 'claude', title: t('model_display.group_claude', 'Claude 系列模型 (Anthropic)') },
        { key: 'other', title: t('model_display.group_other', '其他系列模型') },
    ];

    const isAllSelected = allModels.length > 0 && allModels.every(m => currentPinned.includes(m.id));

    return (
        <div className="space-y-4">
            <section className="rounded-2xl border border-gray-200/80 bg-white p-5 sm:p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/80">
                {/* 顶部操作栏：已选数量 + 全选/取消全选 + 实时刷新模型 */}
                <div className="flex flex-wrap items-center justify-between gap-3 mb-4 pb-3 border-b border-gray-100 dark:border-slate-800">
                    <div className="flex items-center gap-2.5">
                        <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">
                            {t('model_display.selected_models', '自定义展示模型')}
                        </span>
                        <span className="px-2 py-0.5 rounded-md text-xs font-semibold bg-blue-50 text-blue-600 dark:bg-blue-900/30 dark:text-blue-300">
                            {t('model_display.selected_count', { count: currentPinned.length })}
                        </span>
                    </div>

                    <div className="flex items-center gap-2">
                        <button
                            type="button"
                            onClick={handleResetDefault}
                            className="px-2.5 py-1 text-xs font-medium rounded-lg border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-slate-700 transition-colors flex items-center gap-1"
                            title={t('model_display.reset_default_title', '恢复为推荐的 3 个核心模型')}
                        >
                            <RotateCcw className="w-3 h-3 text-gray-500" />
                            <span>{t('model_display.reset_default', '恢复推荐')}</span>
                        </button>
                        <button
                            type="button"
                            onClick={handleSelectAll}
                            className="px-2.5 py-1 text-xs font-medium rounded-lg border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-slate-700 transition-colors"
                        >
                            {isAllSelected ? t('model_display.deselect_all', '取消全选') : t('model_display.select_all', '全选')}
                        </button>
                        <button
                            type="button"
                            disabled={isRefreshing}
                            onClick={handleRefreshLiveModels}
                            className="px-2.5 py-1 text-xs font-medium rounded-lg border border-gray-200 dark:border-slate-700 bg-gray-50 dark:bg-slate-800 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-slate-700 transition-colors flex items-center gap-1.5"
                            title={t('model_display.refresh_models', '从账号重新获取最新的模型列表')}
                        >
                            <RefreshCw className={`w-3 h-3 text-blue-500 ${isRefreshing ? 'animate-spin' : ''}`} />
                            <span>{isRefreshing ? t('model_display.refreshing_models', '正在获取最新模型...') : t('model_display.refresh_models', '刷新最新模型')}</span>
                        </button>
                    </div>
                </div>

                <p className="mb-4 text-xs text-gray-500 dark:text-gray-400">{t('model_display.availability_hint')}</p>

                {/* 分组模型列表 */}
                <div className="space-y-4">
                    {groups.map(group => {
                        const groupModels = allModels.filter(m => m.group === group.key);
                        if (groupModels.length === 0) return null;

                        return (
                            <div key={group.key} className="space-y-2">
                                <div className="text-xs font-semibold text-gray-700 dark:text-gray-300">
                                    {group.title}
                                </div>
                                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                                    {groupModels.map(model => {
                                        const isChecked = currentPinned.includes(model.id);
                                        const sublabel = getCanonicalModelSublabel(model.id, tiers => t('model_display.shared_pool', { tiers }));
                                        return (
                                            <div
                                                key={model.id}
                                                onClick={() => handleToggleModel(model.id)}
                                                className={`flex items-center justify-between p-3 rounded-xl border cursor-pointer select-none transition-all ${isChecked
                                                    ? 'border-blue-500/80 bg-blue-50/50 dark:bg-blue-500/10 dark:border-blue-500/60 shadow-xs'
                                                    : 'border-gray-200/80 dark:border-slate-800/80 bg-gray-50/40 dark:bg-slate-800/30 hover:bg-gray-100/60 dark:hover:bg-slate-800/60'
                                                    }`}
                                            >
                                                <div className="flex items-center gap-2.5 min-w-0">
                                                    <span className="shrink-0">{renderIcon(model.iconType)}</span>
                                                    <div className="min-w-0">
                                                        <div className="text-xs font-semibold text-gray-800 dark:text-gray-200 truncate" title={model.label}>
                                                            {model.label}
                                                        </div>
                                                        {sublabel ? (
                                                            <div className="text-xs text-gray-500 dark:text-gray-400 truncate" title={sublabel}>
                                                                {sublabel}
                                                            </div>
                                                        ) : (
                                                            <div className="text-xs font-mono text-gray-500 dark:text-gray-400 truncate" title={model.id}>
                                                                {model.id}
                                                            </div>
                                                        )}
                                                    </div>
                                                </div>

                                                <div className="flex items-center gap-2 shrink-0 ml-2">
                                                    {model.tag && (
                                                        <span className="text-[10px] font-bold px-1.5 py-0.5 rounded-md bg-gray-200/70 dark:bg-slate-700 text-gray-600 dark:text-gray-300">
                                                            {model.tag}
                                                        </span>
                                                    )}
                                                    <span className={`w-5 h-5 rounded-md border flex items-center justify-center transition-colors ${isChecked
                                                        ? 'bg-blue-600 border-blue-600 text-white'
                                                        : 'border-gray-300 dark:border-slate-600 bg-white dark:bg-slate-800'
                                                        }`}>
                                                        {isChecked && <Check className="w-3.5 h-3.5 stroke-[2.5]" />}
                                                    </span>
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>
                        );
                    })}
                </div>

                <div className="mt-4 pt-3 border-t border-gray-100 dark:border-slate-800 text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
                    {t('model_display.hint', '勾选后实时生效。已为您自动合并同系列多档位（如 High/Medium/Low），前台卡片与表格将以最简洁的合并视图监控其实际共享额度。')}
                </div>
            </section>

            {/* 完成关闭按钮 (如果不是内嵌设置页面) */}
            {!embedded && onClose && (
                <div className="flex justify-end pt-2">
                    <button
                        type="button"
                        onClick={onClose}
                        className="px-5 py-2 rounded-xl bg-blue-600 text-white font-medium text-xs hover:bg-blue-700 active:scale-95 transition-all shadow-sm"
                    >
                        {t('common.confirm', '完成')}
                    </button>
                </div>
            )}
        </div>
    );
}
