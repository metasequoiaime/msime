// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { answerConfirm } from "../support/confirm";
import {
  TypingStatisticsPage,
  SettingsPage,
  type SettingsClient,
  type Snapshot,
  type TypingStatistics,
  type TypingStatisticsStatus,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const preferences: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function key(offset: number): string {
  const now = new Date();
  const date = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  date.setDate(date.getDate() + offset);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function label(offset: number): string {
  const now = new Date();
  const date = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  date.setDate(date.getDate() + offset);
  return `${date.getMonth() + 1}月${date.getDate()}日`;
}

// Built on each call so the day keys follow the clock the test runs under, including a faked one.
function initialStatistics(): TypingStatistics {
  return {
    enabled: true,
    total: 23,
    days: { [key(0)]: 4, [key(-1)]: 6, [key(-8)]: 10 },
    detail: {
      characters: { han: 4, latin: 6, emoji: 10 },
      sources: { quanpin: 4, english: 6, ai: 10 },
    },
    dailyDetails: {
      [key(0)]: { characters: { han: 4 }, sources: { quanpin: 4 } },
      [key(-1)]: { characters: { latin: 6 }, sources: { english: 6 } },
      [key(-8)]: { characters: { emoji: 10 }, sources: { ai: 10 } },
    },
  };
}

function status(statistics: TypingStatistics = initialStatistics()): TypingStatisticsStatus {
  return { statistics, availability: "ready", lastWrittenMs: Date.now() };
}

function baseClient(): SettingsClient {
  return { load: async () => preferences, save: vi.fn() };
}

// 页面默认打开「按键」，每日趋势和日历热力图都在「趋势」标签下。
async function openTrend() {
  fireEvent.click(await screen.findByRole("tab", { name: "趋势" }));
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("desktop settings omit typing statistics without the Android capability", async () => {
  render(<SettingsPage client={baseClient()} />);
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "打字统计" })).toBeNull();
});

test("desktop statistics split into content tabs over the cumulative and selected-day scopes", async () => {
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(<SettingsPage client={{ ...baseClient(), typingStatistics }} />);
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  expect((await screen.findByLabelText("当前范围输入字符数")).textContent).toBe("23");
  expect(screen.queryByRole("form", { name: "设置" })).toBeNull();
  expect(screen.queryByRole("button", { name: "重新读取" })).toBeNull();
  expect(screen.queryByRole("button", { name: "7 天" })).toBeNull();
  // 打开时停在排第一的「按键」标签上。
  const tabs = within(screen.getByRole("tablist", { name: "统计内容" })).getAllByRole("tab");
  expect(tabs.map((tab) => tab.textContent)).toEqual([
    "按键",
    "趋势",
    "类型",
    "模式",
    "方案",
    "候选",
  ]);
  expect(tabs[0].getAttribute("aria-selected")).toBe("true");
  expect(screen.getByRole("heading", { name: "按键热力图 · 累计" })).toBeTruthy();
  expect(screen.queryByRole("heading", { name: /每日趋势/ })).toBeNull();
  await openTrend();
  expect(screen.getByRole("heading", { name: "每日趋势 · 近 30 天" })).toBeTruthy();
  expect(screen.getByRole("heading", { name: "日历热力图" })).toBeTruthy();
  expect(screen.queryByRole("heading", { name: "字符类型" })).toBeNull();

  fireEvent.click(screen.getByRole("tab", { name: "类型" }));
  expect(screen.queryByRole("heading", { name: /每日趋势/ })).toBeNull();
  expect(screen.getByLabelText(/^历史未分类 3 字符/)).not.toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "模式" }));
  expect(screen.getByLabelText(/^历史未分类 3 字符/)).not.toBeNull();

  // A day picked on 趋势 stays picked, so the other tabs break that day down.
  fireEvent.click(screen.getByRole("tab", { name: "趋势" }));
  fireEvent.click(screen.getByRole("button", { name: `${label(0)}，4 字符` }));
  const selectedTotal = screen.getByLabelText("当前范围输入字符数");
  expect(selectedTotal.textContent).toBe("4");
  expect(selectedTotal.parentElement?.parentElement?.querySelector("span")?.textContent).toBe(
    label(0),
  );
  fireEvent.click(screen.getByRole("tab", { name: "类型" }));
  expect(screen.getByLabelText(/汉字 4 字符/)).not.toBeNull();
  expect(screen.getByLabelText("当前范围输入字符数").textContent).toBe("4");
  expect(screen.queryByText("private fixture text")).toBeNull();
});

