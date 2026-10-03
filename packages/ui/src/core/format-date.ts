/** Format a date consistently with the shared Chinese UI surfaces. */
export function formatZhDate(value: string | number | Date): string {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleDateString("zh-CN");
}
