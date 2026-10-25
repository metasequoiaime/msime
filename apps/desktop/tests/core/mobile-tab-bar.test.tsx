// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    global_theme: "shuishan",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function mount(
  initialPage?: string,
  platform = "harmony",
  home: Parameters<typeof SettingsPage>[0]["client"]["home"] = {},
) {
  render(
    <SettingsPage
      initialPage={initialPage}
      client={{
        load: async () => initial,
        save: vi.fn(),
        home,
        // 标签页取决于客户端能力；AI 对话只作为二级页面。
        account: {} as never,
        chat: {
          models: vi
            .fn()
            .mockResolvedValue({ data: [{ id: "fixture-model" }], defaultModel: "fixture-model" }),
          complete: vi.fn(),
        },
        typingStatistics: {
          load: vi.fn().mockResolvedValue({
            availability: "neverWritten",
            statistics: { enabled: false, total: 0, days: {}, detail: {} },
          }),
          setEnabled: vi.fn(),
          reset: vi.fn(),
        },
        communitySkins: {
          list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
        } as never,
        host: testHost({ platform }),
      }}
    />,
  );
}

/** 三个触屏宿主共用的四个主入口：第二格是社区。 */
const tabs = ["首页", "社区", "统计", "我的"];

/** 「首页」根页，把每个没有自己标签页的页面列成一行。 */
const root = () => screen.getByRole("region", { name: "首页" });
const rootRowTitles = () =>
  [...root().querySelectorAll("[data-row-title]")].map((row) => row.textContent ?? "");
const openRootRow = (title: string) =>
  fireEvent.click(within(root()).getByRole("button", { name: title }));

// 底部使用图标与文字组成的四个入口，不使用页面下拉框。
test("the phone tab bar is four tabs, each an icon over a word", async () => {
  mount();
  await settingsFormReady();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  const buttons = [...bar.querySelectorAll("button")];
  expect(buttons.map((button) => button.textContent)).toEqual(tabs);
  const icons = buttons.map((button) => button.querySelector<HTMLElement>("[data-tab-icon]"));
  expect(icons.every((icon) => icon?.getAttribute("aria-hidden") === "true")).toBe(true);
  // 鸿蒙画设计里的 Fluent 图标，是 24px 的 `currentColor` SVG，所以选中时取强调色。
  expect(icons.map((icon) => icon!.getAttribute("data-tab-icon"))).toEqual([
    "home",
    "globe",
    "data_bar_vertical",
    "person",
  ]);
  for (const icon of icons) {
    const svg = icon!.querySelector("svg")!;
    expect(svg.getAttribute("width")).toBe("24");
    expect(svg.getAttribute("fill")).toBe("currentColor");
  }
  expect(bar.querySelector("select")).toBeNull();
});

// 其他触屏宿主保留各自的蒙版页面图标。
test("a touch host other than HarmonyOS keeps the masked tab icons", async () => {
  mount(undefined, "android");
  await settingsFormReady();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  const icons = [...bar.querySelectorAll<HTMLElement>("[data-tab-icon]")];
  expect(icons).toHaveLength(4);
  expect(icons.every((icon) => icon.style.getPropertyValue("--tab-icon").startsWith("url("))).toBe(
    true,
  );
  // 图标必须各不相同，否则标签栏看上去是同一样东西重复了几遍。
  expect(new Set(icons.map((icon) => icon.getAttribute("data-tab-icon"))).size).toBe(4);
  expect(icons[1].getAttribute("data-tab-icon")).toBeTruthy();
  expect(bar.querySelector("svg")).toBeNull();
});

// 这正是当初有 `<select>` 的原因。拿掉它却不给那些页面另开入口，手机上大部分应用就进不去了，所以改为必须满足这个条件：侧栏能到的地方，手机也能到，通过标签页、「首页」根页自己的行，或「我的」。断言对照的是侧栏而不是手写的页面名列表，这样以后新增的页面也会被覆盖，不用有人记得回来改这里。
test("every page the sidebar reaches is reachable on a phone", async () => {
  mount();
  await settingsFormReady();

  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  // 「首页」标签的根页在侧栏里叫应用名「水杉输入法」，它就是第一个标签。
  const reachable = new Set([...tabs, "水杉输入法", ...rootRowTitles()]);

  const stranded = [...sidebar.querySelectorAll("button")]
    .map((button) => button.textContent ?? "")
    .filter((title) => !reachable.has(title));
  expect(stranded).toEqual([]);
});

