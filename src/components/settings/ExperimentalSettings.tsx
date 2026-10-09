import { useEffect, useRef, useState } from 'react';
import { Languages, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { request } from '../../utils/request';

interface AppStatus {
    available: boolean;
    version?: string;
    translation_enabled: boolean;
    translated: number;
    state: string;
}

export default function ExperimentalSettings({ active }: { active: boolean }) {
    const { t } = useTranslation();
    const [status, setStatus] = useState<AppStatus>();
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState('');
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
    return <div className="space-y-6">
        <div className="rounded-xl bg-blue-50 p-4 text-sm text-blue-800 dark:bg-blue-950/40 dark:text-blue-200">{t('app_experiments.scope')}</div>
        <div className="flex items-center justify-between gap-3 text-xs text-gray-500">
            <span>{status?.available ? t('app_experiments.connected', { version: status.version }) : t(`app_experiments.states.${status?.state || 'loading'}`, { defaultValue: t('app_experiments.states.unavailable') })}</span>
            <button type="button" disabled={busy} onClick={() => void reload(true)} className="flex items-center gap-1 rounded-lg px-3 py-2 hover:bg-gray-100 dark:hover:bg-slate-800"><RefreshCw className="h-3.5 w-3.5" />{t('app_experiments.reload')}</button>
        </div>
        {error && <div role="alert" className="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:bg-red-950/30">{t('app_experiments.failed', { error })}</div>}
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
