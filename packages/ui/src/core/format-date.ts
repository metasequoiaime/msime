/** Format a date consistently with the shared Chinese UI surfaces. */
export function formatZhDate(value: string | number | Date): string {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleDateString("zh-CN");
}

/** Format a local calendar date as the month-and-day label used by statistics charts. */
export function formatZhMonthDay(date: Date): string {
  return Number.isNaN(date.getTime()) ? "" : `${date.getMonth() + 1}月${date.getDate()}日`;
}
