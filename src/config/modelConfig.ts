import { Bot, BrainCircuit, Sparkles } from 'lucide-react';
import type { ModelQuota } from '../types/account';
import {
    getModelProtectionKey,
    getCanonicalModelKey,
    getCanonicalModelDisplayName,
} from '../utils/modelCategory';

const Gemini = { Color: Sparkles };
const Claude = { Color: BrainCircuit };
const OpenAI = { Avatar: Bot };

/**
 * 模型配置接口
 */
export interface ModelConfig {
    /** 模型完整显示名称 (作为回退或默认展示) */
    label: string;
    /** 模型简短标签 (用于列表/卡片) */
    shortLabel: string;
    /** 保护模型的键名 */
    protectedKey: string;
    /** 模型图标组件 */
    Icon: React.ComponentType<any>;
    /** 所属系列/分组 */
    group: string;
    /** 选填标签 (用于筛选) */
    tags?: string[];
}

/**
 * 模型配置映射
 * 键为模型 ID，值为模型配置
 */
export const MODEL_CONFIG: Record<string, ModelConfig> = {
    // Gemini 3.x 系列
    // [Migrate] Gemini 3 Pro High/Low -> Gemini 3.1 Pro High/Low
    'gemini-3.1-pro-high': {
        label: 'Gemini 3.1 Pro High',
        shortLabel: 'G3.1 Pro',
        protectedKey: 'gemini-pro',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['pro', 'high'],
    },
    // Backward-compatible alias
    'gemini-3-pro-high': {
        label: 'Gemini 3.1 Pro High',
        shortLabel: 'G3.1 Pro',
        protectedKey: 'gemini-pro',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['pro', 'high'],
    },
    'gemini-3-flash': {
        label: 'Gemini 3 Flash',
        shortLabel: 'G3 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash'],
    },
    'gemini-3.1-flash-image': {
        label: 'Gemini 3.1 Flash Image',
        shortLabel: 'G3.1 Image',
        protectedKey: 'gemini-3.1-flash-image',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['image', 'flash'],
    },
    'gemini-3-pro-image': {
        label: 'Gemini 3 Image',
        shortLabel: 'G3 Image',
        protectedKey: 'gemini-3-pro-image',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['image'],
    },
    'gemini-3.5-flash': {
        label: 'Gemini 3.5 Flash',
        shortLabel: 'G3.5 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash'],
    },
    'gemini-3.8-flash-high': {
        label: 'Gemini 3.8 Flash (High)',
        shortLabel: 'G3.8 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash', 'high'],
    },
    'gemini-3.8-flash-medium': {
        label: 'Gemini 3.8 Flash (Medium)',
        shortLabel: 'G3.8 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash', 'medium'],
    },
    'gemini-3.8-flash-low': {
        label: 'Gemini 3.8 Flash (Low)',
        shortLabel: 'G3.8 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash', 'low'],
    },
    'gemini-3.7-flash-high': {
        label: 'Gemini 3.7 Flash (High)',
        shortLabel: 'G3.7 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash', 'high'],
    },
    'gemini-3.6-flash-high': {
        label: 'Gemini 3.6 Flash (High)',
        shortLabel: 'G3.6 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash', 'high'],
    },
    'gemini-3.7-flash': {
        label: 'Gemini 3.7 Flash',
        shortLabel: 'G3.7 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash'],
    },
    'gemini-3.7-flash-tiered': {
        label: 'Gemini 3.7 Flash Tiered',
        shortLabel: 'G3.7 Tiered',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash', 'tiered'],
    },
    'gemini-3.1-flash-lite': {
        label: 'Gemini 3.1 Flash Lite',
        shortLabel: 'G3.1 Lite',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash', 'lite'],
    },
    'gemini-3.1-pro': {
        label: 'Gemini 3.1 Pro',
        shortLabel: 'G3.1 Pro',
        protectedKey: 'gemini-pro',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['pro'],
    },
    'gemini-3-flash-agent': {
        label: 'Gemini 3.5 Flash (High)',
        shortLabel: 'G3.5 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['flash', 'high'],
    },
    'gemini-pro-agent': {
        label: 'Gemini 3.1 Pro (High)',
        shortLabel: 'G3.1 Pro',
        protectedKey: 'gemini-pro',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['pro', 'high'],
    },
    'gemini-3.1-pro-low': {
        label: 'Gemini 3.1 Pro Low',
        shortLabel: 'G3.1 Low',
        protectedKey: 'gemini-pro',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['pro', 'low'],
    },
    // Backward-compatible alias
    'gemini-3-pro-low': {
        label: 'Gemini 3.1 Pro Low',
        shortLabel: 'G3.1 Low',
        protectedKey: 'gemini-pro',
        Icon: Gemini.Color,
        group: 'Gemini 3',
        tags: ['pro', 'low'],
    },

    // Gemini 2.5 系列
    'gemini-2.5-flash': {
        label: 'Gemini 2.5 Flash',
        shortLabel: 'G2.5 Flash',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 2.5',
        tags: ['flash'],
    },
    'gemini-2.5-flash-lite': {
        label: 'Gemini 2.5 Flash Lite',
        shortLabel: 'G2.5 Lite',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 2.5',
        tags: ['flash', 'lite'],
    },
    'gemini-2.5-flash-thinking': {
        label: 'Gemini 2.5 Flash Think',
        shortLabel: 'G2.5 Think',
        protectedKey: 'gemini-flash',
        Icon: Gemini.Color,
        group: 'Gemini 2.5',
        tags: ['flash', 'thinking'],
    },
    'gemini-2.5-pro': {
        label: 'Gemini 2.5 Pro',
        shortLabel: 'G2.5 Pro',
        protectedKey: 'gemini-pro',
        Icon: Gemini.Color,
        group: 'Gemini 2.5',
        tags: ['pro'],
    },

    // Claude 系列
    'claude-sonnet-4-6': {
        label: 'Claude 4.6',
        shortLabel: 'Claude 4.6',
        protectedKey: 'claude',
        Icon: Claude.Color,
        group: 'Claude',
        tags: ['sonnet'],
    },
    'claude-sonnet-4-6-thinking': {
        label: 'Claude 4.6 TK',
        shortLabel: 'Claude 4.6 TK',
        protectedKey: 'claude',
        Icon: Claude.Color,
        group: 'Claude',
        tags: ['sonnet', 'thinking'],
    },
    'claude-opus-4-6': {
        label: 'Claude Opus 4.6',
        shortLabel: 'Claude Opus 4.6',
        protectedKey: 'claude',
        Icon: Claude.Color,
        group: 'Claude',
        tags: ['opus'],
    },
    'claude-opus-4-6-thinking': {
        label: 'Claude Opus 4.6 TK',
        shortLabel: 'Claude Opus 4.6 TK',
        protectedKey: 'claude',
        Icon: Claude.Color,
        group: 'Claude',
        tags: ['opus', 'thinking'],
    },

    // OpenAI / Outros modelos
    'gpt-oss-120b-medium': {
        label: 'GPT-OSS 120B (Medium)',
        shortLabel: 'GPT-OSS',
        protectedKey: 'gpt-oss',
        Icon: OpenAI.Avatar,
        group: 'Other',
        tags: ['openai'],
    },
};

