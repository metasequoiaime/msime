/** Format a count consistently with the shared Chinese UI surfaces. */
export function formatZhNumber(value: number): string {
  return value.toLocaleString("zh-CN");
}
