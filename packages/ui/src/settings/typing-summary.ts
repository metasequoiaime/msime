import { clamp } from "../core/number";
import type { TypingBreakdown } from "./typing-speed";

/**
 * 打字统计 `summary` 操作的返回：「统计」页的概览、习惯、按键和成就，在 Rust 里（`crates/client-core/src/typing_statistics/metrics.rs`）统一算一次，各平台显示的数字因此一致。字段名沿用 serde 名，即 snake_case；Rust 的 `Option` 在这里是 `null`，页面把它显示为「—」而不是 0。
 *
 * 下面的文案函数移植自 Android 的 `TypingStatisticsSummary.java`，两个产品对同样的数字说同样的话。
 */
export type TypingSummary = {
  overview: OverviewSummary;
  habits: HabitsSummary;
  keys: KeysSummary;
  achievements: AchievementSummary[];
};

export type DayCount = { day: string; count: number };

export type OverviewSummary = {
  week_total: number;
  previous_week_total: number;
  /** 最近七个本地日，从早到晚；最后一项是今天。 */
  last7: DayCount[];
  /** 最近七天每个活跃分钟的可读字符数；没有任何活跃时间时为 null。 */
  average_speed: number | null;
  previous_average_speed: number | null;
  /** 选第一个候选的次数占选词的比例，0–1；不足 50 次选词时为 null。 */
  first_candidate_rate: number | null;
  /** 相对全拼节省的按键比例，0–1；还没有任何全拼计数时为 null。 */
  keystrokes_saved_rate: number | null;
  current_streak: number;
  longest_streak: number;
};

/** 字数最多的连续两小时，`[start, end)`；`end` 可越过午夜回绕。 */
export type PeakWindow = { start: number; end: number };

export type HabitsSummary = {
  /** 最近 84 个本地日，从早到晚。 */
  weeks12: DayCount[];
  /** 最近七天按本地小时累计的字数，共 24 项。 */
  hours24: number[];
  usual_hours: number[] | null;
  peak_window: PeakWindow | null;
  /** 最近 12 周里有输入的天数。 */
  active_days: number;
  /** 全部保留数据里的字符类别和来源，没归类的部分记在 `unknown` 下。 */
  breakdown: TypingBreakdown;
};

/** 一段不间断的输入，以及它开始的那一天。 */
export type TypingRun = { characters: number; day: string };

export type KeysSummary = {
  per_character_keys: number | null;
  previous_per_character_keys: number | null;
  /** 0–1. */
  backspace_rate: number | null;
  /** 0–1. */
  prediction_rate: number | null;
  longest_run: TypingRun | null;
  /** 第一、第二、第三个候选和其后所有位置各自的占比，每项 0–1。 */
  positions: [number, number, number, number] | null;
};

export type AchievementGroup = "volume" | "streak" | "skill" | "fun";

export type AchievementSummary = {
  id: string;
  /** 奖章上的文字。 */
  glyph: string;
  title: string;
  description: string;
  group: AchievementGroup;
  /** 解锁的日子；未解锁时为 null。 */
  unlocked_day: string | null;
  /** 以 `target` 的单位计的进度；可以超过它。 */
  current: number;
  target: number;
};

/** 保留一位小数，不带末尾的 `.0`：`2.3`、`7`。 */
function tenths(value: number): string {
  return String(Math.round(value * 10) / 10);
}

/** 主数字下面的周环比一行；上周没有可比数据时为 null。 */
export function weekDelta(current: number, previous: number): string | null {
  if (previous <= 0) return null;
  const percent = Math.round(((current - previous) * 100) / previous);
  if (percent === 0) return "和上周持平";
  return percent > 0 ? `比上周多 ${percent}%` : `比上周少 ${-percent}%`;
}

/** 取整的数，null 时为「—」。 */
export function whole(value: number | null): string {
  return value === null ? "—" : String(Math.round(value));
}

/** 把 0–1 的比率写成整数百分比的数字部分，null 时为「—」。 */
export function percent(rate: number | null): string {
  return rate === null ? "—" : String(Math.round(rate * 100));
}

/** 把 0–1 的比率写成保留一位小数的百分比（`7.4`），null 时为「—」。 */
export function percentTenths(rate: number | null): string {
  return rate === null ? "—" : tenths(rate * 100);
}

/** 保留一位小数（`2.3`），null 时为「—」。 */
export function decimal(value: number | null): string {
  return value === null ? "—" : tenths(value);
}

/** 平均速度与上周相比；两周都有速度时才有值，否则为 null。 */
export function speedDelta(current: number | null, previous: number | null): string | null {
  if (current === null || previous === null) return null;
  const difference = Math.round(current) - Math.round(previous);
  if (difference === 0) return "和上周一样快";
  return difference > 0 ? `比上周快 ${difference} 字` : `比上周慢 ${-difference} 字`;
}

