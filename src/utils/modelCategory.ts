/**
 * 模型分类工具函数（无 React / icons 依赖，可在 Node 环境直接导入）
 */

export type ModelCategory = 'gemini-pro' | 'gemini-flash' | 'gemini-pro-image' | 'gemini-flash-image' | 'claude' | 'other';

export function categorizeModel(name: string): ModelCategory {
    const n = name.trim().toLowerCase();
    const isGemini = n.startsWith('gemini-');
    const isImage = (isGemini && n.includes('image')) || n.startsWith('image') || n.startsWith('imagen');
    if (isImage) return n.includes('flash') ? 'gemini-flash-image' : 'gemini-pro-image';
    if (isGemini && (n.includes('carbon') || n.includes('argon') || n.includes('orchestrator'))) return 'gemini-pro';
    if (isGemini && (n.includes('hydrogen') || n.includes('flash'))) return 'gemini-flash';
    if (isGemini && n.includes('pro')) return 'gemini-pro';
    if (n.includes('claude') || n.includes('opus') || n.includes('sonnet') || n.includes('haiku')) return 'claude';
    return 'other';
}

export interface ModelDisplayNameInput {
    name: string;
    display_name?: string;
}

const DEFAULT_MODEL_LABELS: Record<string, string> = {
    'gemini-4-carbon': 'Gemini 4 Carbon',
    'gemini-4-carbon-high': 'Gemini 4 Carbon (High)',
    'gemini-4-carbon-medium': 'Gemini 4 Carbon (Medium)',
    'gemini-4-carbon-low': 'Gemini 4 Carbon (Low)',
    'gemini-4-argon': 'Gemini 4 Argon',
    'gemini-4-argon-high': 'Gemini 4 Argon (High)',
    'gemini-4-argon-medium': 'Gemini 4 Argon (Medium)',
    'gemini-4-argon-low': 'Gemini 4 Argon (Low)',
    'gemini-4-hydrogen': 'Gemini 4 Hydrogen',
    'gemini-4-hydrogen-xhigh': 'Gemini 4 Hydrogen (Extra)',
    'gemini-4-hydrogen-high': 'Gemini 4 Hydrogen (High)',
    'gemini-4-hydrogen-medium': 'Gemini 4 Hydrogen (Medium)',
    'gemini-4-hydrogen-low': 'Gemini 4 Hydrogen (Low)',
    'gemini-pro-orchestrator': 'Orchestrator (Carbon)',
    'gemini-pro-orchestrator-argon': 'Orchestrator (Argon)',
    'gemini-pro-agent': 'Gemini 3.1 Pro (High)',
    'gemini-3.1-pro-high': 'Gemini 3.1 Pro High',
    'gemini-3-pro-high': 'Gemini 3.1 Pro High',
    'gemini-3.1-pro': 'Gemini 3.1 Pro',
    'gemini-3.1-pro-low': 'Gemini 3.1 Pro Low',
    'gemini-3-pro-low': 'Gemini 3.1 Pro Low',
    'gemini-2.5-pro': 'Gemini 2.5 Pro',
    'gemini-3-flash-agent': 'Gemini 3.5 Flash (High)',
    'gemini-3.8-flash-high': 'Gemini 3.8 Flash (High)',
    'gemini-3.8-flash-medium': 'Gemini 3.8 Flash (Medium)',
    'gemini-3.8-flash-low': 'Gemini 3.8 Flash (Low)',
    'gemini-3.7-flash-high': 'Gemini 3.7 Flash (High)',
    'gemini-3.7-flash': 'Gemini 3.7 Flash',
    'gemini-3.7-flash-tiered': 'Gemini 3.7 Flash Tiered',
    'gemini-3.6-flash-high': 'Gemini 3.6 Flash (High)',
    'gemini-3.5-flash': 'Gemini 3.5 Flash',
    'gemini-3.5-flash-lite': 'Gemini 3.5 Flash Lite',
    'gemini-3-flash': 'Gemini 3 Flash',
    'gemini-2.5-flash': 'Gemini 2.5 Flash',
    'gemini-3.1-flash-image': 'Gemini 3.1 Flash Image',
    'gemini-3-pro-image': 'Gemini 3 Image',
    'claude-sonnet-5-5': 'Claude Sonnet 5.5 (Thinking)',
    'claude-opus-5-5': 'Claude Opus 5.5 (Thinking)',
    'claude-sonnet-5': 'Claude Sonnet 5 (Thinking)',
    'claude-opus-5': 'Claude Opus 5 (Thinking)',
    'claude-sonnet-4-6': 'Claude Sonnet 4.6 (Thinking)',
    'claude-opus-4-6-thinking': 'Claude Opus 4.6 (Thinking)',
    'claude-sonnet-4-5': 'Claude Sonnet 4.5 (Thinking)',
    'claude-haiku-4-5': 'Claude Haiku 4.5',
};

