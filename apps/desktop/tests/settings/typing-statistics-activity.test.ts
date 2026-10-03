import { expect, test } from "vitest";
import {
  activityMetrics,
  addDays,
  currentStreak,
  dailyDetailRows,
  dailySpeeds,
  formatActiveTime,
  longestStreak,
  statisticsOverviewDetails,
  statisticsOverviewMetrics,
  usualHours,
  type TypingStatistics,
} from "@msime/ui";

function statistics(overrides: Partial<TypingStatistics> = {}): TypingStatistics {
  return { enabled: true, total: 0, days: {}, ...overrides };
}

test("day arithmetic crosses months, years and leap days without a calendar library", () => {
  expect(addDays("2026-09-21", 1)).toBe("2026-09-22");
  expect(addDays("2026-09-01", -1)).toBe("2026-08-31");
  expect(addDays("2026-01-01", -1)).toBe("2025-12-31");
  expect(addDays("2026-12-31", 1)).toBe("2027-01-01");
  // 2028 is a leap year and 2026 is not, so the day after 2 February differs between them.
  expect(addDays("2028-02-28", 1)).toBe("2028-02-29");
  expect(addDays("2026-02-28", 1)).toBe("2026-03-01");
  // A key that is not a date is returned unchanged rather than becoming "NaN-NaN-NaN".
  expect(addDays("not-a-day", 1)).toBe("not-a-day");
});

test("today still in progress does not break a streak", () => {
  const recorded = ["2026-09-18", "2026-09-19", "2026-09-20"];
  // Nothing typed today yet: the run ending yesterday is the current streak.
  expect(currentStreak(recorded, "2026-09-21")).toBe(3);
  // Typing today extends it.
  expect(currentStreak([...recorded, "2026-09-21"], "2026-09-21")).toBe(4);
  // A missing yesterday ends it, whatever came before.
  expect(currentStreak(recorded, "2026-09-22")).toBe(0);
  expect(currentStreak([], "2026-09-21")).toBe(0);
});

test("longest streak counts the longest run, not the last one", () => {
  expect(longestStreak(["2026-09-01", "2026-09-02", "2026-09-03", "2026-09-10"])).toBe(3);
  expect(longestStreak(["2026-09-01", "2026-09-03", "2026-09-05"])).toBe(1);
  expect(longestStreak(["2026-09-01"])).toBe(1);
  expect(longestStreak([])).toBe(0);
  // Across a month boundary, which a naive numeric comparison of the keys would miss.
  expect(longestStreak(["2026-08-30", "2026-08-31", "2026-09-01"])).toBe(3);
});

test("speed counts readable characters per active minute", () => {
  const value = statistics({
    total: 600,
    days: { "2026-09-21": 600 },
    dailyDetails: {
      "2026-09-21": {
        characters: { han: 200, latin: 100, otherLetter: 60, number: 100, punctuation: 140 },
      },
    },
    dailyActiveMs: { "2026-09-21": 120_000 },
  });
  const metrics = activityMetrics(value, "2026-09-21");
  // 360 readable characters over two active minutes. Digits and punctuation stay out: a phone
  // number typed quickly is not prose and would read as a burst of speed.
  expect(metrics.todaySpeed).toBe(180);
  expect(metrics.averageSpeed).toBe(180);
  expect(metrics.todayActiveMs).toBe(120_000);
  expect(metrics.hasActivity).toBe(true);
});

test("kana count toward speed, unlike the Windows baseline", () => {
  const value = statistics({
    total: 60,
    days: { "2026-09-21": 60 },
    dailyDetails: { "2026-09-21": { characters: { otherLetter: 60 } } },
    dailyActiveMs: { "2026-09-21": 60_000 },
  });
  // The baseline files kana under "other" and would report a Japanese-only day as zero speed.
  expect(activityMetrics(value, "2026-09-21").todaySpeed).toBe(60);
});

