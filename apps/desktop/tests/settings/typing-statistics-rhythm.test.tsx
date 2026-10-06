// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import {
  TypingStatisticsPage,
  type TypingStatistics,
  type TypingStatisticsStatus,
} from "@msime/ui";

afterEach(cleanup);

// 页面默认打开「按键」，时段分布、速度趋势和按日明细都在「趋势」标签下。
async function openTrend() {
  fireEvent.click(await screen.findByRole("tab", { name: "趋势" }));
}

function today(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}

function client(statistics: TypingStatistics) {
  const status: TypingStatisticsStatus = { statistics, availability: "ready" };
  return {
    load: () => Promise.resolve(status),
    setEnabled: () => Promise.resolve(status),
    reset: () => Promise.resolve(status),
  };
}

test("the rhythm cards read the measured activity", async () => {
  const key = today();
  const hours = Array.from({ length: 24 }, (_, hour) => (hour === 9 ? 360 : 0));
  render(
    <TypingStatisticsPage
      client={client({
        enabled: true,
        total: 360,
        days: { [key]: 360 },
        dailyDetails: { [key]: { characters: { han: 360 } } },
        dailyActiveMs: { [key]: 120_000 },
        dailyHours: { [key]: hours },
      })}
    />,
  );
  // 360 characters over two active minutes.
  expect((await screen.findByLabelText("今日输入速度")).textContent).toContain("180");
  expect(screen.getByLabelText("平均输入速度").textContent).toContain("180");
  // 平均输入速度下方写着累计活跃时长。
  expect(screen.getByText("共 2分")).toBeTruthy();
  expect(screen.getByLabelText("今日活跃时长").textContent).toContain("2分");
  expect(screen.getByLabelText("连续输入天数").textContent).toContain("1");
  await openTrend();
  expect(screen.getByLabelText("今日各时段输入分布")).toBeTruthy();
  expect(screen.getByLabelText("9 时，360 字符")).toBeTruthy();
});

test("a document with no measured activity says so instead of showing a zero speed", async () => {
  const key = today();
  render(
    <TypingStatisticsPage client={client({ enabled: true, total: 120, days: { [key]: 120 } })} />,
  );
  expect((await screen.findByLabelText("今日输入速度")).textContent).toContain("0");
  // Nothing has ever timed typing here, which is different from typing at zero speed.
  expect(screen.getByText(/还没有测量到活跃时长/)).toBeTruthy();
  expect(screen.queryByText(/^共 /)).toBeNull();
  // No hours recorded, so the section is absent rather than drawn empty.
  expect(screen.queryByLabelText("今日各时段输入分布")).toBeNull();
});

