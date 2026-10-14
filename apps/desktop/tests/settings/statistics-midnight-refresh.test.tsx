// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { TypingStatisticsPage, type TypingStatisticsStatus } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const recorded: TypingStatisticsStatus = {
  statistics: { enabled: true, total: 4, days: { "2026-06-17": 4 } },
  availability: "ready",
  lastWrittenMs: new Date(2026, 5, 17, 23, 59).getTime(),
};

test.each([false, true])(
  "午夜后轮询在统计未变化时更新趋势和今日指标（mobile=%s）",
  async (mobile) => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 5, 17, 23, 59, 58));
    const load = vi.fn().mockResolvedValue(recorded);
    await act(async () => {
      render(
        <TypingStatisticsPage
          client={{ load, setEnabled: vi.fn(), reset: vi.fn() }}
          mobile={mobile}
        />,
      );
    });
    fireEvent.click(screen.getByRole("tab", { name: "趋势" }));
    const trend = screen.getByRole("heading", { name: /每日趋势/ }).parentElement!;
    expect(within(trend).getByText("6月17日")).toBeTruthy();
    expect(screen.getByLabelText("今日输入字符数").textContent).toBe("4");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });

    expect(load).toHaveBeenCalledTimes(2);
    expect(
      within(screen.getByRole("heading", { name: /每日趋势/ }).parentElement!).getByText("6月18日"),
    ).toBeTruthy();
    expect(screen.getByLabelText("今日输入字符数").textContent).toBe("0");
  },
);

test.each([false, true])("回到前台后趋势随本地日期更新（mobile=%s）", async (mobile) => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 5, 17, 23, 59, 58));
  const load = vi.fn().mockResolvedValue(recorded);
  await act(async () => {
    render(
      <TypingStatisticsPage
        client={{ load, setEnabled: vi.fn(), reset: vi.fn() }}
        mobile={mobile}
      />,
    );
  });
  fireEvent.click(screen.getByRole("tab", { name: "趋势" }));
  vi.setSystemTime(new Date(2026, 5, 18, 0, 0, 2));
  await act(async () => {
    window.dispatchEvent(new Event("focus"));
  });

  expect(load).toHaveBeenCalledTimes(2);
  expect(
    within(screen.getByRole("heading", { name: /每日趋势/ }).parentElement!).getByText("6月18日"),
  ).toBeTruthy();
  expect(screen.getByLabelText("今日输入字符数").textContent).toBe("0");
});

test.each([false, true])("午夜后手动刷新把新记录纳入趋势（mobile=%s）", async (mobile) => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 5, 17, 23, 59, 58));
  const load = vi.fn().mockResolvedValue(recorded);
  await act(async () => {
    render(
      <TypingStatisticsPage
        client={{ load, setEnabled: vi.fn(), reset: vi.fn() }}
        mobile={mobile}
      />,
    );
  });
  fireEvent.click(screen.getByRole("tab", { name: "趋势" }));
  vi.setSystemTime(new Date(2026, 5, 18, 0, 0, 2));
  load.mockResolvedValue({
    ...recorded,
    statistics: { ...recorded.statistics, total: 7, days: { "2026-06-17": 4, "2026-06-18": 3 } },
  });
  if (mobile) fireEvent.click(document.querySelector('summary[aria-label="统计选项"]')!);
  await act(async () => {
    fireEvent.click(screen.getByRole(mobile ? "menuitem" : "button", { name: "刷新统计" }));
  });

  const trend = screen.getByRole("heading", { name: /每日趋势/ }).parentElement!;
  expect(within(trend).getByText("6月18日")).toBeTruthy();
  expect(screen.getByLabelText("今日输入字符数").textContent).toBe("3");
  if (!mobile) expect(within(trend).getByRole("button", { name: "6月18日，3 字符" })).toBeTruthy();
});