test("days that predate the measurement are unknown rather than instant", () => {
  const value = statistics({
    total: 500,
    days: { "2026-09-19": 200, "2026-09-20": 300 },
    dailyDetails: { "2026-09-20": { characters: { han: 300 } } },
    // Only one of the two days was measured.
    dailyActiveMs: { "2026-09-20": 60_000 },
  });
  const metrics = activityMetrics(value, "2026-09-21");
  // The unmeasured day contributes neither characters nor time to the average; including it as
  // zero time would divide by zero, and including its characters would inflate the rate.
  expect(metrics.averageSpeed).toBe(300);
  expect(metrics.totalActiveMs).toBe(60_000);
  // It still counts as a recorded day for the per-day average and the streaks.
  expect(metrics.recordedDays).toBe(2);
  expect(metrics.averagePerDay).toBe(250);
  expect(metrics.currentStreak).toBe(2);
});

test("the per-day average divides the recorded days, not a total that outlived them", () => {
  // A document may keep a running total above what its remaining days hold. The baseline divides the sum of its day rows by their count, so the dropped history must not inflate the average.
  const value = statistics({
    total: 900,
    days: { "2026-09-19": 200, "2026-09-20": 300 },
  });
  const metrics = activityMetrics(value, "2026-09-21");
  expect(metrics.recordedDays).toBe(2);
  expect(metrics.averagePerDay).toBe(250);
});

test("a day too short to mean anything cannot win fastest", () => {
  const value = statistics({
    total: 120,
    days: { "2026-09-19": 20, "2026-09-20": 100 },
    dailyDetails: {
      "2026-09-19": { characters: { han: 20 } },
      "2026-09-20": { characters: { han: 100 } },
    },
    // Twenty characters in two seconds is 600/min and would top the ranking forever.
    dailyActiveMs: { "2026-09-19": 2_000, "2026-09-20": 120_000 },
  });
  const metrics = activityMetrics(value, "2026-09-21");
  expect(metrics.fastestDay).toBe("2026-09-20");
  expect(metrics.fastestSpeed).toBe(50);
  // The short day is still part of the average, which is a ratio of totals rather than a ranking.
  expect(metrics.averageSpeed).toBeCloseTo((120 / 122_000) * 60_000, 6);
});

test("the best day is the one with the most characters, earliest on a tie", () => {
  const value = statistics({
    total: 300,
    days: { "2026-09-19": 150, "2026-09-20": 100, "2026-09-21": 150 },
  });
  const metrics = activityMetrics(value, "2026-09-21");
  expect(metrics.bestDay).toBe("2026-09-19");
  expect(metrics.bestDayCharacters).toBe(150);
});

test("an empty document derives zeroes rather than NaN", () => {
  const metrics = activityMetrics(statistics(), "2026-09-21");
  expect(metrics).toMatchObject({
    recordedDays: 0,
    averagePerDay: 0,
    todaySpeed: 0,
    averageSpeed: 0,
    fastestSpeed: 0,
    fastestDay: null,
    currentStreak: 0,
    longestStreak: 0,
    bestDay: null,
    todayHours: null,
    hasActivity: false,
  });
});

