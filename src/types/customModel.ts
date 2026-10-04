export type ModelProvider = 'deepseek' | 'openrouter' | 'custom';
export type ApiFormat = 'openai' | 'anthropic' | 'google';

export interface CustomModelEntry {
    name: string;
    displayName: string;
    provider: ModelProvider | string;
    apiFormat: ApiFormat | string;
    apiUrl: string;
    apiKey: string;
    externalModelName: string;
    enabled: boolean;
    contextWindow?: number;
    maxOutputTokens?: number;
    reasoningEffort?: string;
}

export interface TestConnectionResult {
    success: boolean;
    latency_ms?: number;
    message: string;
}