test("mobile statistics follow Apple tabs and show the full retained trend", async () => {
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "ios" }),
        home: { openKeyboard: vi.fn() },
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "统计" }));
  expect((await screen.findByRole("tab", { name: "按键" })).getAttribute("aria-selected")).toBe(
    "true",
  );
  expect(screen.getByRole("heading", { name: "按键热力图 · 累计" })).toBeTruthy();
  await openTrend();
  expect(screen.getByRole("heading", { name: /每日趋势 · 近 30 天/ })).toBeTruthy();
  expect(screen.queryByRole("heading", { name: "字符类型" })).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "类型" }));
  expect(screen.getByRole("heading", { name: "字符类型" })).toBeTruthy();
  expect(screen.queryByRole("heading", { name: /每日趋势/ })).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "方案" }));
  expect(screen.getByRole("heading", { name: "输入方案" })).toBeTruthy();
});

// The tab strip was laid out for the four tabs it had when it was written, and a fifth was added later without widening it: 候选 wrapped onto a second row at a quarter of the width. The column count now comes from the number of tabs, and this pins the two together.
test("the phone tab strip has a column for every tab", async () => {
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "ios" }),
        home: { openKeyboard: vi.fn() },
        typingStatistics: {
          load: vi.fn().mockResolvedValue(status()),
          setEnabled: vi.fn(),
          reset: vi.fn(),
        },
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "统计" }));

  const strip = await screen.findByRole("tablist", { name: "统计内容" });
  const tabs = within(strip).getAllByRole("tab");
  expect(tabs.map((tab) => tab.textContent)).toEqual([
    "按键",
    "趋势",
    "类型",
    "模式",
    "方案",
    "候选",
  ]);
  expect(strip.className).toContain(`grid-cols-${tabs.length}`);
});

test("mobile statistic tabs use Apple chart shapes", async () => {
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "ios" }),
        home: { openKeyboard: vi.fn() },
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "统计" }));
  await openTrend();
  await screen.findByRole("heading", { name: /每日趋势/ });
  expect(screen.getByRole("img", { name: "每日输入趋势折线图" })).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "类型" }));
  expect(screen.getByRole("img", { name: "字符类型环形图" })).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "模式" }));
  expect(screen.getByRole("img", { name: "语言模式环形图" })).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "方案" }));
  expect(screen.getByRole("img", { name: "输入方案排行" })).toBeTruthy();
});

test("mobile trend includes a calendar heatmap that selects a day", async () => {
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "android" }),
        home: { openKeyboard: vi.fn() },
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "统计" }));
  await openTrend();
  await screen.findByRole("heading", { name: /每日趋势/ });
  expect(screen.getByRole("group", { name: "每日输入热力图" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: `热力图：${label(-1)}，6 字符` }));
  expect(screen.getByLabelText("当前范围输入字符数").textContent).toBe("6");
  expect(screen.getByText("返回整个时间范围")).toBeTruthy();
});

test("desktop statistics show a 12-month calendar heatmap with Monday-first weeks", async () => {
  // The heatmap spans 53 weeks, so on some days a cell from last year shares its 月日 label with a recent day and the by-name lookups below match two cells (on 2026-10-01, 9月29日 twice). A fixed Wednesday keeps the days this test names unique; only Date is faked, so the async queries keep real timers.
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(2026, 5, 17, 12));
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "macos" }),
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  await openTrend();
  expect(await screen.findByRole("heading", { name: "日历热力图" })).toBeTruthy();
  expect(screen.getByText("近 12 个月，颜色越深输入越多")).toBeTruthy();
  const heatmap = screen.getByRole("group", { name: "每日输入热力图" });
  const yesterday = within(heatmap).getByRole("button", { name: `热力图：${label(-1)}，6 字符` });
  expect(yesterday.getAttribute("title")).toBe(`${label(-1)}：6 字符`);
  expect(
    within(heatmap)
      .getByRole("button", { name: `热力图：${label(-2)}，0 字符` })
      .getAttribute("title"),
  ).toBe(`${label(-2)}：无记录`);
  // The first cell is the Monday 52 weeks before this week's Monday, so every column runs Monday to Sunday.
  const today = new Date();
  const mondayOffset = -((today.getDay() + 6) % 7) - 52 * 7;
  expect(within(heatmap).getAllByRole("button")[0].getAttribute("aria-label")).toBe(
    `热力图：${label(mondayOffset)}，0 字符`,
  );
  fireEvent.click(yesterday);
  expect(screen.getByLabelText("当前范围输入字符数").textContent).toBe("6");
  expect(yesterday.getAttribute("aria-pressed")).toBe("true");
  // A day outside the trend range still gets the page's M月D日 title, not its raw key.
  fireEvent.click(within(heatmap).getByRole("button", { name: `热力图：${label(-40)}，0 字符` }));
  expect(screen.getByText(label(-40))).toBeTruthy();
  expect(screen.queryByText(key(-40))).toBeNull();
});