test("overview metric descriptors keep labels, units and notes together", () => {
  const activity = {
    recordedDays: 1,
    averagePerDay: 12,
    todayActiveMs: 120_000,
    totalActiveMs: 120_000,
    todaySpeed: 6,
    averageSpeed: 6,
    fastestSpeed: 6,
    fastestDay: "2026-09-21",
    currentStreak: 1,
    longestStreak: 1,
    bestDay: "2026-09-21",
    bestDayCharacters: 12,
    todayHours: null,
    hasActivity: true,
  };
  const metrics = statisticsOverviewMetrics({
    statistics: statistics({ days: { "2026-09-21": 12 } }),
    todayKey: "2026-09-21",
    scopeTitle: "累计输入",
    scopeTotal: 12,
    activity,
  });

  expect(metrics).toEqual([
    { label: "今日输入", value: "12", unit: "字符", ariaLabel: "今日输入字符数" },
    { label: "累计输入", value: "12", unit: "字符", ariaLabel: "当前范围输入字符数" },
    {
      label: "今日活跃",
      title: "连续打字的时间",
      value: "2分",
      ariaLabel: "今日活跃时长",
    },
    { label: "今日速度", value: "6", unit: "字 / 分钟", ariaLabel: "今日输入速度" },
    {
      label: "平均速度",
      value: "6",
      unit: "字 / 分钟",
      ariaLabel: "平均输入速度",
      note: "共 2分",
    },
    {
      label: "连续天数",
      value: "1",
      unit: "天",
      ariaLabel: "连续输入天数",
      note: "最长 1 天",
    },
  ]);
  expect(statisticsOverviewDetails(activity)).toEqual({
    summary: "日均 12 字符 · 1 天有记录",
    best: "最多 9月21日，12 字符 · 最快 9月21日，6 字 / 分钟",
  });
});

test("hourly buckets are only used when they describe a whole day", () => {
  const hours = Array.from({ length: 24 }, (_, hour) => (hour === 9 ? 42 : 0));
  expect(
    activityMetrics(
      statistics({ total: 42, days: { "2026-09-21": 42 }, dailyHours: { "2026-09-21": hours } }),
      "2026-09-21",
    ).todayHours,
  ).toEqual(hours);
  // A truncated list is dropped rather than padded: padding would silently move the missing
  // hours' typing to midnight.
  expect(
    activityMetrics(
      statistics({ total: 42, days: { "2026-09-21": 42 }, dailyHours: { "2026-09-21": [42, 0] } }),
      "2026-09-21",
    ).todayHours,
  ).toBeNull();
  // Yesterday's buckets are not today's.
  expect(
    activityMetrics(
      statistics({ total: 42, days: { "2026-09-20": 42 }, dailyHours: { "2026-09-20": hours } }),
      "2026-09-21",
    ).todayHours,
  ).toBeNull();
});

test("active time reads as a duration rather than milliseconds", () => {
  expect(formatActiveTime(0)).toBe("0分");
  expect(formatActiveTime(-1)).toBe("0分");
  expect(formatActiveTime(45_000)).toBe("45秒");
  expect(formatActiveTime(12 * 60_000)).toBe("12分");
  expect(formatActiveTime(60 * 60_000)).toBe("1小时");
  expect(formatActiveTime(83 * 60_000)).toBe("1小时23分");
});

test("the per-day details list recorded days up to today, newest first and capped at 30", () => {
  const days: Record<string, number> = {};
  for (let offset = 0; offset < 40; offset += 1) days[addDays("2026-09-21", -offset)] = offset + 1;
  // A key after today (a clock that moved back) and a key that is not a day never become rows.
  days["2026-09-22"] = 5;
  days.bogus = 7;
  const rows = dailyDetailRows(statistics({ total: 0, days }), "2026-09-21");
  expect(rows).toHaveLength(30);
  expect(rows[0].key).toBe("2026-09-21");
  expect(rows[1].key).toBe("2026-09-20");
  expect(rows.at(-1)?.key).toBe(addDays("2026-09-21", -29));
  // Days with no record are not rows, so the window is 30 recorded days rather than 30 calendar days.
  const sparse = dailyDetailRows(
    statistics({ days: { "2026-01-02": 1, "2026-09-01": 2, "2026-09-21": 3 } }),
    "2026-09-21",
  );
  expect(sparse.map((row) => row.key)).toEqual(["2026-09-21", "2026-09-01", "2026-01-02"]);
  expect(dailyDetailRows(statistics(), "2026-09-21")).toEqual([]);
});