/**
 * 获取所有模型 ID 列表
 */
export const getAllModelIds = (): string[] => Object.keys(MODEL_CONFIG);

/**
 * 根据模型 ID 获取配置
 */
export const getModelConfig = (modelId: string): ModelConfig | undefined => {
    return MODEL_CONFIG[modelId.toLowerCase()];
};

/**
 * 获取模型的排序权重（动态识别版本号，确保未来新模型自动优先排序）
 */
export function getModelSortWeight(modelId: string): number {
    const id = modelId.toLowerCase();
    let weight = 0;

    // 1. 系列权重 (动态匹配 Gemini 版本，新版本自动优先)
    const geminiVer = id.match(/^gemini-(\d+(\.\d+)?)/);
    if (geminiVer) {
        const v = parseFloat(geminiVer[1]);
        // 动态计算：版本越高权重数字越小，如 4.0 -> 50000, 3.8 -> 54000, 3.1 -> 68000, 2.5 -> 80000
        weight += Math.max(10000, Math.round(130000 - v * 20000));
    } else if (id.includes('claude')) {
        weight += 200000;
    } else {
        weight += 300000;
    }

    // 2. 性能级别权重
    if (id.includes('opus')) {
        weight += 500;
    } else if (id.includes('pro') || id.includes('sonnet')) {
        weight += 1000;
    } else if (id.includes('flash')) {
        weight += 2000;
    } else if (id.includes('lite')) {
        weight += 3000;
    }

    // 3. 特殊后缀权重
    if (id.includes('high')) {
        weight += 0;
    } else if (id.includes('thinking')) {
        weight += 10;
    } else if (id.includes('image')) {
        weight += 20;
    } else if (id.includes('low')) {
        weight += 30;
    }

    return weight;
}

