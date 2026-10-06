export interface DesktopPreferences {
    launch_at_login: boolean;
    hide_dock_icon: boolean;
    start_minimized: boolean;
}

export type MenuBarQuotaScope = 'all' | 'gemini' | 'other';
export const DASHBOARD_CARD_IDS = ['total_tokens', 'input_tokens', 'output_tokens', 'cache_hit_rate', 'api_cost', 'first_text_latency', 'body_speed', 'account_status', 'aggregate_quota', 'average_input'] as const;
export type DashboardCardId = typeof DASHBOARD_CARD_IDS[number];
export interface DashboardPreferences { cards: DashboardCardId[]; }
export function dashboardCards(cards: unknown): DashboardCardId[] {
    if (!Array.isArray(cards)) return [...DASHBOARD_CARD_IDS];
    return [...new Set(cards.map(id => id === 'quota_reset' ? 'average_input' : id)
        .filter((id): id is DashboardCardId => DASHBOARD_CARD_IDS.includes(id)))];
}
export function dashboardGridClass(count: number): string {
    return [
        '', 'grid-cols-1', 'grid-cols-2', 'grid-cols-2 sm:grid-cols-3',
        'grid-cols-2 lg:grid-cols-4', 'grid-cols-2 lg:grid-cols-5',
        'grid-cols-2 md:grid-cols-3 xl:grid-cols-6', 'grid-cols-2 lg:grid-cols-4 xl:grid-cols-7',
        'grid-cols-2 lg:grid-cols-4', 'grid-cols-2 md:grid-cols-3', 'grid-cols-2 md:grid-cols-3 xl:grid-cols-5',
    ][count] || 'grid-cols-2';
}
export type MenuBarResetTimeDisplay = 'hidden' | 'hover' | 'always';
export interface MenuBarPreferences {
    quota_scope: MenuBarQuotaScope;
    display_scope?: MenuBarQuotaScope;
    hide_unavailable?: boolean;
    label_style?: 'email_then_label' | 'label_then_email' | 'email_only';
    show_aggregate?: boolean;
    show_session?: boolean;
    show_weekly?: boolean;
    show_icons?: boolean;
    show_reset_on_hover?: boolean;
    reset_time_display?: MenuBarResetTimeDisplay | null;
    green_above?: number;
    red_below?: number;
}
export const DEFAULT_MENU_BAR_PREFERENCES = { quota_scope: 'all', display_scope: 'all', hide_unavailable: true,
    label_style: 'email_then_label', show_aggregate: true, show_session: true, show_weekly: true,
    show_icons: true, show_reset_on_hover: true, green_above: 60, red_below: 20 } as const satisfies MenuBarPreferences;
export const menuBarResetTimeDisplay = (preferences: MenuBarPreferences): MenuBarResetTimeDisplay =>
    preferences.reset_time_display ?? (preferences.show_reset_on_hover === false ? 'hidden' : 'hover');

export interface QuotaProtectionConfig {
    enabled: boolean;
    threshold_percentage: number;
    monitored_models: string[];
}

export interface PinnedQuotaModelsConfig {
    models: string[];
}

export interface AppConfig {
    dashboard?: DashboardPreferences;
    desktop?: DesktopPreferences;
    menu_bar?: MenuBarPreferences;
    language: string;
    theme: string;
    check_updates_on_startup?: boolean;
    auto_refresh: boolean;
    refresh_interval: number;
    auto_sync: boolean;
    sync_interval: number;
    antigravity_executable?: string;
    antigravity_ide_executable?: string;
    antigravity_args?: string[];
    quota_protection: QuotaProtectionConfig;
    pinned_quota_models: PinnedQuotaModelsConfig;
}
