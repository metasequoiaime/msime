export function dayKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function recentDays(length: number, today = new Date()): { key: string; label: string }[] {
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  return Array.from({ length }, (_, index) => {
    const date = new Date(start);
    date.setDate(start.getDate() - (length - index - 1));
    return { key: dayKey(date), label: `${date.getMonth() + 1}月${date.getDate()}日` };
  });
}

export function mobileTrendLength(days: Record<string, number>): number {
  const earliest = Object.keys(days)
    .filter((key) => /^\d{4}-\d{2}-\d{2}$/.test(key))
    .sort()[0];
  if (!earliest) return 30;
  const start = new Date(`${earliest}T00:00:00`);
  if (Number.isNaN(start.getTime())) return 30;
  const today = new Date();
  const span =
    Math.floor(
      (new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime() -
        start.getTime()) /
        86_400_000,
    ) + 1;
  return Math.min(366, Math.max(30, span));
}

export function sumStatisticValues(
  values: Record<string, number>,
  keys: readonly string[],
): number {
  return keys.reduce((total, key) => total + (values[key] ?? 0), 0);
}
