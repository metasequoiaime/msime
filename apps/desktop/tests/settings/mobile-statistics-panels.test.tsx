// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MobileStatisticsOverview } from "../../../../packages/ui/src/settings/mobile-statistics-overview";
import { MobileStatisticsHabits } from "../../../../packages/ui/src/settings/mobile-statistics-habits";
import { MobileStatisticsKeys } from "../../../../packages/ui/src/settings/mobile-statistics-keys";
import type {
  HabitsSummary,
  KeysSummary,
  OverviewSummary,
} from "../../../../packages/ui/src/settings/typing-summary";

afterEach(cleanup);

const overview: OverviewSummary = {
  week_total: 0,
  previous_week_total: 0,
  last7: [{ day: "2026-10-23", count: 0 }],
  average_speed: null,
  previous_average_speed: null,
  first_candidate_rate: null,
  keystrokes_saved_rate: null,
  current_streak: 0,
  longest_streak: 0,
};
const habits: HabitsSummary = {
  weeks12: [],
  hours24: [],
  usual_hours: null,
  peak_window: null,
  active_days: 0,
  breakdown: { characters: {}, sources: {} },
};
const keys: KeysSummary = {
  per_character_keys: null,
  previous_per_character_keys: null,
  backspace_rate: null,
  prediction_rate: null,
  longest_run: null,
  positions: null,
};

test("独立概览面板区分未知与零，随摘要更新", () => {
  const { rerender } = render(
    <MobileStatisticsOverview overview={overview} card="rounded-[12px]" />,
  );
  expect(screen.getByRole("group", { name: "平均速度 —，近 7 天的活跃时间里" })).toBeTruthy();
  expect(screen.getByRole("group", { name: "首选命中 —，选词满 50 次后显示" })).toBeTruthy();
  expect(screen.getByRole("region", { name: "近 7 天概览" }).className).toContain("rounded-[12px]");
  rerender(
    <MobileStatisticsOverview
      overview={{ ...overview, average_speed: 0, first_candidate_rate: 0 }}
      card="rounded-[20px]"
    />,
  );
  expect(screen.getByRole("group", { name: "平均速度 0 字/分，近 7 天的活跃时间里" })).toBeTruthy();
  expect(screen.getByRole("group", { name: "首选命中 0 %，第一个候选就是你要的" })).toBeTruthy();
  expect(screen.getByRole("region", { name: "近 7 天概览" }).className).toContain("rounded-[20px]");
});

test("独立习惯面板补齐日历和小时，随摘要显示构成", () => {
  const { rerender } = render(<MobileStatisticsHabits habits={habits} card="card" />);
  expect(screen.getByRole("img", { name: /近 12 周输入热力图/ }).children).toHaveLength(84);
  expect(screen.getByRole("img", { name: /24 小时输入分布/ }).children).toHaveLength(24);
  expect(screen.getByText("还没有记录")).toBeTruthy();
  rerender(
    <MobileStatisticsHabits
      habits={{ ...habits, breakdown: { characters: { han: 3, latin: 1 }, sources: {} } }}
      card="card"
    />,
  );
  expect(screen.getByRole("img", { name: "输入构成：汉字 75%，英文 25%" })).toBeTruthy();
  expect(screen.queryByText("还没有记录")).toBeNull();
});

test("独立按键面板自动跟随布局，手选后跨摘要更新保留选择", () => {
  const { rerender } = render(
    <MobileStatisticsKeys keys={keys} habits={habits} counts={{ KeyA: 2 }} card="card" />,
  );
  expect(screen.getByRole("button", { name: "26 键" }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByRole("group", { name: "每字按键 —，近 7 天平均" })).toBeTruthy();
  rerender(<MobileStatisticsKeys keys={keys} habits={habits} counts={{ Nine2: 3 }} card="card" />);
  expect(screen.getByRole("button", { name: "9 键" }).getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "26 键" }));
  rerender(
    <MobileStatisticsKeys
      keys={{ ...keys, per_character_keys: 0 }}
      habits={habits}
      counts={{ Nine3: 4 }}
      card="card"
    />,
  );
  expect(screen.getByRole("button", { name: "26 键" }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByRole("group", { name: "每字按键 0 次，近 7 天平均" })).toBeTruthy();
  expect(screen.getByText("其他键")).toBeTruthy();
});
