import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import {
    AlertCircle,
    Bot,
    Check,
    CheckCircle2,
    Copy,
    Eye,
    EyeOff,
    FileCode,
    Loader2,
    Plus,
    RefreshCw,
    Sparkles,
    Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { request as invoke } from '../../utils/request';
import { showToast } from '../common/ToastContainer';
import { ApiFormat, CustomModelEntry, TestConnectionResult } from '../../types/customModel';

export default function CustomModelSettings() {
    const { t } = useTranslation();
    const [models, setModels] = useState<CustomModelEntry[]>([]);
    const [loading, setLoading] = useState<boolean>(true);
    const [configPath, setConfigPath] = useState<string>('~/.gemini/antigravity/custom_models.json');
    const [testingId, setTestingId] = useState<string | null>(null);
    const [testResults, setTestResults] = useState<Record<string, TestConnectionResult>>({});

    // 模态框状态
    const [isModalOpen, setIsModalOpen] = useState<boolean>(false);
    const [editingIndex, setEditingIndex] = useState<number | null>(null);
    const [formDisplayName, setFormDisplayName] = useState<string>('');
    const [formApiUrl, setFormApiUrl] = useState<string>('');
    const [formApiKey, setFormApiKey] = useState<string>('');
    const [formExternalModel, setFormExternalModel] = useState<string>('');
    const [formApiFormat, setFormApiFormat] = useState<ApiFormat>('openai');
    const [formContextWindow, setFormContextWindow] = useState<number>(65536);
    const [formMaxOutput, setFormMaxOutput] = useState<number>(8192);
    const [showApiKey, setShowApiKey] = useState<boolean>(false);
    const [modalTesting, setModalTesting] = useState<boolean>(false);
    const [modalTestResult, setModalTestResult] = useState<TestConnectionResult | null>(null);

    // ESC 键快速关闭模态框
    useEffect(() => {
        if (!isModalOpen) return;
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                setIsModalOpen(false);
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [isModalOpen]);

    const loadModels = async () => {
        setLoading(true);
        try {
            const list = await invoke<CustomModelEntry[]>('get_custom_models');
            setModels(list || []);
        } catch (e) {
            console.error('Failed to load custom models', e);
            showToast(t('custom_models.load_failed', '读取自定义模型失败'), 'error');
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadModels();
        invoke<string>('get_custom_models_file_path')
            .then(setConfigPath)
            .catch(() => {});
    }, []);

    const saveModelsToBackend = async (newModels: CustomModelEntry[]) => {
        try {
            await invoke('save_custom_models', { models: newModels });
            setModels(newModels);
            showToast(t('custom_models.saved_success', '自定义模型保存成功'), 'success');
        } catch (e) {
            showToast(t('custom_models.save_failed', { error: String(e) }), 'error');
        }
    };

    const handleToggleEnable = async (index: number) => {
        const updated = [...models];
        updated[index].enabled = !updated[index].enabled;
        await saveModelsToBackend(updated);
    };

    const handleDelete = async (index: number) => {
        if (!window.confirm(t('custom_models.confirm_delete', '确定要删除该模型配置吗？'))) return;
        const updated = models.filter((_, i) => i !== index);
        await saveModelsToBackend(updated);
    };

    const handleTestConnection = async (entry: CustomModelEntry, key: string) => {
        setTestingId(key);
        try {
            const result = await invoke<TestConnectionResult>('test_custom_model_connection', { entry });
            setTestResults(prev => ({ ...prev, [key]: result }));
            if (result.success) {
                showToast(t('custom_models.test_success', { latency: result.latency_ms }), 'success');
            } else {
                showToast(result.message, 'error');
            }
        } catch (e) {
            setTestResults(prev => ({
                ...prev,
                [key]: { success: false, message: String(e) },
            }));
            showToast(String(e), 'error');
        } finally {
            setTestingId(null);
        }
    };

    const openAddModal = () => {
        setEditingIndex(null);
        setFormDisplayName('');
        setFormApiUrl('');
        setFormApiKey('');
        setFormExternalModel('');
        setFormApiFormat('openai');
        setFormContextWindow(65536);
        setFormMaxOutput(8192);
        setShowApiKey(false);
        setModalTestResult(null);
        setIsModalOpen(true);
    };

    const openEditModal = (index: number) => {
        const item = models[index];
        setEditingIndex(index);
        setFormDisplayName(item.displayName);
        setFormApiUrl(item.apiUrl);
        setFormApiKey(item.apiKey || '');
        setFormExternalModel(item.externalModelName);
        setFormApiFormat((item.apiFormat as ApiFormat) || 'openai');
        setFormContextWindow(item.contextWindow || 65536);
        setFormMaxOutput(item.maxOutputTokens || 8192);
        setShowApiKey(false);
        setModalTestResult(null);
        setIsModalOpen(true);
    };

    const handleModalTest = async () => {
        if (!formApiUrl.trim()) {
            showToast(t('custom_models.url_required', '请先填写 API 地址'), 'error');
            return;
        }
        setModalTesting(true);
        const tempEntry: CustomModelEntry = {
            name: `models/${formExternalModel.trim() || 'test'}`,
            displayName: formDisplayName.trim() || 'Test Model',
            provider: 'custom',
            apiFormat: formApiFormat,
            apiUrl: formApiUrl.trim(),
            apiKey: formApiKey.trim(),
            externalModelName: formExternalModel.trim() || 'default',
            enabled: true,
        };
        try {
            const result = await invoke<TestConnectionResult>('test_custom_model_connection', { entry: tempEntry });
            setModalTestResult(result);
            if (result.success) {
                showToast(t('custom_models.test_success', { latency: result.latency_ms }), 'success');
            } else {
                showToast(result.message, 'error');
            }
        } catch (e) {
            setModalTestResult({ success: false, message: String(e) });
            showToast(String(e), 'error');
        } finally {
            setModalTesting(false);
        }
    };

    const handleSaveModal = async () => {
        if (!formApiUrl.trim()) {
            showToast(t('custom_models.url_required', '请填写 API 地址'), 'error');
            return;
        }
        if (!formExternalModel.trim()) {
            showToast(t('custom_models.model_required', '请填写上游模型 ID (externalModelName)'), 'error');
            return;
        }

        const sanitizedExternal = formExternalModel.trim();
        const entryName = `models/${sanitizedExternal.replace(/\//g, '-')}`;
        const newEntry: CustomModelEntry = {
            name: entryName,
            displayName: formDisplayName.trim() || sanitizedExternal,
            provider: 'custom',
            apiFormat: formApiFormat,
            apiUrl: formApiUrl.trim(),
            apiKey: formApiKey.trim(),
            externalModelName: sanitizedExternal,
            enabled: true,
            contextWindow: formContextWindow > 0 ? formContextWindow : undefined,
            maxOutputTokens: formMaxOutput > 0 ? formMaxOutput : undefined,
        };

        const updated = [...models];
        if (editingIndex !== null) {
            updated[editingIndex] = { ...updated[editingIndex], ...newEntry };
        } else {
            const existingIdx = updated.findIndex(m => m.name === entryName || (m.apiUrl === newEntry.apiUrl && m.externalModelName === newEntry.externalModelName));
            if (existingIdx >= 0) {
                updated[existingIdx] = newEntry;
            } else {
                updated.push(newEntry);
            }
        }

        await saveModelsToBackend(updated);
        setIsModalOpen(false);
    };

    const copyPath = () => {
        navigator.clipboard.writeText(configPath);
        showToast(t('custom_models.copied_path', '配置文件路径已复制到剪贴板'), 'success');
    };

    return (
        <div className="space-y-6">
            <section className="rounded-2xl border border-gray-200/80 bg-white p-5 sm:p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/80 space-y-5">
                {/* 标题栏 */}
                <div className="flex flex-wrap items-start justify-between gap-4">
                    <div className="flex items-start gap-3">
                        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-violet-50 text-violet-600 dark:bg-violet-400/10 dark:text-violet-300">
                            <Sparkles className="h-5 w-5" />
                        </span>
                        <div>
                            <div className="flex items-center gap-2">
                                <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100">
                                    {t('custom_models.title', '自定义外部模型 (Custom Models)')}
                                </h3>
                                <span className="px-2 py-0.5 rounded-md text-xs font-medium bg-violet-50 text-violet-700 dark:bg-violet-900/30 dark:text-violet-300">
                                    {t('custom_models.badge', 'OpenAI / Claude / 自建')}
                                </span>
                            </div>
                            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
                                {t('custom_models.desc', '配置外部模型 API 端点与凭据。配置将自动存入社区标准 custom_models.json，打补丁后的 Antigravity 可直接在会话下拉框中调用。')}
                            </p>
                        </div>
                    </div>

                    <div className="flex items-center gap-2">
                        <button
                            type="button"
                            onClick={openAddModal}
                            className="flex items-center gap-1.5 rounded-xl bg-blue-600 px-3.5 py-2 text-xs font-medium text-white shadow-sm hover:bg-blue-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 transition-colors"
                        >
                            <Plus className="h-3.5 w-3.5" />
                            <span>{t('custom_models.add_model', '添加自定义模型')}</span>
                        </button>
                    </div>
                </div>

                {/* 存储路径卡片 */}
                <div className="flex flex-wrap items-center justify-between gap-3 p-3 rounded-xl bg-gray-50/70 dark:bg-slate-800/40 border border-gray-200/70 dark:border-slate-800 text-xs text-gray-500 dark:text-gray-400">
                    <div className="flex items-center gap-2 min-w-0">
                        <FileCode className="h-4 w-4 text-gray-400 shrink-0" />
                        <span className="shrink-0">{t('custom_models.storage_path', '标准配置文件：')}</span>
                        <span className="font-mono text-gray-700 dark:text-gray-300 truncate select-all">{configPath}</span>
                    </div>
                    <button
                        type="button"
                        onClick={copyPath}
                        className="flex items-center gap-1 text-blue-600 dark:text-blue-400 hover:underline shrink-0"
                    >
                        <Copy className="h-3 w-3" />
                        <span>{t('custom_models.copy', '复制路径')}</span>
                    </button>
                </div>

                {/* 模型列表 */}
                {loading ? (
                    <div className="flex items-center justify-center py-8 text-gray-400 text-xs">
                        <Loader2 className="h-4 w-4 animate-spin mr-2" />
                        <span>{t('custom_models.loading', '正在加载自定义模型...')}</span>
                    </div>
                ) : models.length === 0 ? (
                    <div className="rounded-xl border border-dashed border-gray-200 p-8 text-center dark:border-slate-800 space-y-4">
                        <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-gray-50 text-gray-400 dark:bg-slate-800/80">
                            <Bot className="h-6 w-6" />
                        </div>
                        <div>
                            <h4 className="text-sm font-semibold text-gray-800 dark:text-gray-200">
                                {t('custom_models.empty_title', '尚未配置任何自定义模型')}
                            </h4>
                            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400 max-w-md mx-auto">
                                {t('custom_models.empty_desc', '你可以填入任何支持 OpenAI 兼容格式或第三方中转的 API 端点与 Key，通过标准配置在 Antigravity 中协同使用。')}
                            </p>
                        </div>
                        <div className="pt-1">
                            <button
                                type="button"
                                onClick={openAddModal}
                                className="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-xs font-medium text-white hover:bg-blue-700 shadow-sm transition-colors"
                            >
                                <Plus className="h-3.5 w-3.5" />
                                <span>{t('custom_models.add_first', '添加第一个模型')}</span>
                            </button>
                        </div>
                    </div>
                ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-3.5">
                        {models.map((item, idx) => {
                            const cardKey = `model-${idx}-${item.name}`;
                            const isTesting = testingId === cardKey;
                            const testRes = testResults[cardKey];
                            const formatLabel = (item.apiFormat || 'openai').toUpperCase();

                            return (
                                <div
                                    key={cardKey}
                                    className={`rounded-xl border p-4 transition-all ${
                                        item.enabled
                                            ? 'border-gray-200/90 bg-white dark:border-slate-800 dark:bg-slate-900/90 shadow-xs'
                                            : 'border-gray-200/60 bg-gray-50/50 dark:border-slate-800/60 dark:bg-slate-900/40 opacity-75'
                                    }`}
                                >
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="flex items-start gap-2.5 min-w-0">
                                            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-gray-100 dark:bg-slate-800 text-gray-600 dark:text-gray-300">
                                                <Bot className="h-4 w-4" />
                                            </div>
                                            <div className="min-w-0">
                                                <div className="flex items-center gap-2 flex-wrap">
                                                    <span className="font-semibold text-sm text-gray-900 dark:text-gray-100 truncate">
                                                        {item.displayName || item.externalModelName}
                                                    </span>
                                                    <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-medium border bg-slate-50 text-slate-700 border-slate-200 dark:bg-slate-800 dark:text-slate-300 dark:border-slate-700">
                                                        {formatLabel}
                                                    </span>
                                                </div>
                                                <div className="font-mono text-[11px] text-gray-500 dark:text-gray-400 truncate mt-0.5">
                                                    {item.externalModelName}
                                                </div>
                                            </div>
                                        </div>

                                        {/* 启用/停用 Switch */}
                                        <button
                                            type="button"
                                            role="switch"
                                            aria-checked={item.enabled}
                                            onClick={() => handleToggleEnable(idx)}
                                            title={item.enabled ? t('custom_models.enabled', '已启用') : t('custom_models.disabled', '已停用')}
                                            className="shrink-0 rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
                                        >
                                            <span className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${item.enabled ? 'bg-blue-600' : 'bg-gray-300 dark:bg-slate-700'}`}>
                                                <span className={`inline-block h-4 w-4 transform rounded-full bg-white shadow-sm transition-transform ${item.enabled ? 'translate-x-[18px]' : 'translate-x-0.5'}`} />
                                            </span>
                                        </button>
                                    </div>

                                    {/* 端点与 Key 详情 */}
                                    <div className="mt-3 pt-3 border-t border-gray-100 dark:border-slate-800/80 space-y-1.5 text-xs">
                                        <div className="flex items-center justify-between text-gray-500 dark:text-gray-400">
                                            <span>{t('custom_models.endpoint', '端点')}</span>
                                            <span className="font-mono text-gray-700 dark:text-gray-300 truncate max-w-[200px]" title={item.apiUrl}>
                                                {item.apiUrl}
                                            </span>
                                        </div>
                                        <div className="flex items-center justify-between text-gray-500 dark:text-gray-400">
                                            <span>{t('custom_models.api_key', '密钥')}</span>
                                            <span className="font-mono text-gray-600 dark:text-gray-400">
                                                {item.apiKey ? `${item.apiKey.slice(0, 4)}••••${item.apiKey.slice(-4)}` : t('custom_models.no_key', '未设置')}
                                            </span>
                                        </div>
                                    </div>

                                    {/* 测试结果条目 */}
                                    {testRes && (
                                        <div className={`mt-2.5 p-2 rounded-lg text-xs flex items-start gap-1.5 ${
                                            testRes.success
                                                ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-950/30 dark:text-emerald-300 border border-emerald-200 dark:border-emerald-800'
                                                : 'bg-red-50 text-red-700 dark:bg-red-950/30 dark:text-red-300 border border-red-200 dark:border-red-900'
                                        }`}>
                                            {testRes.success ? (
                                                <CheckCircle2 className="h-3.5 w-3.5 shrink-0 mt-0.5 text-emerald-500" />
                                            ) : (
                                                <AlertCircle className="h-3.5 w-3.5 shrink-0 mt-0.5 text-red-500" />
                                            )}
                                            <span className="break-all leading-tight">{testRes.message}</span>
                                        </div>
                                    )}

                                    {/* 操作栏 */}
                                    <div className="mt-3.5 pt-2.5 border-t border-gray-100 dark:border-slate-800 flex items-center justify-between gap-2">
                                        <button
                                            type="button"
                                            disabled={isTesting}
                                            onClick={() => handleTestConnection(item, cardKey)}
                                            className="inline-flex items-center gap-1 rounded-md px-2.5 py-1 text-xs font-medium text-gray-600 hover:text-blue-600 hover:bg-blue-50/80 dark:text-gray-300 dark:hover:text-blue-300 dark:hover:bg-blue-900/30 transition-colors disabled:opacity-50"
                                        >
                                            {isTesting ? (
                                                <Loader2 className="h-3 w-3 animate-spin text-blue-500" />
                                            ) : (
                                                <RefreshCw className="h-3 w-3" />
                                            )}
                                            <span>{isTesting ? t('custom_models.testing', '测试中...') : t('custom_models.test_ping', '测试连接')}</span>
                                        </button>

                                        <div className="flex items-center gap-1">
                                            <button
                                                type="button"
                                                onClick={() => openEditModal(idx)}
                                                className="rounded-md px-2.5 py-1 text-xs text-gray-600 hover:text-gray-900 hover:bg-gray-100 dark:text-gray-300 dark:hover:text-gray-100 dark:hover:bg-slate-800 transition-colors"
                                            >
                                                {t('custom_models.edit', '编辑')}
                                            </button>
                                            <button
                                                type="button"
                                                onClick={() => handleDelete(idx)}
                                                className="rounded-md p-1 text-gray-400 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-950/30 transition-colors"
                                                title={t('custom_models.delete', '删除')}
                                            >
                                                <Trash2 className="h-3.5 w-3.5" />
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                )}

                {/* 新增/编辑模态框 */}
                {isModalOpen && createPortal(
                    <div
                        className="fixed inset-0 z-[99999] overflow-y-auto bg-black/60 backdrop-blur-xs flex justify-center p-3 sm:p-4"
                        style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0 }}
                    >
                        {/* 顶部可拖拽区域 (Tauri) */}
                        <div data-tauri-drag-region className="fixed top-0 left-0 right-0 h-8 z-[100000]" />

                        {/* 点击遮罩外部关闭 */}
                        <div className="fixed inset-0 z-0" onClick={() => setIsModalOpen(false)} />

                        {/* 模态框主体卡片 */}
                        <div className="relative z-10 w-full max-w-lg my-auto rounded-2xl bg-white dark:bg-slate-900 shadow-2xl border border-gray-200 dark:border-slate-800 overflow-hidden flex flex-col max-h-[calc(100vh-1.5rem)] sm:max-h-[calc(100vh-2.5rem)]">
                            {/* 头部 */}
                            <div className="p-4 sm:p-5 border-b border-gray-100 dark:border-slate-800 flex items-center justify-between shrink-0">
                                <div>
                                    <h3 className="text-sm sm:text-base font-semibold text-gray-900 dark:text-gray-100">
                                        {editingIndex !== null
                                            ? t('custom_models.modal_edit_title', '编辑自定义模型')
                                            : t('custom_models.modal_add_title', '添加自定义模型')}
                                    </h3>
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                                        {t('custom_models.modal_subtitle', '输入外部模型服务地址与参数，保存后即时写入 custom_models.json。')}
                                    </p>
                                </div>
                                <button
                                    type="button"
                                    onClick={() => setIsModalOpen(false)}
                                    className="rounded-lg p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 dark:hover:bg-slate-800 transition-colors"
                                >
                                    <span className="sr-only">Close</span>
                                    ✕
                                </button>
                            </div>

                            {/* 表单内容 */}
                            <div className="p-4 sm:p-5 space-y-3.5 sm:space-y-4 overflow-y-auto flex-1 min-h-0 text-xs">
                                {/* 上游模型标识 externalModelName */}
                                <div>
                                    <label className="block text-gray-700 dark:text-gray-300 font-medium mb-1">
                                        {t('custom_models.external_model_id', '上游模型 ID (externalModelName) *')}
                                    </label>
                                    <input
                                        type="text"
                                        value={formExternalModel}
                                        onChange={e => setFormExternalModel(e.target.value)}
                                        placeholder="e.g. deepseek-chat, gpt-4o, claude-3-7-sonnet"
                                        className="w-full rounded-xl border border-gray-200 bg-white px-3 py-2 text-xs text-gray-800 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200 focus:border-blue-500 focus:outline-none"
                                    />
                                    <p className="text-[11px] text-gray-400 mt-1">
                                        {t('custom_models.external_model_desc', '实际传给 API 服务端的 model 标识符。')}
                                    </p>
                                </div>

                                {/* 展示名称 displayName */}
                                <div>
                                    <label className="block text-gray-700 dark:text-gray-300 font-medium mb-1">
                                        {t('custom_models.display_name', '展示名称 (Display Name)')}
                                    </label>
                                    <input
                                        type="text"
                                        value={formDisplayName}
                                        onChange={e => setFormDisplayName(e.target.value)}
                                        placeholder="e.g. DeepSeek-V3 (本地/中转)"
                                        className="w-full rounded-xl border border-gray-200 bg-white px-3 py-2 text-xs text-gray-800 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200 focus:border-blue-500 focus:outline-none"
                                    />
                                </div>

                                {/* API 端点 apiUrl */}
                                <div>
                                    <label className="block text-gray-700 dark:text-gray-300 font-medium mb-1">
                                        {t('custom_models.api_url', 'API 端点 URL *')}
                                    </label>
                                    <input
                                        type="text"
                                        value={formApiUrl}
                                        onChange={e => setFormApiUrl(e.target.value)}
                                        placeholder="https://api.your-provider.com/v1/chat/completions"
                                        className="w-full font-mono rounded-xl border border-gray-200 bg-white px-3 py-2 text-xs text-gray-800 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200 focus:border-blue-500 focus:outline-none"
                                    />
                                </div>

                                {/* 协议格式 apiFormat */}
                                <div>
                                    <label className="block text-gray-700 dark:text-gray-300 font-medium mb-1">
                                        {t('custom_models.api_format', '协议格式 (API Format)')}
                                    </label>
                                    <select
                                        value={formApiFormat}
                                        onChange={e => setFormApiFormat(e.target.value as ApiFormat)}
                                        className="w-full rounded-xl border border-gray-200 bg-white px-3 py-2 text-xs text-gray-800 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200 focus:border-blue-500 focus:outline-none"
                                    >
                                        <option value="openai">OpenAI Chat Completions (默认 / 兼容绝大多数服务)</option>
                                        <option value="anthropic">Anthropic Messages</option>
                                        <option value="google">Google Gemini</option>
                                    </select>
                                </div>

                                {/* API 密钥 apiKey */}
                                <div>
                                    <label className="block text-gray-700 dark:text-gray-300 font-medium mb-1">
                                        {t('custom_models.api_key_label', 'API Key 密钥')}
                                    </label>
                                    <div className="relative">
                                        <input
                                            type={showApiKey ? 'text' : 'password'}
                                            value={formApiKey}
                                            onChange={e => setFormApiKey(e.target.value)}
                                            placeholder="sk-..."
                                            className="w-full font-mono rounded-xl border border-gray-200 bg-white px-3 py-2 pr-9 text-xs text-gray-800 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200 focus:border-blue-500 focus:outline-none"
                                        />
                                        <button
                                            type="button"
                                            onClick={() => setShowApiKey(!showApiKey)}
                                            className="absolute right-2.5 top-2.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
                                        >
                                            {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                                        </button>
                                    </div>
                                </div>

                                {/* 上下文与输出限制 */}
                                <div className="grid grid-cols-2 gap-3 pt-1">
                                    <div>
                                        <label className="block text-gray-600 dark:text-gray-400 text-[11px] mb-1">
                                            {t('custom_models.context_window', '上下文窗口 (Tokens)')}
                                        </label>
                                        <input
                                            type="number"
                                            value={formContextWindow}
                                            onChange={e => setFormContextWindow(Number(e.target.value))}
                                            className="w-full font-mono rounded-lg border border-gray-200 bg-white px-2.5 py-1.5 text-xs text-gray-800 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200"
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-gray-600 dark:text-gray-400 text-[11px] mb-1">
                                            {t('custom_models.max_output', '单次最大输出 (Tokens)')}
                                        </label>
                                        <input
                                            type="number"
                                            value={formMaxOutput}
                                            onChange={e => setFormMaxOutput(Number(e.target.value))}
                                            className="w-full font-mono rounded-lg border border-gray-200 bg-white px-2.5 py-1.5 text-xs text-gray-800 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200"
                                        />
                                    </div>
                                </div>

                                {/* 探测测试反馈 */}
                                {modalTestResult && (
                                    <div className={`p-2.5 rounded-xl text-xs flex items-start gap-2 ${
                                        modalTestResult.success
                                            ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-950/30 dark:text-emerald-300 border border-emerald-200 dark:border-emerald-800'
                                            : 'bg-red-50 text-red-700 dark:bg-red-950/30 dark:text-red-300 border border-red-200 dark:border-red-900'
                                    }`}>
                                        {modalTestResult.success ? (
                                            <CheckCircle2 className="h-4 w-4 shrink-0 text-emerald-500 mt-0.5" />
                                        ) : (
                                            <AlertCircle className="h-4 w-4 shrink-0 text-red-500 mt-0.5" />
                                        )}
                                        <span className="break-all">{modalTestResult.message}</span>
                                    </div>
                                )}
                            </div>

                            {/* 底部按钮栏 */}
                            <div className="p-3.5 sm:p-4 border-t border-gray-100 dark:border-slate-800 bg-gray-50/70 dark:bg-slate-900/70 flex items-center justify-between gap-3 shrink-0">
                                <button
                                    type="button"
                                    disabled={modalTesting || !formApiUrl.trim()}
                                    onClick={handleModalTest}
                                    className="flex items-center gap-1.5 rounded-xl border border-gray-200 bg-white px-3.5 py-2 text-xs font-medium text-gray-700 hover:bg-gray-50 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-300 dark:hover:bg-slate-700 disabled:opacity-50 transition-colors"
                                >
                                    {modalTesting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
                                    <span>{modalTesting ? t('custom_models.testing', '测试中...') : t('custom_models.test_connection', '测试连通性')}</span>
                                </button>

                                <div className="flex items-center gap-2">
                                    <button
                                        type="button"
                                        onClick={() => setIsModalOpen(false)}
                                        className="rounded-xl px-4 py-2 text-xs font-medium text-gray-600 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-slate-800 transition-colors"
                                    >
                                        {t('custom_models.cancel', '取消')}
                                    </button>
                                    <button
                                        type="button"
                                        onClick={handleSaveModal}
                                        className="flex items-center gap-1.5 rounded-xl bg-blue-600 px-4 py-2 text-xs font-medium text-white hover:bg-blue-700 shadow-sm transition-colors"
                                    >
                                        <Check className="h-3.5 w-3.5" />
                                        <span>{t('custom_models.save', '保存并生效')}</span>
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>,
                    document.body
                )}
            </section>
        </div>
    );
}
