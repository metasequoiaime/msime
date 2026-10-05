import { expect, test } from "vitest";
import { activityMetrics, addDays, usualHours, type TypingStatistics } from "@msime/ui";
import cases from "../../../../packages/ui/src/settings/typing-metrics-cases.json";

// 由 Rust 测试 `typing_metrics_cases_match_the_checked_in_copy` 写出：`summary` 是 `typing_statistics::summarize` 的结果，`allTimeAverageSpeed` 是同一算法作用在全部有记录的天上。
type Case = {
  name: string;
  today: string;
  statistics: TypingStatistics;
  allTimeAverageSpeed: number | null;
  summary: {
    overview: {
      average_speed: number | null;
      previous_average_speed: number | null;
      current_streak: number;
      longest_streak: number;
    };
    habits: { usual_hours: number[] | null };
  };
};

const parityCases = cases as unknown as Case[];

// 只保留 `days` 里的这几天，让 TS 的 `activityMetrics` 在和 Rust 概览相同的窗口上计算。
function onlyDays(statistics: TypingStatistics, keys: readonly string[]): TypingStatistics {
  const keep = <T>(values: Record<string, T> | undefined) =>
    Object.fromEntries(Object.entries(values ?? {}).filter(([key]) => keys.includes(key)));
  return {
    ...statistics,
    days: keep(statistics.days),
    dailyDetails: keep(statistics.dailyDetails),
    dailyActiveMs: keep(statistics.dailyActiveMs),
  };
}

function week(today: string, offset: number): string[] {
  return Array.from({ length: 7 }, (_, index) => addDays(today, -(offset + index)));
}

test.each(parityCases)("activityMetrics agrees with the Rust summary: $name", (entry) => {
  const metrics = activityMetrics(entry.statistics, entry.today);
  const overview = entry.summary.overview;
  // TS 没有活跃时间时给 0，Rust 给 null；两边说的是同一件事。
  expect(metrics.hasActivity).toBe(entry.allTimeAverageSpeed !== null);
  expect(metrics.averageSpeed).toBeCloseTo(entry.allTimeAverageSpeed ?? 0, 12);
  expect(metrics.currentStreak).toBe(overview.current_streak);
  expect(metrics.longestStreak).toBe(overview.longest_streak);

  const thisWeek = activityMetrics(onlyDays(entry.statistics, week(entry.today, 0)), entry.today);
  expect(thisWeek.averageSpeed).toBeCloseTo(overview.average_speed ?? 0, 12);
  expect(thisWeek.hasActivity).toBe(overview.average_speed !== null);
  const previousWeek = activityMetrics(
    onlyDays(entry.statistics, week(entry.today, 7)),
    entry.today,
  );
  expect(previousWeek.averageSpeed).toBeCloseTo(overview.previous_average_speed ?? 0, 12);
  expect(previousWeek.hasActivity).toBe(overview.previous_average_speed !== null);
});

test.each(parityCases)("usualHours agrees with the Rust summary: $name", (entry) => {
  const usual = usualHours(entry.statistics.dailyHours, entry.today);
  const expected = entry.summary.habits.usual_hours;
  if (expected === null) {
    expect(usual).toBeNull();
    return;
  }
  expect(usual).not.toBeNull();
  expect(usual!.hours).toHaveLength(expected.length);
  usual!.hours.forEach((value, hour) => expect(value).toBeCloseTo(expected[hour], 12));
});

test("the parity cases exercise every metric they lock", () => {
  expect(parityCases.some((entry) => entry.allTimeAverageSpeed === null)).toBe(true);
  expect(parityCases.some((entry) => (entry.allTimeAverageSpeed ?? 0) > 0)).toBe(true);
  expect(parityCases.some((entry) => entry.summary.overview.previous_average_speed !== null)).toBe(
    true,
  );
  expect(parityCases.some((entry) => entry.summary.habits.usual_hours !== null)).toBe(true);
  // 今天还没有记录、连续天数截止到昨天的情况。
  expect(
    parityCases.some(
      (entry) =>
        entry.statistics.days[entry.today] === undefined &&
        entry.summary.overview.current_streak > 0,
    ),
  ).toBe(true);
});
