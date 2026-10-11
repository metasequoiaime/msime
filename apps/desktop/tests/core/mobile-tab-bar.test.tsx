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
        // 标签页只在对应页面存在时出现，每个页面又取决于它背后的客户端能力。鸿蒙四个都有；社区客户端在，但已没有「社区」标签页，第二格是 AI 对话。
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

/** 设计里的标签页，用它自己的叫法：设置 / 对话 / 统计 / 我的。社区内容并进了皮肤、词库、AI 辅助这些功能页，原来「社区」的第二格换成 AI 对话。 */
const tabs = ["设置", "对话", "统计", "我的"];

/** 「设置」根页，把每个没有自己标签页的页面列成一行。 */
const root = () => screen.getByRole("region", { name: "首页" });
const rootRowTitles = () =>
  [...root().querySelectorAll("[data-row-title]")].map((row) => row.textContent ?? "");
const openRootRow = (title: string) =>
  fireEvent.click(within(root()).getByRole("button", { name: title }));

// 标签栏曾有五格，第五格是列出十三个页面名的 `<select>`：一个表单控件占着标签页的位置，还是进入大部分应用的唯一入口。源应用的标签栏只有图标加文字的标签页，社区并进功能页之后第二格换成了对话，仍是四个。
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
    "settings",
    "chat",
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
  // 「对话」画对话气泡（带尾巴的方框），不用 `chat` 页在注册表里的问号图标。小图标在测试里被内联成 data URL，所以对照路径本身。
  expect(decodeURIComponent(icons[1].getAttribute("data-tab-icon") ?? "")).toContain("H11L7 19V16");
  expect(bar.querySelector("svg")).toBeNull();
});

// 这正是当初有 `<select>` 的原因。拿掉它却不给那些页面另开入口，手机上大部分应用就进不去了，所以改为必须满足这个条件：侧栏能到的地方，手机也能到，通过标签页、「设置」根页自己的行，或「我的」。断言对照的是侧栏而不是手写的页面名列表，这样以后新增的页面也会被覆盖，不用有人记得回来改这里。
test("every page the sidebar reaches is reachable on a phone", async () => {
  mount();
  await settingsFormReady();

  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  const reachable = new Set([...tabs, ...rootRowTitles()]);

  const stranded = [...sidebar.querySelectorAll("button")]
    .map((button) => button.textContent ?? "")
    .filter((title) => !reachable.has(title));
  expect(stranded).toEqual([]);
});

// 「全部设置」已不在根页上，但深链接或旧的历史条目仍能打开它：它按导航分组标题分组列出页面，并去掉那些页面已在标签栏或「我的」里有入口的分组。
test("the 全部设置 list is grouped under the navigation group titles", async () => {
  mount("more");

  const list = await screen.findByRole("region", { name: "全部设置" });
  const groups = within(list)
    .getAllByRole("group")
    .map((group) => ({
      title: group.getAttribute("aria-labelledby")
        ? document.getElementById(group.getAttribute("aria-labelledby")!)?.textContent
        : undefined,
      pages: [...group.querySelectorAll("strong")].map((item) => item.textContent),
    }));
  expect(groups.map((group) => group.title)).toEqual(["打字", "外观", "键盘、语音与手写", "工具"]);
  expect(groups[0].pages[0]).toBe("输入");
  expect(within(list).getByRole("group", { name: "打字" })).toBeTruthy();
});