// 「社区」标签的根页是社区页：点它直接打开，标签高亮，没有返回按钮；「首页」根页不列社区那一行。
test("the 社区 tab opens the community page as its root", async () => {
  mount();
  await settingsFormReady();
  expect(rootRowTitles()).not.toContain("社区");

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  fireEvent.click(within(bar).getByRole("button", { name: "社区" }));
  expect(await screen.findByRole("heading", { level: 1, name: "社区" })).toBeTruthy();
  expect(within(bar).getByRole("button", { name: "社区" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBeNull();
  expect(screen.queryByRole("button", { name: "返回" })).toBeNull();
  expect(window.history.state).toEqual(
    expect.objectContaining({ page: "community", tab: "community" }),
  );
});

// 皮肤页回到「首页」根页的列表里，从那里推入：「首页」保持高亮，标题前带返回按钮，返回回到「首页」根页。
test("the 首页 list holds 皮肤 again and pushes the skin page", async () => {
  mount();
  await settingsFormReady();
  expect(rootRowTitles()).toContain("皮肤");

  openRootRow("皮肤");
  expect(await screen.findByRole("heading", { level: 1, name: "皮肤" })).toBeTruthy();
  const bar = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(window.history.state).toEqual(expect.objectContaining({ page: "skin", tab: "home" }));
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(await screen.findByRole("region", { name: "首页" })).toBeTruthy();
});

// Drilling into a page that has no tab does not leave the bar blank: the page was reached from the
// 首页 tab, so the 首页 tab is still where you are. The source keeps its first tab selected for
// everything its navigation stack pushes.
test("the 首页 tab stays lit on the pages reached from it", async () => {
  mount();
  await settingsFormReady();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  const home = within(bar).getByRole("button", { name: "首页" });
  expect(home.getAttribute("aria-current")).toBe("page");

  openRootRow("输入");

  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(within(bar).getByRole("button", { name: "我的" }).getAttribute("aria-current")).toBeNull();
});

// 「关于」「反馈」「帮助」从「我的」打开，所以留在「我的」的栈上：它的标签保持高亮，从其他标签回到「我的」时回到这些页面。
test("the pages opened from 我的 belong to the 我的 tab", async () => {
  mount("about");
  await settingsFormReady();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(bar).getByRole("button", { name: "我的" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBeNull();

  fireEvent.click(within(bar).getByRole("button", { name: "首页" }));
  expect(screen.getByRole("heading", { level: 1, name: "水杉输入法" })).toBeTruthy();
  fireEvent.click(within(bar).getByRole("button", { name: "我的" }));
  expect(screen.getByRole("heading", { level: 1, name: "关于" })).toBeTruthy();
});

// The source keeps one navigation stack per bottom tab. A flat shared route used to forget the
// keyboard tab's leaf, so returning after visiting another tab always reset it to 首页.
test("each phone tab remembers where the user left it", async () => {
  mount();
  await settingsFormReady();

  openRootRow("输入");
  expect(screen.getByRole("heading", { name: "输入" })).toBeTruthy();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  fireEvent.click(within(bar).getByRole("button", { name: "统计" }));
  expect(await screen.findByRole("heading", { level: 1, name: "统计" })).toBeTruthy();
  fireEvent.click(within(bar).getByRole("button", { name: "首页" }));

  expect(screen.getByRole("heading", { name: "输入" })).toBeTruthy();
  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBe(
    "page",
  );
});

// AI 对话属于打开它的设置栈，返回保留来源；二级对话页不带底部标签栏，返回后标签栏回来。
test("AI chat opens as a secondary page from AI settings", async () => {
  mount("ai");
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: /^AI 对话/ }));
  expect(await screen.findByRole("region", { name: "AI 对话" })).toBeTruthy();
  expect(window.history.state?.page).toBe("chat");
  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(await screen.findByRole("heading", { level: 1, name: "AI 辅助" })).toBeTruthy();
  const bar = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBe(
    "page",
  );
});

// 二级聊天返回后再进来保留草稿，聚焦与失焦都不显示底部标签栏。
test("secondary chat preserves its draft without the tab bar while typing", async () => {
  mount("ai");
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: /^AI 对话/ }));
  const composer = await screen.findByRole("textbox", { name: "聊天消息" });
  expect(document.activeElement).not.toBe(composer);
  fireEvent.change(composer, { target: { value: "合成草稿" } });
  fireEvent.focus(composer);
  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  fireEvent.blur(composer);
  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(await screen.findByRole("heading", { level: 1, name: "AI 辅助" })).toBeTruthy();
  fireEvent.click(await screen.findByRole("button", { name: /^AI 对话/ }));
  expect(
    ((await screen.findByRole("textbox", { name: "聊天消息" })) as HTMLTextAreaElement).value,
  ).toBe("合成草稿");
});

