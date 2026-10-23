export interface Account {
    id: string;
    email: string;
    name?: string;
    token: TokenData;
    device_profile?: DeviceProfile;
    quota?: QuotaData;
    disabled?: boolean;
    disabled_reason?: string;
    disabled_at?: number;
    protected_models?: string[];
    custom_label?: string;  // 用户自定义标签
    validation_blocked?: boolean;
    validation_blocked_until?: number;
    validation_blocked_reason?: string;
    validation_url?: string;
    created_at: number;
    last_used: number;
}

export interface TokenData {
    access_token: string;
    refresh_token: string;
    expires_in: number;
    expiry_timestamp: number;
    token_type: string;
    email?: string;
}

export interface QuotaData {
    models: ModelQuota[];
    last_updated: number;
    is_forbidden?: boolean;
    forbidden_reason?: string;
    subscription_tier?: string;  // 订阅类型: FREE/PRO/ULTRA
    model_forwarding_rules?: Record<string, string>; // 废弃模型转发表
    quota_groups?: QuotaGroup[]; // 按模型组的配额摘要 (weekly + 5h 双窗口)
}

export interface ModelQuota {
    name: string;
    percentage: number;
    percentage_known?: boolean;
    /** Existing fusion inference, not a verified API model-to-pool relationship. */
    quota_bucket_id?: string;
    observed_remaining_fraction?: number;
    /** Model endpoint observation before shared-window effective limits are applied. */
    primary_quota?: { remaining_fraction: number | null; reset_time: string };
    reset_time: string;
    display_name?: string;
    supports_images?: boolean;
    supports_thinking?: boolean;
    thinking_budget?: number;
    recommended?: boolean;
    max_tokens?: number;
    max_output_tokens?: number;
    supported_mime_types?: Record<string, boolean>;
}

/** 单个配额桶 (weekly / 5h) */
export interface QuotaBucket {
    bucket_id: string;
    window: string;  // "weekly" | "5h"
    remaining_fraction: number;
    remaining_fraction_known?: boolean;
    reset_time: string;
    display_name?: string;
    description?: string;
}

/** 模型组配额 (如 Gemini Models / Claude and GPT models) */
export interface QuotaGroup {
    display_name: string;
    description?: string;
    buckets: QuotaBucket[];
}

export interface DeviceProfile {
    machine_id: string;
    mac_machine_id: string;
    dev_device_id: string;
    sqm_id: string;
}
