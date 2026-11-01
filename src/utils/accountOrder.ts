/**
 * Merge a reordered subset of accounts into the full saved order.
 *
 * Drag-and-drop only knows the accounts on screen (the current page, search or
 * filter result). The accounts shown keep occupying the positions they held in
 * the full list, in the new relative order; every other account stays where it
 * was. Unknown and repeated ids are ignored. A complete list passes through.
 */
export function mergeVisibleOrder(fullIds: string[], visibleOrder: string[]): string[] {
    const known = new Set(fullIds);
    const reordered: string[] = [];
    const seen = new Set<string>();
    for (const id of visibleOrder) {
        if (known.has(id) && !seen.has(id)) {
            seen.add(id);
            reordered.push(id);
        }
    }
    let next = 0;
    return fullIds.map(id => (seen.has(id) ? reordered[next++] : id));
}