test("试用键盘 opens secondary chat with the keyboard and no tab bar", async () => {
  mount();
  await settingsFormReady();
  fireEvent.click(
    within(screen.getByRole("group", { name: "水杉输入法状态" })).getByRole("button", {
      name: "试用键盘",
    }),
  );
  expect(await screen.findByRole("region", { name: "AI 对话" })).toBeTruthy();
  const composer = screen.getByRole("textbox", { name: "聊天消息" });
  expect(document.activeElement).toBe(composer);
  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  expect(screen.getByRole("button", { name: "返回" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "收起键盘" }));
  expect(document.activeElement).not.toBe(composer);
  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
});

// 设置没补完时打出来的不是水杉，对话顶栏带用户去补完；都好了就不占位置。
test("the 对话 tab leads to the missing setup step until 水杉 is the keyboard", async () => {
  const listeners = new Set<(state: { enabled: boolean; current: boolean }) => void>();
  let state = { enabled: false, current: false };
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  const showInputMethodPicker = vi.fn().mockResolvedValue(undefined);
  mount("chat", "harmony", {
    openSystemKeyboardSettings,
    showInputMethodPicker,
    setup: {
      read: () => state,
      subscribe: (listener) => {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
    },
  });
  const push = (next: typeof state) =>
    act(() => {
      state = next;
      listeners.forEach((listener) => listener(next));
    });

  fireEvent.click(await screen.findByRole("button", { name: "水杉还没启用？点此开启" }));
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();

  push({ enabled: true, current: false });
  fireEvent.click(screen.getByRole("button", { name: "当前不是水杉输入法？点此切换" }));
  expect(showInputMethodPicker).toHaveBeenCalledOnce();

  push({ enabled: true, current: true });
  expect(screen.queryByRole("button", { name: /点此/ })).toBeNull();
});

// 触屏与桌面都有明确的 AI 对话二级入口。
test("AI settings exposes chat on touch hosts", async () => {
  mount("ai");
  await settingsFormReady();
  expect(await screen.findByRole("button", { name: /^AI 对话/ })).toBeTruthy();
});

// 桌面没有底部标签栏，有对话客户端时仍从「AI 辅助」页里的那一行进入。
test("the desktop AI 辅助 page keeps its row into the chat", async () => {
  mount("ai", "macos");
  await settingsFormReady();

  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  fireEvent.click(await screen.findByRole("button", { name: /^AI 对话/ }));
  expect(await screen.findByRole("region", { name: "AI 对话" })).toBeTruthy();
});

// 已经在「首页」标签里、停在它的某个子页面时，再点「首页」回到它的根页；从别的标签点过来才回到它记住的那一页。
test("tapping the tab you are on goes back to its first page", async () => {
  mount();
  await settingsFormReady();
  openRootRow("输入");
  expect(await screen.findByRole("heading", { level: 1, name: "输入" })).toBeTruthy();
  const bar = () => screen.getByRole("navigation", { name: "主要功能" });

  fireEvent.click(within(bar()).getByRole("button", { name: "统计" }));
  expect(await screen.findByRole("heading", { level: 1, name: "统计" })).toBeTruthy();
  fireEvent.click(within(bar()).getByRole("button", { name: "首页" }));
  expect(await screen.findByRole("heading", { level: 1, name: "输入" })).toBeTruthy();

  fireEvent.click(within(bar()).getByRole("button", { name: "首页" }));
  expect(await screen.findByRole("region", { name: "首页" })).toBeTruthy();
});

// 子页面往下滚，大标题连同它的返回按钮一起滚走，顶上换成收起的标题栏：那里也要有返回按钮，否则只能靠系统手势回去。
test("a scrolled sub-page keeps its way back in the collapsed title bar", async () => {
  mount();
  await settingsFormReady();
  openRootRow("输入");
  await screen.findByRole("heading", { level: 1, name: "输入" });
  expect(screen.getAllByRole("button", { name: "返回" })).toHaveLength(1);

  const content = document.getElementById("settings-content")!;
  content.scrollTop = 200;
  fireEvent.scroll(content);
  const backs = screen.getAllByRole("button", { name: "返回" });
  expect(backs).toHaveLength(2);
  fireEvent.click(backs[0]!);
  expect(await screen.findByRole("region", { name: "首页" })).toBeTruthy();
});

// 设置页里指向皮肤页的链接（键盘页的「屏幕键盘外观」）把皮肤页压在「首页」的栈上，返回回到键盘页。
test("a link to the skin page from a page under 首页 stays on the 首页 tab", async () => {
  mount("screen-keyboard", "android");
  await settingsFormReady();
  const bar = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBe(
    "page",
  );

  fireEvent.click(await screen.findByRole("button", { name: /^屏幕键盘外观/ }));
  expect(await screen.findByRole("heading", { level: 1, name: "皮肤" })).toBeTruthy();
  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(window.history.state).toEqual(expect.objectContaining({ page: "skin", tab: "home" }));

  window.history.back();
  expect(await screen.findByRole("heading", { level: 1, name: "键盘" })).toBeTruthy();
});
