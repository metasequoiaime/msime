import { clamp } from "../core/number";
import { formatZhMonthDay } from "../core/format-date";

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
    return { key: dayKey(date), label: formatZhMonthDay(date) };
  });
}

/** Returns well-formed recorded day keys in chronological order, ignoring malformed host entries. */
export function statisticDayKeys(days: Record<string, number>): string[] {
  return Object.keys(days)
    .filter((key) => /^\d{4}-\d{2}-\d{2}$/.test(key))
    .sort();
}

export function mobileTrendLength(days: Record<string, number>): number {
  const earliest = statisticDayKeys(days)[0];
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
  return clamp(span, 30, 366);
}

export function sumStatisticValues(
  values: Record<string, number>,
  keys: readonly string[],
): number {
  return keys.reduce((total, key) => total + (values[key] ?? 0), 0);
}

export function dayLabel(key: string): string {
  const [, month, day] = key.split("-");
  return `${Number(month)}月${Number(day)}日`;
}

export type HeatmapDay = { key: string; label: string; count: number; future: boolean };

export function statisticsHeatmapWeeks(
  days: Record<string, number>,
  today = new Date(),
): HeatmapDay[][] {
  const current = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const thisMonday = new Date(current);
  thisMonday.setDate(current.getDate() - ((current.getDay() + 6) % 7));
  const start = new Date(thisMonday);
  start.setDate(thisMonday.getDate() - 52 * 7);
  return Array.from({ length: 53 }, (_, week) =>
    Array.from({ length: 7 }, (_, row) => {
      const date = new Date(start);
      date.setDate(start.getDate() + week * 7 + row);
      const key = dayKey(date);
      return {
        key,
        label: formatZhMonthDay(date),
        count: days[key] ?? 0,
        future: date > current,
      };
    }),
  );
}

export function addDays(key: string, days: number): string {
  const parsed = Date.parse(`${key}T00:00:00Z`);
  if (Number.isNaN(parsed)) return key;
  const shifted = new Date(parsed + days * 86_400_000);
  const month = String(shifted.getUTCMonth() + 1).padStart(2, "0");
  const day = String(shifted.getUTCDate()).padStart(2, "0");
  return `${shifted.getUTCFullYear()}-${month}-${day}`;
}

export function currentStreak(recorded: readonly string[], todayKey: string): number {
  const present = new Set(recorded);
  let cursor = present.has(todayKey) ? todayKey : addDays(todayKey, -1);
  let streak = 0;
  while (present.has(cursor)) {
    streak += 1;
    cursor = addDays(cursor, -1);
  }
  return streak;
}

export function longestStreak(recorded: readonly string[]): number {
  if (recorded.length === 0) return 0;
  let longest = 1;
  let run = 1;
  for (let index = 1; index < recorded.length; index += 1) {
    if (recorded[index] === recorded[index - 1]) continue;
    run = recorded[index] === addDays(recorded[index - 1], 1) ? run + 1 : 1;
    if (run > longest) longest = run;
  }
  return longest;
}

export function formatActiveTime(milliseconds: number): string {
  if (milliseconds <= 0) return "0分";
  const totalMinutes = Math.floor(milliseconds / 60_000);
  if (totalMinutes === 0) return `${Math.max(1, Math.round(milliseconds / 1000))}秒`;
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours === 0) return `${minutes}分`;
  return minutes === 0 ? `${hours}小时` : `${hours}小时${minutes}分`;
}
