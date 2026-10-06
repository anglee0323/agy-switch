export async function settleKeyboardDrag(page: import('@playwright/test').Page) {
    // The sensor defers its key listener; the announcement can precede attachment.
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
}

// Installs an in-memory IPC double before importing the app. No native bridge,
// filesystem, accounts, system preference or external quota request is involved.
export function setupSettingsFixture(options: { theme?: string; language?: string; failLoad?: boolean; failLowQuotaLoad?: boolean; dashboardCards?: string[]; quotaScope?: string; autoRefresh?: boolean } = {}) {
    const w = window as any;
    const callbacks: Record<number, Function> = {}; let callback = 1;
    let general = { language: options.language || 'zh', theme: options.theme || 'light', auto_refresh: options.autoRefresh ?? false, refresh_interval: 15, auto_sync: false, sync_interval: 5,
        dashboard: { cards: options.dashboardCards ?? ['total_tokens', 'input_tokens', 'output_tokens', 'cache_hit_rate', 'api_cost', 'first_text_latency', 'body_speed'], order: [] as string[] },
        desktop: { launch_at_login: false, hide_dock_icon: false, start_minimized: false }, menu_bar: { quota_scope: options.quotaScope || 'all' },
        quota_protection: { enabled: false, threshold_percentage: 10, monitored_models: [] }, pinned_quota_models: { models: [] } };
    const accounts = ['primary', 'backup', 'studio', 'personal'].map((name, index) => ({ id: `fixture-${index}`, email: `${name}@example.invalid`, created_at: 1, last_used: 1,
        token: { access_token: '', refresh_token: '', expires_in: 0, expiry_timestamp: 0, token_type: 'Bearer' },
        quota: { models: [{ name: 'gemini-test', display_name: 'Gemini test model', percentage: 80, reset_time: '2027-01-01T00:00:00Z' }], last_updated: 1800000000 } }));
    let lowQuota = { enabled: false, mode: 'wait', reserve_percentage: 10, candidate_min_percentage: 30, monitored_model: '', candidate_account_ids: [], target: 'app' };
    let status = { phase: 'disabled', reason: null, source_account_id: accounts[0].id, pending_id: null, mode: 'wait', process_state: 'unknown', last_checked: null };
    let desktop = { platform: 'macos', tray_available: true, autostart_supported: true, ...general.desktop };
    let pending: { resolve: Function; reject: Function; config: any } | null = null;
    let pendingDashboard: { resolve: Function; reject: Function; cards: string[]; order: string[] } | null = null;
    let pendingGeneral: { resolve: Function; config: any } | null = null;
    const calls: { command: string; args: any }[] = [];
    const copy = (value: any) => JSON.parse(JSON.stringify(value));
    w.__settingsFixture = { calls, holdAutoSave: false, holdDashboardSave: false, holdGeneralSave: false, failLoad: Boolean(options.failLoad), failLowQuotaLoad: Boolean(options.failLowQuotaLoad), lowQuota: () => copy(lowQuota),
        resolveGeneralSave: () => { if (pendingGeneral) { general = { ...copy(pendingGeneral.config), dashboard: general.dashboard }; pendingGeneral.resolve(null); pendingGeneral = null; } },
        rejectDashboardSave: () => { pendingDashboard?.reject('synthetic card save rejection'); pendingDashboard = null; },
        resolveDashboardSave: () => { if (pendingDashboard) { general.dashboard = { cards: copy(pendingDashboard.cards), order: copy(pendingDashboard.order) }; pendingDashboard.resolve(copy(general.dashboard)); pendingDashboard = null; } },
        rejectAutoSave: () => { pending?.reject('synthetic save rejection'); pending = null; },
        resolveAutoSave: () => { if (pending) { lowQuota = copy(pending.config); pending.resolve(copy(lowQuota)); pending = null; } },
    };
    w.__TAURI_INTERNALS__ = {
        transformCallback: (fn: Function) => { const id = callback++; callbacks[id] = fn; return id; }, unregisterCallback: (id: number) => { delete callbacks[id]; }, convertFileSrc: (s: string) => s,
        metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
        invoke: async (command: string, args: any = {}) => {
            calls.push({ command, args: copy(args) });
            if (command === 'load_config') { if (w.__settingsFixture.failLoad) throw 'synthetic load rejection'; return copy(general); }
            if (command === 'save_config') {
                if (w.__settingsFixture.holdGeneralSave) return new Promise(resolve => { pendingGeneral = { resolve, config: copy(args.config) }; });
                general = { ...copy(args.config), dashboard: general.dashboard }; return null;
            }
            if (command === 'set_dashboard_cards') {
                if (w.__settingsFixture.holdDashboardSave) return new Promise((resolve, reject) => { pendingDashboard = { resolve, reject, cards: copy(args.cards), order: copy(args.order) }; });
                general.dashboard = { cards: copy(args.cards), order: copy(args.order) }; return copy(general.dashboard);
            }
            if (command === 'get_data_dir_path') return '/synthetic/antigravity-tools';
            if (command === 'get_auto_switch_config') { if (w.__settingsFixture.failLowQuotaLoad) throw 'synthetic low-quota load rejection'; return copy(lowQuota); }
            if (command === 'set_auto_switch_config') {
                if (w.__settingsFixture.holdAutoSave) return new Promise((resolve, reject) => { pending = { resolve, reject, config: copy(args.config) }; });
                lowQuota = copy(args.config); return copy(lowQuota);
            }
            if (command === 'get_auto_switch_status') return copy(status);
            if (command === 'list_accounts') return copy(accounts);
            if (command === 'get_current_account') return copy(accounts[0]);
            if (command === 'get_account_dashboard_snapshot') return {
                indexed_total: accounts.length, loaded_count: accounts.length, failed_count: 0,
                current_account_id: accounts[0].id, current_identity_source: 'tools_record',
                accounts: accounts.map(account => ({ id: account.id, email: account.email, name: null, custom_label: null,
                    read_status: 'loaded', read_error: null, disabled: false, validation_blocked: false,
                    validation_blocked_until: null, protected_models: [], quota: null })),
            };
            if (command === 'get_desktop_settings') return copy(desktop);
            if (command === 'set_desktop_preferences') { desktop = { ...desktop, ...args.patch }; general.desktop = { launch_at_login: desktop.launch_at_login, hide_dock_icon: desktop.hide_dock_icon, start_minimized: desktop.start_minimized }; return copy(desktop); }
            if (command === 'set_menu_bar_preferences') { general.menu_bar = { ...general.menu_bar, ...args.patch, ...(args.quotaScope ? { quota_scope: args.quotaScope } : {}) }; return copy(general.menu_bar); }
            if (command === 'set_window_theme' || command === 'set_window_language') return null;
            if (command === 'plugin:event|listen') return callback++;
            if (command.startsWith('plugin:window|')) {
                if (/size$/.test(command)) return { width: 1100, height: 780 };
                if (/position$/.test(command)) return { x: 0, y: 0 };
                if (/available_monitors/.test(command)) return [];
                if (/is_/.test(command)) return false;
                return null;
            }
            throw `Unimplemented synthetic command: ${command}`;
        },
    };
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    localStorage.setItem('i18nextLng', general.language);
    const fetchOriginal = window.fetch.bind(window);
    window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
        const url = new URL(typeof input === 'string' ? input : input instanceof URL ? input.href : input.url, location.href);
        if (url.origin !== location.origin) return Promise.reject(new Error('External fetch refused by synthetic fixture'));
        return fetchOriginal(input, init);
    };
}
