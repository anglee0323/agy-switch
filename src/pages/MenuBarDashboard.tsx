import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ArrowLeft, ArrowLeftRight, Ban, BarChart3, CheckCircle2, ChevronLeft, ChevronRight, Clock3, ExternalLink, Loader2, LogOut, RefreshCw, Settings, Users } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { request } from '../utils/request';
import { isTauri } from '../utils/env';
import { useConfigStore } from '../stores/useConfigStore';
import { aggregateMenuBar, menuBarAccount, quotaDisplay, type MenuBarSnapshot, type QuotaReason, type QuotaWindow } from '../utils/menuBarOverview';
import { MenuBarSwitchDetails, useMenuBarSwitchStatus } from '../components/menubar/LowQuotaStatus';
import '../components/menubar/MenuBarDashboard.css';
import { DEFAULT_MENU_BAR_PREFERENCES, menuBarResetTimeDisplay, type MenuBarPreferences } from '../types/config';
import logo from '../assets/logo.png';
import { quotaTone } from '../utils/menuBarOverview';

interface Appearance { platform?: string; native_material: boolean; reduced_transparency: boolean; high_contrast: boolean; material_kind?: string }
interface Usage { today: { input_tokens: number; output_tokens: number; cached_tokens: number; total_tokens: number; request_count: number }; estimated_usd: number | null; unpriced_models: number; pricing_stale: boolean; incomplete: boolean }
function UsageOverview({ usage, zh }: { usage: Usage | null; zh: boolean }) {
  const values = usage ? [usage.today.input_tokens, usage.today.output_tokens, usage.today.cached_tokens] : [0, 0, 0];
  const sum = values.reduce((total, value) => total + value, 0);
  const colors = ['#52a3d8', '#eabf53', '#62b69d'];
  let offset = 0;
  const amount = usage?.estimated_usd;
  const cost = amount == null ? usage ? zh ? '未计价' : 'Unpriced' : '—' : '$ ' + amount.toFixed(amount > 0 && amount < 0.01 ? 4 : 2);
  return <section className="mb-usage-overview" aria-label={zh ? '今日本机用量' : 'Today’s local usage'}>
    <div className="mb-overview-heading"><h2>{zh ? '今日用量' : 'Today’s usage'}</h2><span>{usage?.incomplete ? zh ? '统计不完整' : 'Partial records' : zh ? '本机' : 'Local'}</span></div>
    <div className="mb-usage-summary">
      <div className="mb-usage-chart"><div className="mb-usage-ring">
        <svg viewBox="0 0 88 88" role="img" aria-label={zh ? 'Token 构成' : 'Token composition'}><circle cx="44" cy="44" r="38" className="mb-usage-ring-track" strokeWidth="6" fill="none" />{sum > 0 && values.map((value, index) => { const fraction = value / sum * 100; const start = offset; offset += fraction; return value > 0 && <circle key={index} cx="44" cy="44" r="38" pathLength="100" stroke={colors[index]} strokeWidth="6" fill="none" strokeDasharray={fraction + ' ' + (100 - fraction)} strokeDashoffset={-start} transform="rotate(-90 44 44)" />; })}</svg>
        <div><strong>{usage ? tokens(usage.today.total_tokens) : '—'}</strong><small>tokens</small></div>
      </div></div>
      <div className="mb-usage-metrics"><div className="mb-usage-cost"><span>{zh ? 'API 费用估算' : 'API estimate'}</span><strong>{cost}</strong></div>
        {values.map((value, index) => <div className="mb-usage-type" key={index}><span><i aria-hidden="true" style={{ backgroundColor: colors[index] }} />{(zh ? ['输入', '输出', '缓存'] : ['Input', 'Output', 'Cached'])[index]}</span><strong>{usage ? tokens(value) : '—'}</strong></div>)}
        <div className="mb-usage-meta"><small>{usage ? usage.today.request_count + (zh ? ' 次请求' : ' requests') : '—'}</small><small>{!usage ? zh ? '统计暂不可用' : 'Usage unavailable' : usage.unpriced_models > 0 ? zh ? '部分未计价' : 'Partly unpriced' : usage.pricing_stale && usage.today.total_tokens > 0 ? zh ? '缓存价格' : 'Cached prices' : ''}</small></div>
      </div>
    </div>
  </section>;
}
const tokens = (value: number) => value >= 1e6 ? (value / 1e6).toFixed(1) + 'M' : value >= 1e3 ? (value / 1e3).toFixed(1) + 'K' : String(value);
function Meter({ value, label, preferences, disabled }: { value: number | null; label: string; preferences: MenuBarPreferences; disabled?: boolean }) {
  if (value === null) return <div className="mb-meter unknown" role="img" aria-label={label + ': —'} />;
  return <div className={'mb-meter ' + quotaTone(value, preferences) + (disabled ? ' disabled-quota' : '')} role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value ?? undefined} aria-valuetext={quotaDisplay(value)}>
    <span style={{ width: String(value ?? 0) + '%' }} />
  </div>;
}
export default function MenuBarDashboard() {
  const { i18n, t } = useTranslation();
  const zh = i18n.language.startsWith('zh');
  const config = useConfigStore(state => state.config);
  const lowQuota = useMenuBarSwitchStatus();
  const [snapshot, setSnapshot] = useState<MenuBarSnapshot | null>(null);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [appearance, setAppearance] = useState<Appearance | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [switching, setSwitching] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [now, setNow] = useState(Date.now());
  const [page, setPage] = useState(0);
  const [capacity, setCapacity] = useState(3);
  const [detail, setDetail] = useState<'switch' | null>(null);
  const content = useRef<HTMLDivElement>(null);
  const generation = useRef(0);
  const operation = useRef(false);
  const mounted = useRef(true);
  const preferences = useMemo(() => ({ ...DEFAULT_MENU_BAR_PREFERENCES, ...config?.menu_bar }), [config?.menu_bar]);
  const resetTimeDisplay = menuBarResetTimeDisplay(preferences);
  const scope = preferences.quota_scope;
  const periods: QuotaWindow[] = [...(preferences.show_session ? ['5h' as const] : []), ...(preferences.show_weekly ? ['weekly' as const] : [])];
  const families = preferences.display_scope === 'all' ? ['gemini', 'other'] as const : [preferences.display_scope];
  const scopeName = scope === 'gemini' ? zh ? 'Gemini 系列' : 'Gemini' : scope === 'other' ? zh ? 'Claude 和 GPT 系列' : 'Claude & GPT' : zh ? 'Gemini 与 Claude/GPT' : 'Gemini / Claude & GPT';
  const windowName = (window: QuotaWindow) => window === '5h' ? zh ? '5 小时' : '5 hours' : zh ? '每周' : 'Weekly';
  const reasonName = (reason: QuotaReason | null) => reason ? ({
    disabled: zh ? '禁用' : 'Disabled', blocked: zh ? '待验证' : 'Verification required', forbidden: zh ? '访问受限' : 'Access denied', unreadable: zh ? '读取失败' : 'Unreadable', stale: zh ? '待刷新' : 'Refresh needed', protected: zh ? '额度保护中' : 'Protected', unknown: zh ? '未报告' : 'Not reported', expired: zh ? '已到重置时间' : 'Reset due', conflict: zh ? '数据冲突' : 'Conflicting data',
  })[reason] : '';
  const resetLabel = (reset: string) => {
    const diff = Date.parse(reset) - now;
    if (!Number.isFinite(diff)) return '—';
    if (diff <= 0) return zh ? '请刷新' : 'Refresh needed';
    const minutes = Math.ceil(diff / 60000);
    const countdown = minutes < 60 ? minutes + 'm' : minutes < 1440 ? Math.floor(minutes / 60) + 'h ' + minutes % 60 + 'm' : Math.floor(minutes / 1440) + 'd ' + Math.floor(minutes % 1440 / 60) + 'h';
    return zh ? countdown + ' 后重置' : 'Reset: ' + countdown;
  };
  const reload = useCallback(async () => {
    if (!isTauri()) { setLoading(false); return; }
    const id = ++generation.current;
    try {
      const next = await request<MenuBarSnapshot>('get_menu_bar_snapshot');
      if (mounted.current && id === generation.current) { setSnapshot(next); setNow(Date.now()); setError(''); }
    } catch {
      if (mounted.current && id === generation.current) {
        setSnapshot(null);
        setError(i18n.language.startsWith('zh') ? '账号读取失败，请重试' : 'Could not read accounts. Retry.');
      }
    } finally { if (mounted.current && id === generation.current) setLoading(false); }
  }, [i18n]);
  const readAppearance = useCallback(async () => {
    try { const next = await request<Appearance>('get_menu_bar_appearance'); if (mounted.current) setAppearance(next); }
    catch { if (mounted.current) setAppearance(null); }
  }, []);
  const readUsage = useCallback(async () => {
    try { const next = await request<Usage>('get_menu_bar_usage'); if (mounted.current) setUsage(next); }
    catch { if (mounted.current) setUsage(null); }
  }, []);
  useEffect(() => {
    mounted.current = true;
    document.documentElement.classList.add('panel-window'); document.body.classList.add('panel-window');
    void reload(); void readUsage();
    if (!isTauri()) return;
    void readAppearance();
    const subscriptions = [
      listen('menubar://opened', () => { void reload(); void readUsage(); void readAppearance(); }),
      ...['menubar://data-updated', 'tray://account-switched', 'accounts://refreshed'].map(event => listen(event, () => void reload())),
      listen<Appearance>('menubar://appearance', event => setAppearance(event.payload)),
    ];
    return () => { mounted.current = false; generation.current++; void Promise.all(subscriptions).then(stops => stops.forEach(stop => stop())); };
  }, [reload, readUsage, readAppearance]);
  useEffect(() => {
    const timer = window.setInterval(() => { if (document.visibilityState !== 'hidden') setNow(Date.now()); }, 30000);
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); if (detail) setDetail(null); else if (isTauri()) void request('hide_menu_bar_dashboard'); }
    };
    window.addEventListener('keydown', key);
    return () => { clearInterval(timer); window.removeEventListener('keydown', key); };
  }, [detail]);
  useLayoutEffect(() => {
    if (!content.current) return;
    const element = content.current;
    const measure = () => { const accountArea = element.querySelector('.mb-accounts');
      if (accountArea) { const row = accountArea.querySelector('.mb-account-row'); const rowHeight = row ? row.getBoundingClientRect().height + parseFloat(getComputedStyle(row).marginBottom) : 86; setCapacity(Math.max(1, Math.floor((accountArea.clientHeight + 6) / rowHeight))); } };
    const observer = new ResizeObserver(measure); observer.observe(element); measure();
    return () => observer.disconnect();
  }, [detail, loading, snapshot?.accounts.length]);
  const accounts = useMemo(() => (snapshot?.accounts || []).map(account => menuBarAccount(account, now, config?.refresh_interval)).filter(view => !preferences.hide_unavailable || view.switchable), [snapshot, now, config?.refresh_interval, preferences.hide_unavailable]);
  const threshold = lowQuota.config?.reserve_percentage ?? config?.quota_protection.threshold_percentage ?? 10;
  const aggregate = (window: QuotaWindow) => aggregateMenuBar(accounts, scope, window, threshold);
  const pageCount = Math.max(1, Math.ceil(accounts.length / capacity));
  const visiblePage = Math.min(page, pageCount - 1);
  const visible = accounts.slice(visiblePage * capacity, (visiblePage + 1) * capacity);
  const busy = refreshing || Boolean(switching) || lowQuota.busy || lowQuota.status?.phase === 'switching';
  const openPage = (page: string) => { if (isTauri()) void request('open_app_page', { page }); };
  const refresh = async () => {
    if (operation.current || busy) return;
    operation.current = true; setRefreshing(true); setError('');
    try {
      const result = await request<{ failed: number }>('refresh_all_quotas');
      await reload(); void readUsage();
      if (mounted.current && result.failed) setError(zh ? String(result.failed) + ' 个账号刷新失败，已保留缓存' : String(result.failed) + ' accounts failed to refresh. Cached data retained.');
    } catch { if (mounted.current) setError(zh ? '刷新失败，请重试' : 'Refresh failed. Retry.'); }
    finally { operation.current = false; if (mounted.current) setRefreshing(false); }
  };
  const switchAccount = async (id: string) => {
    const target = accounts.find(view => view.account.id === id);
    if (operation.current || busy || !target?.switchable || (snapshot?.current_identity_source === 'running_app' && id === snapshot.current_account_id)) return;
    operation.current = true; setSwitching(id); setError('');
    try {
      await request('switch_account', { accountId: id }); await reload();
    } catch { await reload(); if (mounted.current) setError(zh ? '切换失败，请在 App 中检查账号状态' : 'Switch failed. Check this account in the app.'); }
    finally { operation.current = false; if (mounted.current) setSwitching(null); }
  };
  const pager = (index: number, total: number, update: (page: number) => void) => <nav className="mb-pagination" aria-label={zh ? '分页' : 'Pagination'}>
    <button aria-label={zh ? '上一页' : 'Previous page'} disabled={index === 0} onClick={() => update(index - 1)}><ChevronLeft size={13} /></button><span>{index + 1} / {total}</span><button aria-label={zh ? '下一页' : 'Next page'} disabled={index + 1 >= total} onClick={() => update(index + 1)}><ChevronRight size={13} /></button>
  </nav>;
  return <div className={'menubar-app ' + (appearance?.native_material ? 'native-material' : 'opaque-material') + (appearance?.high_contrast ? ' high-contrast' : '')} data-language={zh ? 'zh' : 'en'} data-platform={appearance?.platform || 'unknown'} data-material={appearance?.material_kind || (appearance?.native_material ? 'vibrancy' : 'opaque')}>
    <header className="mb-header">
      <div className="mb-brand">{preferences.show_icons && <img src={logo} alt="" width="24" height="24" />}<div><span className="mb-eyebrow">AntiGravity Switch</span>{detail && <h1>{zh ? '智能切换' : 'Auto-switch'}</h1>}</div></div>
      <div className="mb-header-actions">{detail && <button aria-label={zh ? '返回总览' : 'Back to overview'} onClick={() => setDetail(null)}><ArrowLeft size={16} /></button>}<button aria-label={zh ? '刷新全部额度' : 'Refresh all quotas'} disabled={busy || loading} onClick={() => void refresh()}><RefreshCw size={16} className={refreshing ? 'animate-spin' : ''} /></button><button aria-label={zh ? '偏好设置' : 'Settings'} onClick={() => openPage('settings')}><Settings size={16} /></button></div>
    </header>
    {!detail && <UsageOverview usage={usage} zh={zh} />}
    {!detail && preferences.show_aggregate && <section className="mb-overview" aria-label={zh ? '聚合额度' : 'Aggregate quotas'}>
      <div className="mb-overview-heading"><h2>{zh ? '剩余额度' : 'Remaining quota'}</h2><span>{scopeName}  {zh ? '平均剩余' : 'Mean remaining'}</span></div>
      {periods.map(window => { const data = aggregate(window); return <div className="mb-aggregate" key={window}>
        <div><span>{windowName(window)}</span><span className="mb-availability">{zh ? '可用 ' : 'Available '}{data.usable}/{data.total}</span><span>{zh ? '剩余' : 'Remaining'} <strong>{quotaDisplay(data.remaining)}</strong></span></div><Meter preferences={preferences} value={data.remaining} label={scopeName + ' ' + windowName(window)} />
      </div>; })}
    </section>}
    {error && <div className="mb-message error" role="alert">{error}{!refreshing && <button onClick={() => void reload()}>{zh ? '重试读取' : 'Retry read'}</button>}</div>}
    {!detail && lowQuota.visible && <button className="mb-switch-banner" aria-label={zh ? '查看低额度换号详情' : 'View auto-switch details'} onClick={() => setDetail('switch')}><ArrowLeftRight size={13} /><span>{lowQuota.readError ? t('auto_switch.status_failed') : t('auto_switch.reasons.' + (lowQuota.status?.reason || 'checking'), { defaultValue: t('auto_switch.reasons.state_unavailable') })}</span><ChevronRight size={12} /></button>}
    <div className="mb-content" ref={content}>
      {detail === 'switch' ? <MenuBarSwitchDetails state={lowQuota} openSettings={() => openPage('settings')} /> : <>
        <div className="mb-account-heading"><strong>{zh ? '账号列表' : 'Accounts'}</strong><span>{zh ? accounts.length + ' 个账号' : accounts.length + ' accounts'}{snapshot && (snapshot.current_identity_source === 'unavailable' || (snapshot.current_identity_source === 'running_app' && !snapshot.current_account_id)) && <span role="status">{'  ' + t('tray.identity_unavailable')}</span>}</span></div>
        <div className="mb-accounts">{loading ? <div className="mb-empty"><Loader2 size={18} className="animate-spin" />{zh ? '正在读取' : 'Loading'}</div> : !accounts.length ? <div className="mb-empty">{error ? zh ? '暂无可读取的数据' : 'Data unavailable' : zh ? '添加账号后显示额度' : 'Add accounts to see quotas'}<button onClick={() => openPage('accounts')}>{zh ? '管理账号' : 'Manage accounts'}</button></div> : visible.map(view => {
          const account = view.account; const selected = account.id === snapshot?.current_account_id;
          const current = selected && snapshot?.current_identity_source === 'running_app';
          const label = account.custom_label || account.name || account.email.split('@')[0];
          return <article className={'mb-account-row ' + (families.length === 2 ? 'has-families ' : '') + (current ? 'current ' : selected ? 'selected ' : '') + (account.disabled ? 'disabled ' : '') + (resetTimeDisplay === 'hover' ? 'hover-resets' : resetTimeDisplay === 'always' ? 'always-resets' : '')} key={account.id}>
            <div className="mb-account-identity"><div className="mb-account-label"><span>{preferences.label_style === 'label_then_email' && account.custom_label ? account.custom_label : account.email}</span>{families.length === 1 && <small>{preferences.label_style === 'email_only' ? '' : preferences.label_style === 'label_then_email' && account.custom_label ? account.email : account.custom_label ? account.custom_label : ''}</small>}</div><button className="mb-account-switch" aria-label={(zh ? '切换到 ' : 'Switch to ') + label} title={account.disabled ? zh ? '禁用' : 'Disabled' : selected ? snapshot?.current_identity_source === 'running_app' ? zh ? '运行中的 App 已确认' : 'Verified running app' : zh ? 'Tools 保存的账号' : 'Saved Tools account' : zh ? '切换并重新打开 App' : 'Switch and reopen app'} disabled={current || busy || !view.switchable || lowQuota.readError} onClick={() => void switchAccount(account.id)}>{switching === account.id ? <Loader2 size={12} className="animate-spin" /> : account.disabled ? <Ban size={12} /> : current ? <CheckCircle2 size={12} /> : <ArrowLeftRight size={12} />}{account.disabled ? zh ? '禁用' : 'Disabled' : current ? t('tray.current') : selected ? t('tray.saved') : zh ? '切换' : 'Switch'}</button></div>
            <>{families.length === 2 && <div className="mb-account-families"><span>Gemini</span><span>Claude / GPT</span></div>}<div className="mb-account-quotas">{periods.map(window => <div className="mb-account-window" key={window}>{families.map(family => { const quota = view.windows[window][family]; const remaining = account.disabled ? 0 : quota.remaining; const reset = account.disabled ? reasonName('disabled') : quota.resets.length > 1 ? zh ? '多组' : 'Multiple' : quota.resets.length ? resetLabel(quota.resets[0]).replace(zh ? ' 后重置' : 'Reset: ', '') : reasonName(quota.reason) || (zh ? '未报告' : 'Unknown'); return <div className={'mb-mini ' + family} key={family}><span className="mb-period" title={(family === 'gemini' ? 'Gemini' : 'Claude / GPT') + ' · ' + windowName(window)}>{windowName(window)}</span><Meter preferences={preferences} disabled={account.disabled} value={remaining} label={label + ' ' + (family === 'gemini' ? 'Gemini' : 'Claude / GPT') + ' ' + windowName(window)} />{resetTimeDisplay !== 'hidden' && <span className="mb-reset-label"><Clock3 size={9} /><span>{reset}</span></span>}<strong>{quotaDisplay(remaining)}</strong></div>; })}</div>)}</div></>
          </article>;
        })}</div>
        {pageCount > 1 && pager(visiblePage, pageCount, setPage)}
      </>}
    </div>
    <footer className="mb-footer">
      <div className="mb-footer-actions"><button onClick={() => openPage('dashboard')}>{preferences.show_icons && <BarChart3 size={13} />}{zh ? '用量看板' : 'Usage dashboard'}</button><button onClick={() => openPage('accounts')}>{preferences.show_icons && <Users size={13} />}{zh ? '管理账号' : 'Accounts'}</button><button aria-label="GitHub" onClick={() => void request('open_project_page')}>{preferences.show_icons ? <ExternalLink size={14} /> : 'GitHub'}</button><button aria-label={zh ? '退出应用' : 'Quit'} onClick={() => void request('quit_app')}>{preferences.show_icons ? <LogOut size={14} /> : zh ? '退出' : 'Quit'}</button></div>
    </footer>
  </div>;
}