export function getModelDisplayName(
    model: ModelDisplayNameInput | null | undefined,
    fallback?: string,
): string {
    if (model) {
        if (model.display_name) return model.display_name;
        if (model.name) {
            return DEFAULT_MODEL_LABELS[model.name] || model.name;
        }
    }
    return fallback ?? '';
}

/**
 * 标准化/折叠模型ID到统一主模型 Key（消除 High/Medium/Low 等多档位割裂）
 * 例如：
 * gemini-3.8-flash-high / medium / low -> gemini-3.8-flash
 * gemini-pro-agent / gemini-3.1-pro-high / low -> gemini-3.1-pro
 * gemini-3.5-flash-high / extra-low -> gemini-3.5-flash
 * claude-sonnet-4-6 / thinking -> claude-sonnet-4-6
 */
export function getCanonicalModelKey(rawId: string): string {
    const id = (rawId || '').toLowerCase().trim();
    if (!id) return '';

    // 1. 已知固定前缀/别名映射
    // Image variants must not collapse into a chat Flash/Pro family.
    if (id.includes('image') || id.startsWith('imagen')) {
        return 'gemini-3.1-flash-image';
    }
    if (id === 'gemini-pro-agent' || id.startsWith('gemini-3.1-pro') || id.startsWith('gemini-3-pro')) {
        return 'gemini-3.1-pro';
    }
    if (id.startsWith('gemini-4-carbon')) {
        return 'gemini-4-carbon';
    }
    if (id.startsWith('gemini-4-argon')) {
        return 'gemini-4-argon';
    }
    if (id.startsWith('gemini-4-hydrogen')) {
        return 'gemini-4-hydrogen';
    }
    if (id.startsWith('gemini-pro-orchestrator')) {
        return id.includes('argon') || id.includes('g1-pro')
            ? 'gemini-pro-orchestrator-argon'
            : 'gemini-pro-orchestrator';
    }
    if (id.startsWith('gemini-3.8-flash-lite')) {
        return 'gemini-3.8-flash-lite';
    }
    if (id.startsWith('gemini-3.8-flash')) {
        return 'gemini-3.8-flash';
    }
    if (id.startsWith('gemini-3.7-flash')) {
        return 'gemini-3.7-flash';
    }
    if (id.startsWith('gemini-3.6-flash')) {
        return 'gemini-3.6-flash';
    }
    if (id.startsWith('gemini-3.5-flash-lite')) {
        return 'gemini-3.5-flash-lite';
    }
    if (id.startsWith('gemini-3.5-flash') || id === 'gemini-3-flash-agent') {
        return 'gemini-3.5-flash';
    }
    if (id === 'gemini-3-flash') {
        return 'gemini-3-flash';
    }
    if (id.startsWith('gemini-3.1-flash-lite')) {
        return 'gemini-3.1-flash-lite';
    }
    if (id.startsWith('gemini-2.5-pro')) {
        return 'gemini-2.5-pro';
    }
    if (id.startsWith('gemini-2.5-flash')) {
        return 'gemini-2.5-flash';
    }
    if (id.includes('claude') && id.includes('opus')) {
        if (id.includes('5-5') || id.includes('5.5')) return 'claude-opus-5-5';
        if (/claude-opus-5(?:$|[-.])/.test(id)) return 'claude-opus-5';
        return 'claude-opus-4-6-thinking';
    }
    if (id.includes('claude') && id.includes('sonnet')) {
        if (id.includes('5-5') || id.includes('5.5')) return 'claude-sonnet-5-5';
        if (/claude-sonnet-5(?:$|[-.])/.test(id)) return 'claude-sonnet-5';
        return 'claude-sonnet-4-6';
    }
    if (id.includes('gpt-oss')) {
        return 'gpt-oss-120b';
    }

    // 2. 动态剥离任何后缀：-high, -medium, -low, -extra-low, -xhigh, -tiered, -thinking, -disabled
    const stripped = id.replace(/-(high|medium|low|extra-low|xhigh|tiered|thinking|disabled)$/i, '').trim();
    return stripped || id;
}

