import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';
import { showToast } from '../common/ToastContainer';

export default function NetworkSettings() {
    const { t } = useTranslation();
    const { config, saveConfig } = useConfigStore();
    const [mode, setMode] = useState<'system' | 'direct' | 'manual'>('system');
    const [url, setUrl] = useState('');
    const [saving, setSaving] = useState(false);
    useEffect(() => { setMode(config?.network?.proxy_mode || 'system'); setUrl(config?.network?.proxy_url || ''); }, [config?.network?.proxy_mode, config?.network?.proxy_url]);
    const save = async () => {
        const latest = useConfigStore.getState().config;
        if (!latest) return;
        setSaving(true);
        try {
            await saveConfig({ ...latest, network: { proxy_mode: mode, proxy_url: mode === 'manual' ? url.trim() : '' } }, true);
            showToast(t('local_settings.saved'), 'success');
        } catch { showToast(t('network.save_failed'), 'error'); }
        finally { setSaving(false); }
    };
    return <section className="rounded-2xl border border-gray-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-900 space-y-3">
        <h3 className="text-base font-semibold">{t('network.title')}</h3>
        <p className="text-xs text-gray-500 leading-relaxed">{t('network.description')}</p>
        <div className="flex flex-wrap items-center gap-3">
            <select aria-label={t('network.mode')} className="rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm dark:border-slate-700 dark:bg-slate-800" value={mode} disabled={!config || saving} onChange={event => setMode(event.target.value as typeof mode)}>
                <option value="system">{t('network.system')}</option><option value="manual">{t('network.manual')}</option><option value="direct">{t('network.direct')}</option>
            </select>
            {mode === 'manual' && <input aria-label={t('network.address')} value={url} onChange={event => setUrl(event.target.value)} disabled={saving} placeholder="http://127.0.0.1:7890" className="flex-1 min-w-[220px] rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 dark:border-slate-700 dark:bg-slate-800" />}
            <button className="rounded-lg bg-blue-500 px-4 py-2 text-sm font-medium text-white hover:bg-blue-600 disabled:opacity-50" disabled={!config || saving} onClick={save}>{t('common.save')}</button>
        </div>
        {mode === 'manual' && <p className="text-xs text-gray-500">{t('network.manual_hint')}</p>}
    </section>;
}
