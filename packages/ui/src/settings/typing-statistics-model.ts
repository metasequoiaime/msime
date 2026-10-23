import type { DailyKeyCounts } from "./keyboard-heatmap";
import type { TypingSummary } from "./typing-summary";
import {
  charactersPerMinute,
  readableCharacters,
  withUnknown,
  type TypingBreakdown,
} from "./typing-speed";
import { currentStreak, longestStreak, statisticDayKeys } from "./typing-statistics-helpers";

export type SelectionCounts = {
  /** 候选位置 1 到 9 的上屏次数，索引 0 对应首选。 */
  ranks?: number[];
  /** 超过首页位置的候选上屏次数。 */
  beyond?: number;
};

/** 记录日的保留时长；宿主传来的其他值按永久保留读取。 */
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
  /** 自动清理引入前的记录没有此字段，表示永久保留。 */
  retention?: StatisticsRetention;
  total: number;
  days: Record<string, number>;
  detail?: Partial<TypingBreakdown>;
  dailyDetails?: Record<string, Partial<TypingBreakdown>>;
  /** 候选位置计数引入前的记录没有此字段。 */
  selections?: SelectionCounts;
  /** 每日活跃输入毫秒数；测量引入前的日期缺失表示未知，不能当作零参与速度计算。 */
  dailyActiveMs?: Record<string, number>;
  /** 每天按本地小时分为 24 桶；宿主没有提供小时数据的日期不出现。 */
  dailyHours?: Record<string, number[]>;
  /** 每天按本地日期记录的按键次数，以 `KeyboardEvent.code` 或软键盘标识为键；旧记录可能没有此字段。 */
  dailyKeys?: DailyKeyCounts;
};

export type TypingStatisticsStatus = {
  statistics: TypingStatistics;
  availability: "ready" | "neverWritten";
  lastWrittenMs?: number | null;
};

/** 「上传到云端」的状态。`signedIn` 为 false 时开关不能打开。 */
export type TypingStatisticsUploadStatus = {
  enabled: boolean;
  /** 上次上传成功的 Unix 毫秒；还没有上传过时缺省或为 null。 */
  lastUploadMs?: number | null;
  signedIn: boolean;
};

/** 每日计数上传到水杉账号，给 QQ 群助手的 /stats 读。只有计数，没有输入内容。 */
export interface TypingStatisticsUploadClient {
  status(): Promise<TypingStatisticsUploadStatus>;
  /** 打开时立刻上传一次；关闭时删除云端副本，删除失败时开关仍是关的，Promise 以错误码拒绝。 */
  setEnabled(enabled: boolean): Promise<TypingStatisticsUploadStatus>;
  /** 打开了上传、已登录、距上次超过十分钟时上传最近几天，否则什么也不做。 */
  sync(): Promise<TypingStatisticsUploadStatus>;
}

/** 皮肤徽章统计属于后台记录；宿主写入失败不能影响已经完成的换肤动作。 */
export function recordSkinSafely(
  client: Pick<TypingStatisticsClient, "recordSkin"> | undefined,
  id: string,
): void {
  try {
    const result = client?.recordSkin?.(id);
    if (result) void result.catch(() => undefined);
  } catch {
    // 宿主可能在调用时同步失败，同样静默处理。
  }
}

export interface TypingStatisticsClient {
  load(): Promise<TypingStatisticsStatus>;
  setEnabled(enabled: boolean): Promise<TypingStatisticsStatus>;
  /** 宿主不自行保存统计时不提供此方法。 */
  setRetention?(retention: StatisticsRetention): Promise<TypingStatisticsStatus>;
  /** 无法打开文件管理器的宿主不提供此方法，页面据此隐藏按钮。 */
  openDirectory?(): Promise<void>;
  reset(): Promise<TypingStatisticsStatus>;
  /** 共用存储为今天算出的各项数据（概览、习惯、按键、成就）。`userWords` 是用户加进词库的词数，用于「造词者」徽章；不传时存储按零计。不绘制基于摘要的「统计」标签页的宿主不提供它。 */
  summary?(userWords?: number): Promise<TypingSummary>;
  /** 记录用户试过键盘皮肤 `id`，计入「换装达人」徽章。 */
  recordSkin?(id: string): Promise<void>;
  /** 「上传到云端」；不支持的宿主不提供，页面不显示这一项。 */
  cloudUpload?: TypingStatisticsUploadClient;
}

/** 每日小时桶数，与共享存储一致。 */
export const HOURS = 24;

/** 最快日必须至少测得 60 秒活跃时间，避免短暂输入长期占据榜首。 */
const fastestDayMinimumActiveMs = 60_000;

export type ActivityMetrics = {
  /** 有记录的日期数；没有记录的日期不计入平均值。 */
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
  /** 今天的 24 个小时桶；宿主未记录今天的小时数据时为 null。 */
  todayHours: number[] | null;
  /** 从未测得活跃时间时为 false，与测得零速度不同。 */
  hasActivity: boolean;
};

/** 活动卡片的派生数据；今天的日期由调用方提供，计算只依赖参数。 */
export function activityMetrics(statistics: TypingStatistics, todayKey: string): ActivityMetrics {
  const recorded = statisticDayKeys(statistics.days);
  const activeByDay = statistics.dailyActiveMs ?? {};
  let totalActiveMs = 0;
  let totalReadable = 0;
  let fastestSpeed = 0;
  let fastestDay: string | null = null;
  let bestDay: string | null = null;
  let bestDayCharacters = 0;
  // 按记录日行数与行内字数计算平均值；宿主允许 `total` 高于这些行的总和。
  let recordedCharacters = 0;
  for (const key of recorded) {
    const activeMs = activeByDay[key] ?? 0;
    const readable = readableCharacters(statistics.dailyDetails?.[key]);
    if (activeMs > 0) {
      totalActiveMs += activeMs;
      totalReadable += readable;
      if (activeMs >= fastestDayMinimumActiveMs) {
        const speed = charactersPerMinute(readable, activeMs);
        // 日期已升序排列，仅严格更大时更新，使同速时保留最早日期。
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

/** 今天之前各有效记录日的逐小时均值；仅纳入完整 24 桶且至少一桶大于零的日期，无有效记录时返回 null。 */
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

/** 各日期每活跃分钟的可读字符数；不足最快日的 60 秒门槛时返回 null，避免短暂输入形成尖峰。 */
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

/** 按日明细显示的记录日数，对应 Windows 来源的 `DETAIL_DAYS`。 */
export const DETAIL_DAYS = 30;

export type DailyDetailRow = {
  key: string;
  total: number;
  han: number;
  latin: number;
  number: number;
  punctuation: number;
  /** 其他文字、表情、符号以及分类未覆盖的历史字数。 */
  other: number;
  /** 测量引入前的日期为 null，表示未知而非零。 */
  activeMs: number | null;
  /** 仅当 `activeMs` 为 null 时为 null，避免把未测量日显示为零速度。 */
  speed: number | null;
};

/** 截至今天最近的记录日，按日期倒序；来源没有独立列的字符类别合并进 `other`。 */
export function dailyDetailRows(
  statistics: TypingStatistics,
  todayKey: string,
  days = DETAIL_DAYS,
): DailyDetailRow[] {
  const keys = statisticDayKeys(statistics.days)
    .filter((key) => key <= todayKey)
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