test("mobile statistics refresh when the settings surface returns to the foreground", async () => {
  let now = 10_000;
  vi.spyOn(Date, "now").mockImplementation(() => now);
  const load = vi.fn().mockResolvedValue(status());
  const typingStatistics = { load, setEnabled: vi.fn(), reset: vi.fn() };
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "ios" }),
        home: { openKeyboard: vi.fn() },
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "统计" }));
  await openTrend();
  await screen.findByRole("heading", { name: /每日趋势/ });
  load.mockClear();
  now += 1_001;
  window.dispatchEvent(new Event("focus"));
  await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
});

test("desktop statistics refresh when the settings window regains focus", async () => {
  let now = 10_000;
  vi.spyOn(Date, "now").mockImplementation(() => now);
  const load = vi.fn().mockResolvedValue(status());
  render(
    <SettingsPage
      client={{ ...baseClient(), typingStatistics: { load, setEnabled: vi.fn(), reset: vi.fn() } }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  await screen.findByLabelText("当前范围输入字符数");
  load.mockClear();
  let finish!: (value: TypingStatisticsStatus) => void;
  load.mockImplementation(() => new Promise((resolve) => (finish = resolve)));
  now += 1_001;
  window.dispatchEvent(new Event("focus"));
  window.dispatchEvent(new Event("focus"));
  await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
  finish(status());
});

test("a late statistics response is ignored after the page unmounts", async () => {
  let finish!: (value: TypingStatisticsStatus) => void;
  const load = vi.fn(
    () =>
      new Promise<TypingStatisticsStatus>((resolve) => {
        finish = resolve;
      }),
  );
  const view = render(
    <TypingStatisticsPage client={{ load, setEnabled: vi.fn(), reset: vi.fn() }} />,
  );
  view.unmount();
  finish(status());
  await Promise.resolve();
});

test("a statistics mutation from a replaced client cannot overwrite the current page", async () => {
  const pending = deferred<TypingStatisticsStatus>();
  const oldClient = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn().mockReturnValue(pending.promise),
    reset: vi.fn(),
  };
  const nextClient = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  const view = render(<TypingStatisticsPage client={oldClient} />);
  await screen.findByLabelText("记录打字统计");
  fireEvent.click(screen.getByLabelText("记录打字统计"));
  view.rerender(<TypingStatisticsPage client={nextClient} />);
  await waitFor(() =>
    expect((screen.getByLabelText("记录打字统计") as HTMLInputElement).checked).toBe(true),
  );
  pending.resolve(status({ ...initialStatistics(), enabled: false, total: 0, days: {} }));
  await Promise.resolve();
  expect((screen.getByLabelText("记录打字统计") as HTMLInputElement).checked).toBe(true);
});

test("statistics toggle refreshes immediately and reset requires confirmation without re-enabling", async () => {
  const disabled = { ...initialStatistics(), enabled: false };
  const cleared: TypingStatistics = {
    enabled: false,
    total: 0,
    days: {},
    detail: { characters: {}, sources: {} },
    dailyDetails: {},
  };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn().mockResolvedValue(status(disabled)),
    reset: vi.fn().mockResolvedValue(status(cleared)),
  };
  render(<SettingsPage client={{ ...baseClient(), typingStatistics }} />);
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  const toggle = await screen.findByRole("checkbox", { name: "记录打字统计" });
  fireEvent.click(toggle);
  await waitFor(() => expect(typingStatistics.setEnabled).toHaveBeenCalledWith(false));
  await waitFor(() => expect((toggle as HTMLInputElement).checked).toBe(false));

  fireEvent.click(screen.getByRole("button", { name: "清空统计" }));
  await answerConfirm("cancel");
  expect(typingStatistics.reset).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "清空统计" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(typingStatistics.reset).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(screen.getByLabelText("当前范围输入字符数").textContent).toBe("0"));
  expect((toggle as HTMLInputElement).checked).toBe(false);
});

test("never-written status explains the empty local-only data channel", async () => {
  const empty: TypingStatistics = { enabled: true, total: 0, days: {} };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue({
      ...status(empty),
      availability: "neverWritten" as const,
      lastWrittenMs: null,
    }),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(<SettingsPage client={{ ...baseClient(), typingStatistics }} />);
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  expect(await screen.findByText(/键盘从未写入过统计/)).not.toBeNull();
  expect(screen.getByText(/不保存输入内容/)).not.toBeNull();
});

