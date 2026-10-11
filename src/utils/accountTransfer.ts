import type { ExportAccountItem } from '../services/accountService';

/** Preserve transfer metadata; reject malformed JSON instead of extracting stray secrets. */
export function parseAccountTransfer(input: string): ExportAccountItem[] {
    const text = input.trim();
    let rows: unknown[];
    if (text.startsWith('{') || text.startsWith('[')) {
        const parsed = JSON.parse(text);
        if (parsed?.schema_version && ![1, 2].includes(parsed.schema_version)) throw new Error('unsupported_format');
        rows = Array.isArray(parsed) ? parsed : Array.isArray(parsed?.accounts) ? parsed.accounts : [parsed];
    } else {
        rows = (text.match(/1\/\/[a-zA-Z0-9_\-]+/g) || []).map(refresh_token => ({ refresh_token }));
    }
    if (!rows.length) throw new Error('empty_transfer');
    const unique = new Map<string, ExportAccountItem>();
    for (const value of rows) {
        if (!value || typeof value !== 'object') throw new Error('invalid_account');
        const row = value as Record<string, unknown>;
        if (typeof row.refresh_token !== 'string' || !/^1\/\/[a-zA-Z0-9_\-]+$/.test(row.refresh_token)) throw new Error('invalid_token');
        const item: ExportAccountItem = { email: '', refresh_token: row.refresh_token };
        for (const key of ['email', 'name', 'oauth_client_key', 'project_id', 'custom_label'] as const) {
            if (row[key] != null) {
                if (typeof row[key] !== 'string') throw new Error('invalid_metadata');
                item[key] = row[key];
            }
        }
        if (row.is_gcp_tos != null) {
            if (typeof row.is_gcp_tos !== 'boolean') throw new Error('invalid_metadata');
            item.is_gcp_tos = row.is_gcp_tos;
        }
        unique.set(item.refresh_token, item);
    }
    return [...unique.values()];
}
