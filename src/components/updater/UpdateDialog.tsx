import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useUpdateStore } from '../../stores/useUpdateStore';
import { downloadAndInstallUpdate, UpdateProgress } from '../../services/updaterService';
export default function UpdateDialog() {
    const { t } = useTranslation();
    const { updateInfo, isDialogOpen, setDialogOpen } = useUpdateStore();
    const close = useRef<HTMLButtonElement>(null), view = useRef<HTMLButtonElement>(null);
    const [error, setError] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);
    const [progress, setProgress] = useState<UpdateProgress | null>(null);
    const installing = useRef(false);
    const panel = useRef<HTMLElement>(null);
    const dismiss = () => { if (!installing.current) setDialogOpen(false); };
    const install = async () => {
        if (installing.current) return;
        installing.current = true; setBusy(true); setError(null); setProgress(null);
        try { await downloadAndInstallUpdate(updateInfo!.latest_version, setProgress); }
        catch (failure) { setProgress(null); setError(String(failure).includes('update_open_failed') ? 'updater.open_failed' : String(failure).includes('update_mac_manual_required') ? 'updater.mac_manual_required' : String(failure).includes('update_mac_trust_required') ? 'updater.mac_trust_required' : String(failure).includes('update_restore_failed') ? 'updater.restore_failed' : 'updater.install_failed'); }
        finally { installing.current = false; setBusy(false); }
    };
    useEffect(() => { if (!isDialogOpen) return; const previous = document.activeElement as HTMLElement; setError(null); close.current?.focus(); return () => previous?.focus(); }, [isDialogOpen]);
    useEffect(() => { if (isDialogOpen) (busy ? panel.current : close.current)?.focus(); }, [busy, isDialogOpen]);
    if (!isDialogOpen || !updateInfo) return null;
    return <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/40 p-4" onClick={dismiss}>
        <section ref={panel} tabIndex={-1} aria-busy={busy} role="dialog" aria-modal="true" aria-labelledby="update-dialog-title" className="w-full max-w-md rounded-2xl bg-white p-6 shadow-xl dark:bg-slate-900" onClick={e => e.stopPropagation()} onKeyDown={e => {
            if (e.key === 'Escape') dismiss();
            if (e.key === 'Tab') { e.preventDefault(); (document.activeElement === close.current ? view.current : close.current)?.focus(); }
        }}>
            <h2 id="update-dialog-title" className="text-lg font-semibold">{t('updater.dialog_title')}</h2>
            <p className="my-4 text-sm leading-relaxed text-gray-600 dark:text-gray-300">{t('updater.dialog_desc', { version: updateInfo.latest_version })}</p>
            {error && <p role="alert" className="mb-3 text-xs text-red-600">{t(error)}</p>}
            {progress && <p role="status" className="mb-3 text-sm text-gray-600 dark:text-gray-300">{t(progress.stage === 'installing' ? 'updater.installing' : 'updater.downloading')} {progress.stage === 'downloading' && progress.total ? `${Math.min(100, Math.round(progress.downloaded / progress.total * 100))}%` : ''}</p>}
            <div className="flex justify-end gap-3"><button ref={close} disabled={busy} className="rounded-lg border border-gray-200 px-4 py-2 text-sm dark:border-slate-700" onClick={dismiss}>{t('updater.dismiss')}</button><button ref={view} disabled={busy} className="rounded-lg bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700" onClick={() => void install()}>{t(busy ? 'updater.updating' : 'updater.install_now')}</button></div>
        </section>
    </div>;
}