function offsetKey(offset: number): string {
  const now = new Date();
  const date = new Date(now.getFullYear(), now.getMonth(), now.getDate() + offset);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function offsetLabel(offset: number): string {
  const now = new Date();
  const date = new Date(now.getFullYear(), now.getMonth(), now.getDate() + offset);
  return `${date.getMonth() + 1}月${date.getDate()}日`;
}

const detailed: TypingStatistics = {
  enabled: true,
  total: 1_300,
  days: { [offsetKey(0)]: 1_200, [offsetKey(-3)]: 100 },
  dailyDetails: {
    [offsetKey(0)]: {
      characters: { han: 900, latin: 180, number: 40, punctuation: 60, emoji: 20 },
    },
    [offsetKey(-3)]: { characters: { han: 100 } },
  },
  dailyActiveMs: { [offsetKey(0)]: 5_400_000 },
};

test("the desktop page lists recorded days in the per-day detail table", async () => {
  render(<TypingStatisticsPage client={client(detailed)} />);
  await openTrend();
  expect(await screen.findByRole("heading", { name: "按日明细 · 最近 30 天" })).toBeTruthy();
  const table = screen.getByRole("table", { name: "按日明细 · 最近 30 天" });
  expect(
    within(table)
      .getAllByRole("columnheader")
      .map((cell) => cell.textContent),
  ).toEqual(["日期", "字数", "汉字", "字母", "数字", "标点", "其他", "活跃", "速度"]);
  const rows = within(table).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(2);
  const cells = (row: HTMLElement) =>
    within(row)
      .getAllByRole("cell")
      .map((cell) => cell.textContent);
  // 1,080 readable characters over 90 active minutes.
  expect(cells(rows[0])).toEqual([
    offsetLabel(0),
    "1,200",
    "900",
    "180",
    "40",
    "60",
    "20",
    "1小时30分",
    "12 字/分钟",
  ]);
  // A day with no measured active time is unknown in both columns, not zero.
  expect(cells(rows[1])).toEqual([offsetLabel(-3), "100", "100", "0", "0", "0", "0", "—", "—"]);
});

test("the phone layout leaves the per-day detail table out", async () => {
  render(<TypingStatisticsPage client={client(detailed)} mobile />);
  expect(await screen.findByLabelText("今日输入速度")).toBeTruthy();
  expect(screen.queryByRole("heading", { name: /按日明细/ })).toBeNull();
  expect(screen.queryByRole("table")).toBeNull();
});

test("today's hours are set against the usual day once earlier days recorded hours", async () => {
  const key = today();
  const [year, month, day] = key.split("-").map(Number);
  const earlier = new Date(year, month - 1, day - 1);
  const yesterday = `${earlier.getFullYear()}-${String(earlier.getMonth() + 1).padStart(2, "0")}-${String(earlier.getDate()).padStart(2, "0")}`;
  const hours = (hour: number, count: number) =>
    Array.from({ length: 24 }, (_, index) => (index === hour ? count : 0));
  render(
    <TypingStatisticsPage
      client={client({
        enabled: true,
        total: 460,
        days: { [key]: 360, [yesterday]: 100 },
        dailyDetails: {
          [key]: { characters: { han: 360 } },
          [yesterday]: { characters: { han: 100 } },
        },
        dailyActiveMs: { [key]: 120_000, [yesterday]: 100_000 },
        dailyHours: { [key]: hours(9, 360), [yesterday]: hours(21, 100) },
      })}
    />,
  );
  await openTrend();
  expect(await screen.findByRole("img", { name: "今日各时段输入分布，与平时对比" })).toBeTruthy();
  expect(screen.getByText("平时（前 1 天平均）")).toBeTruthy();
  expect(screen.getByLabelText("21 时，0 字符").getAttribute("title")).toBe(
    "21 时：今日 0 字符，平时 100 字符",
  );
  // Both days held a minute of typing, so the speed trend draws them and their average.
  expect(
    screen.getByRole("img", { name: "每日输入速度折线图，2 天有测量，平均 125 字每分钟" }),
  ).toBeTruthy();
});

test("the speed trend explains itself until a day holds a minute of typing", async () => {
  const key = today();
  render(
    <TypingStatisticsPage
      client={client({
        enabled: true,
        total: 10,
        days: { [key]: 10 },
        dailyDetails: { [key]: { characters: { han: 10 } } },
        dailyActiveMs: { [key]: 5_000 },
      })}
    />,
  );
  await openTrend();
  expect(await screen.findByRole("heading", { name: "速度趋势" })).toBeTruthy();
  expect(screen.queryByRole("img", { name: /每日输入速度折线图/ })).toBeNull();
  expect(screen.getByText(/还没有测量到足够的活跃时长/)).toBeTruthy();
});

test("desktop shares are donuts beside their legend and schemes are a ranking", async () => {
  const key = today();
  render(
    <TypingStatisticsPage
      client={client({
        enabled: true,
        total: 100,
        days: { [key]: 100 },
        detail: {
          characters: { han: 70, latin: 30 },
          sources: { quanpin: 60, wubi: 30, english: 10 },
        },
        dailyDetails: {
          [key]: {
            characters: { han: 70, latin: 30 },
            sources: { quanpin: 60, wubi: 30, english: 10 },
          },
        },
      })}
    />,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "类型" }));
  const donut = screen.getByRole("img", { name: "字符类型环形图" });
  expect(screen.getByLabelText("汉字 70 字符，70.0%")).toBeTruthy();
  // 环是两段实心扇区，不再是 conic-gradient 加遮罩画出的细线；总数写在圆心。
  const segments = donut.querySelectorAll("[data-donut-segment]");
  expect(Array.from(segments).map((segment) => segment.tagName.toLowerCase())).toEqual([
    "path",
    "path",
  ]);
  expect(within(donut).getByText("100")).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "方案" }));
  const ranking = screen.getByRole("img", { name: "输入方案排行" });
  // Largest first, and only schemes that were used: the ranking is the legend.
  expect(Array.from(ranking.children).map((row) => row.getAttribute("aria-label"))).toEqual([
    "全拼 26 键 60 字符，60.0%",
    "五笔 30 字符，30.0%",
    "英文键盘 10 字符，10.0%",
  ]);
  expect(screen.getAllByLabelText(/^全拼 26 键 /)).toHaveLength(1);
});
