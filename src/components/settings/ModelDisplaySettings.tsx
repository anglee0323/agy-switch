import { useEffect, useMemo, useState } from 'react';
import { Bot, BrainCircuit, Check, DollarSign, RefreshCw, RotateCcw, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { useAccountStore } from '../../stores/useAccountStore';
import { request } from '../../utils/request';
import { DEFAULT_QUOTA_VALUATION_CONFIG, QuotaValuationConfig } from '../../types/config';
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
    const [isRefreshingRate, setIsRefreshingRate] = useState(false);
    const [liveRate, setLiveRate] = useState<number | null>(null);

    useEffect(() => {
        let mounted = true;
        request<{ rate: number; fetched_at: number; stale: boolean }>('get_exchange_rate')
            .then(snapshot => {
                if (mounted && snapshot?.rate) {
                    setLiveRate(snapshot.rate);
                }
            })
            .catch(() => {});
        return () => { mounted = false; };
    }, []);

    const valuationConfig = config?.quota_valuation ?? DEFAULT_QUOTA_VALUATION_CONFIG;

    const updateValuationConfig = async (patch: Partial<QuotaValuationConfig>) => {
        if (!config) return;
        const nextValuation: QuotaValuationConfig = {
            ...DEFAULT_QUOTA_VALUATION_CONFIG,
            ...config.quota_valuation,
            ...patch,
        };
        try {
            await saveConfig({
                ...config,
                quota_valuation: nextValuation,
            }, true);
        } catch (err) {
            showToast(String(err), 'error');
        }
    };

    const handleRefreshExchangeRate = async () => {
        setIsRefreshingRate(true);
        try {
            const snapshot = await request<{ rate: number; fetched_at: number; stale: boolean }>('get_exchange_rate');
            if (snapshot?.rate) {
                setLiveRate(snapshot.rate);
                await updateValuationConfig({ usd_to_cny_rate: snapshot.rate });
                showToast(t('quota_valuation.rate_updated', { rate: snapshot.rate.toFixed(4) }), 'success');
            }
        } catch (err) {
            showToast(String(err), 'error');
        } finally {
            setIsRefreshingRate(false);
        }
    };

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

            {/* 配额价值与多币种换算配置区 */}
            <section className="bg-white dark:bg-slate-900 border border-gray-200/80 dark:border-slate-800 rounded-2xl p-5 shadow-xs transition-all">
                <div className="flex items-center justify-between pb-3 border-b border-gray-100 dark:border-slate-800">
                    <div className="flex items-center gap-2">
                        <div className="w-8 h-8 rounded-xl bg-emerald-50 dark:bg-emerald-950/40 text-emerald-600 dark:text-emerald-400 flex items-center justify-center">
                            <DollarSign className="w-4 h-4" />
                        </div>
                        <div>
                            <h3 className="text-sm font-semibold text-gray-800 dark:text-gray-100">
                                {t('quota_valuation.section_title', '配额价值与等效 Token 换算')}
                            </h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                {t('quota_valuation.section_desc', '结合活跃时间片与真实消耗，双层诚实反推额度等效价值，防虚标爆炸')}
                            </p>
                        </div>
                    </div>

                    <label className="relative inline-flex items-center cursor-pointer">
                        <input
                            type="checkbox"
                            checked={valuationConfig.enabled}
                            onChange={(e) => updateValuationConfig({ enabled: e.target.checked })}
                            className="sr-only peer"
                        />
                        <div className="w-11 h-6 bg-gray-200 peer-focus:outline-none rounded-full peer dark:bg-slate-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-emerald-600"></div>
                    </label>
                </div>

                {valuationConfig.enabled && (
                    <div className="mt-4 space-y-4">
                        {/* 计价币种与展示模式 */}
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <div>
                                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1.5">
                                    {t('quota_valuation.currency_label', '计价币种')}
                                </label>
                                <div className="grid grid-cols-3 gap-1.5 bg-gray-50 dark:bg-slate-800/60 p-1 rounded-xl border border-gray-200/80 dark:border-slate-800">
                                    {(['usd', 'cny', 'both'] as const).map((curr) => (
                                        <button
                                            key={curr}
                                            type="button"
                                            onClick={() => updateValuationConfig({ currency: curr })}
                                            className={`py-1.5 text-xs font-medium rounded-lg transition-all ${
                                                valuationConfig.currency === curr
                                                    ? 'bg-white dark:bg-slate-700 text-gray-900 dark:text-white shadow-xs'
                                                    : 'text-gray-600 dark:text-gray-400 hover:text-gray-900'
                                            }`}
                                        >
                                            {curr === 'usd' ? '$ USD' : curr === 'cny' ? '¥ CNY' : t('quota_valuation.both', '双币对照')}
                                        </button>
                                    ))}
                                </div>
                            </div>

                            <div>
                                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1.5">
                                    {t('quota_valuation.mode_label', '展示模式')}
                                </label>
                                <div className="grid grid-cols-2 gap-1.5 bg-gray-50 dark:bg-slate-800/60 p-1 rounded-xl border border-gray-200/80 dark:border-slate-800">
                                    {(['remaining', 'used_and_total'] as const).map((mode) => (
                                        <button
                                            key={mode}
                                            type="button"
                                            onClick={() => updateValuationConfig({ display_mode: mode })}
                                            className={`py-1.5 text-xs font-medium rounded-lg transition-all ${
                                                valuationConfig.display_mode === mode
                                                    ? 'bg-white dark:bg-slate-700 text-gray-900 dark:text-white shadow-xs'
                                                    : 'text-gray-600 dark:text-gray-400 hover:text-gray-900'
                                            }`}
                                        >
                                            {mode === 'remaining' ? t('quota_valuation.mode_remaining', '等效剩余') : t('quota_valuation.mode_used_and_total', '消耗 / 总额')}
                                        </button>
                                    ))}
                                </div>
                            </div>
                        </div>

                        {/* 汇率设置 */}
                        <div className="p-3.5 bg-gray-50/70 dark:bg-slate-800/40 rounded-xl border border-gray-200/60 dark:border-slate-800/80 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
                            <div className="min-w-0">
                                <div className="text-xs font-medium text-gray-800 dark:text-gray-200 flex items-center gap-2">
                                    <span>{t('quota_valuation.rate_title', 'USD → CNY 汇率')}</span>
                                    <span className="font-mono font-bold text-emerald-600 dark:text-emerald-400">
                                        1 USD = {(liveRate ?? valuationConfig.usd_to_cny_rate).toFixed(4)} CNY
                                    </span>
                                </div>
                                <div className="text-[11px] text-gray-500 dark:text-gray-400 mt-0.5">
                                    {t('quota_valuation.rate_desc', '支持 24 小时自动更新或手动自定义')}
                                </div>
                            </div>

                            <div className="flex items-center gap-2 shrink-0">
                                <label className="text-xs text-gray-600 dark:text-gray-400 flex items-center gap-1.5 cursor-pointer">
                                    <input
                                        type="checkbox"
                                        checked={valuationConfig.auto_exchange_rate}
                                        onChange={(e) => updateValuationConfig({ auto_exchange_rate: e.target.checked })}
                                        className="checkbox checkbox-xs rounded"
                                    />
                                    <span>{t('quota_valuation.auto_rate', '自动同步')}</span>
                                </label>

                                <button
                                    type="button"
                                    onClick={handleRefreshExchangeRate}
                                    disabled={isRefreshingRate}
                                    className="p-1.5 rounded-lg border border-gray-200 dark:border-slate-700 text-gray-600 dark:text-gray-300 hover:bg-white dark:hover:bg-slate-700 transition-colors disabled:opacity-50"
                                    title={t('common.refresh', '刷新')}
                                >
                                    <RefreshCw className={`w-3.5 h-3.5 ${isRefreshingRate ? 'animate-spin' : ''}`} />
                                </button>

                                {!valuationConfig.auto_exchange_rate && (
                                    <input
                                        type="number"
                                        step="0.01"
                                        min="3.0"
                                        max="20.0"
                                        defaultValue={valuationConfig.usd_to_cny_rate}
                                        onBlur={(e) => {
                                            const val = parseFloat(e.target.value);
                                            if (!isNaN(val) && val >= 3.0 && val <= 20.0) {
                                                updateValuationConfig({ usd_to_cny_rate: val });
                                            }
                                        }}
                                        className="w-20 px-2 py-1 text-xs font-mono bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-lg text-right"
                                    />
                                )}
                            </div>
                        </div>

                        {/* 第二层参考上限折算 */}
                        <div className="pt-2 border-t border-gray-100 dark:border-slate-800">
                            <div className="flex items-center justify-between mb-2">
                                <div>
                                    <span className="text-xs font-semibold text-gray-800 dark:text-gray-200">
                                        {t('quota_valuation.reference_caps_title', '100% 满额基准折算与自定义参考上限')}
                                    </span>
                                    <p className="text-[11px] text-gray-500 dark:text-gray-400">
                                        {t('quota_valuation.reference_caps_desc', '当账号未产生消耗（100%）时，依次取自定义上限或历史样本中位数折算等效价值')}
                                    </p>
                                </div>
                                <label className="text-xs text-gray-600 dark:text-gray-400 flex items-center gap-1.5 cursor-pointer shrink-0 ml-2">
                                    <input
                                        type="checkbox"
                                        checked={valuationConfig.use_reference_cap}
                                        onChange={(e) => updateValuationConfig({ use_reference_cap: e.target.checked })}
                                        className="checkbox checkbox-xs rounded"
                                    />
                                    <span>{t('quota_valuation.enable_ref_cap', '启用满额折算')}</span>
                                </label>
                            </div>

                            {valuationConfig.use_reference_cap && (
                                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5 mt-2">
                                    {[
                                        { key: 'gemini:5h', label: 'Gemini 5h ($)' },
                                        { key: 'gemini:weekly', label: 'Gemini Weekly ($)' },
                                        { key: '3p:5h', label: 'Claude / GPT 5h ($)' },
                                        { key: '3p:weekly', label: 'Claude / GPT Weekly ($)' },
                                        { key: 'argon:weekly', label: 'Argon Weekly ($)' },
                                    ].map((cap) => (
                                        <div key={cap.key} className="p-2.5 bg-gray-50/70 dark:bg-slate-800/40 rounded-xl border border-gray-200/60 dark:border-slate-800/80">
                                            <div className="text-[11px] font-medium text-gray-600 dark:text-gray-400 mb-1 truncate" title={cap.label}>
                                                {cap.label}
                                            </div>
                                            <input
                                                type="number"
                                                step="0.5"
                                                min="0"
                                                placeholder={t('quota_valuation.auto_calc', '自动校准中位数')}
                                                defaultValue={valuationConfig.reference_caps_usd?.[cap.key] ?? ''}
                                                onBlur={(e) => {
                                                    const raw = e.target.value.trim();
                                                    const nextMap = { ...(valuationConfig.reference_caps_usd ?? {}) };
                                                    if (!raw) {
                                                        delete nextMap[cap.key];
                                                    } else {
                                                        const val = parseFloat(raw);
                                                        if (!isNaN(val) && val > 0) {
                                                            nextMap[cap.key] = val;
                                                        } else {
                                                            delete nextMap[cap.key];
                                                        }
                                                    }
                                                    updateValuationConfig({ reference_caps_usd: nextMap });
                                                }}
                                                className="w-full px-2 py-1 text-xs font-mono bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-lg text-right"
                                            />
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    </div>
                )}
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
