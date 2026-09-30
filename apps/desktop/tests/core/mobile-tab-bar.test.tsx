// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type HostCapabilities, type Snapshot } from "@msime/ui";

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

function mount() {
  render(
    <SettingsPage
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
        host: { platform: "harmony" } as HostCapabilities,
      }}
    />,
  );
}

/** The four the design shows, in its own words: 设置 / 社区 / 统计 / 我的. */
const tabs = ["设置", "社区", "统计", "我的"];

// The bar used to hold five cells, and the fifth was a `<select>` of thirteen page names — a form
// control sitting where a tab belongs, and the only way into most of the app. The source's bar is
// four tabs of an icon over a word and nothing else.
test("the phone tab bar is the source's four tabs, each an icon over a word", async () => {
  mount();
  await settingsFormReady();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  const buttons = [...bar.querySelectorAll("button")];
  expect(buttons.map((button) => button.textContent)).toEqual(tabs);
  // The glyph is a mask over the text colour rather than an `<img>`, so it takes the accent when selected; it stays out of the accessible name.
  const icons = buttons.map((button) => button.querySelector<HTMLElement>("[data-tab-icon]"));
  expect(icons.every((icon) => icon?.getAttribute("aria-hidden") === "true")).toBe(true);
  expect(icons.every((icon) => icon!.style.getPropertyValue("--tab-icon").startsWith("url("))).toBe(
    true,
  );
  // The icons have to differ from one another, or the bar reads as four of the same thing.
  const sources = icons.map((icon) => icon!.getAttribute("data-tab-icon"));
  expect(new Set(sources).size).toBe(tabs.length);
  expect(bar.querySelector("select")).toBeNull();
});

// The reason the `<select>` existed. Dropping it without giving those pages another door would have
// left most of the app unreachable on a phone, so this is the condition that has to hold instead:
// whatever the sidebar can reach, a phone can reach too, through a tab or through the 设置 tab's own
// list. Asserted against the sidebar rather than a written-out list of names so that a page added
// later is covered without anyone remembering to come back here.
test("every page the sidebar reaches is reachable on a phone", async () => {
  mount();
  await settingsFormReady();

  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  const reachable = new Set(tabs);
  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  const list = screen.getByRole("region", { name: "全部设置" });
  for (const button of list.querySelectorAll("button")) {
    // The row is a title, a note and a chevron, so the title is the part to compare.
    reachable.add(button.querySelector("strong")?.textContent ?? "");
  }
  // The tabs carry the source's shorter words; the sidebar carries the page's own title.
  for (const title of ["首页", "打字统计"]) reachable.add(title);

  const stranded = [...sidebar.querySelectorAll("button")]
    .map((button) => button.textContent ?? "")
    .filter((title) => !reachable.has(title));
  expect(stranded).toEqual([]);
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

  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  const list = screen.getByRole("region", { name: "全部设置" });
  const row = [...list.querySelectorAll("button")].find(
    (item) => item.querySelector("strong")?.textContent === "输入",
  )!;
  fireEvent.click(row);

  expect(within(bar).getByRole("button", { name: "设置" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(within(bar).getByRole("button", { name: "我的" }).getAttribute("aria-current")).toBeNull();
});

// The source keeps one navigation stack per bottom tab. A flat shared route used to forget the
// keyboard tab's leaf, so returning after visiting another tab always reset it to 首页.
test("each phone tab remembers where the user left it", async () => {
  mount();
  await settingsFormReady();

  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  const list = screen.getByRole("region", { name: "全部设置" });
  fireEvent.click(
    [...list.querySelectorAll("button")].find(
      (item) => item.querySelector("strong")?.textContent === "输入",
    )!,
  );
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
