import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useConfirm } from "../core/confirm";
import {
  keyboardHeatmapModel,
  keyHeatLevel,
  scopedKeyCounts,
  type DailyKeyCounts,
  type KeyboardHeatmapKey,
} from "./keyboard-heatmap";
import { scopedBreakdown } from "./typing-breakdown";
import { DONUT_OUTER, DONUT_THICKNESS, donutSegments } from "./typing-chart";
import { SelectSettingField } from "./select-setting-field";
import { SettingToggle } from "./setting-toggle";
import { ActionButton } from "./action-button";
import { ErrorAlert } from "../core/error-alert";
import { StatusMessage } from "../core/status-message";
import { SettingsEmptyMessage } from "./settings-empty-message";
import {
  charactersPerMinute,
  readableCharacters,
  withUnknown,
  type TypingBreakdown,
} from "./typing-speed";
export type { TypingBreakdown } from "./typing-speed";
import {
  dayLabel,
  currentStreak,
  formatActiveTime,
  longestStreak,
  mobileTrendLength,
  recentDays,
  statisticsHeatmapWeeks,
  sumStatisticValues,
} from "./typing-statistics-helpers";
export {
  addDays,
  currentStreak,
  formatActiveTime,
  longestStreak,
} from "./typing-statistics-helpers";

const heading = "m-0 [font-size:var(--p-row-fs)] font-semibold [color:var(--p-text)]";
// 摘要在标签行上方，切换标签时保持不动，所以压成紧凑的 3 × 2 网格，单位和数字放在同一行；会随标签变化的内容都放在标签下方。
const metricGrid = "grid grid-cols-3 gap-x-6 gap-y-[18px] max-phone:grid-cols-2 max-phone:gap-x-3";
const metric = "flex min-w-0 flex-col gap-1 [&>span]:text-xs [&>span]:[color:var(--p-sub)]";
const metricLine =
  "flex min-w-0 flex-wrap items-baseline gap-x-1 [&>small]:text-xs [&>small]:[color:var(--p-sub)]";
const metricValue =
  "text-[22px] font-[650] leading-tight break-anywhere tabular-nums [color:var(--p-accent-text)]";
const metricNote = "text-xs [color:var(--p-sub)]";
const footerNote = "mt-3.5 mb-0 text-xs leading-relaxed [color:var(--p-sub)]";
const privacy = "mt-4 mb-0 text-xs leading-[1.7] [color:var(--p-sub)]";
const overviewPollMs = 5_000;
const overviewMinIntervalMs = 1_000;
const overviewStaleMs = 15_000;
// The phone's overflow menu: a details/summary disclosure, because it closes on an outside tap
// without any state to keep in sync.
const menuSummary =
  "grid h-[30px] w-[34px] cursor-pointer list-none place-items-center rounded-[9px] border border-edge bg-card text-xl leading-none text-secondary hover:bg-[var(--button-secondary-hover)] hover:text-body [&::-webkit-details-marker]:hidden";
const menuPopover =
  "absolute top-9 right-0 flex min-w-[180px] flex-col gap-[3px] rounded-[10px] border border-edge bg-card p-1.5 shadow-card";
const menuItem =
  "flex min-h-[34px] w-full items-center justify-between gap-3 rounded-[7px] border-0 bg-transparent px-[9px] py-1.5 text-left text-body not-disabled:hover:bg-[var(--button-secondary-hover)]";
// The page container. Named because this component returns it from three places -- loading, error and
// loaded -- and the loading branch was the one that got left behind when the class it used was
// replaced.
const page = "flex flex-col gap-3.5 max-phone:gap-2.5";
const rankChart = "mt-4 mb-[18px] flex flex-col gap-[11px]";
// The first column has to hold the longest label without the row's ellipsis cutting it. The count and share columns are fixed rather than auto because every row is its own grid: sized to their own content, a seven-digit count beside a two-digit one would start each track at a different x.
const rankRow =
  "grid grid-cols-[minmax(88px,1fr)_minmax(80px,2fr)_72px_48px] items-center gap-[9px] text-xs [&>span]:overflow-hidden [&>span]:text-ellipsis [&>span]:whitespace-nowrap [&>strong]:text-right [&>strong]:tabular-nums [&>strong]:text-secondary [&>small]:text-right [&>small]:tabular-nums [&>small]:text-muted";
const rankTrack = "h-[11px] overflow-hidden rounded-full bg-subtle";
const legendRow =
  "grid animate-row-reveal grid-cols-[22px_minmax(0,1fr)_auto_58px] items-center gap-[9px] motion-reduce:animate-none max-phone:grid-cols-[22px_minmax(0,1fr)_auto_50px] max-phone:gap-[7px] [&>strong]:font-medium [&>strong]:tabular-nums [&>small]:m-0 [&>small]:text-right [&>small]:tabular-nums";
const legendDot =
  "grid size-[22px] place-items-center rounded-[7px] text-xs font-[650] leading-none";

// The heatmap's five shades are one accent at five opacities, indexed by level, so the level a day
// falls into picks its class directly. Level 0 is the empty track rather than a faint accent.
const heatLevels = [
  "bg-subtle",
  "bg-accent opacity-[0.28]",
  "bg-accent opacity-[0.46]",
  "bg-accent opacity-[0.68]",
  "bg-accent",
] as const;
const heatCell = "block size-[14px] box-border rounded-[3px] border-0 p-0";
// A week is a column of seven fixed-height rows; the weekday gutter uses the same row track so the
// labels line up with the cells beside them.
const heatWeek = "grid grid-rows-[repeat(7,14px)] gap-[3px]";
const bar = "block w-full min-h-0.5 rounded-t-[3px] rounded-b-[1px]";
const axis = "mt-[7px] flex justify-between text-xs text-muted";

