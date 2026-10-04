import { useEffect, useRef, useState } from 'react';
import { Check, Clock3, Database, FolderOpen, Globe2, HardDrive, Monitor, Moon, RefreshCw, Sun } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../stores/useConfigStore';
import { AppConfig } from '../types/config';
import { request as invoke } from '../utils/request';
import { showToast } from '../components/common/ToastContainer';
import { open } from '@tauri-apps/plugin-dialog';
import UpdateSettings from '../components/settings/UpdateSettings';
import DesktopSettings from '../components/settings/DesktopSettings';
import ModelDisplaySettings from '../components/settings/ModelDisplaySettings';
import CustomModelSettings from '../components/settings/CustomModelSettings';
import { AutoSwitchSettings } from '../components/autoSwitch/AutoSwitch';
import SettingsNavigation, { SETTINGS_SECTIONS, SettingsSection } from '../components/settings/SettingsNavigation';
import '../components/settings/SettingsLayout.css';

const LANGUAGES = [
    { code: 'zh', label: '简体中文' },
    { code: 'en', label: 'English' },
];

const REFRESH_INTERVALS = [5, 15, 30, 60];
const SYNC_INTERVALS = [1, 5, 15, 30];

function Settings() {
    const { t } = useTranslation();
    const { config, loading, error, loadConfig, saveConfig } = useConfigStore();
    const [dataDirPath, setDataDirPath] = useState('~/.antigravity_tools');
    const [section, setSection] = useState<SettingsSection>('general');
    const layout = useRef<HTMLDivElement>(null);
    const contentScroller = useRef<HTMLDivElement>(null);

    // Categories start at the top; mounted panels retain their unsaved drafts.
    useEffect(() => {
        if (layout.current) layout.current.scrollTop = 0;
        if (contentScroller.current) contentScroller.current.scrollTop = 0;
    }, [section]);

    useEffect(() => {
        loadConfig();
        invoke<string>('get_data_dir_path')
            .then(setDataDirPath)
            .catch(() => {});
    }, [loadConfig]);

    const updateConfig = async (patch: Partial<AppConfig>) => {
        if (!config) return;
        try {
            await saveConfig({ ...config, ...patch }, true);
            showToast(t('local_settings.saved'), 'success');
        } catch (error) {
            showToast(t('local_settings.save_failed', { error: String(error) }), 'error');
        }
    };

    const openDataFolder = async () => {
        try {
            await invoke('open_data_folder');
        } catch (error) {
            showToast(t('local_settings.open_failed', { error: String(error) }), 'error');
        }
    };

    const chooseExecutable = async (field: 'antigravity_executable' | 'antigravity_ide_executable') => {
        try {
            const selected = await open({ multiple: false, directory: false, title: t('local_settings.choose_executable') });
            if (typeof selected === 'string') await updateConfig({ [field]: selected });
        } catch (error) {
            showToast(t('local_settings.save_failed', { error: String(error) }), 'error');
        }
    };

    const themeOptions = [
        { value: 'system', label: t('local_settings.system'), icon: Monitor },
        { value: 'light', label: t('local_settings.light'), icon: Sun },
        { value: 'dark', label: t('local_settings.dark'), icon: Moon },
    ];
    const selectedLanguage = config?.language?.toLowerCase().startsWith('en') ? 'en' : 'zh';
    const refreshInterval = config?.refresh_interval ?? 15;
    const syncInterval = config?.sync_interval ?? 5;
    const refreshOptions = [...new Set([...REFRESH_INTERVALS, refreshInterval])].sort((a, b) => a - b);
    const syncOptions = [...new Set([...SYNC_INTERVALS, syncInterval])].sort((a, b) => a - b);

    const renderSwitch = (enabled: boolean, label: string, onClick: () => void) => (
        <button
            type="button"
            role="switch"
            aria-label={label}
            aria-checked={enabled}
            disabled={!config}
            onClick={onClick}
            className="shrink-0 rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-slate-800 disabled:cursor-not-allowed disabled:opacity-60"
        >
            <span className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${enabled ? 'bg-blue-600' : 'bg-gray-300 dark:bg-slate-600'}`}>
                <span className={`inline-block h-5 w-5 transform rounded-full bg-white shadow-sm transition-transform ${enabled ? 'translate-x-[22px]' : 'translate-x-0.5'}`} />
            </span>
        </button>
    );

    const content: Record<SettingsSection, React.ReactNode> = {
        general: (
            <div className="space-y-6">
                {/* 1. 界面外观与语言 */}
                <section className="rounded-2xl border border-gray-200/80 bg-white p-5 sm:p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/80">
                    <div className="mb-5 flex items-start gap-3">
                        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-50 text-amber-600 dark:bg-amber-400/10 dark:text-amber-300">
                            <Sun className="h-5 w-5" />
                        </span>
                        <div>
                            <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100">
                                {t('settings_sections.appearance_and_language', '界面外观与语言')}
                            </h3>
                            <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
                                {t('settings_sections.appearance_and_language_desc', '自定义应用的主题色彩偏好与多语言界面展示。')}
                            </p>
                        </div>
                    </div>
                    <div className="space-y-5">
                        <div>
                            <div className="mb-2 text-xs font-semibold text-gray-700 dark:text-gray-300 flex items-center gap-1.5">
                                <span>{t('local_settings.theme')}</span>
                            </div>
                            <div className="grid grid-cols-3 gap-3">
                                {themeOptions.map(({ value, label, icon: Icon }) => {
                                    const selected = (config?.theme || 'system') === value;
                                    return (
                                        <button
                                            key={value}
                                            type="button"
                                            onClick={() => updateConfig({ theme: value })}
                                            disabled={!config}
                                            aria-pressed={selected}
                                            className={`flex min-h-[70px] flex-col items-center justify-center gap-1.5 rounded-xl border p-2.5 text-xs font-medium transition-all disabled:cursor-not-allowed disabled:opacity-60 ${
                                                selected
                                                    ? 'border-blue-500 bg-blue-50/70 text-blue-700 shadow-xs ring-1 ring-blue-500/20 dark:border-blue-500 dark:bg-blue-500/15 dark:text-blue-300'
                                                    : 'border-gray-200 bg-gray-50/60 text-gray-600 hover:border-gray-300 hover:bg-gray-100 dark:border-slate-800 dark:bg-slate-800/40 dark:text-gray-300 dark:hover:border-slate-700'
                                            }`}
                                        >
                                            <Icon className={`h-4 w-4 ${selected ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400 dark:text-gray-500'}`} />
                                            <span>{label}</span>
                                        </button>
                                    );
                                })}
                            </div>
                        </div>

                        <div>
                            <div className="mb-2 text-xs font-semibold text-gray-700 dark:text-gray-300 flex items-center gap-1.5">
                                <Globe2 className="h-3.5 w-3.5 text-blue-500" />
                                <span>{t('local_settings.language')}</span>
                            </div>
                            <div className="grid grid-cols-2 gap-2 rounded-xl bg-gray-100/80 p-1 dark:bg-slate-800/60">
                                {LANGUAGES.map((language) => {
                                    const selected = selectedLanguage === language.code;
                                    return (
                                        <button
                                            key={language.code}
                                            type="button"
                                            onClick={() => updateConfig({ language: language.code })}
                                            disabled={!config}
                                            aria-pressed={selected}
                                            className={`flex items-center justify-center gap-2 rounded-lg px-3 py-2 text-xs font-medium transition-all disabled:cursor-not-allowed disabled:opacity-60 ${
                                                selected
                                                    ? 'bg-white text-blue-700 shadow-xs dark:bg-slate-700 dark:text-blue-300'
                                                    : 'text-gray-500 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-100'
                                            }`}
                                        >
                                            {selected && <Check className="h-3.5 w-3.5" />}
                                            {language.label}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    </div>
                </section>

                {/* 2. 桌面与系统窗口 */}
                <UpdateSettings />
                <DesktopSettings />

                {/* 3. 数据存储与关联应用 */}
                <section className="rounded-2xl border border-gray-200/80 bg-white p-5 sm:p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/80 space-y-5">
                    <div className="flex items-start gap-3">
                        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-emerald-50 text-emerald-600 dark:bg-emerald-400/10 dark:text-emerald-300">
                            <HardDrive className="h-5 w-5" />
                        </span>
                        <div>
                            <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100">
                                {t('settings_sections.storage_and_executables', '关联应用与数据存储')}
                            </h3>
                            <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
                                {t('settings_sections.storage_and_executables_desc', '管理本机账号凭据存储位置与外部关联客户端的识别路径。')}
                            </p>
                        </div>
                    </div>

                    {/* 卡片 1：本地数据存储 */}
                    <div className="rounded-xl border border-gray-200/70 bg-gray-50/60 p-4 dark:border-slate-800 dark:bg-slate-800/40">
                        <div className="flex flex-wrap items-start justify-between gap-3">
                            <div className="min-w-0">
                                <div className="text-xs font-semibold text-gray-800 dark:text-gray-200 flex items-center gap-1.5">
                                    <Database className="h-3.5 w-3.5 text-blue-500" />
                                    {t('local_settings.data_directory', '本地数据存储')}
                                </div>
                                <p className="mt-1 text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
                                    {t('local_settings.data_directory_desc', '保存多账号配置、加密凭证与配额缓存。所有数据仅存储于本机，绝不上传云端。')}
                                </p>
                                <div className="mt-2.5 inline-block rounded-lg bg-white px-2.5 py-1 font-mono text-xs text-gray-600 dark:bg-slate-900/90 dark:text-gray-300 border border-gray-200/80 dark:border-slate-700/80 break-all shadow-2xs">
                                    {dataDirPath}
                                </div>
                            </div>
                            <button
                                type="button"
                                onClick={openDataFolder}
                                className="flex shrink-0 items-center gap-1.5 rounded-lg border border-gray-200 bg-white px-3 py-1.5 text-xs font-medium text-gray-700 transition-colors hover:border-blue-400 hover:text-blue-600 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200 dark:hover:border-blue-500 dark:hover:text-blue-300 shadow-2xs"
                            >
                                <FolderOpen className="h-3.5 w-3.5" />
                                {t('local_settings.open_directory', '在访达中打开')}
                            </button>
                        </div>
                    </div>

                    {/* 卡片 2：关联客户端路径 */}
                    <div className="rounded-xl border border-gray-200/70 bg-gray-50/60 p-4 dark:border-slate-800 dark:bg-slate-800/40 space-y-3">
                        <div>
                            <div className="text-xs font-semibold text-gray-800 dark:text-gray-200 flex items-center gap-1.5">
                                <Monitor className="h-3.5 w-3.5 text-indigo-500" />
                                {t('local_settings.application_paths', '关联客户端路径')}
                            </div>
                            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
                                {t('local_settings.application_paths_desc', '账号切换时用于注入登录凭据并重启应用。系统默认自动识别，无需手动配置。')}
                            </p>
                        </div>

                        <div className="space-y-2.5 pt-1">
                            {([
                                { field: 'antigravity_executable', nameKey: 'app_label', defaultName: 'AntiGravity 桌面应用', descKey: 'app_desc', defaultDesc: '用于桌面应用账号与会话无缝切换' },
                                { field: 'antigravity_ide_executable', nameKey: 'ide_label', defaultName: 'AntiGravity IDE / VS Code 插件', descKey: 'ide_desc', defaultDesc: '用于独立 IDE 客户端或 VS Code 插件宿主会话同步' },
                            ] as const).map(({ field, nameKey, defaultName, descKey, defaultDesc }) => {
                                const isCustom = Boolean(config?.[field]);
                                return (
                                    <div key={field} className="flex flex-wrap items-center justify-between gap-3 p-3.5 rounded-xl bg-white dark:bg-slate-900/70 border border-gray-200/80 dark:border-slate-700/80 shadow-2xs">
                                        <div className="min-w-0">
                                            <div className="flex items-center gap-2">
                                                <span className="text-xs font-medium text-gray-800 dark:text-gray-200">{t(`local_settings.${nameKey}`, defaultName)}</span>
                                                <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-xs font-medium ${isCustom ? 'bg-amber-50 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300' : 'bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300'}`}>
                                                    <span className={`w-1.5 h-1.5 rounded-full ${isCustom ? 'bg-amber-500' : 'bg-emerald-500'}`} />
                                                    {isCustom ? t('local_settings.custom_path', '已自定义路径') : t('local_settings.automatic_detection', '已自动识别')}
                                                </span>
                                            </div>
                                            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                                                {t(`local_settings.${descKey}`, defaultDesc)}
                                            </p>
                                            <div className="mt-1.5 font-mono text-xs text-gray-600 dark:text-gray-300 break-all">
                                                {config?.[field] || t('local_settings.auto_detect_note', '默认自动识别（推荐）')}
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-2 shrink-0">
                                            <button
                                                type="button"
                                                disabled={!config}
                                                onClick={() => chooseExecutable(field)}
                                                className="rounded-lg px-2.5 py-1 text-xs font-medium text-blue-600 hover:bg-blue-50 dark:text-blue-400 dark:hover:bg-blue-900/30 transition-colors disabled:opacity-50"
                                            >
                                                {t('local_settings.choose_executable', '自定义路径…')}
                                            </button>
                                            {isCustom && (
                                                <button
                                                    type="button"
                                                    onClick={() => updateConfig({ [field]: undefined })}
                                                    className="rounded-lg px-2.5 py-1 text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 transition-colors"
                                                >
                                                    {t('local_settings.reset_detection', '重置为自动')}
                                                </button>
                                            )}
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    </div>
                </section>
            </div>
        ),
        quota: (
            <div className="space-y-6">
                {/* 1. 卡片与列表展示模型 */}
                <ModelDisplaySettings embedded />

                {/* 2. 定时后台刷新与多端同步 */}
                <section className="rounded-2xl border border-gray-200/80 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/80 divide-y divide-gray-100 dark:divide-slate-800">
                    <div className="p-5 sm:p-6">
                        <div className="flex items-start justify-between gap-4">
                            <div>
                                <div className="flex items-center gap-2 text-sm font-semibold text-gray-800 dark:text-gray-100">
                                    <RefreshCw className="h-4 w-4 text-blue-500" />
                                    <span>{t('local_settings.auto_refresh')}</span>
                                    <span className="px-2 py-0.5 rounded-md text-xs font-medium bg-blue-50 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300">
                                        {t('local_settings.auto_refresh_badge', '推荐开启')}
                                    </span>
                                </div>
                                <p className="mt-1 text-xs leading-relaxed text-gray-500 dark:text-gray-400">
                                    {t('local_settings.auto_refresh_desc')}
                                </p>
                            </div>
                            {renderSwitch(Boolean(config?.auto_refresh), t('local_settings.auto_refresh'), () => updateConfig({ auto_refresh: !config?.auto_refresh }))}
                        </div>
                        <div className="mt-4 flex items-center justify-between gap-3 border-t border-gray-100 pt-3 dark:border-slate-800">
                            <label htmlFor="refresh-interval" className="flex items-center gap-2 text-xs font-medium text-gray-700 dark:text-gray-300">
                                <Clock3 className="h-3.5 w-3.5 text-gray-400" />
                                {t('local_settings.refresh_interval')}
                            </label>
                            <select
                                id="refresh-interval"
                                value={refreshInterval}
                                disabled={!config || !config.auto_refresh}
                                onChange={(event) => updateConfig({ refresh_interval: Number(event.target.value) })}
                                className="select select-sm select-bordered w-32 border-gray-200 bg-white text-gray-700 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200 text-xs rounded-lg"
                            >
                                {refreshOptions.map((minutes) => <option key={minutes} value={minutes}>{t('local_settings.minutes', { count: minutes })}</option>)}
                            </select>
                        </div>
                    </div>

                    <div className="p-5 sm:p-6">
                        <div className="flex items-start justify-between gap-4">
                            <div>
                                <div className="flex items-center gap-2 text-sm font-semibold text-gray-800 dark:text-gray-100">
                                    <Database className="h-4 w-4 text-emerald-500" />
                                    <span>{t('local_settings.auto_sync')}</span>
                                    <span className="px-2 py-0.5 rounded-md text-xs font-medium bg-gray-100 text-gray-600 dark:bg-slate-800 dark:text-gray-300">
                                        {t('local_settings.auto_sync_badge', '可选 · 默认关闭')}
                                    </span>
                                </div>
                                <p className="mt-1 text-xs leading-relaxed text-gray-500 dark:text-gray-400">
                                    {t('local_settings.auto_sync_desc')}
                                </p>
                            </div>
                            {renderSwitch(Boolean(config?.auto_sync), t('local_settings.auto_sync'), () => updateConfig({ auto_sync: !config?.auto_sync }))}
                        </div>
                        <div className="mt-4 flex items-center justify-between gap-3 border-t border-gray-100 pt-3 dark:border-slate-800">
                            <label htmlFor="sync-interval" className="flex items-center gap-2 text-xs font-medium text-gray-700 dark:text-gray-300">
                                <Clock3 className="h-3.5 w-3.5 text-gray-400" />
                                {t('local_settings.sync_interval')}
                            </label>
                            <select
                                id="sync-interval"
                                value={syncInterval}
                                disabled={!config || !config.auto_sync}
                                onChange={(event) => updateConfig({ sync_interval: Number(event.target.value) })}
                                className="select select-sm select-bordered w-32 border-gray-200 bg-white text-gray-700 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-gray-200 text-xs rounded-lg"
                            >
                                {syncOptions.map((minutes) => <option key={minutes} value={minutes}>{t('local_settings.minutes', { count: minutes })}</option>)}
                            </select>
                        </div>
                    </div>
                </section>
            </div>
        ),
        customModels: <CustomModelSettings />,
        autoSwitch: <AutoSwitchSettings />,
    };

    return (
        <div ref={layout} className="settings-layout">
            <aside className="settings-sidebar">
                <h1 className="settings-sidebar-title text-gray-900 dark:text-gray-100">{t('local_settings.title')}</h1>
                <SettingsNavigation selected={section} onSelect={setSection} />
            </aside>
            <div ref={contentScroller} className="settings-content">
                {error && <div role="alert" className="settings-panel mb-5 rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/30 dark:text-red-300">
                    <p>{t('settings_sections.config_failed', { error })}</p>
                    <button type="button" disabled={loading} onClick={() => void loadConfig()} className="mt-3 rounded-lg border border-current px-3 py-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-50">{t(loading ? 'auto_switch.loading' : 'auto_switch.retry')}</button>
                </div>}
                {SETTINGS_SECTIONS.map(({ id, title, description }) => (
                    <section key={id} id={`settings-panel-${id}`} role="tabpanel" aria-labelledby={`settings-tab-${id}`} tabIndex={0} hidden={section !== id} className={section === id ? 'settings-panel' : 'settings-panel hidden'}>
                        <header className="settings-section-header">
                            <h2 className="text-xl font-bold tracking-tight text-gray-900 dark:text-gray-100">{t(title)}</h2>
                            <p className="mt-1 text-xs leading-relaxed text-gray-500 dark:text-gray-400">{t(description)}</p>
                        </header>
                        {content[id]}
                    </section>
                ))}
            </div>
        </div>
    );
}

export default Settings;