test("candidate positions show a first-candidate rate and keep rank order", async () => {
  const statistics: TypingStatistics = {
    ...initialStatistics(),
    selections: { ranks: [30, 6, 3, 0, 0, 0, 0, 0, 1], beyond: 10 },
  };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status(statistics)),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(<SettingsPage client={{ ...baseClient(), typingStatistics }} />);
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  fireEvent.click(await screen.findByRole("tab", { name: "候选" }));

  // 30 of 50 commits came from the first candidate.
  expect((await screen.findByLabelText("首选命中率")).textContent).toBe("60.0%");
  expect(screen.getByText("共 50 次上屏")).not.toBeNull();
  expect(screen.getByLabelText("第 1 条：30 次，60.0%")).not.toBeNull();
  expect(screen.getByLabelText("第 10 条以后：10 次，20.0%")).not.toBeNull();

  // Order is the position, never the size: the ninth row outranks every empty one before it. Each row
  // is found by the label it already carries for assistive technology, not by a styling class.
  const rows = screen.getByLabelText("候选命中位置分布").querySelectorAll("[aria-label*='次，']");
  expect(Array.from(rows).map((row) => row.querySelector("span")?.textContent)).toEqual([
    "第 1 条",
    "第 2 条",
    "第 3 条",
    "第 4 条",
    "第 5 条",
    "第 6 条",
    "第 7 条",
    "第 8 条",
    "第 9 条",
    "第 10 条以后",
  ]);
});

test("statistics written before candidate positions existed render an empty state", async () => {
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(<SettingsPage client={{ ...baseClient(), typingStatistics }} />);
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  fireEvent.click(await screen.findByRole("tab", { name: "候选" }));
  expect(await screen.findByText("暂无候选记录。用水杉键盘上屏几次后再回来查看。")).not.toBeNull();
  expect(screen.queryByLabelText("候选命中位置分布")).toBeNull();
});

test("Korean input has its own scheme and language slices", async () => {
  const statistics: TypingStatistics = {
    enabled: true,
    total: 5,
    days: { [key(0)]: 5 },
    detail: { characters: { otherLetter: 5 }, sources: { korean: 5 } },
    dailyDetails: { [key(0)]: { characters: { otherLetter: 5 }, sources: { korean: 5 } } },
  };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status(statistics)),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(<SettingsPage client={{ ...baseClient(), typingStatistics }} />);
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  fireEvent.click(await screen.findByRole("tab", { name: "方案" }));
  expect(screen.getByLabelText(/^韩语 5 字符/)).not.toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "模式" }));
  expect(screen.getByLabelText(/^韩语模式 5 字符/)).not.toBeNull();
});

test("Cantonese and Zhuyin count as Chinese mode and Vietnamese has its own slice", async () => {
  const sources = { cantonese: 3, zhuyin: 2, quanpin: 1, vietnamese: 4 };
  const statistics: TypingStatistics = {
    enabled: true,
    total: 10,
    days: { [key(0)]: 10 },
    detail: { characters: { han: 6, latin: 4 }, sources },
    dailyDetails: { [key(0)]: { characters: { han: 6, latin: 4 }, sources } },
  };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status(statistics)),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(<SettingsPage client={{ ...baseClient(), typingStatistics }} />);
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  fireEvent.click(await screen.findByRole("tab", { name: "方案" }));
  expect(screen.getByLabelText(/^粤拼 3 字符/)).not.toBeNull();
  expect(screen.getByLabelText(/^注音 2 字符/)).not.toBeNull();
  expect(screen.getByLabelText(/^越南语 4 字符/)).not.toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "模式" }));
  expect(screen.getByLabelText(/^中文模式 6 字符/)).not.toBeNull();
  expect(screen.getByLabelText(/^越南语模式 4 字符/)).not.toBeNull();
});