/** Shared selection semantics for the statistics charts; each chart supplies its own face and contents. */
function StatisticsChartButton({
  className,
  title,
  ariaLabel,
  selected,
  onClick,
  children,
}: {
  className: string;
  title: string;
  ariaLabel: string;
  selected: boolean;
  onClick: () => void;
  children?: ReactNode;
}) {
  return (
    <button
      type="button"
      className={className}
      title={title}
      aria-label={ariaLabel}
      aria-pressed={selected}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

// The segmented control behind the content tabs. The column count is a parameter because the tab row silently kept four columns after a fifth tab was added -- the extra one wrapped onto a second row at a quarter width. Each count is spelled out so Tailwind sees the class.
const segmentedColumns: Record<number, string> = {
  5: "grid-cols-5",
  6: "grid-cols-6",
};
const segmented = (columns: number) =>
  `grid gap-[3px] rounded-[9px] bg-subtle p-[3px] ${segmentedColumns[columns]} [&>button]:min-h-[34px] [&>button]:rounded-[7px] [&>button]:border-0 [&>button]:bg-transparent [&>button]:text-secondary [&>button[aria-selected=true]]:bg-raised [&>button[aria-selected=true]]:text-body [&>button[aria-selected=true]]:shadow-card`;

export type SelectionCounts = {
  /** Commits from positions 1..9, index 0 being the first candidate. */
  ranks?: number[];
  /** Commits from further down the list than the first page. */
  beyond?: number;
};

/** How long recorded days are kept. Anything else a host sends is read as "forever". */
export type StatisticsRetention = "forever" | "30d" | "90d" | "180d" | "365d";

export const retentionChoices: [StatisticsRetention, string][] = [
  ["forever", "永久保留"],
  ["30d", "保留最近 30 天"],
  ["90d", "保留最近 90 天"],
  ["180d", "保留最近 180 天"],
  ["365d", "保留最近 365 天"],
];

export type TypingStatistics = {
  enabled: boolean;
  /** Absent in statistics written before automatic cleanup existed, which means "forever". */
  retention?: StatisticsRetention;
  total: number;
  days: Record<string, number>;
  detail?: Partial<TypingBreakdown>;
  dailyDetails?: Record<string, Partial<TypingBreakdown>>;
  /** Absent in statistics written before candidate positions were counted. */
  selections?: SelectionCounts;
  /**
   * Active typing time per day, in milliseconds. A day is absent when it predates the
   * measurement, which is not the same as zero: it means unknown, and every metric divided by it
   * has to leave that day out rather than read it as instant.
   */
  dailyActiveMs?: Record<string, number>;
  /** Characters per local hour, 24 buckets per day. Absent for days the host sent no hour for. */
  dailyHours?: Record<string, number[]>;
  /** Presses per key per local day, keyed by `KeyboardEvent.code` or soft-keyboard id. Absent in statistics written before keys were counted. */
  dailyKeys?: DailyKeyCounts;
};

export type TypingStatisticsStatus = {
  statistics: TypingStatistics;
  availability: "ready" | "neverWritten";
  lastWrittenMs?: number | null;
};

export interface TypingStatisticsClient {
  load(): Promise<TypingStatisticsStatus>;
  setEnabled(enabled: boolean): Promise<TypingStatisticsStatus>;
  /** Absent on hosts that do not keep the statistics themselves. */
  setRetention?(retention: StatisticsRetention): Promise<TypingStatisticsStatus>;
  /** Absent where a file manager is not reachable - iOS and Android render no button rather than a dead one. */
  openDirectory?(): Promise<void>;
  reset(): Promise<TypingStatisticsStatus>;
}

/** Days the desktop trend bars cover. */
const DESKTOP_TREND_DAYS = 30;
type Slice = { id: string; title: string; count: number; color: string; symbol: string };

const palette = [
  "#19a78d",
  "#4c8ee8",
  "#7772df",
  "#e59b43",
  "#db6f9f",
  "#a879d7",
  "#98705a",
  "#888b92",
];
const characterKinds = [
  ["han", "汉字"],
  ["latin", "拉丁字母"],
  ["otherLetter", "其他文字"],
  ["number", "数字"],
  ["punctuation", "标点"],
  ["emoji", "表情"],
  ["symbol", "其他符号"],
  ["unknown", "历史未分类"],
] as const;
const sources = [
  ["quanpin", "全拼 26 键"],
  ["nineKey", "全拼 9 键"],
  ["shuangpin", "小鹤双拼"],
  ["ziranma", "自然码双拼"],
  ["microsoft", "微软双拼"],
  ["shoudao", "首道双拼"],
  ["wubi", "五笔"],
  ["japanese", "日语"],
  ["korean", "韩语"],
  ["cantonese", "粤拼"],
  ["zhuyin", "注音"],
  ["vietnamese", "越南语"],
  ["tibetan", "藏文"],
  ["stroke", "笔画"],
  ["handwriting", "手写"],
  ["english", "英文键盘"],
  ["local", "本地输入"],
  ["ai", "AI 润色"],
  ["reply", "高情商回复"],
  ["voice", "语音输入"],
  ["unknown", "历史未分类"],
] as const;
const characterSymbols: Record<string, string> = {
  han: "汉",
  latin: "A",
  otherLetter: "文",
  number: "123",
  punctuation: "，",
  emoji: "😀",
  symbol: "#",
  unknown: "?",
};
const sourceSymbols: Record<string, string> = {
  quanpin: "全",
  nineKey: "9",
  shuangpin: "鹤",
  ziranma: "自",
  microsoft: "微",
  shoudao: "首",
  wubi: "五",
  japanese: "日",
  korean: "韩",
  cantonese: "粤",
  zhuyin: "注",
  vietnamese: "越",
  tibetan: "藏",
  stroke: "笔",
  handwriting: "手",
  english: "A",
  local: "本",
  ai: "✦",
  reply: "回",
  voice: "♪",
  unknown: "?",
};

/** Buckets in a day, matching the shared store. */
export const HOURS = 24;
/**
 * Character classes the speed metric counts.
 *
 * Speed is characters per active minute, and digits, punctuation, emoji and symbols are not prose:
 * counting them reads as a burst of speed for someone entering a phone number. This differs from
 * the Windows baseline in one place on purpose - there `latin` is ASCII letters only and kana fall
 * into `other`, so Japanese input measures as zero speed. This product has a full Japanese mode,
 * so `otherLetter` (kana, hangul, and every other script that is not Han or Latin) counts too.
 */
/**
 * Offsets a `YYYY-MM-DD` key by whole days.
 *
 * Through UTC deliberately: the keys are calendar labels rather than instants, and local-time
 * arithmetic would lose or repeat a day at a daylight-saving boundary - which on those two days a
 * year would break a streak that was never broken.
 */
/**
 * A day needs this much active time before it can win "fastest day".
 *
 * Without it a day holding a dozen characters typed in two seconds tops the ranking forever.
 */
const fastestDayMinimumActiveMs = 60_000;

export type ActivityMetrics = {
  /** Days that have any record. Days with no record are not rows and never enter an average. */
  recordedDays: number;
  averagePerDay: number;
  todayActiveMs: number;
  totalActiveMs: number;
  todaySpeed: number;
  averageSpeed: number;
  fastestSpeed: number;
  fastestDay: string | null;
  currentStreak: number;
  longestStreak: number;
  bestDay: string | null;
  bestDayCharacters: number;
  /** Today's 24 hourly buckets, or null when the host recorded no hours for today. */
  todayHours: number[] | null;
  /** False when nothing has ever measured active time, which is not the same as zero speed. */
  hasActivity: boolean;
};

/**
 * Everything the rhythm cards show, derived in one place so the markup only formats.
 *
 * `todayKey` is passed in rather than read from the clock so the result is a function of its
 * arguments alone.
 */
export function activityMetrics(statistics: TypingStatistics, todayKey: string): ActivityMetrics {
  const recorded = Object.keys(statistics.days)
    .filter((key) => /^\d{4}-\d{2}-\d{2}$/.test(key))
    .sort();
  const activeByDay = statistics.dailyActiveMs ?? {};
  let totalActiveMs = 0;
  let totalReadable = 0;
  let fastestSpeed = 0;
  let fastestDay: string | null = null;
  let bestDay: string | null = null;
  let bestDayCharacters = 0;
  // The baseline's average is the sum of its day rows over the row count, so it divides what the recorded days hold rather than `total`, which the host's validation allows to run above them.
  let recordedCharacters = 0;
  for (const key of recorded) {
    const activeMs = activeByDay[key] ?? 0;
    const readable = readableCharacters(statistics.dailyDetails?.[key]);
    if (activeMs > 0) {
      totalActiveMs += activeMs;
      totalReadable += readable;
      if (activeMs >= fastestDayMinimumActiveMs) {
        const speed = charactersPerMinute(readable, activeMs);
        // Strictly greater, so the earliest day keeps a tie - `recorded` is sorted ascending.
        if (speed > fastestSpeed) {
          fastestSpeed = speed;
          fastestDay = key;
        }
      }
    }
    const characters = statistics.days[key] ?? 0;
    recordedCharacters += characters;
    if (characters > bestDayCharacters) {
      bestDayCharacters = characters;
      bestDay = key;
    }
  }
  const todayHours = statistics.dailyHours?.[todayKey];
  return {
    recordedDays: recorded.length,
    averagePerDay: recorded.length === 0 ? 0 : recordedCharacters / recorded.length,
    todayActiveMs: activeByDay[todayKey] ?? 0,
    totalActiveMs,
    todaySpeed: charactersPerMinute(
      readableCharacters(statistics.dailyDetails?.[todayKey]),
      activeByDay[todayKey] ?? 0,
    ),
    averageSpeed: charactersPerMinute(totalReadable, totalActiveMs),
    fastestSpeed,
    fastestDay,
    currentStreak: currentStreak(recorded, todayKey),
    longestStreak: longestStreak(recorded),
    bestDay,
    bestDayCharacters,
    todayHours: Array.isArray(todayHours) && todayHours.length === HOURS ? todayHours : null,
    hasActivity: totalActiveMs > 0,
  };
}

/**
 * The average hour-by-hour profile of the days before today, the baseline the hourly chart sets today against.
 *
 * Only days with a full 24-bucket record and at least one character take part: a day the host recorded no hours for is unknown, and averaging it in as zeros would flatten the profile. Null when no such day exists.
 */
export function usualHours(
  dailyHours: TypingStatistics["dailyHours"],
  todayKey: string,
): { hours: number[]; days: number } | null {
  const sums = Array.from({ length: HOURS }, () => 0);
  let days = 0;
  for (const [key, hours] of Object.entries(dailyHours ?? {})) {
    if (key >= todayKey || !Array.isArray(hours) || hours.length !== HOURS) continue;
    if (!hours.some((count) => count > 0)) continue;
    hours.forEach((count, hour) => (sums[hour] += count));
    days += 1;
  }
  return days === 0 ? null : { hours: sums.map((sum) => sum / days), days };
}

/**
 * Characters per active minute for each of `keys`, or null for a day whose speed is unknown.
 *
 * A day needs the same minimum active time as "fastest day" before it gets a point, so a few characters typed in two seconds do not draw a spike that flattens every real day beside it.
 */
export function dailySpeeds(
  statistics: TypingStatistics,
  keys: readonly string[],
): (number | null)[] {
  return keys.map((key) => {
    const activeMs = statistics.dailyActiveMs?.[key] ?? 0;
    if (activeMs < fastestDayMinimumActiveMs) return null;
    return charactersPerMinute(readableCharacters(statistics.dailyDetails?.[key]), activeMs);
  });
}

/**
 * Consecutive recorded days ending today, or ending yesterday when today has no record yet.
 *
 * Today is still in progress, so it must not reset a streak the user has not actually broken.
 */
/** `9月21日` from a `YYYY-MM-DD` key, matching the labels the trend axis uses. */
/** How many recorded days the per-day detail table lists, as in the Windows source's `DETAIL_DAYS`. */
export const DETAIL_DAYS = 30;

export type DailyDetailRow = {
  key: string;
  total: number;
  han: number;
  latin: number;
  number: number;
  punctuation: number;
  /** Other scripts, emoji, symbols and the unclassified remainder a day's breakdown does not cover. */
  other: number;
  /** Null when the day predates active-time measurement, which is unknown rather than zero. */
  activeMs: number | null;
  /** Null exactly when `activeMs` is, so an unmeasured day never reads as zero speed. */
  speed: number | null;
};

/**
 * The per-day detail table's rows: the most recent recorded days up to today, newest first.
 *
 * Only recorded days become rows, like the source's `detailRows(overview.daily, DETAIL_DAYS)`, which takes the last rows of the recorded daily series and reverses them. The source's cjk/latin/digit/punct/other columns map onto this product's finer classes, with every class the source has no column for folded into `other`.
 */
export function dailyDetailRows(
  statistics: TypingStatistics,
  todayKey: string,
  days = DETAIL_DAYS,
): DailyDetailRow[] {
  const keys = Object.keys(statistics.days)
    .filter((key) => /^\d{4}-\d{2}-\d{2}$/.test(key) && key <= todayKey)
    .sort()
    .slice(-days)
    .reverse();
  return keys.map((key) => {
    const total = statistics.days[key] ?? 0;
    const detail = statistics.dailyDetails?.[key];
    const characters = withUnknown(detail, total).characters;
    const han = characters.han ?? 0;
    const latin = characters.latin ?? 0;
    const number = characters.number ?? 0;
    const punctuation = characters.punctuation ?? 0;
    const classified = Object.values(characters).reduce((sum, value) => sum + value, 0);
    const recordedActive = statistics.dailyActiveMs?.[key];
    const activeMs = typeof recordedActive === "number" ? recordedActive : null;
    return {
      key,
      total,
      han,
      latin,
      number,
      punctuation,
      other: Math.max(0, classified - han - latin - number - punctuation),
      activeMs,
      speed: activeMs === null ? null : charactersPerMinute(readableCharacters(detail), activeMs),
    };
  });
}

/** The table's columns: 汉字/字母 rather than the source's 中文/英文, because here kana and other scripts are neither and sit in 其他. */
const detailColumns = [
  "日期",
  "字数",
  "汉字",
  "字母",
  "数字",
  "标点",
  "其他",
  "活跃",
  "速度",
] as const;

function DailyDetails({ rows }: { rows: DailyDetailRow[] }) {
  const count = (value: number) => value.toLocaleString("zh-CN");
  return (
    <section className="section m-0" aria-labelledby="statistics-details-title">
      <h2 className={heading} id="statistics-details-title">
        按日明细 · 最近 {DETAIL_DAYS} 天
      </h2>
      {rows.length === 0 ? (
        <SettingsEmptyMessage centered className="mt-3.5">
          暂无输入记录
        </SettingsEmptyMessage>
      ) : (
        <div className="mt-3 overflow-x-auto">
          <table
            className="w-full border-collapse text-xs tabular-nums [&_td]:border-t [&_td]:border-edge [&_td]:px-2 [&_td]:py-1.5 [&_td]:text-right [&_td]:whitespace-nowrap [&_td:first-child]:text-left [&_th]:px-2 [&_th]:pb-1.5 [&_th]:text-right [&_th]:font-medium [&_th]:whitespace-nowrap [&_th]:text-muted [&_th:first-child]:text-left"
            aria-labelledby="statistics-details-title"
          >
            <thead>
              <tr>
                {detailColumns.map((title) => (
                  <th scope="col" key={title}>
                    {title}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.key}>
                  <td>{dayLabel(row.key)}</td>
                  <td className="font-medium text-body">{count(row.total)}</td>
                  {[row.han, row.latin, row.number, row.punctuation, row.other].map(
                    (value, index) => (
                      <td className="text-secondary" key={index}>
                        {count(value)}
                      </td>
                    ),
                  )}
                  <td>{row.activeMs === null ? "—" : formatActiveTime(row.activeMs)}</td>
                  <td>{row.speed === null ? "—" : `${count(Math.round(row.speed))} 字/分钟`}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className={footerNote}>
        列出最近 {DETAIL_DAYS}{" "}
        个有记录的日期，新的在上；「其他」含其他文字、表情、符号与历史未分类。活跃与速度显示「—」的日期早于活跃时长的记录。
      </p>
    </section>
  );
}

function StatisticsHeatmap({
  days,
  selectedDay,
  onSelect,
}: {
  days: Record<string, number>;
  selectedDay: string | null;
  onSelect: (key: string) => void;
}) {
  const weeks = useMemo(() => statisticsHeatmapWeeks(days), [days]);
  const maximum = Math.max(1, ...weeks.flat().map((day) => day.count));
  // A month is labelled only on the column holding its 1st, so each label sits over the column where that month begins.
  const monthLabels = weeks.map((week) => {
    const first = week.find((day) => day.key.endsWith("-01"));
    return first ? `${first.label.split("月")[0]}月` : "";
  });
  return (
    <div className="mt-2" role="group" aria-label="每日输入热力图">
      <div className="overflow-x-auto pb-1 [scrollbar-width:thin]">
        <div
          className="flex h-4 w-max min-w-full items-end text-[9px] text-muted [&>span:first-child]:w-[18px] [&>span:first-child]:flex-[0_0_18px] [&>span:not(:first-child)]:mr-[3px] [&>span:not(:first-child)]:w-[14px] [&>span:not(:first-child)]:overflow-visible [&>span:not(:first-child)]:whitespace-nowrap"
          aria-hidden="true"
        >
          <span />
          {monthLabels.map((label, index) => (
            <span key={index}>{label}</span>
          ))}
        </div>
        <div className="flex w-max min-w-full gap-1">
          <div
            className={`${heatWeek} w-[18px] flex-[0_0_18px] text-right text-[9px] leading-[14px] text-muted`}
            aria-hidden="true"
          >
            {["一", "", "三", "", "五", "", ""].map((label, index) => (
              <span key={index}>{label}</span>
            ))}
          </div>
          <div className="flex gap-[3px]" role="grid">
            {weeks.map((week, index) => (
              <div className={heatWeek} key={index}>
                {week.map((day) => {
                  if (day.future)
                    return (
                      <span className={`${heatCell} invisible`} key={day.key} aria-hidden="true" />
                    );
                  const level =
                    day.count === 0 ? 0 : Math.max(1, Math.ceil((day.count / maximum) * 4));
                  return (
                    <StatisticsChartButton
                      className={`${heatCell} ${heatLevels[level]} cursor-pointer${selectedDay === day.key ? " outline-2 outline-offset-1 outline-[#e59b43]" : ""}`}
                      key={day.key}
                      title={`${day.label}：${day.count > 0 ? `${day.count.toLocaleString("zh-CN")} 字符` : "无记录"}`}
                      ariaLabel={`热力图：${day.label}，${day.count} 字符`}
                      selected={selectedDay === day.key}
                      onClick={() => onSelect(day.key)}
                    />
                  );
                })}
              </div>
            ))}
          </div>
        </div>
      </div>
      <div className="mt-2 flex items-center gap-1 text-[10px] text-muted" aria-hidden="true">
        <span>少</span>
        {heatLevels.map((shade, level) => (
          <i className={`block size-3 rounded-[2px] ${shade}`} key={level} />
        ))}
        <span>多</span>
      </div>
    </div>
  );
}

function KeyboardHeatmapRows({
  rows,
  maximum,
  className,
}: {
  rows: KeyboardHeatmapKey[][];
  maximum: number;
  className: string;
}) {
  return (
    <div className={className}>
      {rows.map((row, rowIndex) => (
        <div className="flex gap-1" key={rowIndex}>
          {row.map((key, index) => {
            const style = { flexGrow: key.weight, flexBasis: 0 };
            // An unlabelled entry is spacing that lines a row up with the one above, not a key.
            if (!key.code)
              return <span className="min-w-0" style={style} key={index} aria-hidden="true" />;
            const level = keyHeatLevel(key.count, maximum);
            const name = `${key.name}，${key.count.toLocaleString("zh-CN")} 次`;
            return (
              <span
                role="img"
                className="relative grid h-9 min-w-0 place-items-center overflow-hidden rounded-[5px] border border-edge text-[11px] leading-none max-phone:h-8 max-phone:text-[10px]"
                style={style}
                key={index}
                title={name}
                aria-label={name}
              >
                <i className={`absolute inset-0 ${heatLevels[level]}`} aria-hidden="true" />
                <span
                  className={`relative overflow-hidden px-0.5 text-ellipsis whitespace-nowrap ${level >= 3 ? "[color:var(--p-on-accent,#fff)]" : "text-secondary"}`}
                  aria-hidden="true"
                >
                  {key.label}
                </span>
              </span>
            );
          })}
        </div>
      ))}
    </div>
  );
}

/**
 * How often each key was pressed in the page's scope, drawn on the keyboard the counts came from: the ANSI board for a physical keyboard, the phone's 26-key layout (plus the nine-key grid once one of its cells was pressed) for an on-screen one.
 */
function KeyboardHeatmap({
  dailyKeys,
  scopeKeys,
  scopeLabel,
  mobile,
  platform,
}: {
  dailyKeys: DailyKeyCounts | undefined;
  scopeKeys: string[] | null;
  scopeLabel: string;
  mobile: boolean;
  platform?: string;
}) {
  const model = keyboardHeatmapModel(scopedKeyCounts(dailyKeys, scopeKeys), mobile, platform);
  const count = (value: number) => value.toLocaleString("zh-CN");
  return (
    <section className="section m-0" aria-labelledby="statistics-keys-title">
      <h2 className={heading} id="statistics-keys-title">
        按键热力图 · {scopeLabel}
      </h2>
      {model.total === 0 ? (
        <SettingsEmptyMessage centered className="mt-3.5">
          这段时间还没有按键记录
        </SettingsEmptyMessage>
      ) : (
        <>
          <p className="mt-[7px] mb-0 text-xs text-muted">
            共 {count(model.total)} 次按键，颜色越深按得越多
          </p>
          <div role="group" aria-label="按键热力图">
            <KeyboardHeatmapRows
              rows={model.rows}
              maximum={model.maximum}
              className="mt-3 flex flex-col gap-1"
            />
            {model.nineRows && (
              <div role="group" aria-label="九宫格按键">
                <p className="mt-3.5 mb-0 text-xs text-muted">九宫格</p>
                <KeyboardHeatmapRows
                  rows={model.nineRows}
                  maximum={model.maximum}
                  className="mx-auto mt-2 flex max-w-[260px] flex-col gap-1"
                />
              </div>
            )}
            {model.others.length > 0 && (
              <div className="mt-3.5">
                <p className="m-0 text-xs text-muted" id="statistics-keys-others">
                  其他键
                </p>
                <ul
                  className="m-0 mt-2 flex list-none flex-wrap gap-1.5 p-0 text-xs"
                  aria-labelledby="statistics-keys-others"
                >
                  {model.others.map((key) => (
                    <li
                      className="flex items-center gap-1.5 rounded-[5px] border border-edge px-2 py-1"
                      key={key.code}
                      aria-label={`${key.label}，${count(key.count)} 次`}
                    >
                      <i
                        className={`block size-2.5 rounded-[2px] ${heatLevels[keyHeatLevel(key.count, model.maximum)]}`}
                        aria-hidden="true"
                      />
                      <span aria-hidden="true">{key.label}</span>
                      <strong
                        className="font-medium tabular-nums text-secondary"
                        aria-hidden="true"
                      >
                        {count(key.count)}
                      </strong>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
          <div className="mt-2 flex items-center gap-1 text-[10px] text-muted" aria-hidden="true">
            <span>少</span>
            {heatLevels.map((shade, level) => (
              <i className={`block size-3 rounded-[2px] ${shade}`} key={level} />
            ))}
            <span>多</span>
          </div>
          <ol
            className="m-0 mt-3.5 flex list-none flex-col gap-1.5 p-0 text-xs"
            aria-label="最常按的键"
          >
            {model.top.map((key, index) => (
              <li className="flex items-center gap-2" key={key.code}>
                <span className="w-4 text-right tabular-nums text-muted">{index + 1}</span>
                <span className="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
                  {key.label}
                </span>
                <strong className="font-medium tabular-nums text-secondary">
                  {count(key.count)} 次
                </strong>
              </li>
            ))}
          </ol>
        </>
      )}
      <p className={footerNote}>
        只保存每个键每天被按下的次数，不保存按键顺序和输入内容；按住不放只算一次，密码框中的按键不计入。
      </p>
    </section>
  );
}

/** Horizontal position, in viewBox units, of the middle of column `index` of `count`, so a line drawn over a row of bars passes through each bar's centre. */
const columnCenter = (index: number, count: number) => ((index + 0.5) / Math.max(1, count)) * 100;

/** An SVG path through the known values, lifting the pen over every null so an unknown day reads as a gap rather than a dip to zero. */
function linePath(values: readonly (number | null)[], y: (value: number) => number): string {
  let path = "";
  let drawing = false;
  values.forEach((value, index) => {
    if (value === null) {
      drawing = false;
      return;
    }
    path += `${drawing ? "L" : "M"}${columnCenter(index, values.length)},${y(value)} `;
    drawing = true;
  });
  return path.trim();
}

/**
 * A dashed reference line across a plot. Dashed on purpose: it marks a threshold, not a grid. Its value is named in the caption above the plot rather than on the line, where a label would sit on top of the data.
 */
function ReferenceLine({ bottom }: { bottom: number }) {
  return (
    <i
      className="pointer-events-none absolute inset-x-0 block h-px bg-[repeating-linear-gradient(90deg,var(--text-muted)_0_4px,transparent_4px_8px)] opacity-70"
      style={{ bottom: `${bottom}%` }}
      aria-hidden="true"
    />
  );
}

/**
 * Characters per active minute for each day of the trend window.
 *
 * Speed is a rate, so it gets a line rather than bars, and the scale starts near the slowest day instead of at zero: a typist moving between 60 and 75 characters a minute would otherwise see a flat line at the top of the plot. The axis labels name both ends so the cropped scale is never hidden. Days without enough measured time are gaps.
 */
function SpeedTrend({
  days,
  speeds,
  averageSpeed,
  selectedDay,
}: {
  days: { key: string; label: string }[];
  speeds: (number | null)[];
  averageSpeed: number;
  selectedDay: string | null;
}) {
  const known = speeds.filter((speed): speed is number => speed !== null);
  const count = (value: number) => Math.round(value).toLocaleString("zh-CN");
  if (known.length === 0)
    return (
      <SettingsEmptyMessage centered className="mt-3.5">
        这段时间还没有测量到足够的活跃时长。每天连续打字满 1 分钟后，这里会画出当天的速度。
      </SettingsEmptyMessage>
    );
  const high = Math.max(...known, averageSpeed);
  const low = Math.min(...known, averageSpeed);
  const spread = Math.max(high - low, high * 0.2, 1);
  const top = Math.ceil(high + spread * 0.15);
  const bottom = Math.max(0, Math.floor(low - spread * 0.25));
  const height = (value: number) => ((value - bottom) / Math.max(1, top - bottom)) * 100;
  const y = (value: number) => 100 - height(value);
  // Every point gets a dot while they are far enough apart to read; past that only a point with no neighbour does, since a lone day draws no line at all.
  const dotted = (index: number) =>
    speeds[index] !== null &&
    (days.length <= 45 || (speeds[index - 1] == null && speeds[index + 1] == null));
  return (
    <>
      <p className="mt-[7px] mb-0 text-xs text-muted">
        最快 {count(Math.max(...known))} · 最慢 {count(Math.min(...known))} · 虚线为平均{" "}
        {count(averageSpeed)} 字 / 分钟 · {known.length} 天有测量
      </p>
      <div
        className="relative mt-3 h-[120px] border-b border-edge text-accent"
        role="img"
        aria-label={`每日输入速度折线图，${known.length} 天有测量，平均 ${count(averageSpeed)} 字每分钟`}
      >
        {/* The scale does not start at zero, so both ends are labelled inside the plot, where they keep its columns aligned with the daily bars above. */}
        <span
          className="pointer-events-none absolute top-0 left-0 text-[10px] leading-none text-muted tabular-nums"
          aria-hidden="true"
        >
          {top}
        </span>
        <span
          className="pointer-events-none absolute bottom-1 left-0 text-[10px] leading-none text-muted tabular-nums"
          aria-hidden="true"
        >
          {bottom}
        </span>
        <ReferenceLine bottom={height(averageSpeed)} />
        <svg
          className="absolute inset-0 block size-full overflow-visible"
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          <path
            d={linePath(speeds, y)}
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
          />
        </svg>
        {/* Dots and hover columns are HTML rather than SVG: the plot stretches to its box, which would squash an SVG circle into an ellipse. */}
        {days.map((day, index) => {
          const speed = speeds[index];
          const chosen = selectedDay === day.key;
          return (
            <span
              className="absolute inset-y-0"
              style={{
                left: `${(index / days.length) * 100}%`,
                width: `${100 / days.length}%`,
              }}
              key={day.key}
              title={`${day.label}：${speed === null ? "活跃不足 1 分钟，未计算" : `${count(speed)} 字 / 分钟`}`}
              aria-hidden="true"
            >
              {speed !== null && (dotted(index) || chosen) && (
                <i
                  className={`absolute left-1/2 block -translate-x-1/2 translate-y-1/2 rounded-full ring-2 ring-card-solid ${chosen ? "size-2.5 bg-[#e59b43]" : "size-[7px] bg-accent"}`}
                  style={{ bottom: `${height(speed)}%` }}
                />
              )}
            </span>
          );
        })}
      </div>
      <div className={axis}>
        <span>{days[0]?.label}</span>
        <span>{days.at(-1)?.label}</span>
      </div>
    </>
  );
}

function StatisticsTrendLine({
  days,
  counts,
  selectedDay,
  average,
}: {
  days: { key: string; label: string }[];
  counts: Record<string, number>;
  selectedDay: string | null;
  average: number;
}) {
  const values = days.map((day) => counts[day.key] ?? 0);
  const lineValues =
    days.length > 120
      ? values.map((_, index) => {
          const start = Math.max(0, index - 6);
          const window = values.slice(start, index + 1);
          return window.reduce((total, value) => total + value, 0) / window.length;
        })
      : values;
  const maximum = Math.max(1, ...values, ...lineValues);
  const point = (value: number, index: number) =>
    `${(index / Math.max(1, days.length - 1)) * 100},${96 - (value / maximum) * 88}`;
  const line = lineValues.map(point).join(" ");
  const area = `0,100 ${values.map(point).join(" ")} 100,100`;
  const selectedIndex = selectedDay ? days.findIndex((day) => day.key === selectedDay) : -1;
  const selectedValue = selectedIndex >= 0 ? values[selectedIndex] : 0;
  return (
    <div
      className="relative mt-3 h-[170px] w-full text-accent"
      role="img"
      aria-label={days.length > 120 ? "每日输入趋势折线图，显示七日均线" : "每日输入趋势折线图"}
    >
      {average > 0 && <ReferenceLine bottom={4 + (average / maximum) * 88} />}
      <svg
        className="block size-full overflow-visible"
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <defs>
          <linearGradient id="statistics-trend-area" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="currentColor" stopOpacity=".34" />
            <stop offset="1" stopColor="currentColor" stopOpacity=".03" />
          </linearGradient>
        </defs>
        <polygon points={area} fill="url(#statistics-trend-area)" />
        <polyline
          points={line}
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          vectorEffect="non-scaling-stroke"
        />
        {selectedIndex >= 0 && (
          <circle
            className="fill-[#e59b43] stroke-card stroke-[1.5px]"
            cx={(selectedIndex / Math.max(1, days.length - 1)) * 100}
            cy={96 - (selectedValue / maximum) * 88}
            r="2.2"
            vectorEffect="non-scaling-stroke"
          />
        )}
      </svg>
    </div>
  );
}

/**
 * How a distribution is drawn. A donut is part-to-whole at a glance, which suits character types and language modes: a handful of classes with one usually dominant. Input schemes can number sixteen, past what hue can tell apart, so they are a ranking in one colour where the label carries identity.
 */
type DistributionVariant = "donut" | "rank";

const shareText = (count: number, total: number) =>
  total === 0 ? "—" : `${((count / total) * 100).toFixed(1)}%`;

function ShapeChart({
  title,
  slices,
  variant,
  total,
}: {
  title: string;
  slices: Slice[];
  variant: DistributionVariant;
  total: number;
}) {
  if (variant === "rank") {
    const ranked = slices.filter((slice) => slice.count > 0).sort((a, b) => b.count - a.count);
    const peak = Math.max(1, ...ranked.map((slice) => slice.count));
    return (
      <div className={rankChart} role="img" aria-label={`${title}排行`}>
        {ranked.map((slice, index) => (
          <div
            className={`${rankRow} animate-row-reveal motion-reduce:animate-none`}
            style={{ animationDelay: `${Math.min(index, 8) * 0.03}s` }}
            key={slice.id}
            aria-label={`${slice.title} ${slice.count} 字符，${shareText(slice.count, total)}`}
          >
            <span>{slice.title}</span>
            <div className={rankTrack}>
              <i
                className="block h-full origin-left animate-bar-reveal rounded-[inherit] bg-accent motion-reduce:animate-none"
                style={{ width: `${(slice.count / peak) * 100}%` }}
              />
            </div>
            <strong>{slice.count.toLocaleString("zh-CN")}</strong>
            <small>{shareText(slice.count, total)}</small>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div
      className="relative mx-auto size-[190px] shrink-0"
      role="img"
      aria-label={`${title}环形图`}
    >
      {/* 用 SVG 画环：环宽约为外半径的四分之一，扇区之间留底色缝隙，原先用 conic-gradient 加遮罩只能画出一道细线。 */}
      <svg className="block size-full" viewBox="0 0 100 100" aria-hidden="true">
        {donutSegments(slices).map((segment, index) =>
          "ring" in segment ? (
            <circle
              key={index}
              cx="50"
              cy="50"
              r={DONUT_OUTER - DONUT_THICKNESS / 2}
              fill="none"
              stroke={segment.color}
              strokeWidth={DONUT_THICKNESS}
              data-donut-segment=""
            />
          ) : (
            <path key={index} d={segment.d} fill={segment.color} data-donut-segment="" />
          ),
        )}
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <strong className="text-[25px] leading-tight font-semibold tabular-nums text-body">
          {total.toLocaleString("zh-CN")}
        </strong>
        <span className="text-[11px] text-muted">字符</span>
      </div>
    </div>
  );
}

function Distribution({
  title,
  slices,
  footer,
  variant,
  wide = false,
}: {
  title: string;
  slices: Slice[];
  footer?: string;
  variant: DistributionVariant;
  /** Sets a donut beside its legend instead of above it, where the page is wide enough. */
  wide?: boolean;
}) {
  const total = slices.reduce((value, slice) => value + slice.count, 0);
  const visible = slices.filter((slice) => slice.count > 0 || slice.id !== "unknown");
  return (
    <section className="section m-0 @container" aria-labelledby={`statistics-${title}`}>
      <h2 className={heading} id={`statistics-${title}`}>
        {title}
      </h2>
      {total === 0 ? (
        <SettingsEmptyMessage centered className="mt-3.5">
          暂无输入记录
        </SettingsEmptyMessage>
      ) : (
        // 标题独占一行，图表区整体放在标题下方；宽版按容器宽度而不是视口宽度决定是否并排，窄窗口里环形图回到图例上方，不再把图例挤成一字一行。
        <div
          className={
            variant === "donut"
              ? `mt-4 grid gap-5 ${wide ? "@min-[520px]:grid-cols-[190px_minmax(0,1fr)] @min-[520px]:items-center @min-[520px]:gap-8" : ""}`
              : undefined
          }
        >
          <ShapeChart title={title} slices={slices} variant={variant} total={total} />
          {/* A ranking already names, counts and shares every row, so it needs no legend. */}
          {variant === "donut" && (
            <div className="flex flex-col gap-[11px]">
              {visible.map((slice, index) => (
                <div
                  // The rows reveal in sequence. The stylesheet staggered them with eight :nth-child rules;
                  // the index is already here, so the delay comes from it and any row count works.
                  className={legendRow}
                  style={{ animationDelay: `${Math.min(index, 8) * 0.03}s` }}
                  key={slice.id}
                  aria-label={`${slice.title} ${slice.count} 字符，${shareText(slice.count, total)}`}
                >
                  <span
                    className={legendDot}
                    style={{ color: slice.color, backgroundColor: `${slice.color}1a` }}
                    aria-hidden="true"
                  >
                    {slice.symbol}
                  </span>
                  <span>{slice.title}</span>
                  <strong>{slice.count.toLocaleString("zh-CN")}</strong>
                  <small>{shareText(slice.count, total)}</small>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
      {footer && <p className={footerNote}>{footer}</p>}
    </section>
  );
}

/** Positions the first page holds; anything past it is counted together. */
const RANK_SLOTS = 9;

/**
 * How often each candidate position was the one committed.
 *
 * One series, so one colour and no legend — the heading names it. The bars stay in position order
 * and are never sorted by size: the whole point is the shape of the fall-off from the first
 * candidate, and ranking the ranks would destroy it. The mark colour sits below 3:1 against the
 * surface, so every row carries its count and share as text; the numbers are the relief, not
 * decoration.
 */
function CandidateRanks({ selections }: { selections: SelectionCounts | undefined }) {
  const ranks = Array.from({ length: RANK_SLOTS }, (_, index) => selections?.ranks?.[index] ?? 0);
  const beyond = selections?.beyond ?? 0;
  const total = ranks.reduce((sum, count) => sum + count, 0) + beyond;
  const peak = Math.max(1, ...ranks, beyond);
  const rows = [
    ...ranks.map((count, index) => ({
      id: `rank-${index + 1}`,
      label: `第 ${index + 1} 条`,
      count,
    })),
    { id: "beyond", label: "第 10 条以后", count: beyond },
  ];
  const share = (count: number) => (total === 0 ? "—" : `${((count / total) * 100).toFixed(1)}%`);
  return (
    <section className="section m-0" aria-labelledby="statistics-candidate-ranks">
      <h2 className={heading} id="statistics-candidate-ranks">
        候选命中位置
      </h2>
      <p className="mt-3.5 mb-1 flex items-baseline gap-2">
        <strong className="text-[30px] leading-[1.1] tabular-nums" aria-label="首选命中率">
          {total === 0 ? "—" : `${((ranks[0] / total) * 100).toFixed(1)}%`}
        </strong>
        <span className="text-[13px] text-secondary" aria-hidden="true">
          首选命中率
        </span>
        <small className="ml-auto text-xs text-muted">
          {total === 0 ? "暂无记录" : `共 ${total.toLocaleString("zh-CN")} 次上屏`}
        </small>
      </p>
      {total === 0 ? (
        <SettingsEmptyMessage centered>
          暂无候选记录。用水杉键盘上屏几次后再回来查看。
        </SettingsEmptyMessage>
      ) : (
        <div className={rankChart} role="img" aria-label="候选命中位置分布">
          {rows.map((row) => (
            <div
              className={rankRow}
              key={row.id}
              aria-label={`${row.label}：${row.count} 次，${share(row.count)}`}
            >
              <span>{row.label}</span>
              <div className={rankTrack}>
                <i
                  className="block h-full rounded-[inherit]"
                  style={{ width: `${(row.count / peak) * 100}%`, backgroundColor: palette[0] }}
                />
              </div>
              <strong>{row.count.toLocaleString("zh-CN")}</strong>
              <small>{share(row.count)}</small>
            </div>
          ))}
        </div>
      )}
      <p className={footerNote}>
        每次上屏记录选中的是第几条候选，只记位置，不记任何文字。首选命中率越高，说明排序越贴合你的输入。
      </p>
    </section>
  );
}

/**
 * Today's characters by local hour, set against the average of earlier days.
 *
 * Every hour gets a column, including the empty ones: a chart that only drew the hours with input would put 9am next to 3pm and read as continuous typing. Today is the point, so it is the accent bars; the usual day is context, so it is a thin neutral line over them, on the same scale.
 */
function StatisticsHourlyBars({
  hours,
  usual,
}: {
  hours: readonly number[];
  usual: { hours: number[]; days: number } | null;
}) {
  const peak = Math.max(1, ...hours, ...(usual?.hours ?? []));
  const total = hours.reduce((sum, count) => sum + count, 0);
  const count = (value: number) => Math.round(value).toLocaleString("zh-CN");
  return (
    <>
      <p className="mt-[7px] mb-0 text-xs text-muted">
        最高 {count(Math.max(0, ...hours))} 字符 / 小时 · 共 {count(total)} 字符
      </p>
      {usual && (
        <div className="mt-2 flex items-center gap-4 text-[11px] text-secondary" aria-hidden="true">
          <span className="flex items-center gap-1.5">
            <i className="block size-2.5 rounded-[2px] bg-accent" />
            今日
          </span>
          <span className="flex items-center gap-1.5">
            <i className="block h-0.5 w-3.5 rounded-full bg-[var(--text-secondary)]" />
            平时（前 {usual.days.toLocaleString("zh-CN")} 天平均）
          </span>
        </div>
      )}
      <div
        className="relative mt-3 flex h-[110px] items-end gap-[3px]"
        role="img"
        aria-label={usual ? "今日各时段输入分布，与平时对比" : "今日各时段输入分布"}
      >
        {hours.map((value, hour) => (
          <div
            key={hour}
            className="flex h-full min-w-0 flex-1 flex-col justify-end"
            title={`${hour} 时：今日 ${value} 字符${usual ? `，平时 ${count(usual.hours[hour])} 字符` : ""}`}
            aria-label={`${hour} 时，${value} 字符`}
          >
            <i
              className={`${bar} bg-accent ${value === 0 ? "opacity-25" : "opacity-85"}`}
              style={{ height: `${Math.max(2, (value / peak) * 100)}%` }}
            />
          </div>
        ))}
        {usual && (
          <svg
            className="pointer-events-none absolute inset-0 block size-full overflow-visible text-[var(--text-secondary)]"
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <path
              d={linePath(usual.hours, (value) => 100 - (value / peak) * 100)}
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
            />
          </svg>
        )}
      </div>
      <div className={axis}>
        <span>0 时</span>
        <span>12 时</span>
        <span>23 时</span>
      </div>
    </>
  );
}

/** Why the page has nothing to show, or "" when there is nothing to explain.
 *
 * Every message here ends in "type a few more characters and come back". That is only true advice
 * while recording is on: with it off the counts are zero because nothing is being recorded, and
 * typing more records nothing. Recording ships off, so that is the state a new profile lands in -
 * it gets the call to action above instead, and this stays quiet rather than sending anyone off to
 * type for no effect.
 *
 * On iOS the keyboard extension cannot reach the shared App Group container without Full Access,
 * so there the prerequisite is named rather than the typing.
 */
export function availabilityNotice(
  statistics: Pick<TypingStatistics, "enabled" | "total">,
  status: Pick<TypingStatisticsStatus, "availability" | "lastWrittenMs">,
  iosPlatform: boolean,
): string {
  if (!statistics.enabled) return "";
  if (status.availability === "neverWritten")
    return iosPlatform
      ? "键盘从未写入过统计。请在系统设置 → 通用 → 键盘 → 键盘 → 水杉输入法中开启“允许完全访问”，然后用水杉键盘输入几个字再回来刷新。未开启时仍可正常打字，只是不记录统计。"
      : "键盘从未写入过统计。请用水杉键盘成功输入几个字符，再返回此页刷新。";
  if (statistics.total === 0 && status.lastWrittenMs)
    return `统计最后写入于 ${new Date(status.lastWrittenMs).toLocaleString("zh-CN")}，当前计数为零；如果刚刚清空过统计，这是正常的。`;
  if (statistics.total === 0) return "统计文件已建立，但当前还没有输入记录。";
  return "";
}

export function TypingStatisticsPage({
  client,
  mobile = false,
  platform,
  openSystemSettings,
}: {
  client: TypingStatisticsClient;
  mobile?: boolean;
  platform?: string;
  /** iOS only: opens this app's page in Settings, from which Full Access is reachable. */
  openSystemSettings?: () => Promise<void>;
}) {
  const { confirm, confirmation } = useConfirm();
  const [status, setStatus] = useState<TypingStatisticsStatus>();
  const [selectedDay, setSelectedDay] = useState<string | null>(null);
  // 打开页面时先看按键热力图，它排在标签行的第一位；标签只存在组件状态里，不跨次打开记忆。
  const [contentTab, setContentTab] = useState<
    "keys" | "trend" | "kind" | "mode" | "scheme" | "ranks"
  >("keys");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const requestRef = useRef<Promise<TypingStatisticsStatus> | null>(null);
  const requestStartedAtRef = useRef(0);
  const lastRequestAtRef = useRef(0);
  const statusSignatureRef = useRef("");
  const mounted = useRef(true);
  const clientGeneration = useRef(0);
  const mobileTrendDays = useMemo(
    () => recentDays(mobileTrendLength(status?.statistics.days ?? {})),
    [status?.statistics.days],
  );
  const desktopTrendDays = useMemo(() => recentDays(DESKTOP_TREND_DAYS), []);
  const trendDays = mobile ? mobileTrendDays : desktopTrendDays;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      requestRef.current = null;
    };
  }, []);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    requestRef.current = null;
    requestStartedAtRef.current = 0;
    lastRequestAtRef.current = 0;
    setBusy(false);
    setError("");
    return () => {
      if (generation === clientGeneration.current) {
        clientGeneration.current++;
        requestRef.current = null;
      }
    };
  }, [client]);

  async function update(operation: () => Promise<TypingStatisticsStatus>, overview = false) {
    if (!mounted.current || requestRef.current) return;
    const generation = clientGeneration.current;
    setBusy(true);
    setError("");
    const request = operation();
    requestRef.current = request;
    requestStartedAtRef.current = Date.now();
    if (overview) lastRequestAtRef.current = requestStartedAtRef.current;
    try {
      const next = await request;
      if (
        !mounted.current ||
        clientGeneration.current !== generation ||
        requestRef.current !== request
      )
        return;
      statusSignatureRef.current = JSON.stringify(next);
      setStatus(next);
    } catch {
      if (
        mounted.current &&
        clientGeneration.current === generation &&
        requestRef.current === request
      )
        setError("无法读取或保存统计，请稍后重试。原有统计不会被自动重置。");
    } finally {
      if (requestRef.current === request) {
        requestRef.current = null;
        if (mounted.current && clientGeneration.current === generation) setBusy(false);
      }
    }
  }

  useEffect(() => {
    let active = true;
    const generation = clientGeneration.current;
    const refreshWhenVisible = () => {
      if (!mounted.current || clientGeneration.current !== generation) return;
      const now = Date.now();
      if (
        document.visibilityState === "hidden" ||
        (requestRef.current && now - requestStartedAtRef.current <= overviewStaleMs) ||
        now - lastRequestAtRef.current < overviewMinIntervalMs
      )
        return;
      lastRequestAtRef.current = now;
      setError("");
      const request = client.load();
      requestRef.current = request;
      requestStartedAtRef.current = now;
      void request
        .then((next) => {
          if (!active || clientGeneration.current !== generation || requestRef.current !== request)
            return;
          const signature = JSON.stringify(next);
          if (signature !== statusSignatureRef.current) {
            statusSignatureRef.current = signature;
            setStatus(next);
          }
        })
        .catch(() => {
          if (active && clientGeneration.current === generation && requestRef.current === request)
            setError("无法读取或保存统计，请稍后重试。原有统计不会被自动重置。");
        })
        .finally(() => {
          if (requestRef.current === request) {
            requestRef.current = null;
            if (active && mounted.current && clientGeneration.current === generation)
              setBusy(false);
          }
        });
    };
    refreshWhenVisible();
    window.addEventListener("focus", refreshWhenVisible);
    document.addEventListener("visibilitychange", refreshWhenVisible);
    const poll = window.setInterval(refreshWhenVisible, overviewPollMs);
    return () => {
      active = false;
      window.clearInterval(poll);
      window.removeEventListener("focus", refreshWhenVisible);
      document.removeEventListener("visibilitychange", refreshWhenVisible);
    };
  }, [client]);

  if (!status)
    return (
      <div className={page}>
        {error ? (
          <ErrorAlert>{error}</ErrorAlert>
        ) : (
          <StatusMessage role="status">正在读取打字统计…</StatusMessage>
        )}
      </div>
    );
  const statistics = status.statistics;
  const scopeKeys = selectedDay ? [selectedDay] : null;
  const breakdown = scopedBreakdown(statistics, scopeKeys);
  const scopeTotal =
    scopeKeys === null
      ? statistics.total
      : scopeKeys.reduce((total, key) => total + (statistics.days[key] ?? 0), 0);
  const today = recentDays(1)[0];
  const selectedLabel = selectedDay
    ? (trendDays.find((day) => day.key === selectedDay)?.label ?? dayLabel(selectedDay))
    : null;
  const scopeTitle = selectedLabel ?? "累计输入";
  const keyScopeLabel = selectedLabel ?? "累计";
  const maximum = Math.max(1, ...trendDays.map((day) => statistics.days[day.key] ?? 0));
  const activity = activityMetrics(statistics, today.key);
  // The reference line is the average of the window's recorded days, the same "日均" the rhythm card uses, so empty days do not drag it down.
  const trendRecorded = trendDays
    .map((day) => statistics.days[day.key] ?? 0)
    .filter((count) => count > 0);
  const trendAverage =
    trendRecorded.length === 0
      ? 0
      : trendRecorded.reduce((sum, count) => sum + count, 0) / trendRecorded.length;
  const characterSlices = characterKinds.map(([id, title], index) => ({
    id,
    title,
    count: breakdown.characters[id] ?? 0,
    color: palette[index % palette.length],
    symbol: characterSymbols[id] ?? "?",
  }));
  const sourceSlices = sources.map(([id, title], index) => ({
    id,
    title,
    count: breakdown.sources[id] ?? 0,
    color: palette[index % palette.length],
    symbol: sourceSymbols[id] ?? "?",
  }));
  const languageSlices: Slice[] = [
    {
      id: "chinese",
      title: "中文模式",
      count: sumStatisticValues(breakdown.sources, [
        "quanpin",
        "nineKey",
        "shuangpin",
        "ziranma",
        "microsoft",
        "shoudao",
        "wubi",
        "cantonese",
        "zhuyin",
        "stroke",
      ]),
      color: palette[0],
      symbol: "中",
    },
    {
      id: "japanese",
      title: "日语模式",
      count: breakdown.sources.japanese ?? 0,
      color: palette[4],
      symbol: "日",
    },
    {
      id: "korean",
      title: "韩语模式",
      count: breakdown.sources.korean ?? 0,
      color: palette[6],
      symbol: "韩",
    },
    {
      id: "vietnamese",
      title: "越南语模式",
      count: breakdown.sources.vietnamese ?? 0,
      // Every palette colour already names a slice here, so this one has its own, as 高情商回复 does.
      color: "#d0605e",
      symbol: "越",
    },
    {
      id: "tibetan",
      title: "藏文模式",
      count: breakdown.sources.tibetan ?? 0,
      // 调色板的颜色都已被这里的扇区占用，藏文和越南语一样用自己的颜色。
      color: "#b8873a",
      symbol: "藏",
    },
    {
      id: "english",
      title: "英文模式",
      count: breakdown.sources.english ?? 0,
      color: palette[1],
      symbol: "A",
    },
    {
      id: "local",
      title: "本地输入",
      count: breakdown.sources.local ?? 0,
      color: palette[5],
      symbol: "本",
    },
    {
      id: "ai",
      title: "AI 润色",
      count: breakdown.sources.ai ?? 0,
      color: palette[3],
      symbol: "✦",
    },
    {
      id: "reply",
      title: "高情商回复",
      count: breakdown.sources.reply ?? 0,
      color: "#55bfa0",
      symbol: "回",
    },
    {
      id: "voice",
      title: "语音输入",
      count: breakdown.sources.voice ?? 0,
      color: palette[2],
      symbol: "♪",
    },
    {
      id: "unknown",
      title: "历史未分类",
      count: breakdown.sources.unknown ?? 0,
      color: palette[7],
      symbol: "?",
    },
  ];
  const iosPlatform = platform === "ios";
  const availabilityMessage = availabilityNotice(statistics, status, iosPlatform);
  const resetStatistics = async () => {
    const confirmed = await confirm({
      title: "清空打字统计",
      message: "累计字数、分类、每日记录和按键次数都会被删除，无法恢复。",
      confirmLabel: "清空",
      danger: true,
    });
    if (!confirmed) return;
    setSelectedDay(null);
    await update(client.reset);
  };

  return (
    <div className={page}>
      {confirmation}
      {error && <ErrorAlert>{error}</ErrorAlert>}
      {mobile && (
        <div className="-mb-1 flex min-h-0 justify-end">
          <details className="relative z-[3]">
            <summary className={menuSummary} aria-label="统计选项">
              ⋯
            </summary>
            <div className={menuPopover} role="menu" aria-label="统计选项">
              <label className={menuItem}>
                <span>记录打字统计</span>
                <input
                  aria-label="记录打字统计"
                  className="toggle"
                  type="checkbox"
                  checked={statistics.enabled}
                  disabled={busy}
                  onChange={(event) => void update(() => client.setEnabled(event.target.checked))}
                />
              </label>
              <ActionButton
                action={() => void update(() => client.load(), true)}
                ariaBusy={busy}
                className=""
                disabled={busy}
                label={busy ? "处理中…" : "刷新统计"}
                role="menuitem"
              />
              <ActionButton
                action={() => void resetStatistics()}
                className={`${menuItem} text-danger`}
                disabled={busy}
                label="清空统计"
                role="menuitem"
              />
            </div>
          </details>
        </div>
      )}
      {!statistics.enabled && (
        <section className="section m-0" aria-labelledby="statistics-disabled-title">
          <h2 id="statistics-disabled-title" className={heading}>
            输入统计已关闭
          </h2>
          <p className="mt-2 mb-0 leading-relaxed text-secondary">
            开启后这里会显示输入字数、速度、时段分布与按键热力图。统计只保存在本机，不记录输入内容，也不联网。
          </p>
          <ActionButton
            action={() => void update(() => client.setEnabled(true))}
            ariaBusy={busy}
            className="secondary"
            disabled={busy}
            label={busy ? "处理中…" : "启用输入统计"}
          />
        </section>
      )}
      <section className="section m-0" aria-label="统计概览">
        <div className={metricGrid}>
          <div className={metric}>
            <span>今日输入</span>
            <div className={metricLine}>
              <strong className={metricValue} aria-label="今日输入字符数">
                {(statistics.days[today.key] ?? 0).toLocaleString("zh-CN")}
              </strong>
              <small>字符</small>
            </div>
          </div>
          <div className={metric}>
            <span>{scopeTitle}</span>
            <div className={metricLine}>
              <strong className={metricValue} aria-label="当前范围输入字符数">
                {scopeTotal.toLocaleString("zh-CN")}
              </strong>
              <small>字符</small>
            </div>
          </div>
          <div className={metric}>
            <span title="连续打字的时间">今日活跃</span>
            <div className={metricLine}>
              <strong className={metricValue} aria-label="今日活跃时长">
                {formatActiveTime(activity.todayActiveMs)}
              </strong>
            </div>
          </div>
          <div className={metric}>
            <span>今日速度</span>
            <div className={metricLine}>
              <strong className={metricValue} aria-label="今日输入速度">
                {Math.round(activity.todaySpeed).toLocaleString("zh-CN")}
              </strong>
              <small>字 / 分钟</small>
            </div>
          </div>
          <div className={metric}>
            <span>平均速度</span>
            <div className={metricLine}>
              <strong className={metricValue} aria-label="平均输入速度">
                {Math.round(activity.averageSpeed).toLocaleString("zh-CN")}
              </strong>
              <small>字 / 分钟</small>
            </div>
            {activity.hasActivity && (
              <small className={metricNote}>共 {formatActiveTime(activity.totalActiveMs)}</small>
            )}
          </div>
          <div className={metric}>
            <span>连续天数</span>
            <div className={metricLine}>
              <strong className={metricValue} aria-label="连续输入天数">
                {activity.currentStreak.toLocaleString("zh-CN")}
              </strong>
              <small>天</small>
            </div>
            <small className={metricNote}>
              最长 {activity.longestStreak.toLocaleString("zh-CN")} 天
            </small>
          </div>
        </div>
        <div className={`${axis} flex-wrap gap-x-4`}>
          <span>
            日均 {Math.round(activity.averagePerDay).toLocaleString("zh-CN")} 字符 ·{" "}
            {activity.recordedDays.toLocaleString("zh-CN")} 天有记录
          </span>
          <span>
            {activity.bestDay
              ? `最多 ${dayLabel(activity.bestDay)}，${activity.bestDayCharacters.toLocaleString("zh-CN")} 字符`
              : "还没有记录"}
            {activity.fastestDay
              ? ` · 最快 ${dayLabel(activity.fastestDay)}，${Math.round(activity.fastestSpeed).toLocaleString("zh-CN")} 字 / 分钟`
              : ""}
          </span>
        </div>
        <p className={footerNote}>
          {activity.hasActivity
            ? "速度按连续打字的时间计算，两次上屏间隔超过 10 秒算休息、不计入；只统计汉字与字母，数字和标点不参与。"
            : "还没有测量到活跃时长。这项从本次更新后开始记录，之前的输入只有字数。"}
        </p>
      </section>
      <div className={segmented(6)} role="tablist" aria-label="统计内容">
        {(
          [
            ["keys", "按键"],
            ["trend", "趋势"],
            ["kind", "类型"],
            ["mode", "模式"],
            ["scheme", "方案"],
            ["ranks", "候选"],
          ] as const
        ).map(([value, label]) => (
          <button
            type="button"
            role="tab"
            key={value}
            aria-selected={contentTab === value}
            onClick={() => {
              setContentTab(value);
              // 桌面端切换标签时保留选中的那一天：它的柱子在「趋势」里，而其他标签正是用来看这一天的分项。
              if (mobile) setSelectedDay(null);
            }}
          >
            {label}
          </button>
        ))}
      </div>
      {contentTab === "trend" && (
        <section className="section m-0" aria-labelledby="statistics-trend-title">
          <h2 className={heading} id="statistics-trend-title">
            每日趋势 · {mobile && trendDays.length >= 360 ? "近一年" : `近 ${trendDays.length} 天`}
          </h2>
          <p className="mt-[7px] mb-0 text-xs text-muted">
            最高{" "}
            {maximum === 1 && trendDays.every((day) => !statistics.days[day.key])
              ? 0
              : maximum.toLocaleString("zh-CN")}{" "}
            字符 / 天
            {trendAverage > 0 &&
              ` · 虚线为日均 ${Math.round(trendAverage).toLocaleString("zh-CN")} 字符`}
          </p>
          {mobile ? (
            <StatisticsTrendLine
              days={trendDays}
              counts={statistics.days}
              selectedDay={selectedDay}
              average={trendAverage}
            />
          ) : (
            <div
              className={`relative mt-3 flex h-[145px] items-end ${trendDays.length === 7 ? "gap-2.5" : "gap-[3px]"} max-phone:gap-0.5`}
            >
              {trendDays.map((day) => {
                const count = statistics.days[day.key] ?? 0;
                const chosen = selectedDay === day.key;
                // A selection dims every other bar. The old stylesheet did this with two `:has()`
                // selectors because CSS could not see which day was picked; here the component
                // already holds it, so the state answers directly.
                const dimmed = selectedDay !== null && !chosen;
                return (
                  <StatisticsChartButton
                    className="flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-1 border-0 bg-transparent p-0 text-[10px] text-muted"
                    key={day.key}
                    title={`${day.label}：${count} 字符`}
                    ariaLabel={`${day.label}，${count} 字符`}
                    selected={chosen}
                    onClick={() =>
                      setSelectedDay((current) => (current === day.key ? null : day.key))
                    }
                  >
                    {trendDays.length === 7 && <span>{count}</span>}
                    <i
                      className={`${bar} ${chosen ? "bg-[#e59b43]" : "bg-accent"} ${dimmed ? "opacity-40" : chosen ? "opacity-100" : "opacity-85"}`}
                      style={{ height: `${Math.max(2, (count / maximum) * 100)}%` }}
                    />
                  </StatisticsChartButton>
                );
              })}
              {trendAverage > 0 && <ReferenceLine bottom={(trendAverage / maximum) * 100} />}
            </div>
          )}
          <div className={axis}>
            <span>{trendDays[0]?.label}</span>
            <span>{trendDays.at(-1)?.label}</span>
          </div>
          {mobile && (
            <>
              <p className="mt-[18px] mb-0 text-xs text-muted">每天一格，一列一周</p>
              <StatisticsHeatmap
                days={statistics.days}
                selectedDay={selectedDay}
                onSelect={(key) => setSelectedDay((current) => (current === key ? null : key))}
              />
            </>
          )}
          <p className={footerNote}>
            {mobile ? "点按热力图查看当天的分类与占比。" : "点按柱形查看当天的分类与占比。"}
          </p>
          {selectedDay && (
            <ActionButton action={() => setSelectedDay(null)} label="返回整个时间范围" />
          )}
        </section>
      )}
      {contentTab === "trend" && activity.todayHours && (
        <section className="section m-0" aria-labelledby="statistics-hours-title">
          <h2 className={heading} id="statistics-hours-title">
            今日时段
          </h2>
          <StatisticsHourlyBars
            hours={activity.todayHours}
            usual={usualHours(statistics.dailyHours, today.key)}
          />
        </section>
      )}
      {contentTab === "trend" && (
        <section className="section m-0" aria-labelledby="statistics-speed-title">
          <h2 className={heading} id="statistics-speed-title">
            速度趋势
          </h2>
          <SpeedTrend
            days={trendDays}
            speeds={dailySpeeds(
              statistics,
              trendDays.map((day) => day.key),
            )}
            averageSpeed={activity.averageSpeed}
            selectedDay={selectedDay}
          />
        </section>
      )}
      {!mobile && contentTab === "trend" && (
        <section className="section m-0" aria-labelledby="statistics-calendar-title">
          <h2 className={heading} id="statistics-calendar-title">
            日历热力图
          </h2>
          <p className="mt-[7px] mb-0 text-xs text-muted">近 12 个月，颜色越深输入越多</p>
          <StatisticsHeatmap
            days={statistics.days}
            selectedDay={selectedDay}
            onSelect={(key) => setSelectedDay((current) => (current === key ? null : key))}
          />
        </section>
      )}
      {contentTab === "keys" && (
        <KeyboardHeatmap
          dailyKeys={statistics.dailyKeys}
          scopeKeys={scopeKeys}
          scopeLabel={keyScopeLabel}
          mobile={mobile}
          platform={platform}
        />
      )}
      {contentTab === "kind" && (
        <Distribution title="字符类型" slices={characterSlices} variant="donut" wide={!mobile} />
      )}
      {contentTab === "mode" && (
        <Distribution
          title="语言模式"
          slices={languageSlices}
          variant="donut"
          wide={!mobile}
          footer="按提交时使用的键盘模式统计，不推测文本语言；中文模式下输入的数字仍计入中文模式。AI 润色和语音输入单独按来源统计。"
        />
      )}
      {contentTab === "scheme" && (
        <Distribution
          title="输入方案"
          slices={sourceSlices}
          variant="rank"
          footer="输入方案统计其上屏字符数；拼音等按键另由按键热力图计数，只记每个键每天的按下次数。来源无法归类的字数计入历史未分类。"
        />
      )}
      {!mobile && contentTab === "trend" && (
        <DailyDetails rows={dailyDetailRows(statistics, today.key)} />
      )}
      {contentTab === "ranks" && <CandidateRanks selections={statistics.selections} />}
      {mobile ? (
        <section className="section m-0 pt-0.5">
          <p className={`${privacy} mt-0`}>
            字数仅统计水杉键盘成功提交的字符，含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。按键热力图只保存每个键每天被按下的次数，不保存按键顺序和输入内容。仅在本机保存日期、分类和数量，不保存输入内容。每日明细默认永久保留。
          </p>
        </section>
      ) : (
        <section className="section m-0">
          <SettingToggle
            label="记录打字统计"
            description="关闭后，新提交不会增加统计。"
            ariaLabel="记录打字统计"
            rowClassName="mb-4"
            compact
            checked={statistics.enabled}
            disabled={busy}
            onChange={(enabled) => void update(() => client.setEnabled(enabled))}
          />
          {client.setRetention && (
            <SelectSettingField
              label="自动清理"
              inputLabel="自动清理"
              description="按保留策略删除超期的每日记录并从累计中扣除，跨天后首次记录时执行。"
              fieldClassName="section-header mb-4"
              value={statistics.retention ?? "forever"}
              disabled={busy}
              onChange={(value) => {
                const setRetention = client.setRetention;
                if (!setRetention) return;
                const chosen = value as StatisticsRetention;
                void update(() => setRetention(chosen));
              }}
            >
              {retentionChoices.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </SelectSettingField>
          )}
          <div className="flex flex-wrap gap-[9px]">
            <ActionButton
              action={() => void update(() => client.load(), true)}
              ariaBusy={busy}
              className="secondary m-0"
              disabled={busy}
              label={busy ? "处理中…" : "刷新统计"}
            />
            {client.openDirectory && (
              <ActionButton
                action={async () => {
                  const openDirectory = client.openDirectory;
                  if (!openDirectory) return;
                  setError("");
                  try {
                    await openDirectory();
                  } catch {
                    setError("无法打开数据目录，可能是文件管理器不可用。");
                  }
                }}
                ariaBusy={busy}
                className="secondary m-0"
                disabled={busy}
                label="打开数据目录"
              />
            )}
            <ActionButton
              action={() => void resetStatistics()}
              ariaBusy={busy}
              className="secondary m-0 text-danger"
              disabled={busy}
              label="清空统计"
            />
          </div>
          <p className={privacy}>
            字数统计水杉键盘提交的字符，以及英文模式和放行给应用的字母、数字与符号（按按键时估计），含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。按键热力图只保存每个键每天被按下的次数，不保存按键顺序和输入内容。仅在本机保存日期、分类和数量，不保存输入内容。每日明细默认永久保留，可在「自动清理」中改为只保留最近一段时间；清理删除的日期同时从累计总数与分类中扣除。
          </p>
        </section>
      )}
      {availabilityMessage && (
        <section className="section m-0">
          <h2 className={heading}>统计没有数据</h2>
          <p className="mt-2 mb-0 leading-relaxed text-secondary">{availabilityMessage}</p>
          {iosPlatform && status.availability === "neverWritten" && openSystemSettings && (
            <ActionButton action={openSystemSettings} label="打开系统键盘设置" />
          )}
        </section>
      )}
    </div>
  );
}
