import { useEffect, useRef, useState } from 'react';
import { ArrowLeftRight, Check, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Account } from '../../types/account';
import { AutoSwitchConfig, AutoSwitchStatus, AutoSwitchTarget } from '../../types/autoSwitch';
import * as service from '../../services/autoSwitchService';
import { listAccounts, getCurrentAccount } from '../../services/accountService';
import { CandidateAccounts } from './CandidateAccounts';
import { isTauri } from '../../utils/env';

const PRIMARY_BUTTON = 'inline-flex min-h-9 items-center justify-center gap-2 rounded-lg border border-blue-600 bg-blue-600 px-3 py-2 text-sm font-medium text-white transition hover:bg-blue-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:border-slate-200 disabled:bg-slate-100 disabled:text-slate-400 dark:disabled:border-slate-700 dark:disabled:bg-slate-800 dark:disabled:text-slate-500';
const SECONDARY_BUTTON = 'inline-flex min-h-9 items-center justify-center gap-2 rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800';
const FIELD = 'h-10 w-full rounded-lg border border-slate-300 bg-white px-3 pr-8 text-sm text-slate-800 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 disabled:opacity-50 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-100';

function useStatus() {
    const [status, setStatus] = useState<AutoSwitchStatus | null>(null);
    const [error, setError] = useState('');
    useEffect(() => {
        if (!isTauri()) return;
        let live = true; let reading = false;
        const read = async () => {
            if (reading) return; reading = true;
            try { const s = await service.getAutoSwitchStatus(); if (live) { setStatus(s); setError(''); } }
            catch { if (live) setError('status_failed'); }
            finally { reading = false; }
        };
        void read(); const timer = setInterval(read, 3000);
        return () => { live = false; clearInterval(timer); };
    }, []);
    return { status, setStatus, error };
}