test("desktop statistics draw an ANSI key heatmap for the cumulative or selected-day scope", async () => {
  const statistics: TypingStatistics = {
    ...initialStatistics(),
    dailyKeys: {
      [key(0)]: { KeyA: 120, Space: 40, ArrowLeft: 3 },
      [key(-1)]: { KeyA: 3, MetaLeft: 2 },
      [key(-8)]: { KeyZ: 500 },
    },
  };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status(statistics)),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "macos" }),
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  // 按键是默认标签，不用点就能看到热力图。
  expect(await screen.findByRole("heading", { name: "按键热力图 · 累计" })).toBeTruthy();
  const heatmap = screen.getByRole("group", { name: "按键热力图" });
  expect(within(heatmap).getByRole("img", { name: "A，123 次" })).toBeTruthy();
  expect(within(heatmap).getByRole("img", { name: "Z，500 次" })).toBeTruthy();
  expect(within(heatmap).getByRole("img", { name: "左 Command，2 次" })).toBeTruthy();
  expect(within(heatmap).getByRole("img", { name: "F1，0 次" })).toBeTruthy();
  // A desktop page never draws the phone's on-screen keys.
  expect(within(heatmap).queryByRole("img", { name: /^符号/ })).toBeNull();
  expect(screen.getByLabelText("左箭头，3 次")).toBeTruthy();
  const top = screen.getByRole("list", { name: "最常按的键" });
  expect(
    within(top)
      .getAllByRole("listitem")
      .map((item) => item.textContent),
  ).toEqual(["1Z500 次", "2A123 次", "3空格40 次", "4左箭头3 次", "5左 Command2 次"]);

  fireEvent.click(screen.getByRole("tab", { name: "趋势" }));
  fireEvent.click(screen.getByRole("button", { name: `${label(-1)}，6 字符` }));
  fireEvent.click(screen.getByRole("tab", { name: "按键" }));
  expect(screen.getByRole("heading", { name: `按键热力图 · ${label(-1)}` })).toBeTruthy();
  expect(screen.getByRole("img", { name: "A，3 次" })).toBeTruthy();
  expect(screen.getByRole("img", { name: "空格，0 次" })).toBeTruthy();
});

test("statistics written before keys were counted show an empty key heatmap", async () => {
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(<SettingsPage client={{ ...baseClient(), typingStatistics }} />);
  fireEvent.click(await screen.findByRole("button", { name: "打字统计" }));
  fireEvent.click(await screen.findByRole("tab", { name: "按键" }));
  expect(await screen.findByText("这段时间还没有按键记录")).toBeTruthy();
  expect(screen.queryByRole("group", { name: "按键热力图" })).toBeNull();
  expect(screen.getAllByText(/只保存每个键每天被按下的次数，不保存按键顺序和输入内容/).length).toBe(
    2,
  );
  expect(screen.queryByText(/未上屏/)).toBeNull();
});

test("the phone's 按键 tab draws the soft keyboard and a nine-key grid once its cells were pressed", async () => {
  const statistics: TypingStatistics = {
    ...initialStatistics(),
    dailyKeys: {
      [key(0)]: { KeyQ: 9, Space: 4, SoftSymbol: 2, Nine2: 7, SoftEmoji: 1, Digit1: 5 },
      [key(-40)]: { Nine2: 1 },
    },
  };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status(statistics)),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "android" }),
        home: { openKeyboard: vi.fn() },
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "统计" }));
  expect(await screen.findByRole("heading", { name: "按键热力图 · 累计" })).toBeTruthy();
  expect(screen.queryByRole("heading", { name: /每日趋势/ })).toBeNull();
  const heatmap = screen.getByRole("group", { name: "按键热力图" });
  expect(within(heatmap).getByRole("img", { name: "Q，9 次" })).toBeTruthy();
  expect(within(heatmap).getByRole("img", { name: "空格，4 次" })).toBeTruthy();
  expect(within(heatmap).getByRole("img", { name: "符号，2 次" })).toBeTruthy();
  // The phone has no function row.
  expect(within(heatmap).queryByRole("img", { name: /^F1，/ })).toBeNull();
  const nine = screen.getByRole("group", { name: "九宫格按键" });
  expect(within(nine).getByRole("img", { name: "九宫格 2，8 次" })).toBeTruthy();
  expect(screen.getByLabelText("表情，1 次")).toBeTruthy();
  expect(screen.getByLabelText("1，5 次")).toBeTruthy();
});

test("a phone with only 26-key presses has no nine-key grid", async () => {
  const statistics: TypingStatistics = {
    ...initialStatistics(),
    dailyKeys: { [key(0)]: { KeyQ: 9 } },
  };
  const typingStatistics = {
    load: vi.fn().mockResolvedValue(status(statistics)),
    setEnabled: vi.fn(),
    reset: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        ...baseClient(),
        host: testHost({ platform: "ios" }),
        home: { openKeyboard: vi.fn() },
        typingStatistics,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "统计" }));
  fireEvent.click(await screen.findByRole("tab", { name: "按键" }));
  expect(screen.getByRole("img", { name: "Q，9 次" })).toBeTruthy();
  expect(screen.queryByRole("group", { name: "九宫格按键" })).toBeNull();
});
