export interface ModelPricing {
    model: string;
    input: number;
    output: number;
    cached: number;
}

// Match the native estimator, including the owner's explicit EXP-A estimate mapping.
// Other versions and variants retain their own pricing identities.
const normalize = (model: string) => {
    const name = model.toLowerCase().replace(/-n$/, '');
    return (name === 'gemini-3.8-flash-exp-a' ? 'gemini-3.8-flash' : name).replace(/[^a-z0-9]/g, '');
};

export function findModelPricing(model: string, snapshot: { prices: ModelPricing[] } | null) {
    const matches = snapshot?.prices.filter(entry => normalize(entry.model) === normalize(model)) || [];
    if (matches.length !== 1) return undefined;
    const price = matches[0];
    return [price.input, price.output, price.cached].every(value => Number.isFinite(value) && value >= 0) ? price : undefined;
}

export function estimateApiCost(models: Array<{ model: string; input_tokens: number; output_tokens: number; cached_tokens: number }>, snapshot: { prices: ModelPricing[] } | null) {
    return models.reduce((result, model) => {
        if (model.input_tokens === 0 && model.output_tokens === 0 && model.cached_tokens === 0) return result;
        const price = findModelPricing(model.model, snapshot);
        if (!price) result.unpricedModels += 1;
        else {
            result.pricedModels += 1;
            result.usd += (model.input_tokens * price.input + model.output_tokens * price.output + model.cached_tokens * price.cached) / 1_000_000;
        }
        return result;
    }, { usd: 0, pricedModels: 0, unpricedModels: 0 });
}
