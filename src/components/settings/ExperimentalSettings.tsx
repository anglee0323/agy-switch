import { useEffect, useRef, useState } from 'react';
import { FlaskConical, Languages, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { request } from '../../utils/request';

interface AppStatus {
    available: boolean;
    version?: string;
    translation_enabled: boolean;
    translated: number;
    state: string;
    recommended?: boolean;
    settings?: Record<string, number | boolean | { allow: string[]; ask: string[]; deny: string[] }>;
    native: { keepComputerAwake?: boolean; runInBackground?: boolean };
}

export default function ExperimentalSettings({ active }: { active: boolean }) {
    const { t } = useTranslation();
    const [status, setStatus] = useState<AppStatus>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState('');
    const [rulesOpen, setRulesOpen] = useState(false);
    const [rulesText, setRulesText] = useState('');
    const pending = useRef(false);
    const generation = useRef(0);
    const mounted = useRef(true);
    useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current++; }; }, []);
    const reload = async (clearError = false) => {
        if (pending.current) return;
        const ticket = ++generation.current;
        try {
            const next = await request<AppStatus>('get_app_experiments');
            if (mounted.current && ticket === generation.current) { setStatus(next); if (clearError) setError(''); }
        } catch (e) {
            if (mounted.current && ticket === generation.current) setError(String(e));
        }
    };
    useEffect(() => {
        if (!active) return;
        void reload();
        const timer = window.setInterval(() => void reload(), 5000);
        return () => { window.clearInterval(timer); generation.current++; };
    }, [active]);
    const mutate = async (command: string, args: Record<string, unknown>) => {
        if (pending.current) return;
        pending.current = true; generation.current++; setBusy(true); setError('');
        try {
            await request(command, args);
        } catch (e) {
            if (mounted.current) setError(String(e) === 'app_translation_restore_pending' ? t('app_experiments.restore_pending') : String(e));
        } finally {
            pending.current = false;
            if (mounted.current) { setBusy(false); await reload(); }
        }
    };
    const checkbox = (key: 'keepComputerAwake' | 'runInBackground') => <label className="flex items-center justify-between gap-4 rounded-xl border border-gray-200 p-4 dark:border-slate-700">
        <span className="text-sm">{t(`app_experiments.${key}`)}</span>
        <input type="checkbox" className="toggle toggle-sm toggle-primary" checked={Boolean(status?.native[key])} disabled={busy || !status?.available}
            onChange={e => void mutate('set_app_native_preferences', { patch: { [key]: e.target.checked } })} />
    </label>;
    const fields: [string, [number, string][]][] = [
        ['permissionPreset', [[3, 'turbo'], [1, 'default'], [2, 'review']]],
        ['artifactReviewMode', [[2, 'proceed'], [3, 'model_decides'], [1, 'review']]],
        ['nonWorkspaceFileAccessPolicy', [[1, 'allow'], [2, 'ask'], [3, 'deny']]],
        ['internetAccessPolicy', [[1, 'allow'], [2, 'ask'], [3, 'deny']]],
        ['browserJsExecutionPolicy', [[4, 'proceed'], [2, 'ask'], [1, 'disabled']]],
        ['queuedMessageDeliveryStrategy', [[1, 'immediately'], [2, 'queue']]],
        ['conversationWidth', [[3, 'wide'], [1, 'default'], [2, 'narrow']]],
    ];
    const saveRules = () => {
        try {
            const value = JSON.parse(rulesText);
            if (!value || typeof value !== 'object' || Object.keys(value).length !== 3 || !['allow', 'ask', 'deny'].every(key => Array.isArray(value[key]) && value[key].every((item: unknown) => typeof item === 'string'))) throw new Error(t('app_experiments.rules_invalid'));
            void mutate('set_app_shared_preferences', { patch: { globalPermissionGrants: value } });
        } catch { setError(t('app_experiments.rules_invalid')); }
    };
    return <div className="space-y-6">
        <div className="rounded-xl bg-blue-50 p-4 text-sm text-blue-800 dark:bg-blue-950/40 dark:text-blue-200">{t('app_experiments.scope')}</div>
        <div className="flex items-center justify-between gap-3 text-xs text-gray-500">
            <span>{status?.available ? t('app_experiments.connected', { version: status.version }) : t(`app_experiments.states.${status?.state || 'loading'}`, { defaultValue: t('app_experiments.states.unavailable') })}</span>
            <button type="button" disabled={busy} onClick={() => void reload(true)} className="flex items-center gap-1 rounded-lg px-3 py-2 hover:bg-gray-100 dark:hover:bg-slate-800"><RefreshCw className="h-3.5 w-3.5" />{t('app_experiments.reload')}</button>
        </div>
        {error && <div role="alert" className="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:bg-red-950/30">{t('app_experiments.failed', { error })}</div>}
        <section className="space-y-4 rounded-2xl border border-gray-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-900">
            <h3 className="flex items-center gap-2 text-base font-semibold"><FlaskConical className="h-5 w-5 text-blue-500" />{t('app_experiments.preset_title')}</h3>
            <p className="text-xs leading-relaxed text-gray-500">{t('app_experiments.preset_description')}</p>
            <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="flex gap-1 rounded-xl bg-gray-100 p-1 dark:bg-slate-800">
                    {[true, false].map(recommended => <button key={String(recommended)} type="button" disabled={busy || !status?.available} aria-pressed={Boolean(status?.recommended) === recommended}
                        onClick={() => void mutate('set_app_preset', { recommended })}
                        className={`rounded-lg px-4 py-2 text-xs disabled:opacity-50 ${Boolean(status?.recommended) === recommended ? 'bg-white text-blue-600 shadow-sm dark:bg-slate-700' : 'text-gray-500'}`}>
                        {t(recommended ? 'app_experiments.recommended' : 'app_experiments.custom')}
                    </button>)}
                </div>
                {status?.available && <span className="text-xs text-gray-500">{t(status.recommended ? 'app_experiments.recommended_applied' : 'app_experiments.customized')}</span>}
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
                {fields.map(([key, options]) => <label key={key} className="space-y-2 rounded-xl border border-gray-200 p-4 dark:border-slate-700">
                    <span className="block text-sm">{t(`app_experiments.fields.${key}`)}</span>
                    <select aria-label={t(`app_experiments.fields.${key}`)} className="select select-sm select-bordered w-full text-xs" disabled={busy || !status?.available} value={String(status?.settings?.[key] ?? '')}
                        onChange={e => void mutate('set_app_shared_preferences', { patch: { [key]: Number(e.target.value) } })}>
                        {!options.some(([value]) => value === status?.settings?.[key]) && <option value="">{t('app_experiments.unknown')}</option>}
                        {options.map(([value, label]) => <option key={value} value={value}>{t(`app_experiments.options.${label}`)}</option>)}
                    </select>
                </label>)}
            </div>
            {(['verboseAgentChat', 'useAiCredits'] as const).map(key => <label key={key} className="flex items-center justify-between gap-4 rounded-xl border border-gray-200 p-4 dark:border-slate-700">
                <span className="text-sm">{t(`app_experiments.fields.${key}`)}</span>
                <input type="checkbox" className="toggle toggle-sm toggle-primary" checked={Boolean(status?.settings?.[key])} disabled={busy || !status?.available}
                    onChange={e => void mutate('set_app_shared_preferences', { patch: { [key]: e.target.checked } })} />
            </label>)}
            {checkbox('keepComputerAwake')}
            {checkbox('runInBackground')}
            <p className="text-xs leading-relaxed text-gray-500">{t('app_experiments.permission_note')}</p>
            <button type="button" disabled={busy || !status?.available} onClick={() => { if (!rulesOpen) setRulesText(JSON.stringify(status?.settings?.globalPermissionGrants ?? { allow: [], ask: [], deny: [] }, null, 2)); setRulesOpen(!rulesOpen); }}
                className="rounded-lg border border-gray-200 px-3 py-2 text-xs dark:border-slate-700">{t('app_experiments.rules_title')}</button>
            {rulesOpen && <div className="space-y-3">
                <p className="text-xs leading-relaxed text-gray-500">{t('app_experiments.rules_description')}</p>
                <textarea aria-label={t('app_experiments.rules_title')} rows={12} disabled={busy} value={rulesText} onChange={e => setRulesText(e.target.value)} className="textarea textarea-bordered w-full font-mono text-xs" spellCheck={false} />
                <button type="button" disabled={busy || !status?.available} onClick={saveRules} className="rounded-lg bg-blue-600 px-4 py-2 text-xs text-white disabled:opacity-50">{t('app_experiments.save_rules')}</button>
            </div>}
        </section>
        <section className="space-y-4 rounded-2xl border border-gray-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-900">
            <div className="flex items-center justify-between gap-4">
                <h3 className="flex items-center gap-2 text-base font-semibold"><Languages className="h-5 w-5 text-blue-500" />{t('app_experiments.translation_title')}</h3>
                <button type="button" role="switch" aria-label={t('app_experiments.translation_title')} aria-checked={Boolean(status?.translation_enabled)}
                    disabled={busy || (!status?.available && !status?.translation_enabled)} onClick={() => void mutate('set_app_translation', { enabled: !status?.translation_enabled })}
                    className={`relative h-6 w-11 shrink-0 rounded-full transition-colors focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-50 ${status?.translation_enabled ? 'bg-blue-600' : 'bg-gray-300 dark:bg-slate-600'}`}>
                    <span className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-transform ${status?.translation_enabled ? 'left-0.5 translate-x-5' : 'left-0.5'}`} />
                </button>
            </div>
            <p className="text-xs leading-relaxed text-gray-500">{t('app_experiments.translation_description')}</p>
            {status?.translation_enabled && <p className="text-xs text-blue-600 dark:text-blue-300">{t('app_experiments.translated', { count: status.translated })}</p>}
        </section>
    </div>;
}