/**
 * 对模型列表进行排序
 * @param models 模型列表
 * @returns 排序后的模型列表
 */
export function sortModels<T extends { id: string }>(models: T[]): T[] {
    return [...models].sort((a, b) => {
        const weightA = getModelSortWeight(a.id);
        const weightB = getModelSortWeight(b.id);

        // 按权重升序排序
        if (weightA !== weightB) {
            return weightA - weightB;
        }

        // 权重相同时，按字母顺序排序
        return a.id.localeCompare(b.id);
    });
}

// ── 模型分类与保护键（实现在 src/utils/modelCategory.ts，此处只 re-export）───

export {
    categorizeModel,
    getModelProtectionKey,
    getModelDisplayName,
    getCanonicalModelKey,
    getCanonicalModelDisplayName,
    getCanonicalModelSublabel,
    getCanonicalModelTag,
    findQuotaModel,
    findImageQuotaModel,
    ensurePinnedImageSelector,
    DEFAULT_IMAGE_PIN_SELECTOR,
    resolveQuotaModels,
    type ModelCategory,
    type QuotaModelSelection,
} from '../utils/modelCategory';

export const DEFAULT_PINNED_MODELS: string[] = [
    'gemini-3.1-pro',
    'gemini-3.8-flash',
    'claude-sonnet-4-6',
];

export interface DisplayQuotaModelItem {
    id: string;
    label: string;
    protectedKey: string;
    Icon: any;
    data?: ModelQuota;
}

/**
 * 获取卡片与表格统一展示的模型列表
 * 智能折叠同模型的不同档位（如 High/Medium/Low），同系列合并展示，确保前台越简洁越好且不遗漏
 */
export function getDisplayQuotaModels(
    accountModels: ModelQuota[] | undefined,
    pinnedConfigIds: string[] | undefined
): DisplayQuotaModelItem[] {
    const rawPinned = (pinnedConfigIds && pinnedConfigIds.length > 0)
        ? pinnedConfigIds
        : DEFAULT_PINNED_MODELS;

    // 1. 归一化并去重用户勾选的主模型 Key 列表
    const pinnedKeys: string[] = [];
    for (const selectorId of rawPinned) {
        const canon = getCanonicalModelKey(selectorId);
        if (canon && !pinnedKeys.includes(canon)) {
            pinnedKeys.push(canon);
        }
    }

    // 2. 将账号的实际模型按主模型 Key 汇聚（若存在多个档位，择优选取 High 或有效项）
    const accountCanonMap = new Map<string, ModelQuota>();
    for (const m of (accountModels || [])) {
        if (!m.name) continue;
        const canon = getCanonicalModelKey(m.name);
        const existing = accountCanonMap.get(canon);
        if (!existing) {
            accountCanonMap.set(canon, m);
        } else {
            const mLower = m.name.toLowerCase();
            const exLower = existing.name.toLowerCase();
            if (mLower.includes('high') && !exLower.includes('high')) {
                accountCanonMap.set(canon, m);
            }
        }
    }

    const results: DisplayQuotaModelItem[] = [];

    for (const canonKey of pinnedKeys) {
        let rawModel = accountCanonMap.get(canonKey);
        if (!rawModel && canonKey === 'gemini-3.1-pro' && !pinnedKeys.includes('gemini-2.5-pro')) {
            rawModel = accountCanonMap.get('gemini-2.5-pro');
        } else if (!rawModel && canonKey === 'gemini-2.5-pro' && !pinnedKeys.includes('gemini-3.1-pro')) {
            rawModel = accountCanonMap.get('gemini-3.1-pro');
        }
        const label = getCanonicalModelDisplayName(rawModel?.name || canonKey);
        const protectedKey = getModelProtectionKey(canonKey) || canonKey;

        let Icon = Gemini.Color;
        if (canonKey.includes('claude')) {
            Icon = Claude.Color;
        } else if (canonKey.includes('gpt') || canonKey.includes('oss')) {
            Icon = OpenAI.Avatar;
        }

        results.push({
            id: canonKey,
            label,
            protectedKey,
            Icon,
            data: rawModel || ({ name: canonKey, percentage: 0 } as ModelQuota),
        });
    }

    return results;
}

