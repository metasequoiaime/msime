import {
  DETAIL_DAYS,
  retentionChoices,
  activityMetrics,
  usualHours,
  dailySpeeds,
  dailyDetailRows,
  type StatisticsRetention,
  type TypingStatistics,
  type TypingStatisticsStatus,
  type TypingStatisticsClient,
  type DailyDetailRow,
} from "./typing-statistics-model";
export {
  HOURS,
  DETAIL_DAYS,
  retentionChoices,
  activityMetrics,
  usualHours,
  dailySpeeds,
  dailyDetailRows,
  type SelectionCounts,
  type StatisticsRetention,
  type TypingStatistics,
  type TypingStatisticsStatus,
  type TypingStatisticsClient,
  type ActivityMetrics,
  type DailyDetailRow,
} from "./typing-statistics-model";
import { ControlledCheckbox } from "../core/controlled-checkbox";
import { rowHeading } from "../core/row-style";
import { compactPage } from "../core/page-style";
import { useMemo, useState } from "react";
import { useConfirm } from "../core/confirm";
import { KeyboardHeatmap, StatisticsHeatmap } from "./statistics-heatmaps";
import { StatisticsChartButton } from "./statistics-chart-button";
import { scopedBreakdown } from "./typing-breakdown";
import { statisticsDistributionSlices } from "./statistics-distribution-model";
import { CandidateRanks, Distribution } from "./statistics-distributions";
import { SelectSettingField } from "./select-setting-field";
import { SettingToggle } from "./setting-toggle";
import { ActionButton } from "./action-button";
import { ErrorAlert } from "../core/error-alert";
import { StatusMessage } from "../core/status-message";
import { formatZhNumber } from "../core/format-number";
import { formatDateTime } from "../core/format-date";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { useTypingStatisticsState } from "./use-typing-statistics-state";
import { MobileStatistics, statisticsCardClass, type StatisticsLook } from "./mobile-statistics";
import { StatisticsNote } from "./statistics-note";
import { TypingStatisticsUpload } from "./typing-statistics-upload";
export type { TypingBreakdown } from "./typing-speed";
export type { TypingSummary } from "./typing-summary";
import {
  dayLabel,
  formatActiveTime,
  mobileTrendLength,
  recentDays,
} from "./typing-statistics-helpers";
import {
  StatisticsMetric,
  statisticsOverviewDetails,
  statisticsOverviewMetrics,
} from "./typing-statistics-overview";
export {
  addDays,
  currentStreak,
  formatActiveTime,
  longestStreak,
  statisticDayKeys,
} from "./typing-statistics-helpers";
export {
  statisticsOverviewDetails,
  statisticsOverviewMetrics,
  type StatisticsOverviewMetric,
} from "./typing-statistics-overview";

const heading = rowHeading;
// 摘要在标签行上方，切换标签时保持不动，所以压成紧凑的 3 × 2 网格，单位和数字放在同一行；会随标签变化的内容都放在标签下方。
const metricGrid = "grid grid-cols-3 gap-x-6 gap-y-[18px] max-phone:grid-cols-2 max-phone:gap-x-3";
const footerNote = "mt-3.5 mb-0 text-xs leading-relaxed [color:var(--p-sub)]";
const privacy = "mt-4 mb-0 text-xs leading-[1.7] [color:var(--p-sub)]";
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
const page = compactPage;
// HarmonyOS 外观下的纵向排列：分段控件、各卡片组和隐私说明之间按设计间隔 22px。
const summaryPage = "flex flex-col gap-[22px] text-[color:var(--p-text)]";
const bar = "block w-full min-h-0.5 rounded-t-[3px] rounded-b-[1px]";
const axis = "mt-[7px] flex justify-between text-xs text-muted";

// The segmented control behind the content tabs. The column count is a parameter because the tab row silently kept four columns after a fifth tab was added -- the extra one wrapped onto a second row at a quarter width. Each count is spelled out so Tailwind sees the class.
const segmentedColumns: Record<number, string> = {
  5: "grid-cols-5",
  6: "grid-cols-6",
};
const segmented = (columns: number) =>
  `grid gap-[3px] rounded-[9px] bg-subtle p-[3px] ${segmentedColumns[columns]} [&>button]:min-h-[34px] [&>button]:rounded-[7px] [&>button]:border-0 [&>button]:bg-transparent [&>button]:text-secondary [&>button[aria-selected=true]]:bg-raised [&>button[aria-selected=true]]:text-body [&>button[aria-selected=true]]:shadow-card`;

