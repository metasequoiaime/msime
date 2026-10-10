// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  ToastProvider,
  TypingStatisticsPage,
  type TypingStatistics,
  type TypingStatisticsClient,
  type TypingStatisticsStatus,
} from "@msime/ui";
import type {
  AchievementSummary,
  TypingSummary,
} from "../../../../packages/ui/src/settings/typing-summary";
import { summaryMethods } from "../../../../packages/ui/src/settings/typing-breakdown";
import { answerConfirm } from "../support/confirm";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("14 键 is an input method of its own, not part of 26 键", () => {
  expect(summaryMethods({ quanpin: 6, shuangpin: 2, fourteenKey: 3, nineKey: 1, ai: 5 })).toEqual([
    { title: "26 键", count: 8 },
    { title: "14 键", count: 3 },
    { title: "9 键", count: 1 },
  ]);
});

function key(offset: number): string {
  const now = new Date();
  const date = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  date.setDate(date.getDate() + offset);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function weekday(offset: number): string {
  const now = new Date();
  const date = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  date.setDate(date.getDate() + offset);
  return "日一二三四五六"[date.getDay()];
}

function statistics(): TypingStatistics {
  return {
    enabled: true,
    total: 12_846,
    days: { [key(0)]: 1_286 },
    dailyKeys: { [key(0)]: { KeyN: 96, KeyA: 40, Space: 64 }, [key(-30)]: { KeyQ: 500 } },
  };
}

function status(value: TypingStatistics = statistics()): TypingStatisticsStatus {
  return { statistics: value, availability: "ready", lastWrittenMs: Date.now() };
}

function badge(
  id: string,
  glyph: string,
  title: string,
  description: string,
  group: AchievementSummary["group"],
  current: number,
  target: number,
  unlockedDay: string | null,
): AchievementSummary {
  return { id, glyph, title, description, group, unlocked_day: unlockedDay, current, target };
}

// 每次调用时构建，让日期键跟随测试运行时的时钟。
function summary(): TypingSummary {
  const week = [1620, 2140, 1880, 2410, 1990, 1520, 1286];
  return {
    overview: {
      week_total: 12_846,
      previous_week_total: 10_886,
      last7: week.map((count, index) => ({ day: key(index - 6), count })),
      average_speed: 52.4,
      previous_average_speed: 48.2,
      first_candidate_rate: 0.912,
      keystrokes_saved_rate: 0.38,
      current_streak: 23,
      longest_streak: 41,
    },
    habits: {
      weeks12: Array.from({ length: 84 }, (_, index) => ({
        day: key(index - 83),
        count: index % 3 === 0 ? 0 : index * 10,
      })),
      hours24: [
        4, 2, 1, 1, 1, 2, 6, 14, 30, 42, 46, 40, 28, 34, 44, 48, 42, 36, 30, 38, 52, 60, 54, 22,
      ],
      usual_hours: null,
      peak_window: { start: 21, end: 23 },
      active_days: 56,
      breakdown: {
        characters: { han: 82, latin: 11, punctuation: 3, number: 2, emoji: 2 },
        sources: { quanpin: 60, shuangpin: 8, nineKey: 19, voice: 9, handwriting: 4, ai: 30 },
      },
    },
    keys: {
      per_character_keys: 2.3,
      previous_per_character_keys: 2.5,
      backspace_rate: 0.074,
      prediction_rate: 0.41,
      longest_run: { characters: 86, day: "2026-09-28" },
      positions: [0.71, 0.16, 0.07, 0.06],
    },
    achievements: [
      badge("chars_10k", "1万", "初出茅庐", "累计输入 1 万字", "volume", 12_846, 10_000, key(-3)),
      badge(
        "chars_1m",
        "百万",
        "著作等身",
        "累计输入 100 万字",
        "volume",
        483_000,
        1_000_000,
        null,
      ),
      badge("speed_60", "60", "快手", "平均每分钟 60 字", "skill", 52, 60, null),
    ],
  };
}

/** 同样的摘要，但所有可选数据都缺失。 */
function emptySummary(): TypingSummary {
  const value = summary();
  return {
    ...value,
    overview: {
      ...value.overview,
      previous_week_total: 0,
      average_speed: null,
      previous_average_speed: null,
      first_candidate_rate: null,
      keystrokes_saved_rate: null,
    },
    keys: {
      per_character_keys: null,
      previous_per_character_keys: null,
      backspace_rate: null,
      prediction_rate: null,
      longest_run: null,
      positions: null,
    },
  };
}

function client(overrides: Partial<TypingStatisticsClient> = {}): TypingStatisticsClient {
  return {
    load: vi.fn().mockResolvedValue(status()),
    setEnabled: vi.fn().mockResolvedValue(status()),
    setRetention: vi.fn().mockResolvedValue(status()),
    reset: vi.fn().mockResolvedValue(status()),
    summary: vi.fn().mockResolvedValue(summary()),
    ...overrides,
  };
}

function renderPage(value: TypingStatisticsClient, look: "harmony" | "hm2" = "harmony") {
  return render(
    <ToastProvider>
      <TypingStatisticsPage client={value} mobile={look === "harmony"} look={look} />
    </ToastProvider>,
  );
}

const footer = "统计只保存在本机，不包含输入内容";

function tile(name: string): HTMLElement {
  return screen.getByRole("group", { name: new RegExp(`^${name} `) });
}

test("the HarmonyOS statistics open on 概览 with the week's hero and tiles", async () => {
  const value = client();
  renderPage(value);

  const tabs = await screen.findByRole("tablist", { name: "统计内容" });
  expect(
    within(tabs)
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["概览", "习惯", "按键", "成就"]);
  expect(within(tabs).getByRole("tab", { name: "概览" }).getAttribute("aria-selected")).toBe(
    "true",
  );
  const hero = screen.getByRole("region", { name: "近 7 天概览" });
  expect(within(hero).getByText("12,846")).toBeTruthy();
  expect(within(hero).getByText("比上周多 18%")).toBeTruthy();
  const bars = within(hero).getByRole("img");
  expect(bars.getAttribute("aria-label")).toContain(`星期${weekday(0)} 1,286 字`);
  expect(within(bars).getByText(weekday(0)).className).toContain("var(--p-text)");
  expect(tile("平均速度").getAttribute("aria-label")).toBe("平均速度 52 字/分，比上周快 4 字");
  expect(tile("首选命中").getAttribute("aria-label")).toBe("首选命中 91 %，第一个候选就是你要的");
  expect(tile("少按键").getAttribute("aria-label")).toBe("少按键 38 %，联想和整句帮你省下");
  expect(tile("连续使用").getAttribute("aria-label")).toBe("连续使用 23 天，最长 41 天");
  expect(screen.getByText(footer)).toBeTruthy();
  // 长篇隐私说明和桌面端图表不属于这种样式。
  expect(screen.queryByText(/字数仅统计水杉键盘成功提交的字符/)).toBeNull();
  expect(screen.queryByRole("tab", { name: "趋势" })).toBeNull();
  expect(value.summary).toHaveBeenCalledOnce();
});

test("missing figures read as a dash rather than zero", async () => {
  renderPage(client({ summary: vi.fn().mockResolvedValue(emptySummary()) }));

  const hero = await screen.findByRole("region", { name: "近 7 天概览" });
  expect(within(hero).getByText("—")).toBeTruthy();
  expect(tile("平均速度").getAttribute("aria-label")).toBe("平均速度 —，近 7 天的活跃时间里");
  expect(tile("首选命中").getAttribute("aria-label")).toBe("首选命中 —，选词满 50 次后显示");
  expect(tile("少按键").getAttribute("aria-label")).toBe("少按键 —，联想和整句帮你省下");

  fireEvent.click(screen.getByRole("tab", { name: "按键" }));
  expect(tile("每字按键").getAttribute("aria-label")).toBe("每字按键 —，近 7 天平均");
  expect(tile("退格占比").getAttribute("aria-label")).toBe("退格占比 —，近 7 天全部按键里");
  expect(tile("联想上屏").getAttribute("aria-label")).toBe("联想上屏 —，不用打完就上屏的词");
  expect(tile("单次最长").getAttribute("aria-label")).toBe("单次最长 —，还没有记录");
  expect(screen.queryByRole("heading", { name: "选词位置" })).toBeNull();
});

test("习惯 and 按键 draw the summary's habits and key figures", async () => {
  renderPage(client());

  fireEvent.click(await screen.findByRole("tab", { name: "习惯" }));
  expect(screen.getByRole("heading", { name: "近 12 周" })).toBeTruthy();
  expect(screen.getByText("活跃 56 天")).toBeTruthy();
  const heat = screen.getByRole("img", { name: /^近 12 周输入热力图/ });
  expect(heat.children).toHaveLength(84);
  expect(screen.getByText("最常在 晚上 9–11 点")).toBeTruthy();
  expect(screen.getByRole("img", { name: /^输入构成/ }).getAttribute("aria-label")).toBe(
    "输入构成：汉字 82%，英文 11%，符号 5%，表情 2%",
  );
  expect(screen.getByText(footer)).toBeTruthy();

  fireEvent.click(screen.getByRole("tab", { name: "按键" }));
  // 只计最近七天：一个月前按的 500 次 Q 在窗口之外。
  expect(screen.getByText("字母键最常按 N · 48%")).toBeTruthy();
  expect(screen.getByRole("img", { name: "Q，0 次" })).toBeTruthy();
  expect(tile("每字按键").getAttribute("aria-label")).toBe("每字按键 2.3 次，比上周少 0.2 次");
  expect(tile("退格占比").getAttribute("aria-label")).toBe("退格占比 7.4 %，近 7 天全部按键里");
  expect(tile("单次最长").getAttribute("aria-label")).toBe("单次最长 86 字，9 月 28 日 · 不停顿");
  expect(screen.getByRole("group", { name: "第 1 个 71%" })).toBeTruthy();
  expect(screen.getByRole("group", { name: "翻页后 6%" })).toBeTruthy();
  // AI 改写不是一种输入方式，所以其余部分由四种输入方式分摊。
  expect(screen.getByRole("img", { name: /^输入方式/ }).getAttribute("aria-label")).toBe(
    "输入方式：26 键 68%，9 键 19%，语音 9%，手写 4%",
  );

  fireEvent.click(screen.getByRole("button", { name: "9 键" }));
  expect(screen.getByRole("button", { name: "9 键" }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByText("这段时间还没有按键记录")).toBeTruthy();
  expect(screen.getByText(footer)).toBeTruthy();
});

test("成就 counts unlocked badges and toasts what a tapped badge is for", async () => {
  renderPage(client());

  fireEvent.click(await screen.findByRole("tab", { name: "成就" }));
  expect(screen.getByRole("region", { name: "1 / 3 枚成就已解锁" })).toBeTruthy();
  const unlocked = screen.getByRole("button", { name: "初出茅庐，已解锁，累计输入 1 万字" });
  const locked = screen.getByRole("button", { name: "著作等身，未解锁，还差 51.7 万字" });
  // 速度徽章没有“还差 N”的措辞，所以保留它的描述。
  expect(screen.getByRole("button", { name: "快手，未解锁，平均每分钟 60 字" })).toBeTruthy();
  expect(within(locked).getByText("48%")).toBeTruthy();

  fireEvent.click(unlocked);
  expect(await screen.findByText("已解锁「初出茅庐」· 累计输入 1 万字")).toBeTruthy();
  fireEvent.click(locked);
  expect(await screen.findByText("「著作等身」· 还差 51.7 万字")).toBeTruthy();
  expect(screen.getByText(footer)).toBeTruthy();
});

test("the summary is read again with the statistics and the menu keeps retention and reset", async () => {
  const value = client();
  renderPage(value);
  await screen.findByRole("region", { name: "近 7 天概览" });
  expect(value.summary).toHaveBeenCalledOnce();

  const menu = document.querySelector('summary[aria-label="统计选项"]');
  if (!menu) throw new Error("missing statistics menu");
  fireEvent.click(menu);
  fireEvent.change(screen.getByRole("combobox", { name: "自动清理" }), {
    target: { value: "90d" },
  });
  await waitFor(() => expect(value.setRetention).toHaveBeenCalledWith("90d"));
  await waitFor(() => expect(value.summary).toHaveBeenCalledTimes(2));

  fireEvent.click(screen.getByRole("menuitem", { name: "清空统计" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(value.reset).toHaveBeenCalledOnce());
  await waitFor(() => expect(value.summary).toHaveBeenCalledTimes(3));
});

test("a recording switch that is off says so above the summary", async () => {
  const off = { ...statistics(), enabled: false };
  renderPage(client({ load: vi.fn().mockResolvedValue(status(off)) }));

  expect(await screen.findByRole("heading", { name: "输入统计已关闭" })).toBeTruthy();
  expect(await screen.findByRole("region", { name: "近 7 天概览" })).toBeTruthy();
});

test("a failed summary is reported instead of an empty page", async () => {
  renderPage(client({ summary: vi.fn().mockRejectedValue(new Error("busy")) }));

  expect(await screen.findByText("统计暂时读不到，可以在右上角菜单里刷新。")).toBeTruthy();
  expect(screen.queryByRole("tablist", { name: "统计内容" })).toBeNull();
});

test("the 2-in-1 look draws the same content on 12px cards", async () => {
  renderPage(client(), "hm2");

  const hero = await screen.findByRole("region", { name: "近 7 天概览" });
  expect(hero.className).toContain("rounded-[12px]");
  expect(screen.getByText(footer)).toBeTruthy();
});

test("a look without a summary-capable client keeps the existing page", async () => {
  const { summary: _summary, ...withoutSummary } = client();
  renderPage(withoutSummary);

  expect(await screen.findByRole("tab", { name: "趋势" })).toBeTruthy();
  expect(screen.queryByRole("tab", { name: "概览" })).toBeNull();
  expect(screen.queryByText(footer)).toBeNull();
});
