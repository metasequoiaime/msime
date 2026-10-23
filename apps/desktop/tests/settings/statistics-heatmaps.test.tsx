// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import {
  KeyboardHeatmap,
  StatisticsHeatmap,
} from "../../../../packages/ui/src/settings/statistics-heatmaps";
import { StatisticsChartButton } from "../../../../packages/ui/src/settings/statistics-chart-button";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("图表按钮保持原生按钮和受控选择语义", () => {
  const onClick = vi.fn();
  const { rerender } = render(
    <StatisticsChartButton
      className="chart-test"
      title="合成日期"
      ariaLabel="选择合成日期"
      selected={false}
      onClick={onClick}
    >
      12
    </StatisticsChartButton>,
  );
  const button = screen.getByRole("button", { name: "选择合成日期" });
  expect(button.getAttribute("type")).toBe("button");
  expect(button.getAttribute("aria-pressed")).toBe("false");
  expect(button.getAttribute("title")).toBe("合成日期");
  expect(button.className).toBe("chart-test");
  expect(button.textContent).toBe("12");
  fireEvent.click(button);
  expect(onClick).toHaveBeenCalledTimes(1);
  expect(button.getAttribute("aria-pressed")).toBe("false");
  rerender(
    <StatisticsChartButton
      className="chart-test"
      title="合成日期"
      ariaLabel="选择合成日期"
      selected
      onClick={onClick}
    />,
  );
  expect(button.getAttribute("aria-pressed")).toBe("true");
});

test("日历热力图回传完整日期并由外部选择驱动高亮", () => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(2026, 9, 21, 12));
  const onSelect = vi.fn();
  // 53 周会包含上一年的同月日，用唯一计数区分未来日与上一年的空日。
  const days = { "2026-10-20": 20, "2026-10-21": 5, "2026-10-22": 99 };
  const { rerender } = render(
    <StatisticsHeatmap days={days} selectedDay={null} onSelect={onSelect} />,
  );
  const group = screen.getByRole("group", { name: "每日输入热力图" });
  const yesterday = within(group).getByRole("button", { name: "热力图：10月20日，20 字符" });
  expect(yesterday.title).toBe("10月20日：20 字符");
  expect(yesterday.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(yesterday);
  expect(onSelect).toHaveBeenCalledWith("2026-10-20");
  expect(yesterday.getAttribute("aria-pressed")).toBe("false");
  rerender(<StatisticsHeatmap days={days} selectedDay="2026-10-20" onSelect={onSelect} />);
  expect(yesterday.getAttribute("aria-pressed")).toBe("true");
  expect(within(group).queryByRole("button", { name: "热力图：10月22日，99 字符" })).toBeNull();
  expect(within(group).getByRole("button", { name: "热力图：10月19日，0 字符" }).title).toBe(
    "10月19日：无记录",
  );
});

test("日历热力图跟随新的计数快照并保留空日选择", () => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(2026, 9, 21, 12));
  const onSelect = vi.fn();
  const { rerender } = render(
    <StatisticsHeatmap days={{}} selectedDay={null} onSelect={onSelect} />,
  );
  fireEvent.click(screen.getByRole("button", { name: "热力图：10月19日，0 字符" }));
  expect(onSelect).toHaveBeenCalledWith("2026-10-19");
  rerender(
    <StatisticsHeatmap days={{ "2026-10-19": 8 }} selectedDay="2026-10-19" onSelect={onSelect} />,
  );
  expect(
    screen.getByRole("button", { name: "热力图：10月19日，8 字符" }).getAttribute("aria-pressed"),
  ).toBe("true");
});

test("按键热力图随日期范围切换并保留平台键名和其他键", () => {
  const dailyKeys = {
    "2026-10-20": { KeyA: 4, MetaLeft: 2, ArrowUp: 1 },
    "2026-10-21": { KeyA: 3, KeyB: 8 },
  };
  const { rerender } = render(
    <KeyboardHeatmap
      dailyKeys={dailyKeys}
      scopeKeys={null}
      scopeLabel="累计"
      mobile={false}
      platform="macos"
    />,
  );
  const group = screen.getByRole("group", { name: "按键热力图" });
  expect(within(group).getByRole("img", { name: "A，7 次" })).toBeTruthy();
  expect(within(group).getByRole("img", { name: "左 Command，2 次" })).toBeTruthy();
  expect(screen.getByText("共 18 次按键，颜色越深按得越多")).toBeTruthy();
  expect(
    within(screen.getByRole("list", { name: "其他键" })).getByLabelText("上箭头，1 次"),
  ).toBeTruthy();
  expect(
    within(screen.getByRole("list", { name: "最常按的键" })).getAllByRole("listitem")[0]
      .textContent,
  ).toContain("B");
  rerender(
    <KeyboardHeatmap
      dailyKeys={dailyKeys}
      scopeKeys={["2026-10-20"]}
      scopeLabel="10月20日"
      mobile={false}
      platform="windows"
    />,
  );
  expect(screen.getByRole("heading", { name: "按键热力图 · 10月20日" })).toBeTruthy();
  expect(within(group).getByRole("img", { name: "A，4 次" })).toBeTruthy();
  expect(within(group).getByRole("img", { name: "左 Win，2 次" })).toBeTruthy();
  expect(within(group).getByRole("img", { name: "B，0 次" })).toBeTruthy();
});

test("通用触屏按键热力图按数据显隐九键并保留未绘制键", () => {
  const { rerender } = render(
    <KeyboardHeatmap
      dailyKeys={{ day: { KeyQ: 2, Nine2: 4, Digit1: 1 } }}
      scopeKeys={null}
      scopeLabel="累计"
      mobile
    />,
  );
  expect(screen.getByRole("img", { name: "九宫格 2，4 次" })).toBeTruthy();
  expect(screen.getByRole("group", { name: "九宫格按键" })).toBeTruthy();
  expect(
    within(screen.getByRole("list", { name: "其他键" })).getByLabelText("1，1 次"),
  ).toBeTruthy();
  rerender(
    <KeyboardHeatmap dailyKeys={{ day: { KeyQ: 2 } }} scopeKeys={null} scopeLabel="累计" mobile />,
  );
  expect(screen.queryByRole("group", { name: "九宫格按键" })).toBeNull();
  expect(screen.queryByRole("list", { name: "其他键" })).toBeNull();
  expect(screen.getByRole("img", { name: "Q，2 次" })).toBeTruthy();
});

test("旧文档与没有记录的范围显示空态且保留隐私说明", () => {
  const { rerender } = render(
    <KeyboardHeatmap dailyKeys={undefined} scopeKeys={null} scopeLabel="累计" mobile={false} />,
  );
  expect(screen.getByText("这段时间还没有按键记录")).toBeTruthy();
  expect(screen.queryByRole("group", { name: "按键热力图" })).toBeNull();
  expect(screen.getByText(/不保存按键顺序和输入内容/)).toBeTruthy();
  rerender(
    <KeyboardHeatmap
      dailyKeys={{ "2026-10-20": { KeyA: 3 } }}
      scopeKeys={["2026-10-21"]}
      scopeLabel="10月21日"
      mobile
    />,
  );
  expect(screen.getByText("这段时间还没有按键记录")).toBeTruthy();
});