// Drilling into a page that has no tab does not leave the bar blank: the page was reached from the
// 设置 tab, so the 设置 tab is still where you are. The source keeps its first tab selected for
// everything its navigation stack pushes.
test("the 设置 tab stays lit on the pages reached from it", async () => {
  mount();
  await settingsFormReady();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  const home = within(bar).getByRole("button", { name: "设置" });
  expect(home.getAttribute("aria-current")).toBe("page");

  openRootRow("输入");

  expect(within(bar).getByRole("button", { name: "设置" }).getAttribute("aria-current")).toBe(
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
  expect(within(bar).getByRole("button", { name: "设置" }).getAttribute("aria-current")).toBeNull();

  fireEvent.click(within(bar).getByRole("button", { name: "设置" }));
  expect(screen.getByRole("heading", { level: 1, name: "设置" })).toBeTruthy();
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
  fireEvent.click(within(bar).getByRole("button", { name: "设置" }));

  expect(screen.getByRole("heading", { name: "输入" })).toBeTruthy();
  expect(within(bar).getByRole("button", { name: "设置" }).getAttribute("aria-current")).toBe(
    "page",
  );
});

// 「对话」标签页直接打开现有的 AI 对话页。它是标签页的根页面：鸿蒙手机上不滑入、不带返回按钮，「设置」标签不再高亮；从别的标签回来仍是对话。
test("the 对话 tab opens the AI chat page as a tab of its own", async () => {
  mount();
  await settingsFormReady();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  fireEvent.click(within(bar).getByRole("button", { name: "对话" }));

  expect(await screen.findByRole("region", { name: "AI 对话" })).toBeTruthy();
  expect(screen.getByRole("textbox", { name: "聊天消息" })).toBeTruthy();
  expect(window.history.state?.page).toBe("chat");
  expect(within(bar).getByRole("button", { name: "对话" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(within(bar).getByRole("button", { name: "设置" }).getAttribute("aria-current")).toBeNull();
  expect(screen.queryByRole("button", { name: "返回" })).toBeNull();
  expect(screen.queryByRole("button", { name: "返回AI 辅助" })).toBeNull();

  fireEvent.click(within(bar).getByRole("button", { name: "统计" }));
  expect(await screen.findByRole("heading", { level: 1, name: "统计" })).toBeTruthy();
  fireEvent.click(within(bar).getByRole("button", { name: "对话" }));
  expect(await screen.findByRole("region", { name: "AI 对话" })).toBeTruthy();
});

// 对话是常驻标签：点进来不弹键盘（输入框不自动聚焦），切到别的标签只是隐藏，回来时没发出去的文字还在。
test("the 对话 tab keeps its conversation across tabs and does not raise the keyboard", async () => {
  mount();
  await settingsFormReady();
  const bar = screen.getByRole("navigation", { name: "主要功能" });
  fireEvent.click(within(bar).getByRole("button", { name: "对话" }));
  const composer = await screen.findByRole("textbox", { name: "聊天消息" });
  await act(async () => {
    await new Promise((resolve) => window.setTimeout(resolve, 10));
  });
  expect(document.activeElement).not.toBe(composer);
  fireEvent.change(composer, { target: { value: "还没发出去的一句话" } });

  fireEvent.click(within(bar).getByRole("button", { name: "统计" }));
  expect(await screen.findByRole("heading", { level: 1, name: "统计" })).toBeTruthy();
  expect(screen.queryByRole("region", { name: "AI 对话" })).toBeNull();

  fireEvent.click(within(bar).getByRole("button", { name: "对话" }));
  expect(await screen.findByRole("region", { name: "AI 对话" })).toBeTruthy();
  expect((screen.getByRole("textbox", { name: "聊天消息" }) as HTMLTextAreaElement).value).toBe(
    "还没发出去的一句话",
  );
});

// 在对话里打字时底部标签栏让开，键盘上方只留输入栏；输入框失焦后标签栏回来。
test("the tab bar steps aside while typing in the 对话 tab", async () => {
  mount();
  await settingsFormReady();
  fireEvent.click(
    within(screen.getByRole("navigation", { name: "主要功能" })).getByRole("button", {
      name: "对话",
    }),
  );
  const composer = await screen.findByRole("textbox", { name: "聊天消息" });
  fireEvent.focus(composer);
  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  fireEvent.blur(composer);
  expect(screen.getByRole("navigation", { name: "主要功能" })).toBeTruthy();
});

// 试用键盘和对话是同一页：首页卡片上的「试用键盘」切到「对话」标签并弹出键盘，没有第二个试用页。
test("试用键盘 on the 设置 root opens the 对话 tab with the keyboard up", async () => {
  mount();
  await settingsFormReady();
  fireEvent.click(
    within(screen.getByRole("group", { name: "水杉输入法状态" })).getByRole("button", {
      name: "试用键盘",
    }),
  );

  expect(await screen.findByRole("region", { name: "AI 对话" })).toBeTruthy();
  expect(screen.queryByRole("region", { name: "试用键盘" })).toBeNull();
  const composer = screen.getByRole("textbox", { name: "聊天消息" });
  expect(document.activeElement).toBe(composer);
  // 打字时标签栏让开；「收起键盘」把键盘收掉，标签栏回来。
  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "收起键盘" }));
  expect(document.activeElement).not.toBe(composer);
  expect(screen.queryByRole("button", { name: "收起键盘" })).toBeNull();
  expect(screen.getByRole("navigation", { name: "主要功能" })).toBeTruthy();
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

// 进入 AI 对话的入口由标签页承担，「AI 辅助」页不再列第二个「AI 对话」行。
test("the AI 辅助 page no longer lists a second way into the chat on a touch host", async () => {
  mount("ai");
  await settingsFormReady();

  expect(await screen.findByRole("heading", { level: 1, name: "AI 辅助" })).toBeTruthy();
  expect(screen.queryByRole("region", { name: "AI 对话" })).toBeNull();
  expect(screen.queryByRole("button", { name: /^AI 对话/ })).toBeNull();
});

// 桌面没有底部标签栏，有对话客户端时仍从「AI 辅助」页里的那一行进入。
test("the desktop AI 辅助 page keeps its row into the chat", async () => {
  mount("ai", "macos");
  await settingsFormReady();

  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  fireEvent.click(await screen.findByRole("button", { name: /^AI 对话/ }));
  expect(await screen.findByRole("region", { name: "AI 对话" })).toBeTruthy();
});

// 已经在「设置」标签里、停在它的某个子页面时，再点「设置」回到设置首页；从别的标签点过来才回到它记住的那一页。
test("tapping the tab you are on goes back to its first page", async () => {
  mount();
  await settingsFormReady();
  openRootRow("皮肤");
  expect(await screen.findByRole("heading", { level: 1, name: "皮肤" })).toBeTruthy();
  const bar = () => screen.getByRole("navigation", { name: "主要功能" });

  fireEvent.click(within(bar()).getByRole("button", { name: "统计" }));
  expect(await screen.findByRole("heading", { level: 1, name: "统计" })).toBeTruthy();
  fireEvent.click(within(bar()).getByRole("button", { name: "设置" }));
  expect(await screen.findByRole("heading", { level: 1, name: "皮肤" })).toBeTruthy();

  fireEvent.click(within(bar()).getByRole("button", { name: "设置" }));
  expect(await screen.findByRole("region", { name: "首页" })).toBeTruthy();
});

// 子页面往下滚，大标题连同它的返回按钮一起滚走，顶上换成收起的标题栏：那里也要有返回按钮，否则只能靠系统手势回去。
test("a scrolled sub-page keeps its way back in the collapsed title bar", async () => {
  mount();
  await settingsFormReady();
  openRootRow("皮肤");
  await screen.findByRole("heading", { level: 1, name: "皮肤" });
  expect(screen.getAllByRole("button", { name: "返回" })).toHaveLength(1);

  const content = document.getElementById("settings-content")!;
  content.scrollTop = 200;
  fireEvent.scroll(content);
  const backs = screen.getAllByRole("button", { name: "返回" });
  expect(backs).toHaveLength(2);
  fireEvent.click(backs[0]!);
  expect(await screen.findByRole("region", { name: "首页" })).toBeTruthy();
});
