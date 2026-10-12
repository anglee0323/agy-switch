import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import type { AppConfig } from '../../types/config';

type Network = NonNullable<AppConfig['network']>;

function validAddress(value: string) {
    try {
        const address = new URL(value);
        return ['http:', 'https:', 'socks5:', 'socks5h:'].includes(address.protocol)
            && Boolean(address.hostname) && !address.username && !address.password;
    } catch { return false; }
}

export default function NetworkSettings() {
    const { t } = useTranslation();
    const { config, saveConfig } = useConfigStore();
    const [mode, setMode] = useState<Network['proxy_mode']>('system');
    const [url, setUrl] = useState('');
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState('');
    const initialized = useRef(false);
    const live = useRef(true);
    const revision = useRef(0);
    const draft = useRef<Network>({ proxy_mode: 'system', proxy_url: '' });
    const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
    const queue = useRef<Promise<void>>(Promise.resolve());
    useEffect(() => {
        if (!config || initialized.current) return;
        initialized.current = true;
        draft.current = config.network || { proxy_mode: 'system', proxy_url: '' };
        setMode(draft.current.proxy_mode);
        setUrl(draft.current.proxy_url);
    }, [config]);
    useEffect(() => {
        live.current = true;
        return () => { live.current = false; if (timer.current) clearTimeout(timer.current); };
    }, []);
    const save = async () => {
        if (timer.current) clearTimeout(timer.current);
        const next = { ...draft.current, proxy_url: draft.current.proxy_mode === 'manual' ? draft.current.proxy_url.trim() : '' };
        if (next.proxy_mode === 'manual' && !validAddress(next.proxy_url)) {
            setError(t('network.invalid_address'));
            return;
        }
        const edit = revision.current;
        // Read current general preferences when this ordered write starts.
        // Older responses never replace a newer proxy draft.
        setSaving(true);
        setError('');
        const write = queue.current.then(async () => {
            const latest = useConfigStore.getState().config;
            if (!latest) return;
            if (latest.network?.proxy_mode === next.proxy_mode && latest.network?.proxy_url === next.proxy_url) return;
            await saveConfig({ ...latest, network: next }, true);
        });
        queue.current = write.catch(() => {});
        try {
            await write;
        } catch { if (live.current && edit === revision.current) setError(t('network.save_failed')); }
        finally { if (live.current && edit === revision.current) setSaving(false); }
    };
    const edit = (next: Network, delayed = false) => {
        if (timer.current) clearTimeout(timer.current);
        draft.current = next;
        revision.current++;
        setMode(next.proxy_mode);
        setUrl(next.proxy_url);
        setError('');
        setSaving(false);
        if (next.proxy_mode === 'manual' && !validAddress(next.proxy_url.trim())) return;
        if (delayed) timer.current = setTimeout(() => void save(), 500);
        else void save();
    };
    return <section className="rounded-2xl border border-gray-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-900 space-y-3">
        <h3 className="text-base font-semibold">{t('network.title')}</h3>
        <p className="text-xs text-gray-500 leading-relaxed">{t('network.description')}</p>
        <div className="flex flex-wrap items-center gap-3">
            <select aria-label={t('network.mode')} className="rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-800" value={mode} disabled={!config} onChange={event => edit({ ...draft.current, proxy_mode: event.target.value as typeof mode })}>
                <option value="system">{t('network.system')}</option><option value="manual">{t('network.manual')}</option><option value="direct">{t('network.direct')}</option>
            </select>
            {mode === 'manual' && <input aria-label={t('network.address')} aria-invalid={Boolean(error)} value={url} onChange={event => edit({ ...draft.current, proxy_url: event.target.value }, true)} onBlur={() => void save()} onKeyDown={event => { if (event.key === 'Enter') void save(); }} disabled={!config} placeholder="http://127.0.0.1:7890" className="flex-1 min-w-[220px] rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:border-slate-700 dark:bg-slate-800" />}
            {saving && <span role="status" className="text-xs text-gray-500">{t('network.applying')}</span>}
        </div>
        {mode === 'manual' && <p className="text-xs text-gray-500">{t('network.manual_hint')}</p>}
        {error && <p role="alert" className="text-xs text-red-600">{error}</p>}
    </section>;
}