function StatusBody({ status, compact = false }: { status: AutoSwitchStatus; compact?: boolean }) {
    const { t } = useTranslation();
    const [busy, setBusy] = useState(false); const [error, setError] = useState('');
    const [guide, setGuide] = useState(false); const close = useRef<HTMLButtonElement>(null);
    const guideTrigger = useRef<HTMLButtonElement>(null);
    useEffect(() => { if (guide) close.current?.focus(); }, [guide]);
    const dismiss = () => { setGuide(false); requestAnimationFrame(() => guideTrigger.current?.focus()); };
    const reason = status.reason || (status.phase === 'monitoring' ? 'monitoring' : 'checking');
    const action = async (cancel: boolean) => {
        setBusy(true); setError('');
        try { if (cancel && status.pending_id) await service.cancelAutoSwitch(status.pending_id); else await service.checkAutoSwitchNow(); }
        catch { setError(t('auto_switch.action_failed')); }
        finally { setBusy(false); }
    };
    return <div className={compact ? 'px-4 py-3' : 'rounded-xl bg-slate-50 p-4 dark:bg-slate-800/60'}>
        <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 space-y-1">
                <p className="text-sm font-medium text-slate-800 dark:text-slate-100" role="status">
                    {status.remaining_percentage !== null && ['pending', 'switching'].includes(status.phase)
                        ? t('auto_switch.remaining', { percent: Math.floor(status.remaining_percentage) }) + t('auto_switch.status_separator') : ''}
                    {t(`auto_switch.reasons.${reason}`, { defaultValue: t('auto_switch.reasons.switch_failed') })}
                </p>
                {status.target_email && <p className="break-all text-xs text-slate-500 dark:text-slate-400">{t('auto_switch.next_account', { email: status.target_email })}</p>}
                {['clients_running', 'closing_clients', 'client_close_failed'].includes(status.reason || '') && <p className="max-w-3xl text-xs leading-relaxed text-slate-500 dark:text-slate-400">{t(`auto_switch.${status.mode}_instructions`)}</p>}
                {status.phase === 'completed' && <p className="text-xs text-slate-500 dark:text-slate-400">{t('auto_switch.manual_continue')}</p>}
            </div>
            <div className="flex shrink-0 flex-wrap gap-2">
                {status.mode === 'stop' && ['clients_running', 'closing_clients', 'client_close_failed'].includes(status.reason || '') && <button ref={guideTrigger} onClick={() => setGuide(true)} className={SECONDARY_BUTTON}>{t('auto_switch.stop_guide')}</button>}
                {status.pending_id && <button disabled={busy || status.phase === 'switching'} onClick={() => action(true)} className={SECONDARY_BUTTON}>{t('auto_switch.cancel')}</button>}
                <button disabled={busy || status.phase === 'switching'} onClick={() => action(false)} className={SECONDARY_BUTTON}><RefreshCw size={14} className={busy ? 'animate-spin' : ''} />{t('auto_switch.check_now')}</button>
            </div>
        </div>
        {error && <p role="alert" className="mt-2 text-xs text-red-600">{error}</p>}
        {guide && <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/40 p-4" onClick={dismiss}>
            <section role="dialog" aria-modal="true" aria-labelledby="auto-switch-guide-title" className="w-full max-w-md rounded-2xl bg-white p-6 shadow-xl dark:bg-slate-900" onClick={e => e.stopPropagation()} onKeyDown={e => {
                if (e.key === 'Escape') { e.preventDefault(); dismiss(); }
                // This dialog has one action; keep keyboard focus inside it.
                if (e.key === 'Tab') { e.preventDefault(); close.current?.focus(); }
            }}>
                <h3 id="auto-switch-guide-title" className="font-semibold">{t('auto_switch.stop_guide')}</h3>
                <ol className="my-4 list-decimal space-y-3 pl-5 text-sm leading-relaxed text-slate-600 dark:text-slate-300">
                    <li>{t('auto_switch.stop_step_1')}</li><li>{t('auto_switch.stop_step_2')}</li><li>{t('auto_switch.stop_step_3')}</li>
                </ol>
                <p className="mb-4 text-xs text-amber-700 dark:text-amber-300">{t('auto_switch.stop_warning')}</p>
                <button ref={close} onClick={dismiss} className={`${PRIMARY_BUTTON} w-full`}>{t('auto_switch.understood')}</button>
            </section>
        </div>}
    </div>;
}

export function AutoSwitchStatusBar() {
    const { t } = useTranslation(); const { status, error } = useStatus();
    if (!status || ['disabled', 'monitoring'].includes(status.phase)) return null;
    return <aside aria-label={t('auto_switch.title')} className="shrink-0 border-b border-amber-200 bg-amber-50/80 dark:border-amber-800/60 dark:bg-amber-950/20">
        {error ? <p className="px-4 py-3 text-sm" role="alert">{t(`auto_switch.${error}`)}</p> : <StatusBody status={status} compact />}
    </aside>;
}

