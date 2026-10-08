// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
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

function mount(initialPage?: string, platform = "harmony") {
  render(
    <SettingsPage
      initialPage={initialPage}
      client={{
        load: async () => initial,
        save: vi.fn(),
        home: {},
        // The four tabs only exist where their pages do, and each page is gated on the capability
        // behind it. Harmony has all four.
        account: {} as never,
        typingStatistics: {} as never,
        communitySkins: {
          list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
        } as never,
        host: testHost({ platform }),
      }}
    />,
  );
}

/** The four the design shows, in its own words: 设置 / 社区 / 统计 / 我的. */
const tabs = ["设置", "社区", "统计", "我的"];

/** 「设置」根页，把每个没有自己标签页的页面列成一行。 */
const root = () => screen.getByRole("region", { name: "首页" });
const rootRowTitles = () =>
  [...root().querySelectorAll("[data-row-title]")].map((row) => row.textContent ?? "");
const openRootRow = (title: string) =>
  fireEvent.click(within(root()).getByRole("button", { name: title }));

// The bar used to hold five cells, and the fifth was a `<select>` of thirteen page names — a form
// control sitting where a tab belongs, and the only way into most of the app. The source's bar is
// four tabs of an icon over a word and nothing else.
test("the phone tab bar is the source's four tabs, each an icon over a word", async () => {
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
    "people_community",
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
  // The icons have to differ from one another, or the bar reads as four of the same thing.
  expect(new Set(icons.map((icon) => icon.getAttribute("data-tab-icon"))).size).toBe(4);
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
  fireEvent.click(within(bar).getByRole("button", { name: "社区" }));
  expect(await screen.findByRole("heading", { name: "社区" })).toBeTruthy();
  fireEvent.click(within(bar).getByRole("button", { name: "设置" }));

  expect(screen.getByRole("heading", { name: "输入" })).toBeTruthy();
  expect(within(bar).getByRole("button", { name: "设置" }).getAttribute("aria-current")).toBe(
    "page",
  );
});