/** 每字按键数与上周相比；两周都有值时才有值，否则为 null。 */
export function perKeyDelta(current: number | null, previous: number | null): string | null {
  if (current === null || previous === null) return null;
  const difference = Math.round(current * 10) - Math.round(previous * 10);
  if (difference === 0) return "和上周持平";
  const amount = tenths(Math.abs(difference) / 10);
  return difference < 0 ? `比上周少 ${amount} 次` : `比上周多 ${amount} 次`;
}

const hoursPerDay = 24;

function period(hour: number): string {
  if (hour < 5) return "凌晨";
  if (hour < 8) return "早上";
  if (hour < 11) return "上午";
  if (hour < 13) return "中午";
  if (hour < 18) return "下午";
  if (hour < 19) return "傍晚";
  return "晚上";
}

function clock(hour: number): number {
  if (hour === 0) return 12;
  return hour > 12 ? hour - 12 : hour;
}

/** 用文字说出最忙的两小时，如 `晚上 9–11 点`；没有高峰时为 null。 */
export function peakLabel(window: PeakWindow | null): string | null {
  if (!window) return null;
  const start = ((window.start % hoursPerDay) + hoursPerDay) % hoursPerDay;
  const end = ((window.end % hoursPerDay) + hoursPerDay) % hoursPerDay;
  return `${period(start)} ${clock(start)}–${clock(end)} 点`;
}

/** 把 `2026-09-28` 写成 `9 月 28 日`；其他输入原样返回。 */
export function monthDay(day: string): string {
  const match = /^\d{4}-(\d{2})-(\d{2})$/.exec(day);
  return match ? `${Number(match[1])} 月 ${Number(match[2])} 日` : day;
}

/** `YYYY-MM-DD` 本地日是星期几，`一` … `日`；日期格式不对时为空串。 */
export function weekdayLabel(day: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  if (!match) return "";
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return "日一二三四五六"[date.getDay()];
}

export function unlocked(badge: AchievementSummary): boolean {
  return badge.unlocked_day !== null;
}

/** 0 到 1 的进度；已解锁的徽章总是满的。 */
export function progress(badge: AchievementSummary): number {
  if (unlocked(badge)) return 1;
  if (badge.target <= 0) return 0;
  return clamp(badge.current / badge.target, 0, 1);
}

/** 进度环里的百分比，向下取整，差一点才达成的徽章不会显示 100%。 */
export function progressLabel(badge: AchievementSummary): string {
  return `${Math.floor(progress(badge) * 100)}%`;
}

/** 每个徽章进度的计数单位；无法用「还差 N」描述的门槛（速度、准确率、早起鸟）为 null。 */
function progressUnit(id: string): string | null {
  switch (id) {
    case "chars_10k":
    case "chars_100k":
    case "chars_1m":
    case "night_owl":
    case "shuangpin_10k":
    case "handwriting_500":
      return "字";
    case "streak_7":
    case "streak_30":
    case "streak_100":
      return "天";
    case "sentence_1000":
      return "次";
    case "voice_1h":
      return "分钟";
    case "words_50":
      return "个词";
    case "skins_5":
      return "款皮肤";
    default:
      return null;
  }
}

/** 字数，一万及以上写成 `51.7 万字`。 */
function characters(count: number): string {
  return count >= 10_000 ? `${tenths(count / 10_000)} 万字` : `${count} 字`;
}

/** 徽章下面那一行：解锁后是它的描述；否则能说清还差多少就说还差多少，说不清仍用描述。 */
export function caption(badge: AchievementSummary): string {
  if (unlocked(badge)) return badge.description;
  const missing = Math.max(0, badge.target - badge.current);
  const unit = progressUnit(badge.id);
  if (unit === null || missing === 0) return badge.description;
  if (unit === "字") return `还差 ${characters(missing)}`;
  return `还差 ${missing} ${unit}`;
}

/** 点按徽章时显示的 toast。 */
export function badgeToast(badge: AchievementSummary): string {
  return unlocked(badge)
    ? `已解锁「${badge.title}」· ${badge.description}`
    : `「${badge.title}」· ${caption(badge)}`;
}

/** `count / total` 的整数百分比数字；total 为零时为 `0`。 */
export function share(count: number, total: number): string {
  return total <= 0 ? "0" : String(Math.round((count * 100) / total));
}

export function shareTotal(shares: readonly { count: number }[]): number {
  return shares.reduce((sum, item) => sum + item.count, 0);
}