test("the per-day detail columns fold the finer classes into 其他", () => {
  const [row] = dailyDetailRows(
    statistics({
      total: 100,
      days: { "2026-09-21": 100 },
      dailyDetails: {
        "2026-09-21": {
          characters: {
            han: 30,
            latin: 20,
            otherLetter: 10,
            number: 5,
            punctuation: 8,
            emoji: 2,
            symbol: 3,
          },
        },
      },
      dailyActiveMs: { "2026-09-21": 120_000 },
    }),
    "2026-09-21",
  );
  expect(row).toMatchObject({ total: 100, han: 30, latin: 20, number: 5, punctuation: 8 });
  // otherLetter 10 + emoji 2 + symbol 3 + the 22 characters the breakdown does not classify.
  expect(row.other).toBe(37);
  expect(row.activeMs).toBe(120_000);
  // Speed counts han, latin and otherLetter only: 60 characters over two minutes.
  expect(row.speed).toBe(30);
});

test("a day that predates active-time measurement has unknown activity and speed, not zero", () => {
  const rows = dailyDetailRows(
    statistics({
      total: 20,
      days: { "2026-09-20": 10, "2026-09-21": 10 },
      dailyDetails: {
        "2026-09-20": { characters: { han: 10 } },
        "2026-09-21": { characters: { han: 10 } },
      },
      dailyActiveMs: { "2026-09-21": 0 },
    }),
    "2026-09-21",
  );
  // Measured at zero is a known value; absent is not.
  expect(rows[0]).toMatchObject({ key: "2026-09-21", activeMs: 0, speed: 0 });
  expect(rows[1]).toMatchObject({ key: "2026-09-20", activeMs: null, speed: null });
  const [unmeasured] = dailyDetailRows(
    statistics({ total: 5, days: { "2026-09-21": 5 } }),
    "2026-09-21",
  );
  expect(unmeasured.activeMs).toBeNull();
  expect(unmeasured.speed).toBeNull();
  // With no breakdown at all, the whole day is unclassified and lands in 其他.
  expect(unmeasured.other).toBe(5);
});

test("the usual hourly profile averages earlier whole days only", () => {
  const morning = Array.from({ length: 24 }, (_, hour) => (hour === 9 ? 40 : 0));
  const evening = Array.from({ length: 24 }, (_, hour) => (hour === 21 ? 20 : 0));
  const usual = usualHours(
    {
      "2026-09-19": morning,
      "2026-09-20": evening,
      // Today is what the profile is compared against, so it stays out of it.
      "2026-09-21": morning,
      // A truncated day is unknown, and a day with every bucket empty recorded no hours at all.
      "2026-09-18": [5, 5],
      "2026-09-17": Array.from({ length: 24 }, () => 0),
    },
    "2026-09-21",
  );
  expect(usual?.days).toBe(2);
  expect(usual?.hours[9]).toBe(20);
  expect(usual?.hours[21]).toBe(10);
  expect(usual?.hours[0]).toBe(0);
  expect(usualHours({ "2026-09-21": morning }, "2026-09-21")).toBeNull();
  expect(usualHours(undefined, "2026-09-21")).toBeNull();
});

test("a day's speed is unknown until it holds a minute of active time", () => {
  const value = statistics({
    total: 330,
    days: { "2026-09-19": 120, "2026-09-20": 10, "2026-09-21": 200 },
    dailyDetails: {
      "2026-09-19": { characters: { han: 120 } },
      "2026-09-20": { characters: { han: 10 } },
      "2026-09-21": { characters: { han: 150, number: 50 } },
    },
    // 2026-09-20 typed ten characters in two seconds: a spike, not a speed.
    dailyActiveMs: { "2026-09-19": 120_000, "2026-09-20": 2_000, "2026-09-21": 60_000 },
  });
  expect(dailySpeeds(value, ["2026-09-18", "2026-09-19", "2026-09-20", "2026-09-21"])).toEqual([
    null,
    60,
    null,
    // Digits stay out of speed, as everywhere else.
    150,
  ]);
});
