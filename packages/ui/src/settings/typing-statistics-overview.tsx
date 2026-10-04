import { formatZhNumber } from "../core/format-number";
import { dayLabel, formatActiveTime } from "./typing-statistics-helpers";
import type { ActivityMetrics, TypingStatistics } from "./typing-statistics";

export type StatisticsOverviewMetric = {
  label: string;
  title?: string;
  value: string;
  unit?: string;
  ariaLabel: string;
  note?: string;
};

export function statisticsOverviewMetrics({
  statistics,
  todayKey,
  scopeTitle,
  scopeTotal,
  activity,
}: {
  statistics: TypingStatistics;
  todayKey: string;
  scopeTitle: string;
  scopeTotal: number;
  activity: ActivityMetrics;
}): StatisticsOverviewMetric[] {
  return [
    {
      label: "今日输入",
      value: formatZhNumber(statistics.days[todayKey] ?? 0),
      unit: "字符",
      ariaLabel: "今日输入字符数",
    },
    {
      label: scopeTitle,
      value: formatZhNumber(scopeTotal),
      unit: "字符",
      ariaLabel: "当前范围输入字符数",
    },
    {
      label: "今日活跃",
      title: "连续打字的时间",
      value: formatActiveTime(activity.todayActiveMs),
      ariaLabel: "今日活跃时长",
    },
    {
      label: "今日速度",
      value: formatZhNumber(Math.round(activity.todaySpeed)),
      unit: "字 / 分钟",
      ariaLabel: "今日输入速度",
    },
    {
      label: "平均速度",
      value: formatZhNumber(Math.round(activity.averageSpeed)),
      unit: "字 / 分钟",
      ariaLabel: "平均输入速度",
      ...(activity.hasActivity ? { note: `共 ${formatActiveTime(activity.totalActiveMs)}` } : {}),
    },
    {
      label: "连续天数",
      value: formatZhNumber(activity.currentStreak),
      unit: "天",
      ariaLabel: "连续输入天数",
      note: `最长 ${formatZhNumber(activity.longestStreak)} 天`,
    },
  ];
}

export function StatisticsMetric({ metric }: { metric: StatisticsOverviewMetric }) {
  return (
    <div className="flex min-w-0 flex-col gap-1 [&>span]:text-xs [&>span]:[color:var(--p-sub)]">
      <span title={metric.title}>{metric.label}</span>
      <div className="flex min-w-0 flex-wrap items-baseline gap-x-1 [&>small]:text-xs [&>small]:[color:var(--p-sub)]">
        <strong
          className="text-[22px] font-[650] leading-tight break-anywhere tabular-nums [color:var(--p-accent-text)]"
          aria-label={metric.ariaLabel}
        >
          {metric.value}
        </strong>
        {metric.unit && <small>{metric.unit}</small>}
      </div>
      {metric.note && <small className="text-xs [color:var(--p-sub)]">{metric.note}</small>}
    </div>
  );
}

export function statisticsOverviewDetails(activity: ActivityMetrics): {
  summary: string;
  best: string;
} {
  return {
    summary: `日均 ${formatZhNumber(Math.round(activity.averagePerDay))} 字符 · ${formatZhNumber(activity.recordedDays)} 天有记录`,
    best: `${activity.bestDay ? `最多 ${dayLabel(activity.bestDay)}，${formatZhNumber(activity.bestDayCharacters)} 字符` : "还没有记录"}${activity.fastestDay ? ` · 最快 ${dayLabel(activity.fastestDay)}，${formatZhNumber(Math.round(activity.fastestSpeed))} 字 / 分钟` : ""}`,
  };
}
