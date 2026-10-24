// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { StatisticsDailyDetails } from "../../../../packages/ui/src/settings/statistics-daily-details";
import type { DailyDetailRow } from "../../../../packages/ui/src/settings/typing-statistics-model";

afterEach(cleanup);

const measured: DailyDetailRow = {
  key: "2026-04-02",
  total: 1_234,
  han: 900,
  latin: 200,
  number: 50,
  punctuation: 60,
  other: 24,
  activeMs: 5_400_000,
  speed: 12.5,
};

const cells = (row: HTMLElement) =>
  within(row)
    .getAllByRole("cell")
    .map((cell) => cell.textContent);

test("按模型顺序显示九列，保留计数、日期与速度格式", () => {
  const historical: DailyDetailRow = {
    ...measured,
    key: "2026-03-31",
    total: 100,
    han: 100,
    latin: 0,
    number: 0,
    punctuation: 0,
    other: 0,
    activeMs: null,
    speed: null,
  };
  render(<StatisticsDailyDetails rows={[measured, historical]} />);
  const region = screen.getByRole("region", { name: "按日明细 · 最近 30 天" });
  const table = within(region).getByRole("table", { name: "按日明细 · 最近 30 天" });
  const headers = within(table).getAllByRole("columnheader");
  expect(headers.map((header) => header.textContent)).toEqual([
    "日期",
    "字数",
    "汉字",
    "字母",
    "数字",
    "标点",
    "其他",
    "活跃",
    "速度",
  ]);
  expect(headers.every((header) => header.getAttribute("scope") === "col")).toBe(true);
  const rows = within(table).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(2);
  expect(cells(rows[0])).toEqual([
    "4月2日",
    "1,234",
    "900",
    "200",
    "50",
    "60",
    "24",
    "1小时30分",
    "13 字/分钟",
  ]);
  expect(cells(rows[1])).toEqual(["3月31日", "100", "100", "0", "0", "0", "0", "—", "—"]);
  expect(
    within(region).getByText(/新的在上；「其他」含其他文字、表情、符号与历史未分类/),
  ).toBeTruthy();
});

test("空态与数据更新切换，已测量的零值不显示未知符号", () => {
  const { rerender } = render(<StatisticsDailyDetails rows={[]} />);
  expect(screen.getByRole("heading", { name: "按日明细 · 最近 30 天" })).toBeTruthy();
  expect(screen.getByText("暂无输入记录")).toBeTruthy();
  expect(screen.queryByRole("table")).toBeNull();
  expect(screen.getByText(/列出最近 30 个有记录的日期/)).toBeTruthy();
  rerender(<StatisticsDailyDetails rows={[{ ...measured, activeMs: 0, speed: 0 }]} />);
  const table = screen.getByRole("table");
  expect(cells(within(table).getAllByRole("row")[1]).slice(-2)).toEqual(["0分", "0 字/分钟"]);
  expect(screen.queryByText("暂无输入记录")).toBeNull();
  rerender(<StatisticsDailyDetails rows={[]} />);
  expect(screen.queryByRole("table")).toBeNull();
  expect(screen.getByText("暂无输入记录")).toBeTruthy();
});
