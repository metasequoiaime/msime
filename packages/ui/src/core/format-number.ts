/** Format a count consistently with the shared Chinese UI surfaces. */
export function formatZhNumber(value: number): string {
  return value.toLocaleString("zh-CN");
}

/** Format a part-to-whole percentage for the shared Chinese statistics surfaces. */
export function formatZhPercent(value: number, total: number): string {
  return total === 0 ? "—" : `${((value / total) * 100).toFixed(1)}%`;
}
