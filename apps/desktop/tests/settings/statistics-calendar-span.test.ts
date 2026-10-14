import { afterEach, expect, test, vi } from "vitest";
import {
  dayKey,
  mobileTrendLength,
  recentDays,
} from "../../../../packages/ui/src/settings/typing-statistics-helpers";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllEnvs();
});

test.each([
  {
    zone: "America/New_York",
    now: "2026-04-15T12:00:00-04:00",
    first: "2026-03-01",
    last: "2026-04-15",
  },
  {
    zone: "Europe/Berlin",
    now: "2026-04-15T12:00:00+02:00",
    first: "2026-03-01",
    last: "2026-04-15",
  },
  {
    zone: "Australia/Sydney",
    now: "2026-11-15T12:00:00+11:00",
    first: "2026-10-01",
    last: "2026-11-15",
  },
  {
    zone: "Australia/Lord_Howe",
    now: "2026-11-15T12:00:00+11:00",
    first: "2026-10-01",
    last: "2026-11-15",
  },
  {
    zone: "America/New_York",
    now: "2026-11-15T12:00:00-05:00",
    first: "2026-10-01",
    last: "2026-11-15",
  },
  { zone: "Asia/Tokyo", now: "2026-04-15T12:00:00+09:00", first: "2026-03-01", last: "2026-04-15" },
  {
    zone: "America/New_York",
    now: "2026-04-15T23:30:00-04:00",
    first: "2026-03-01",
    last: "2026-04-15",
  },
  { zone: "Asia/Tokyo", now: "2026-04-15T00:30:00+09:00", first: "2026-03-01", last: "2026-04-15" },
])("$zone 在 $now 的趋势包含最早记录与本地今天", ({ zone, now, first, last }) => {
  vi.stubEnv("TZ", zone);
  vi.useFakeTimers();
  vi.setSystemTime(new Date(now));
  expect(dayKey(new Date())).toBe(last);

  const length = mobileTrendLength({ [first]: 123 });
  const range = recentDays(length);
  expect(range[0]?.key).toBe(first);
  expect(range.at(-1)?.key).toBe(last);
  expect(length).toBe(46);
});

test("趋势的默认长度和最长一年边界保持有效", () => {
  vi.stubEnv("TZ", "America/New_York");
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-04-15T12:00:00-04:00"));

  expect(mobileTrendLength({})).toBe(30);
  expect(mobileTrendLength({ "2026-04-15": 123 })).toBe(30);
  expect(mobileTrendLength({ "2024-01-01": 123 })).toBe(366);
});
