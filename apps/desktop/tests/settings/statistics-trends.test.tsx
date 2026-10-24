// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  SpeedTrend,
  StatisticsDailyTrend,
  StatisticsHourlyBars,
} from "../../../../packages/ui/src/settings/statistics-trends";

afterEach(cleanup);

const days = Array.from({ length: 3 }, (_, index) => ({
  key: `synthetic-${index}`,
  label: `合成第 ${index + 1} 日`,
}));

test("每日柱图保留缺测日和非零日均，选择由调用方更新", () => {
  const onSelect = vi.fn();
  const props = { days, counts: { [days[0].key]: 20, [days[2].key]: 60 }, onSelect };
  const { container, rerender } = render(<StatisticsDailyTrend {...props} selectedDay={null} />);
  expect(screen.getByText(/最高 60 字符 \/ 天/).textContent).toContain("虚线为日均 40 字符");
  const buttons = screen.getAllByRole("button");
  expect(buttons).toHaveLength(3);
  expect(buttons[1].getAttribute("aria-label")).toBe("合成第 2 日，0 字符");
  expect(buttons[1].querySelector("i")?.style.height).toBe("2%");
  expect(
    Number.parseFloat(container.querySelector<HTMLElement>("i[aria-hidden]")!.style.bottom),
  ).toBeCloseTo(200 / 3);
  fireEvent.click(buttons[0]);
  expect(onSelect).toHaveBeenCalledWith(days[0].key);
  expect(buttons[0].getAttribute("aria-pressed")).toBe("false");
  rerender(<StatisticsDailyTrend {...props} selectedDay={days[0].key} />);
  expect(buttons[0].getAttribute("aria-pressed")).toBe("true");
  expect(buttons[2].querySelector("i")?.className).toContain("opacity-40");
  fireEvent.click(buttons[0]);
  expect(onSelect).toHaveBeenLastCalledWith(days[0].key);
});

test("全零范围不显示日均参考线，七日柱图保留逐日计数", () => {
  const week = Array.from({ length: 7 }, (_, index) => ({
    key: `week-${index}`,
    label: `${index}`,
  }));
  const { container } = render(
    <StatisticsDailyTrend days={week} counts={{}} selectedDay={null} onSelect={() => {}} />,
  );
  expect(screen.getByText("最高 0 字符 / 天")).toBeTruthy();
  expect(container.querySelector("i[aria-hidden]")).toBeNull();
  expect(screen.getAllByRole("button").every((button) => button.textContent === "0")).toBe(true);
});

test("手机超过 120 天才平滑折线，面积和选中点保留原始值", () => {
  const longDays = Array.from({ length: 121 }, (_, index) => ({
    key: `long-${index}`,
    label: `${index}`,
  }));
  const counts = { [longDays[0].key]: 70, [longDays[120].key]: 70 };
  const props = { counts, selectedDay: longDays[120].key, onSelect: vi.fn(), mobile: true };
  const { container, rerender } = render(<StatisticsDailyTrend {...props} days={longDays} />);
  expect(screen.getByRole("img", { name: "每日输入趋势折线图，显示七日均线" })).toBeTruthy();
  expect(container.querySelector("polyline")?.getAttribute("points")?.split(" ").at(-1)).toBe(
    `100,${96 - 88 / 7}`,
  );
  expect(container.querySelector("polygon")?.getAttribute("points")?.split(" ").slice(-2)).toEqual([
    "100,8",
    "100,100",
  ]);
  expect(container.querySelector("circle")?.getAttribute("cy")).toBe("8");
  rerender(<StatisticsDailyTrend {...props} days={longDays.slice(0, 120)} />);
  expect(screen.getByRole("img", { name: "每日输入趋势折线图" })).toBeTruthy();
  expect(container.querySelector("polyline")?.getAttribute("points")?.split(" ")[0]).toBe("0,8");
  expect(container.querySelector("circle")).toBeNull();
  expect(screen.queryByRole("button")).toBeNull();
});

test("速度缺测日断线并保留截断量程上下界，空范围显示说明", () => {
  const { container, rerender } = render(
    <SpeedTrend days={days} speeds={[60, null, 75]} averageSpeed={65} selectedDay={days[2].key} />,
  );
  expect(
    screen.getByRole("img", { name: "每日输入速度折线图，2 天有测量，平均 65 字每分钟" }),
  ).toBeTruthy();
  const path = container.querySelector("path")!.getAttribute("d")!;
  expect(path.match(/M/g)).toHaveLength(2);
  expect(path).not.toContain("L");
  expect(screen.getByText("78")).toBeTruthy();
  expect(screen.getByText("56")).toBeTruthy();
  expect(container.querySelectorAll("i.rounded-full")).toHaveLength(2);
  expect(container.querySelector("i.size-2\\.5")?.className).toContain("bg-[#e59b43]");
  rerender(
    <SpeedTrend days={days} speeds={[null, null, null]} averageSpeed={0} selectedDay={null} />,
  );
  expect(screen.queryByRole("img")).toBeNull();
  expect(screen.getByText(/还没有测量到足够的活跃时长/)).toBeTruthy();
});

test("长范围仅显示孤立速度点和选中点，更新选择即可重画", () => {
  const longDays = Array.from({ length: 46 }, (_, index) => ({
    key: `speed-${index}`,
    label: `${index}`,
  }));
  const speeds = Array.from({ length: 46 }, (_, index) =>
    index === 0 || index === 1 || index === 4 ? 60 : null,
  );
  const { container, rerender } = render(
    <SpeedTrend days={longDays} speeds={speeds} averageSpeed={60} selectedDay={longDays[0].key} />,
  );
  expect(container.querySelectorAll("i.rounded-full")).toHaveLength(2);
  rerender(<SpeedTrend days={longDays} speeds={speeds} averageSpeed={60} selectedDay={null} />);
  expect(container.querySelectorAll("i.rounded-full")).toHaveLength(1);
});

test("小时图保留 24 桶且与历史均值共用量程，可移除历史基线", () => {
  const hours = Array.from({ length: 24 }, (_, hour) => (hour === 9 ? 20 : 0));
  const usual = { hours: Array.from({ length: 24 }, (_, hour) => (hour === 21 ? 40 : 0)), days: 2 };
  const { container, rerender } = render(<StatisticsHourlyBars hours={hours} usual={usual} />);
  expect(screen.getByRole("img", { name: "今日各时段输入分布，与平时对比" })).toBeTruthy();
  expect(screen.getAllByLabelText(/^\d+ 时，\d+ 字符$/)).toHaveLength(24);
  expect(screen.getByLabelText("9 时，20 字符").querySelector("i")?.style.height).toBe("50%");
  expect(screen.getByLabelText("21 时，0 字符").getAttribute("title")).toContain("平时 40 字符");
  expect(container.querySelector("path")?.getAttribute("d")).toContain(`${(21.5 / 24) * 100},0`);
  rerender(<StatisticsHourlyBars hours={hours} usual={null} />);
  expect(screen.getByLabelText("9 时，20 字符").querySelector("i")?.style.height).toBe("100%");
  expect(container.querySelector("svg")).toBeNull();
  expect(screen.queryByText(/平时（前/)).toBeNull();
});
