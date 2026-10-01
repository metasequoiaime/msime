// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import css from "../../../../packages/ui/src/styles.css?raw";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function renderSettings(
  platform: string,
  host: Record<string, unknown> = {},
  initialPage?: string,
  extraClient: Record<string, unknown> = {},
) {
  render(
    <SettingsPage
      initialPage={initialPage}
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn().mockResolvedValue(undefined),
        host: { platform, ...host } as never,
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        // Both window commands are present here because one Tauri binary serves every platform: the
        // app exposes them on a phone too, which is exactly how the titlebar reached Android.
        windowControl: vi.fn(),
        beginWindowDrag: vi.fn(),
        ...extraClient,
      }}
    />,
  );
}

// The OS owns the frame on a phone. Minimise, maximise and close have nothing to act on there, and a
// drag handle above the content only steals a row of screen.
test("a phone host draws no window titlebar", async () => {
  renderSettings("android");
  await settingsFormReady();

  expect(screen.queryByRole("banner", { name: "窗口控制" })).toBeNull();
});

test("Harmony settings follow the actual phone or 2-in-1 form factor", async () => {
  renderSettings("harmony", { mobile_settings: true });
  await settingsFormReady();
  expect(screen.getByRole("navigation", { name: "主要功能" })).toBeTruthy();
  cleanup();

  renderSettings("harmony", { mobile_settings: false });
  await settingsFormReady();
  expect(screen.queryByRole("navigation", { name: "主要功能" })).toBeNull();
  expect(screen.getByRole("navigation", { name: "设置分类" })).toBeTruthy();
  const preview = document.querySelector(".screen-keyboard-artwork");
  const keys = Array.from(preview!.querySelectorAll("[data-keyboard-key]")).map((key) =>
    key.getAttribute("data-keyboard-key"),
  );
  expect(keys).toContain("Caps Lock");
  expect(keys).toContain("Tab");
});