/** Days the desktop trend bars cover. */
const DESKTOP_TREND_DAYS = 30;
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
  const count = (value: number) => formatZhNumber(value);
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
  const count = (value: number) => formatZhNumber(Math.round(value));
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
      <StatisticsNote>
        最快 {count(Math.max(...known))} · 最慢 {count(Math.min(...known))} · 虚线为平均{" "}
        {count(averageSpeed)} 字 / 分钟 · {known.length} 天有测量
      </StatisticsNote>
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
  const count = (value: number) => formatZhNumber(Math.round(value));
  return (
    <>
      <StatisticsNote>
        最高 {count(Math.max(0, ...hours))} 字符 / 小时 · 共 {count(total)} 字符
      </StatisticsNote>
      {usual && (
        <div className="mt-2 flex items-center gap-4 text-[11px] text-secondary" aria-hidden="true">
          <span className="flex items-center gap-1.5">
            <i className="block size-2.5 rounded-[2px] bg-accent" />
            今日
          </span>
          <span className="flex items-center gap-1.5">
            <i className="block h-0.5 w-3.5 rounded-full bg-[var(--text-secondary)]" />
            平时（前 {formatZhNumber(usual.days)} 天平均）
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
 *
 * iOS 上统计文件存在也不能说明键盘写过：在 app 里开启记录时 `client-services` 的 `set_enabled` 会无条件写出统计文件，而没有完全访问的键盘什么都不记，计数一直是零。app 读不到键盘是否有完全访问，所以计数为零时也先提醒这一项，再说其他可能。
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
  if (statistics.total !== 0) return "";
  const fullAccess = iosPlatform
    ? "请先确认已在系统设置 → 通用 → 键盘 → 键盘 → 水杉输入法中为水杉键盘开启“允许完全访问”，未开启时键盘不记录统计。"
    : "";
  if (status.lastWrittenMs) {
    const written = formatDateTime(status.lastWrittenMs, "zh-CN");
    return iosPlatform
      ? `统计最后写入于 ${written}，当前计数为零。${fullAccess}已开启的话，如果刚刚清空过统计，这是正常的。`
      : `统计最后写入于 ${written}，当前计数为零；如果刚刚清空过统计，这是正常的。`;
  }
  return `统计文件已建立，但当前还没有输入记录。${fullAccess}`;
}

export function TypingStatisticsPage({
  client,
  mobile = false,
  platform,
  openSystemSettings,
  look,
}: {
  client: TypingStatisticsClient;
  mobile?: boolean;
  platform?: string;
  /** iOS only: opens this app's page in Settings, from which Full Access is reachable. */
  openSystemSettings?: () => Promise<void>;
  /** HarmonyOS 手机或 2in1 外观。客户端能应答 `summary` 时，页面是基于摘要的「概览 / 习惯 / 按键 / 成就」标签页，而不是趋势图和分布图。 */
  look?: StatisticsLook;
}) {
  const { confirm, confirmation } = useConfirm();
  const { status, today, busy, error, summary, summaryFailed, update, openDataDirectory } =
    useTypingStatisticsState(client, look);
  const [selectedDay, setSelectedDay] = useState<string | null>(null);
  // 打开页面时先看按键热力图，它排在标签行的第一位；标签只存在组件状态里，不跨次打开记忆。
  const [contentTab, setContentTab] = useState<
    "keys" | "trend" | "kind" | "mode" | "scheme" | "ranks"
  >("keys");
  // 「上传到云端」打开时，页面上「只保存在本机」的说明要改写。
  const [uploadEnabled, setUploadEnabled] = useState(false);
  const summaryLook = look && client.summary ? look : undefined;
  const mobileTrendDays = useMemo(
    () => recentDays(mobileTrendLength(status?.statistics.days ?? {})),
    [status?.statistics.days, today.key],
  );
  const desktopTrendDays = useMemo(() => recentDays(DESKTOP_TREND_DAYS), [today.key]);
  const trendDays = mobile ? mobileTrendDays : desktopTrendDays;

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
  const { characterSlices, sourceSlices, languageSlices } = statisticsDistributionSlices(breakdown);
  const iosPlatform = platform === "ios";
  const availabilityMessage = availabilityNotice(statistics, status, iosPlatform);
  const resetStatistics = async () => {
    const confirmed = await confirm({
      title: "清空打字统计",
      message: summaryLook
        ? "累计字数、分类、每日记录、按键次数和已解锁的成就都会被删除，无法恢复。"
        : "累计字数、分类、每日记录和按键次数都会被删除，无法恢复。",
      confirmLabel: "清空",
      danger: true,
    });
    if (!confirmed) return;
    setSelectedDay(null);
    await update(client.reset);
  };
  // 手机的溢出菜单。HarmonyOS 外观的图表下方没有设置区，所以保留期选项和开关、刷新、重置一起移到这里。
  const optionsMenu = (rowClassName: string, retention: boolean) => (
    <div className={`${rowClassName} flex min-h-0 justify-end`}>
      <details className="relative z-[3]">
        <summary className={menuSummary} aria-label="统计选项">
          ⋯
        </summary>
        <div className={menuPopover} role="menu" aria-label="统计选项">
          <label className={menuItem}>
            <span>记录打字统计</span>
            <ControlledCheckbox
              aria-label="记录打字统计"
              className="toggle"
              checked={statistics.enabled}
              disabled={busy}
              onChange={(checked) => void update(() => client.setEnabled(checked))}
            />
          </label>
          {retention && client.setRetention && (
            <label className={menuItem}>
              <span>自动清理</span>
              <select
                aria-label="自动清理"
                className="max-w-[140px] rounded-[6px] border border-edge bg-transparent px-1.5 py-1 text-xs text-body"
                value={statistics.retention ?? "forever"}
                disabled={busy}
                onChange={(event) => {
                  const setRetention = client.setRetention;
                  if (!setRetention) return;
                  const chosen = event.target.value as StatisticsRetention;
                  void update(() => setRetention(chosen));
                }}
              >
                {retentionChoices.map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
          )}
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
  );

  if (summaryLook) {
    const card = statisticsCardClass(summaryLook);
    return (
      <div className={summaryPage}>
        {confirmation}
        {error && <ErrorAlert>{error}</ErrorAlert>}
        {optionsMenu("-mb-3.5", true)}
        {!statistics.enabled && (
          <section
            className={`${card} flex flex-col items-start gap-2 p-4`}
            aria-labelledby="statistics-disabled-title"
          >
            <h2 id="statistics-disabled-title" className="m-0 text-[15px] font-semibold">
              输入统计已关闭
            </h2>
            <p className="m-0 text-[13px] leading-relaxed text-[color:var(--p-sub)]">
              已有的计数保留在本机，新的输入不再计入。开启后这里会显示输入字数、速度、时段分布、按键热力图与成就。
            </p>
            <ActionButton
              action={() => void update(() => client.setEnabled(true))}
              ariaBusy={busy}
              className="secondary m-0"
              disabled={busy}
              label={busy ? "处理中…" : "启用输入统计"}
            />
          </section>
        )}
        {availabilityMessage && (
          <section className={`${card} flex flex-col items-start gap-2 p-4`}>
            <h2 className="m-0 text-[15px] font-semibold">统计没有数据</h2>
            <p className="m-0 text-[13px] leading-relaxed text-[color:var(--p-sub)]">
              {availabilityMessage}
            </p>
          </section>
        )}
        {summary ? (
          <MobileStatistics
            look={summaryLook}
            summary={summary}
            dailyKeys={statistics.dailyKeys}
            uploading={uploadEnabled}
          />
        ) : summaryFailed ? (
          <ErrorAlert>统计暂时读不到，可以在右上角菜单里刷新。</ErrorAlert>
        ) : (
          <StatusMessage role="status">正在读取打字统计…</StatusMessage>
        )}
        {/* 鸿蒙外观的图表下面没有设置区，「上传到云端」单独占一张卡片，排在最后。 */}
        {client.cloudUpload && (
          <section className={`${card} px-4 pt-4`}>
            <TypingStatisticsUpload
              client={client.cloudUpload}
              onEnabledChange={setUploadEnabled}
            />
          </section>
        )}
      </div>
    );
  }

  return (
    <div className={page}>
      {confirmation}
      {error && <ErrorAlert>{error}</ErrorAlert>}
      {mobile && optionsMenu("-mb-1", false)}
      {!statistics.enabled && (
        <section className="section m-0" aria-labelledby="statistics-disabled-title">
          <h2 id="statistics-disabled-title" className={heading}>
            输入统计已关闭
          </h2>
          <p className="mt-2 mb-0 leading-relaxed text-secondary">
            {uploadEnabled
              ? "开启后这里会显示输入字数、速度、时段分布与按键热力图。统计不记录输入内容；已打开「上传到云端」，每日计数会上传到你的水杉账号。"
              : "开启后这里会显示输入字数、速度、时段分布与按键热力图。统计只保存在本机，不记录输入内容，也不联网。"}
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
          {statisticsOverviewMetrics({
            statistics,
            todayKey: today.key,
            scopeTitle,
            scopeTotal,
            activity,
          }).map((metric) => (
            <StatisticsMetric key={metric.ariaLabel} metric={metric} />
          ))}
        </div>
        {/* Windows 的概览在六格下面并排画「日均…」和「最多…·最快…」两行；共享页把日均放进了「累计输入」的说明，最好成绩这行一直没画。这里按 Windows 的措辞补上，靠右对齐。 */}
        <p className="mt-3.5 mb-0 text-right text-[11px] leading-relaxed [color:var(--p-sub)]">
          {statisticsOverviewDetails(activity).best}
        </p>
        <p className="mt-4 mb-0 border-t border-edge pt-3 text-[11px] leading-relaxed [color:var(--p-sub)]">
          {activity.hasActivity
            ? "速度统计汉字与字母；两次上屏间隔超过 10 秒视为休息，不计入活跃时长。"
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
          <StatisticsNote>
            最高{" "}
            {maximum === 1 && trendDays.every((day) => !statistics.days[day.key])
              ? 0
              : formatZhNumber(maximum)}{" "}
            字符 / 天
            {trendAverage > 0 && ` · 虚线为日均 ${formatZhNumber(Math.round(trendAverage))} 字符`}
          </StatisticsNote>
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
          <StatisticsNote>近 12 个月，颜色越深输入越多</StatisticsNote>
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
          {client.cloudUpload && (
            <TypingStatisticsUpload
              client={client.cloudUpload}
              onEnabledChange={setUploadEnabled}
            />
          )}
          <p className={`${privacy} mt-0`}>
            字数仅统计水杉键盘成功提交的字符，含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。按键热力图只保存每个键每天被按下的次数，不保存按键顺序和输入内容。
            {uploadEnabled
              ? "日期、分类和数量保存在本机，不保存输入内容；已打开「上传到云端」，最近 7 天的每日计数和候选位次会上传到你的水杉账号，按键热力图不上传。"
              : "仅在本机保存日期、分类和数量，不保存输入内容。"}
            每日明细默认永久保留。
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
          {client.cloudUpload && (
            <TypingStatisticsUpload
              client={client.cloudUpload}
              onEnabledChange={setUploadEnabled}
            />
          )}
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
          {/* 桌面没有「刷新统计」：页面每 5 秒自动重读一次，窗口回到前台时也会重读。手机的溢出菜单仍保留它。 */}
          <div className="flex flex-wrap gap-[9px]">
            {client.openDirectory && (
              <ActionButton
                action={openDataDirectory}
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
            字数统计水杉键盘提交的字符，以及英文模式和放行给应用的字母、数字与符号（按按键时估计），含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。按键热力图只保存每个键每天被按下的次数，不保存按键顺序和输入内容。
            {uploadEnabled
              ? "日期、分类和数量保存在本机，不保存输入内容；已打开「上传到云端」，最近 7 天的每日计数和候选位次会上传到你的水杉账号，按键热力图不上传。"
              : "仅在本机保存日期、分类和数量，不保存输入内容。"}
            每日明细默认永久保留，可在「自动清理」中改为只保留最近一段时间；清理删除的日期同时从累计总数与分类中扣除。
          </p>
        </section>
      )}
      {availabilityMessage && (
        <section className="section m-0">
          <h2 className={heading}>统计没有数据</h2>
          <p className="mt-2 mb-0 leading-relaxed text-secondary">{availabilityMessage}</p>
          {iosPlatform && openSystemSettings && (
            <ActionButton action={openSystemSettings} label="打开系统键盘设置" />
          )}
        </section>
      )}
    </div>
  );
}