/**
 * 获取主模型的规范友好显示名
 */
export function getCanonicalModelDisplayName(keyOrRawId: string): string {
    const canon = getCanonicalModelKey(keyOrRawId);
    const knownLabels: Record<string, string> = {
        'gemini-4-carbon': 'Gemini 4 Carbon',
        'gemini-4-argon': 'Gemini 4 Argon',
        'gemini-4-hydrogen': 'Gemini 4 Hydrogen',
        'gemini-3.8-flash': 'Gemini 3.8 Flash',
        'gemini-3.1-pro': 'Gemini 3.1 Pro',
        'gemini-3.7-flash': 'Gemini 3.7 Flash',
        'gemini-3.6-flash': 'Gemini 3.6 Flash',
        'gemini-3.5-flash': 'Gemini 3.5 Flash',
        'gemini-3.5-flash-lite': 'Gemini 3.5 Flash Lite',
        'gemini-3.8-flash-lite': 'Gemini 3.8 Flash Lite',
        'gemini-3-flash': 'Gemini 3 Flash',
        'gemini-3.1-flash-lite': 'Gemini 3.1 Flash Lite',
        'gemini-2.5-pro': 'Gemini 2.5 Pro',
        'gemini-2.5-flash': 'Gemini 2.5 Flash',
        'gemini-3.1-flash-image': 'Gemini 3.1 Flash Image',
        'gemini-pro-orchestrator': 'Orchestrator (Carbon)',
        'gemini-pro-orchestrator-argon': 'Orchestrator (Argon)',
        'claude-sonnet-5-5': 'Claude Sonnet 5.5 (Thinking)',
        'claude-opus-5-5': 'Claude Opus 5.5 (Thinking)',
        'claude-sonnet-5': 'Claude Sonnet 5 (Thinking)',
        'claude-opus-5': 'Claude Opus 5 (Thinking)',
        'claude-sonnet-4-6': 'Claude Sonnet 4.6 (Thinking)',
        'claude-opus-4-6-thinking': 'Claude Opus 4.6 (Thinking)',
        'gpt-oss-120b': 'GPT-OSS 120B',
    };

    if (knownLabels[canon]) return knownLabels[canon];

    // 动态自适应格式化（针对未来未知的 Google / 其他模型）
    return canon
        .split('-')
        .map(part => {
            const p = part.toLowerCase();
            if (p === 'gpt') return 'GPT';
            if (p === 'oss') return 'OSS';
            if (p === 'pro') return 'Pro';
            if (p === 'flash') return 'Flash';
            if (p === 'lite') return 'Lite';
            if (p === 'image') return 'Image';
            return part.charAt(0).toUpperCase() + part.slice(1);
        })
        .join(' ');
}

/**
 * 获取主模型的档位归并提示（说明其包含哪些共享池档位）
 */
export function getCanonicalModelSublabel(keyOrRawId: string, format: (tiers: string) => string = tiers => `Shared quota pool: ${tiers}`): string | undefined {
    const canon = getCanonicalModelKey(keyOrRawId);
    switch (canon) {
        case 'gemini-4-carbon':
        case 'gemini-4-argon':
            return format('Low / Medium / High');
        case 'gemini-4-hydrogen':
            return format('Low / Medium / High / Extra');
        case 'gemini-3.8-flash':
            return format('High / Medium / Low');
        case 'gemini-3.1-pro':
            return format('Pro Agent / High / Low');
        case 'gemini-3.5-flash':
            return format('High / Low');
        case 'gemini-2.5-flash':
            return format('Flash / Lite / Think');
        default:
            return undefined;
    }
}

/**
 * 获取主模型的标签
 */
export function getCanonicalModelTag(keyOrRawId: string): string | undefined {
    const canon = getCanonicalModelKey(keyOrRawId);
    if (canon.includes('orchestrator')) return 'ORCH';
    if (canon.includes('pro') || canon.includes('carbon') || canon.includes('argon')) return 'PRO';
    if (canon.includes('image')) return 'IMAGE';
    if (canon.includes('lite')) return 'LITE';
    if (canon.includes('flash') || canon.includes('hydrogen')) return 'FLASH';
    if (canon.includes('opus')) return 'OPUS';
    if (canon.includes('sonnet')) return 'SONNET';
    if (canon.includes('gpt')) return 'OPENAI';
    return undefined;
}

/**
 * 按优先级查找配额模型：先精确匹配首选名，再按类别 fallback。
 */