export function AutoSwitchSettings() {
    const { t } = useTranslation();
    const { status, error: statusError } = useStatus();
    const [draft, setDraft] = useState<AutoSwitchConfig | null>(null);
    const [accounts, setAccounts] = useState<Account[]>([]);
    const [currentId, setCurrentId] = useState<string | null>(null);
    const [error, setError] = useState('');
    const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved'>('idle');
    const [busy, setBusy] = useState(false);
    const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const saveQueueRef = useRef<Promise<void>>(Promise.resolve());
    const draftRef = useRef<AutoSwitchConfig | null>(null);
    const draftRevisionRef = useRef(0);
    const liveRef = useRef(true);

    const eligibleAccounts = accounts.filter(a => !a.disabled && !a.validation_blocked && !a.quota?.is_forbidden);

    const isConfigValid = (c: AutoSwitchConfig): boolean => {
        if (!c) return false;
        if (c.reserve_percentage < 1 || c.reserve_percentage > 98) return false;
        if (c.candidate_min_percentage <= c.reserve_percentage || c.candidate_min_percentage > 100) return false;
        if (!Number.isInteger(c.reserve_percentage) || !Number.isInteger(c.candidate_min_percentage)) return false;
        if (c.enabled && !c.candidate_account_ids.length) return false;
        return true;
    };

    const reload = async () => {
        setBusy(true);
        setError('');
        try {
            const [c, a, current] = await Promise.all([service.getAutoSwitchConfig(), listAccounts(), getCurrentAccount()]);
            setDraft(c);
            draftRef.current = c;
            draftRevisionRef.current++;
            setAccounts(a);
            setCurrentId(current?.id || null);
        } catch {
            setError(t('auto_switch.load_failed'));
        } finally {
            setBusy(false);
        }
    };

    useEffect(() => {
        liveRef.current = true;
        void reload();
        return () => {
            liveRef.current = false;
            if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
            if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
        };
    }, []);

    const performSave = async (targetConfig: AutoSwitchConfig) => {
        const eligibleIds = eligibleAccounts.map(a => a.id);
        const filteredCandidateIds = targetConfig.candidate_account_ids.filter(id => eligibleIds.includes(id));
        const finalConfig = { ...targetConfig, candidate_account_ids: filteredCandidateIds };

        if (!isConfigValid(finalConfig)) return;

        const revision = draftRevisionRef.current;
        if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
        setSaveState('saving');
        setError('');
        // Keep backend writes in edit order; an older response must not replace
        // newer edits while the settings panels remain mounted.
        const write = saveQueueRef.current.then(() => service.setAutoSwitchConfig(finalConfig));
        saveQueueRef.current = write.then(() => {}, () => {});
        try {
            const savedConfig = await write;
            if (!liveRef.current || revision !== draftRevisionRef.current) return;
            draftRef.current = savedConfig;
            setDraft(savedConfig);
            setSaveState('saved');
            if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
            saveTimerRef.current = setTimeout(() => {
                setSaveState('idle');
            }, 2500);
        } catch (e) {
            if (!liveRef.current || revision !== draftRevisionRef.current) return;
            setError(t('auto_switch.save_failed', { error: String(e) }));
            setSaveState('idle');
        }
    };

    const patchAndSave = (patch: Partial<AutoSwitchConfig>) => {
        if (!draftRef.current) return;
        if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
        let next = { ...draftRef.current, ...patch };
        if (patch.enabled && (!next.candidate_account_ids || next.candidate_account_ids.length === 0)) {
            next.candidate_account_ids = eligibleAccounts.map(a => a.id);
        }
        setDraft(next);
        draftRef.current = next;
        draftRevisionRef.current++;
        setSaveState('idle');
        if (isConfigValid(next)) {
            void performSave(next);
        }
    };

    const handleNumberChange = (field: 'reserve_percentage' | 'candidate_min_percentage', val: number) => {
        if (!draftRef.current) return;
        const next = { ...draftRef.current, [field]: val };
        setDraft(next);
        draftRef.current = next;
        draftRevisionRef.current++;
        setSaveState('idle');

        if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = setTimeout(() => {
            if (isConfigValid(next)) {
                void performSave(next);
            }
        }, 500);
    };

    const handleNumberBlur = () => {
        if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
        if (draftRef.current && isConfigValid(draftRef.current)) {
            void performSave(draftRef.current);
        }
    };

    const reserveError = draft && (draft.reserve_percentage < 1 || draft.reserve_percentage > 98);
    const candidateMinError = draft && (draft.candidate_min_percentage <= draft.reserve_percentage || draft.candidate_min_percentage > 100);
    const noAccountsError = draft && draft.enabled && draft.candidate_account_ids.length === 0;

    return (
        <div className="space-y-4">
            <section className="rounded-2xl border border-gray-200/80 bg-white p-5 sm:p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/80">
                <div className="flex items-start justify-between gap-3">
                    <div className="flex items-start gap-3">
                        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-50 text-amber-600 dark:bg-amber-400/10 dark:text-amber-300">
                            <ArrowLeftRight className="h-5 w-5" />
                        </span>
                        <div>
                            <div className="flex items-center gap-2.5">
                                <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100">{t('auto_switch.title')}</h3>
                                {saveState === 'saving' && (
                                    <span className="inline-flex items-center gap-1 text-xs text-slate-400">
                                        <RefreshCw className="h-3 w-3 animate-spin" />
                                        {t('auto_switch.saving')}
                                    </span>
                                )}
                                {saveState === 'saved' && (
                                    <span className="inline-flex items-center gap-1 rounded-md bg-emerald-50 px-2 py-0.5 text-xs font-medium text-emerald-600 dark:bg-emerald-950/40 dark:text-emerald-400">
                                        <Check className="h-3.5 w-3.5" />
                                        {t('auto_switch.auto_saved')}
                                    </span>
                                )}
                            </div>
                            <p className="mt-0.5 max-w-3xl text-xs leading-relaxed text-gray-500 dark:text-gray-400">{t('auto_switch.description')}</p>
                        </div>
                    </div>
                </div>

                {error && <p role="alert" className="mt-3 text-sm text-red-600">{error}</p>}

                {!draft ? (
                    <button disabled={busy} onClick={reload} className={`${SECONDARY_BUTTON} mt-4`}>
                        {t(busy ? 'auto_switch.loading' : 'auto_switch.retry')}
                    </button>
                ) : (
                    <>
                        <label className="my-5 flex cursor-pointer items-center gap-3 text-sm font-medium">
                            <input
                                type="checkbox"
                                className="peer sr-only"
                                checked={draft.enabled}
                                disabled={busy}
                                onChange={e => patchAndSave({ enabled: e.target.checked })}
                            />
                            <span
                                aria-hidden="true"
                                className="relative inline-block h-6 w-11 shrink-0 rounded-full bg-slate-300 transition-colors after:absolute after:left-0.5 after:top-0.5 after:h-5 after:w-5 after:rounded-full after:bg-white after:shadow-sm after:transition-transform after:content-[''] peer-checked:bg-blue-600 peer-checked:after:translate-x-5 peer-focus-visible:ring-2 peer-focus-visible:ring-blue-500 peer-focus-visible:ring-offset-2 peer-disabled:opacity-50 dark:bg-slate-600"
                            />
                            {t('auto_switch.enable')}
                        </label>

                        <fieldset>
                            <legend className="mb-2.5 text-sm font-semibold text-slate-700 dark:text-slate-200">{t('auto_switch.mode')}</legend>
                            <div className="grid gap-3 md:grid-cols-2">
                                {(['wait', 'stop'] as const).map(mode => (
                                    <label
                                        key={mode}
                                        className={`flex cursor-pointer items-start gap-3 rounded-xl border p-4 transition-colors ${draft.mode === mode ? 'border-blue-400 bg-blue-50/60 dark:border-blue-500/40 dark:bg-blue-500/10' : 'border-slate-200 hover:border-slate-300 dark:border-slate-700 dark:hover:border-slate-600'}`}
                                    >
                                        <input
                                            type="radio"
                                            name="auto-switch-mode"
                                            checked={draft.mode === mode}
                                            disabled={busy}
                                            onChange={() => patchAndSave({ mode })}
                                            className="mt-0.5 h-4 w-4 shrink-0 accent-blue-600"
                                        />
                                        <span>
                                            <span className="block text-sm font-medium text-slate-900 dark:text-slate-100">{t(`auto_switch.mode_${mode}`)}</span>
                                            <span className="mt-1 block text-xs leading-relaxed text-slate-500 dark:text-slate-400">{t(`auto_switch.${mode}_instructions`)}</span>
                                        </span>
                                    </label>
                                ))}
                            </div>
                        </fieldset>

                        <fieldset className="mt-5">
                            <legend className="mb-2.5 text-sm font-semibold text-slate-700 dark:text-slate-200">{t('auto_switch.strategy_title')}</legend>
                            <div className="grid gap-1 rounded-xl bg-slate-100/70 p-1.5 md:grid-cols-2 dark:bg-slate-800/60">
                                {(['priority', 'round_robin'] as const).map(strategy => (
                                    <label key={strategy} className={`flex cursor-pointer items-start gap-3 rounded-lg p-3 transition-colors ${(draft.strategy || 'priority') === strategy ? 'bg-white shadow-xs dark:bg-slate-700/70' : 'hover:bg-white/50 dark:hover:bg-slate-700/30'}`}>
                                        <input type="radio" name="auto-switch-strategy" checked={(draft.strategy || 'priority') === strategy} disabled={busy} onChange={() => patchAndSave({ strategy })} className="mt-0.5 h-4 w-4 accent-blue-600" />
                                        <span><span className="block text-sm font-medium">{t(`auto_switch.strategy_${strategy}`)}</span><span className="mt-1 block text-xs leading-relaxed text-slate-500 dark:text-slate-400">{t(`auto_switch.strategy_${strategy}_desc`)}</span></span>
                                    </label>
                                ))}
                            </div>
                        </fieldset>
                        <div className="my-5 grid gap-4 sm:grid-cols-2">
                            <div className="space-y-1.5">
                                <label htmlFor="auto-switch-target" className="block text-xs font-semibold text-gray-700 dark:text-gray-300">
                                    {t('auto_switch.target')}
                                </label>
                                <select
                                    id="auto-switch-target"
                                    aria-label={t('auto_switch.target')}
                                    className={FIELD}
                                    disabled={busy}
                                    value={draft.target === 'all' ? 'app' : draft.target}
                                    onChange={e => patchAndSave({ target: e.target.value as AutoSwitchTarget })}
                                >
                                    <option value="app">{t('auto_switch.target_all')}</option>
                                    <option value="app_cli">{t('auto_switch.target_app_cli')}</option>
                                    <option value="ide">{t('auto_switch.target_ide')}</option>
                                    <option value="vscode">{t('auto_switch.target_vscode')}</option>
                                </select>
                                <p className="text-[11px] leading-tight text-slate-500 dark:text-slate-400">
                                    {draft.target === 'ide'
                                        ? t('auto_switch.target_hint_ide')
                                        : draft.target === 'app_cli'
                                        ? t('auto_switch.target_hint_app_cli')
                                        : draft.target === 'vscode'
                                        ? t('auto_switch.target_hint_vscode')
                                        : t('auto_switch.target_hint_all')}
                                </p>
                            </div>

                            <div className="space-y-1.5">
                                <label htmlFor="auto-switch-model" className="block text-xs font-semibold text-gray-700 dark:text-gray-300">
                                    {t('auto_switch.model')}
                                </label>
                                <select
                                    id="auto-switch-model"
                                    aria-label={t('auto_switch.model')}
                                    className={FIELD}
                                    disabled={busy}
                                    value={draft.monitored_model || 'all'}
                                    onChange={e => patchAndSave({ monitored_model: e.target.value })}
                                >
                                    <option value="all">{t('auto_switch.all_models')}</option>
                                    <option value="gemini">{t('auto_switch.model_gemini')}</option>
                                    <option value="claude">{t('auto_switch.model_claude')}</option>
                                </select>
                                <p className="text-[11px] leading-tight text-slate-500 dark:text-slate-400">
                                    {draft.monitored_model === 'gemini'
                                        ? t('auto_switch.model_gemini_hint')
                                        : draft.monitored_model === 'claude'
                                        ? t('auto_switch.model_claude_hint')
                                        : t('auto_switch.model_all_hint')}
                                </p>
                            </div>

                            <div className="space-y-1.5">
                                <label htmlFor="auto-switch-reserve" className="block text-xs font-semibold text-gray-700 dark:text-gray-300">
                                    {t('auto_switch.reserve')}
                                </label>
                                <input
                                    id="auto-switch-reserve"
                                    aria-label={t('auto_switch.reserve')}
                                    className={`${FIELD} ${reserveError ? 'border-red-400 focus:border-red-500' : ''}`}
                                    type="number"
                                    min={1}
                                    max={98}
                                    step={1}
                                    disabled={busy}
                                    value={draft.reserve_percentage}
                                    onChange={e => handleNumberChange('reserve_percentage', Number(e.target.value))}
                                    onBlur={handleNumberBlur}
                                />
                                <p className={`text-[11px] leading-tight ${reserveError ? 'text-red-500 dark:text-red-400' : 'text-slate-500 dark:text-slate-400'}`}>
                                    {reserveError ? t('auto_switch.reserve_range_error') : t('auto_switch.reserve_hint')}
                                </p>
                            </div>

                            <div className="space-y-1.5">
                                <label htmlFor="auto-switch-candidate-min" className="block text-xs font-semibold text-gray-700 dark:text-gray-300">
                                    {t('auto_switch.candidate_min')}
                                </label>
                                <input
                                    id="auto-switch-candidate-min"
                                    aria-label={t('auto_switch.candidate_min')}
                                    className={`${FIELD} ${candidateMinError ? 'border-red-400 focus:border-red-500' : ''}`}
                                    type="number"
                                    min={draft.reserve_percentage + 1}
                                    max={100}
                                    step={1}
                                    disabled={busy}
                                    value={draft.candidate_min_percentage}
                                    onChange={e => handleNumberChange('candidate_min_percentage', Number(e.target.value))}
                                    onBlur={handleNumberBlur}
                                />
                                <p className={`text-[11px] leading-tight ${candidateMinError ? 'text-red-500 dark:text-red-400' : 'text-slate-500 dark:text-slate-400'}`}>
                                    {candidateMinError ? t('auto_switch.candidate_min_error') : t('auto_switch.candidate_hint')}
                                </p>
                            </div>
                        </div>
                    </>
                )}
            </section>

            {draft && (
                <>
                    <section className="rounded-2xl border border-gray-200/80 bg-white p-5 sm:p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/80">
                        <fieldset className="space-y-2">
                            <legend className="mb-2 text-sm font-semibold text-slate-800 dark:text-slate-200">{t('auto_switch.accounts')}</legend>
                            <p className="mb-1 text-xs text-slate-500 dark:text-slate-400">{t('auto_switch.accounts_hint')}</p>
                            {noAccountsError && <p className="mb-2 text-xs font-medium text-red-500 dark:text-red-400">{t('auto_switch.no_candidate_error')}</p>}
                            {!eligibleAccounts.length && <p className="text-xs text-amber-700 dark:text-amber-400">{t('auto_switch.no_accounts')}</p>}
                            <CandidateAccounts accounts={eligibleAccounts} selected={draft.candidate_account_ids} currentId={currentId} disabled={busy} onChange={candidate_account_ids => patchAndSave({ candidate_account_ids })} />
                        </fieldset>
                    </section>

                    <div className="rounded-xl border border-amber-200/80 bg-amber-50/60 p-3.5 text-xs leading-relaxed text-amber-800 dark:border-amber-900/40 dark:bg-amber-950/20 dark:text-amber-300">
                        <p>{t('auto_switch.safety_note')}</p>
                    </div>

                    {statusError && <p role="alert" className="text-xs text-red-600">{t(`auto_switch.${statusError}`)}</p>}
                    {status && status.phase !== 'disabled' && <div className="mt-4"><StatusBody status={status} /></div>}
                </>
            )}
        </div>
    );
}