test("Harmony capability chrome stays split between phone and 2-in-1", async () => {
  const desktopCapabilities = {
    mobile_settings: false,
    panel_windows: true,
    floating_toolbar: true,
    candidate_font_controls: true,
    floating_toolbar_appearance: true,
    floating_toolbar_components: true,
    mode_switch_shortcuts: true,
    panel_shortcuts: true,
    number_row_selection: true,
    candidate_follow_cursor: true,
    input_mode_hud: true,
  };
  renderSettings("harmony", {
    ...desktopCapabilities,
    translation_secondary_language: undefined,
  });
  await settingsFormReady();
  expect(
    screen.getByText(
      "在系统设置中启用并选择水杉输入法，再使用实体键盘、候选窗口和悬浮工具栏输入。默认是全拼输入法。",
    ),
  ).toBeTruthy();
  expect(screen.getByText("为 HarmonyOS 2-in-1 桌面输入体验打造的开放中文输入法。")).toBeTruthy();
  expect(screen.getByRole("button", { name: "悬浮工具栏" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(screen.getByRole("group", { name: "输入模式切换快捷键" })).toBeTruthy();
  expect(screen.getByRole("group", { name: "面板快捷键" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "数字键选词" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  expect(screen.getByRole("combobox", { name: "候选词翻译第二种语言" })).toBeTruthy();

  cleanup();
  renderSettings("harmony", {
    mobile_settings: true,
    panel_windows: false,
    floating_toolbar: false,
    candidate_font_controls: true,
    floating_toolbar_appearance: false,
    floating_toolbar_components: false,
    mode_switch_shortcuts: false,
    panel_shortcuts: false,
    number_row_selection: false,
    candidate_follow_cursor: false,
    input_mode_hud: false,
  });
  await settingsFormReady();
  expect(
    screen.getByText(
      "在系统设置中启用并选择水杉输入法，再从输入法键盘使用语音和触屏输入。默认是全拼输入法。",
    ),
  ).toBeTruthy();
  expect(screen.getByText("为 HarmonyOS 触屏输入体验打造的开放中文输入法。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "悬浮工具栏" })).toBeNull();
  fireEvent.click(
    within(screen.getByRole("navigation", { name: "主要功能" })).getByRole("button", {
      name: "设置",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  const phoneSettings = screen.getByRole("region", { name: "全部设置" });
  expect(within(phoneSettings).queryByRole("button", { name: "悬浮工具栏" })).toBeNull();
  expect(within(phoneSettings).queryByRole("button", { name: "快捷键" })).toBeNull();
  fireEvent.click(within(phoneSettings).getByRole("button", { name: "标点与翻译" }));
  expect(screen.getByRole("combobox", { name: "候选词翻译第二种语言" })).toBeTruthy();
});

test("Harmony appearance names only the surfaces the form factor actually has", async () => {
  renderSettings("harmony", {
    mobile_settings: true,
    panel_windows: false,
    floating_toolbar: false,
    candidate_font_controls: true,
  });
  await settingsFormReady();
  fireEvent.click(
    within(screen.getByRole("navigation", { name: "主要功能" })).getByRole("button", {
      name: "设置",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  fireEvent.click(
    within(screen.getByRole("region", { name: "全部设置" })).getByRole("button", {
      name: "候选栏",
    }),
  );
  expect(screen.getByRole("region", { name: "候选栏预览" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "候选栏字号" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "候选栏预编辑字号" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "候选栏预编辑" })).toBeTruthy();
  expect(screen.queryByText("候选窗口预览")).toBeNull();
  expect(screen.queryByLabelText("候选窗字号")).toBeNull();

  // The phone hides the physical-keyboard shortcut page everywhere, the sidebar included: a sidebar entry here was a button `selectPage` refused, and asserting the page's text passed only because the hidden fieldset stays in the DOM.
  expect(screen.queryByRole("button", { name: "快捷键" })).toBeNull();

  // The per-surface theme overrides are in the 高级 group of 主题.
  cleanup();
  renderSettings(
    "harmony",
    {
      mobile_settings: true,
      panel_windows: false,
      floating_toolbar: false,
      candidate_font_controls: true,
    },
    "skin",
  );
  await settingsFormReady();
  expect(screen.getByRole("combobox", { name: "候选栏主题" })).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "候选窗口主题" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "悬浮工具栏主题" })).toBeNull();

  cleanup();
  renderSettings("harmony", {
    mobile_settings: false,
    panel_windows: true,
    floating_toolbar: true,
    candidate_font_controls: true,
  });
  await settingsFormReady();
  expect(screen.getByRole("button", { name: "悬浮工具栏" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  expect(screen.getByRole("region", { name: "候选窗口预览" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "候选窗预编辑" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  expect(screen.getByRole("combobox", { name: "候选窗口主题" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "悬浮工具栏主题" })).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "候选栏主题" })).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  const shortcuts = screen.getByRole("group", { name: "快捷键" }) as HTMLFieldSetElement;
  expect(shortcuts.hidden).toBe(false);
  expect(
    within(shortcuts).getByText(
      "输入法快捷键仅在对应输入状态或候选窗口显示时生效。翻页方式和以词定字在「输入 › 选词与翻页」中设置。",
    ),
  ).toBeTruthy();
});

test("Harmony handwriting instructions do not leak 2-in-1 controls onto phones", async () => {
  renderSettings("harmony", { mobile_settings: true }, "handwriting");
  await settingsFormReady();
  expect(screen.getByText(/再从键盘的方案选择器切换到“手写”/)).toBeTruthy();
  expect(screen.queryByText(/2-in-1|2in1|候选窗口不绘制键面/)).toBeNull();

  cleanup();
  renderSettings("harmony", { mobile_settings: false }, "handwriting");
  await settingsFormReady();
  expect(screen.getByText(/2-in-1 候选窗口不绘制键面/)).toBeTruthy();
});

test("a desktop host keeps its window titlebar", async () => {
  renderSettings("windows");
  await settingsFormReady();

  expect(screen.getByRole("banner", { name: "窗口控制" })).toBeTruthy();
});

// The home card previews the keyboard the user will actually see. A phone has no number row, no Tab
// and no Win key, so the desktop artwork misdescribes every touch host.
test("a phone previews the touch keyboard, not the desktop one", async () => {
  renderSettings("android");
  await settingsFormReady();

  const preview = document.querySelector(".screen-keyboard-artwork");
  expect(preview).toBeTruthy();
  const keys = Array.from(preview!.querySelectorAll("[data-keyboard-key]")).map((key) =>
    key.getAttribute("data-keyboard-key"),
  );
  expect(keys).toContain("⇧");
  expect(keys).toContain("空格");
  expect(keys).not.toContain("Caps Lock");
  expect(keys).not.toContain("Win");
});

test("the desktop home card keeps the full keyboard", async () => {
  renderSettings("windows");
  await settingsFormReady();

  const preview = document.querySelector(".screen-keyboard-artwork");
  const keys = Array.from(preview!.querySelectorAll("[data-keyboard-key]")).map((key) =>
    key.getAttribute("data-keyboard-key"),
  );
  expect(keys).toContain("Caps Lock");
  expect(keys).toContain("Tab");
});

test("the phone home surface hides its duplicate page heading", async () => {
  renderSettings("android");
  await settingsFormReady();

  const heading = screen.getByRole("heading", { name: "首页" });
  expect(heading.parentElement?.className).toContain("max-phone:sr-only");

  cleanup();
  renderSettings("windows");
  await settingsFormReady();
  const desktopHeading = screen.getByRole("heading", { name: "首页" });
  expect(desktopHeading.parentElement?.className).not.toContain("max-phone:sr-only");
});

// A phone opens on the headline and carries no brand mark: the source shows none there, and the app
// is already the thing being looked at. Desktop keeps it, and there the path still has to resolve —
// home-page.tsx sits one directory deeper than index.tsx, and the same relative path written in both
// left a broken image on this very screen once already.
//
// Queried through the landmark rather than a class, because the home page's styling is Tailwind
// utilities: a class name there is a styling detail with no reason to stay put.
test("the home hero image is a desktop-only mark, and resolves", async () => {
  renderSettings("android");
  await settingsFormReady();

  const home = screen.getByRole("region", { name: "首页" });
  expect(home.querySelector("header img")).toBeNull();

  cleanup();
  renderSettings("windows");
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "首页" }));
  const desktopHero = screen
    .getByRole("region", { name: "首页" })
    .querySelector("header img") as HTMLImageElement;
  expect(desktopHero).toBeTruthy();
  expect(desktopHero.src).not.toContain("/keyboard/assets/");
  // Either form is a resolved asset: a path to the file, or the file itself once it is small enough
  // for the bundler to inline. What this guards against is a path that resolves to nothing.
  expect(
    desktopHero.src.includes("msime.svg") || desktopHero.src.startsWith("data:image/svg+xml"),
  ).toBe(true);
});

// A phone's primary navigation has to stay reachable by thumb, and it is a bottom tab bar on every
// touch host. The DOM deliberately keeps it ahead of the content so assistive technology and keyboard
// focus meet the navigation first; `order-2` is what seats it below. Both halves of that arrangement
// are asserted, because either one alone is wrong: the DOM order without the utility puts the bar back
// at the top, and the utility without the DOM order sends focus to the bottom of the page first.
test("the phone navigation leads the content but is seated below it", async () => {
  renderSettings("android");
  await settingsFormReady();

  const primary = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(primary).getByRole("button", { name: "设置" })).toBeTruthy();

  const body = primary.parentElement!;
  const content = body.querySelector("#settings-content")!;
  expect(primary.compareDocumentPosition(content) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

  // jsdom loads the stylesheet but resolves no media query, so the utility is read off the element
  // rather than from a computed style. `max-phone` is the project's own 600px breakpoint.
  const utilities = primary.className.split(/\s+/);
  expect(utilities).toContain("max-phone:order-2");
  // Each host draws its own bar: iOS a glass capsule floating clear of the edges, Android the Material 3 80px bar, HarmonyOS a flat 76px one. The bottom inset clears the gesture area on all three.
  expect(utilities).toContain("ios:rounded-full");
  expect(utilities).toContain("ios:mb-[max(0.5rem,env(safe-area-inset-bottom,0px))]");
  expect(utilities).toContain("android:min-h-20");
  expect(utilities).toContain("android:pb-[env(safe-area-inset-bottom,0px)]");
  expect(utilities).toContain("harmony:min-h-[76px]");
  // Android marks the selected tab with the 64x32 active indicator behind its glyph.
  const selected = within(primary).getByRole("button", { name: "设置" });
  const indicator = selected.firstElementChild!.className.split(/\s+/);
  expect(indicator).toContain("android:w-16");
  expect(indicator).toContain("android:h-8");
  expect(indicator).toContain("android:bg-accent-soft");
});

// A touch host names the keyboard pages the way its own settings do. The route ids do not change, so a host deep-linking `screen-keyboard` still lands on the page.
test("a touch host calls the keyboard pages by their touch names", async () => {
  renderSettings("android", { mode_switch_shortcuts: true }, "screen-keyboard");
  await settingsFormReady();
  expect(screen.getByRole("heading", { level: 1, name: "键盘" })).toBeTruthy();
  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  expect(within(sidebar).getByRole("button", { name: "外接键盘快捷键" })).toBeTruthy();
  expect(within(sidebar).queryByRole("button", { name: "屏幕键盘" })).toBeNull();
  expect(within(sidebar).queryByRole("button", { name: "快捷键" })).toBeNull();

  cleanup();
  renderSettings("windows", {}, "screen-keyboard");
  await settingsFormReady();
  expect(screen.getByRole("heading", { level: 1, name: "屏幕键盘" })).toBeTruthy();
});

// Each of the four tabs exists only where its page does, and each page is gated on the client capability behind it.
const allTabs = {
  account: {},
  typingStatistics: {},
  communitySkins: { list: vi.fn().mockResolvedValue({ skins: [], has_more: false }) },
};

// An iOS host wider than the phone breakpoint is an iPad, which the design lays out as a 320px settings sidebar beside the detail over the same four tabs. jsdom has no `matchMedia`, so the viewport is stubbed as wide for this one render.
test("a wide iOS host splits the 设置 tab and gives the other tabs the whole width", async () => {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: (query: string) => ({
      matches: query === "(min-width: 601px)",
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
    }),
  });
  try {
    renderSettings("ios", {}, "input", allTabs);
    await settingsFormReady();
    expect(document.querySelector("[data-platform]")?.getAttribute("data-platform")).toBe("ipad");
    const sidebar = screen.getByRole("navigation", { name: "设置分类" });
    expect(within(sidebar).getByRole("heading", { level: 2, name: "设置" })).toBeTruthy();
    expect(within(sidebar).getByRole("searchbox", { name: "搜索设置" })).toBeTruthy();
    // The tab bar already carries these, so the split does not list them a second time.
    for (const name of ["社区", "打字统计", "我的"]) {
      expect(within(sidebar).queryByRole("button", { name })).toBeNull();
    }
    const main = document.getElementById("settings-content")!;
    expect(sidebar.className).not.toContain("ipad:hidden");
    expect(main.className).not.toContain("ipad:col-span-2");

    fireEvent.click(
      within(screen.getByRole("navigation", { name: "主要功能" })).getByRole("button", {
        name: "社区",
      }),
    );
    expect(sidebar.className).toContain("ipad:hidden");
    expect(document.getElementById("settings-content")!.className).toContain("ipad:col-span-2");
  } finally {
    Reflect.deleteProperty(window, "matchMedia");
  }
});

// A phone page's large title scrolls away with the content, and a compact bar carrying the same name fades in over it. The bar is decorative (the `h1` still names the page) and belongs only to the 设置 tab's titled pages.
test("a phone page collapses its large title into a compact bar once scrolled", async () => {
  renderSettings("android", {}, "input", allTabs);
  await settingsFormReady();
  const main = document.getElementById("settings-content")!;
  const title = screen.getByRole("heading", { level: 1 }).textContent!;
  const bar = [...main.querySelectorAll('[aria-hidden="true"]')].find(
    (node) => node.textContent === title && node.className.includes("max-phone:sticky"),
  );
  if (!bar) throw new Error("no compact title bar");
  expect(bar.className).toContain("opacity-0");

  main.scrollTop = 120;
  fireEvent.scroll(main);
  expect(bar.className).toContain("opacity-100");
  main.scrollTop = 0;
  fireEvent.scroll(main);
  expect(bar.className).toContain("opacity-0");

  fireEvent.click(
    within(screen.getByRole("navigation", { name: "主要功能" })).getByRole("button", {
      name: "社区",
    }),
  );
  expect(
    [...document.getElementById("settings-content")!.querySelectorAll('[aria-hidden="true"]')].some(
      (node) => node.className.includes("max-phone:sticky"),
    ),
  ).toBe(false);
});

// The breakpoint the phone layout keys on has to keep meaning what the stylesheet used to say, or
// every `max-phone:` utility silently moves. Tailwind derives `max-*` as `width < value`, so 601px is
// the rendering of `@media (max-width: 600px)`.
test("the phone breakpoint is the 600px one the layout was written for", () => {
  expect(css).toContain("--breakpoint-phone: 601px");
});

// A phone has no Ctrl, no Alt and no Win key, and no panel window to theme. Both blocks were gated
// on `!androidPlatform` — a platform name, not a capability — so they reached every host that was
// not Android, and HarmonyOS and iOS were both being shown `Ctrl+F9 切换语音`. Asserted for the two
// hosts that were wrong and for one that is right, because a gate that hides it everywhere passes
// the first half of this on its own.
test("a phone is not offered the desktop's modifier-chord voice shortcuts", async () => {
  for (const platform of ["harmony", "ios"]) {
    renderSettings(platform);
    await settingsFormReady();
    fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
    const list = screen.getByRole("region", { name: "全部设置" });
    const row = [...list.querySelectorAll("button")].find(
      (item) => item.querySelector("strong")?.textContent === "语音输入",
    );
    if (!row) throw new Error(`no 语音输入 row on ${platform}`);
    fireEvent.click(row);
    expect(screen.queryByText("语音快捷键")).toBeNull();
    expect(screen.queryByText("语音输入弹出条主题")).toBeNull();
    cleanup();
    window.history.replaceState({}, "");
  }

  renderSettings("windows");
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  expect(screen.getByText("语音快捷键")).toBeTruthy();
});

// A HarmonyOS phone routes an attached keyboard's voice chords (VoiceHotkeyPolicy has no desktop check), so it declares `voice_hotkeys` and its owner can reach the switches that decide what Right Alt and Ctrl+F9 do there.
test("a phone that routes an attached keyboard's voice chords offers their switches", async () => {
  renderSettings("harmony", { mobile_settings: true, panel_windows: false, voice_hotkeys: true });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  const list = screen.getByRole("region", { name: "全部设置" });
  const row = [...list.querySelectorAll("button")].find(
    (item) => item.querySelector("strong")?.textContent === "语音输入",
  );
  if (!row) throw new Error("no 语音输入 row on harmony");
  fireEvent.click(row);
  expect(screen.getByText("语音快捷键")).toBeTruthy();
  expect(screen.getByText(/连接实体键盘后/)).toBeTruthy();
  expect(screen.queryByText("语音输入弹出条主题")).toBeNull();
});

// A route value reaches the page from a host menu or restored history. A name that exists only on Object.prototype (`toString`, `constructor`) once passed the alias lookup and resolved to a function, which a phone then pushed into history: jsdom keeps it, but a WebView's pushState structured-clones the state and throws, taking the settings page down with it. An unknown id has to fall back to the default page like any other.
test("a route naming an Object.prototype member pushes no uncloneable history state", async () => {
  const client = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockResolvedValue(undefined),
    host: { platform: "android" } as never,
    home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
  };
  const view = render(<SettingsPage client={client} />);
  await settingsFormReady();

  for (const [nonce, page] of ["toString", "constructor", "hasOwnProperty"].entries()) {
    view.rerender(<SettingsPage route={{ page, nonce: nonce + 1 }} client={client} />);
    await settingsFormReady();
    const pushed = window.history.state?.page;
    expect(pushed === undefined || typeof pushed === "string").toBe(true);
    expect(() => structuredClone(window.history.state)).not.toThrow();
  }
});