export function findQuotaModel<T extends { name: string }>(
    models: T[] | undefined,
    category: ModelCategory,
): T | undefined {
    if (!models || models.length === 0) return undefined;
    const preferred: Partial<Record<ModelCategory, string[]>> = {
        'gemini-pro': ['gemini-4-carbon-high', 'gemini-4-carbon', 'gemini-4-argon-high', 'gemini-4-argon-medium', 'gemini-4-argon-low', 'gemini-4-argon', 'gemini-pro-orchestrator', 'gemini-pro-agent', 'gemini-3.1-pro-high', 'gemini-3.1-pro', 'gemini-3.1-pro-low', 'gemini-2.5-pro'],
        'gemini-flash': ['gemini-4-hydrogen-xhigh', 'gemini-4-hydrogen-high', 'gemini-4-hydrogen', 'gemini-3-flash-agent', 'gemini-3-flash', 'gemini-3.5-flash'],
        'claude': ['claude-opus-5-5', 'claude-sonnet-5-5', 'claude-sonnet-4-6', 'claude-opus-4-6-thinking'],
    };
    const names = preferred[category];
    if (names) {
        for (const name of names) {
            const found = models.find(m => m.name === name);
            if (found) return found;
        }
    }
    return models.find(m => categorizeModel(m.name) === category);
}

export function getModelProtectionKey(name: string): string | null {
    switch (categorizeModel(name)) {
        case 'gemini-flash': return 'gemini-3-flash';
        case 'gemini-pro': return 'gemini-3-pro-high';
        case 'gemini-flash-image': return 'gemini-3.1-flash-image';
        case 'gemini-pro-image': return 'gemini-3-pro-image';
        case 'claude': return 'claude';
        default: return null;
    }
}

/**
 * 在任意图片类别中查找第一个实际模型。
 * 用于让新旧 image selector 共享同一配额槽位。
 */
export function findImageQuotaModel<T extends { name: string }>(
    models: T[] | undefined,
): T | undefined {
    if (!models || models.length === 0) return undefined;
    return models.find(m => {
        const c = categorizeModel(m.name);
        return c === 'gemini-flash-image' || c === 'gemini-pro-image';
    });
}

/** 账号管理 pin 列表缺省图像选择器时补入代表 Image，与仪表盘对齐。 */
export const DEFAULT_IMAGE_PIN_SELECTOR = 'gemini-3.1-flash-image';

export function ensurePinnedImageSelector(selectorIds: string[] | undefined): string[] {
    const pinned = selectorIds ? [...selectorIds] : [];
    const hasImage = pinned.some(id => {
        const category = categorizeModel(id);
        return category === 'gemini-flash-image' || category === 'gemini-pro-image';
    });
    if (hasImage) return pinned;
    pinned.push(DEFAULT_IMAGE_PIN_SELECTOR);
    return pinned;
}

export interface QuotaModelSelection<T> {
    selectorId: string;
    selectionKey: string;
    model: T | undefined;
}

export function resolveQuotaModels<T extends { name: string }>(
    models: T[] | undefined,
    selectorIds: string[],
): QuotaModelSelection<T>[] {
    const seen = new Set<string>();
    const results: QuotaModelSelection<T>[] = [];

    for (const selectorId of selectorIds) {
        const normalizedId = selectorId.trim().toLowerCase();
        const category = categorizeModel(normalizedId);

        const isImage = category === 'gemini-pro-image' || category === 'gemini-flash-image';

        // Exact-match first: a pinned id that names a real quota model must render
        // that model, not collapse into its category slot.
        const exact = !isImage
            ? models?.find(m => m.name.trim().toLowerCase() === normalizedId)
            : undefined;
        if (exact) {
            const selectionKey = `model:${normalizedId}`;
            if (seen.has(selectionKey)) continue;
            seen.add(selectionKey);
            results.push({ selectorId, selectionKey, model: exact });
            continue;
        }

        const selectionKey = isImage
            ? 'category:gemini-image'
            : category === 'other'
                ? `model:${normalizedId}`
                : `category:${category}`;

        if (seen.has(selectionKey)) continue;
        seen.add(selectionKey);

        const model = isImage
            ? findImageQuotaModel(models)
            : category === 'other'
                ? models?.find(m => m.name.trim().toLowerCase() === normalizedId)
                : findQuotaModel(models, category);

        results.push({ selectorId, selectionKey, model });
    }
    return results;
}
