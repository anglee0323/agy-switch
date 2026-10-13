export function getQuotaColor(percentage: number): string {
    if (percentage >= 50) return 'success';
    if (percentage >= 20) return 'warning';
    return 'error';
}

export function parseFlexibleDate(dateStr: string | undefined | null): Date | null {
    if (!dateStr || dateStr.trim() === '') return null;
    const trimmed = dateStr.trim();
    // 兼容纯数字 Unix 时间戳（秒或毫秒）
    if (/^\d+$/.test(trimmed)) {
        const num = parseInt(trimmed, 10);
        return new Date(trimmed.length <= 10 ? num * 1000 : num);
    }
    const parsed = new Date(trimmed);
    return isNaN(parsed.getTime()) ? null : parsed;
}

export function formatTimeRemaining(dateStr: string): string {
    const targetDate = parseFlexibleDate(dateStr);
    if (!targetDate) return '0h 0m';

    const now = new Date();
    const diffMs = targetDate.getTime() - now.getTime();

    if (diffMs <= 0) return '0h 0m';

    const diffHrs = Math.floor(diffMs / (1000 * 60 * 60));
    const diffMins = Math.floor((diffMs % (1000 * 60 * 60)) / (1000 * 60));

    if (diffHrs >= 24) {
        const diffDays = Math.floor(diffHrs / 24);
        const remainingHrs = diffHrs % 24;
        return `${diffDays}d ${remainingHrs}h`;
    }

    return `${diffHrs}h ${diffMins}m`;
}
