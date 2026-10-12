export interface ModelPricing {
    model: string;
    input: number;
    output: number;
    cached: number;
}

// Keep this policy aligned with menu_bar_usage::pricing_keys. Experimental
// names estimate the same version/family, without borrowing a media model's rate.
const pricingKeys = (model: string) => {
    const name = model.toLowerCase().replace(/-n$/, '');
    const tokens = name.match(/[a-z]+|[0-9]+/g) || [];
    const exact = tokens.join('-');
    const gemini = /^gemini-([0-9]+(?:-[0-9]+)*)-(flash(?:-lite)?|pro)(?:-(.*))?$/.exec(exact);
    if (gemini) {
        const suffix = gemini[3]?.split('-') || [];
        const distinctVariant = suffix.some(token => ['lite', 'image', 'vision', 'audio', 'tts', 'live', 'embedding', 'embeddings', 'robotics', 'native'].includes(token));
        let experimental = false;
        const alias = !distinctVariant && suffix.every(token => {
            if (['exp', 'experimental', 'preview', 'test'].includes(token)) experimental = true;
            return experimental || /^[a-z]$/.test(token) || ['high', 'medium', 'low', 'minimal', 'thinking'].includes(token);
        });
        return { exact, base: alias ? `gemini-${gemini[1]}-${gemini[2]}` : undefined };
    }
    return { exact: name.replace(/[^a-z0-9]/g, '')
        // Thinking uses the same model's token rates, not a separate price tier.
        .replace(/^(claude(?:opus|sonnet|haiku)\d+)thinking$/, '$1'), base: undefined };
};

export function findModelPricing(model: string, snapshot: { prices: ModelPricing[] } | null) {
    const keys = pricingKeys(model);
    const entries = snapshot?.prices.map(price => ({ price, keys: pricingKeys(price.model) })) || [];
    let matches = entries.filter(entry => entry.keys.exact === keys.exact);
    if (!matches.length && keys.base) {
        const aliases = entries.filter(entry => entry.keys.base === keys.base);
        const base = aliases.filter(entry => entry.keys.exact === keys.base);
        matches = base.length ? base : aliases;
    }
    if (matches.length !== 1) return undefined;
    const price = matches[0].price;
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
