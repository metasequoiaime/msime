// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type ResolvedAppTheme, type Snapshot } from "@msime/ui";
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
        host: testHost({ platform, ...host }),
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
  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  // 2-in-1 是没有手机根页面的桌面窗口：侧栏从 输入 开始，也在 输入 打开。
  expect(within(sidebar).queryByRole("button", { name: "首页" })).toBeNull();
  expect(within(sidebar).queryByRole("button", { name: "设置" })).toBeNull();
  expect(sidebar.querySelector("button")?.textContent).toBe("输入");
  expect(screen.getByRole("heading", { level: 1, name: "输入" })).toBeTruthy();
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
  // HarmonyOS 关于页的头图在版本行里写明设备形态，而不是放标语。
  expect(screen.getByText(/^版本 .* · HarmonyOS 2in1$/)).toBeTruthy();
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
  expect(screen.getByText(/^版本 .* · HarmonyOS$/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "悬浮工具栏" })).toBeNull();
  fireEvent.click(
    within(screen.getByRole("navigation", { name: "主要功能" })).getByRole("button", {
      name: "设置",
    }),
  );
  const phoneSettings = screen.getByRole("region", { name: "首页" });
  expect(within(phoneSettings).queryByRole("button", { name: "悬浮工具栏" })).toBeNull();
  expect(within(phoneSettings).queryByRole("button", { name: "快捷键" })).toBeNull();
  expect(within(phoneSettings).queryByRole("button", { name: "外接键盘快捷键" })).toBeNull();
  // 手机把候选翻译放在 输入 页的 翻译 分组里。
  fireEvent.click(within(phoneSettings).getByRole("button", { name: "输入" }));
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
  fireEvent.click(
    within(screen.getByRole("region", { name: "首页" })).getByRole("button", {
      name: "候选栏",
    }),
  );
  // 手机的 候选栏 页没有预览，用滑块调整候选大小。
  expect(screen.queryByRole("region", { name: "候选栏预览" })).toBeNull();
  expect(screen.getByRole("slider", { name: "候选字号" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "预编辑字号" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "候选栏预编辑" })).toBeTruthy();
  expect(screen.queryByText("候选窗口预览")).toBeNull();
  expect(screen.queryByRole("combobox", { name: "候选窗口预编辑" })).toBeNull();

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
  expect(screen.getByRole("combobox", { name: "候选窗口预编辑" })).toBeTruthy();
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

// 手机的 设置 根页面是设计稿里的搜索框、状态卡片和各行；它不画键盘卡片，所以带数字行、Tab 和 Win 键的桌面键盘图永远不会在那里冒充触屏键盘。
test("a phone root draws no desktop keyboard artwork", async () => {
  renderSettings("android");
  await settingsFormReady();

  expect(
    screen.getByRole("region", { name: "首页" }).querySelector(".screen-keyboard-artwork"),
  ).toBeNull();
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

// 触屏宿主的键盘就是三排字母加一排功能键：设置页在根部声明宿主的键盘布局，没写 `layout` 的预览（键盘页、皮肤编辑器、社区皮肤详情等）都按它画，不会在手机上画出 Win、Alt、Caps Lock。
test.each([
  ["harmony", { mobile_settings: true }],
  ["android", {}],
])("a %s keyboard page previews the touch keyboard", async (platform, host) => {
  renderSettings(platform, host, "screen-keyboard");
  await settingsFormReady();
  const keys = Array.from(document.querySelectorAll("[data-keyboard-key]")).map((key) =>
    key.getAttribute("data-keyboard-key"),
  );
  expect(keys.length).toBeGreaterThan(0);
  expect(keys).not.toContain("Caps Lock");
  expect(keys).not.toContain("Win");
  expect(keys).not.toContain("Tab");
});

test("a desktop keyboard page keeps the full keyboard", async () => {
  renderSettings("windows", {}, "screen-keyboard");
  await settingsFormReady();
  const keys = Array.from(document.querySelectorAll("[data-keyboard-key]")).map((key) =>
    key.getAttribute("data-keyboard-key"),
  );
  expect(keys).toContain("Caps Lock");
});

test("the phone home surface hides its duplicate page heading", async () => {
  renderSettings("android");
  await settingsFormReady();

  // 触屏宿主把根页面叫作 设置，也就是它所在的标签页。
  const heading = screen.getByRole("heading", { name: "设置" });
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
    fireEvent.click(
      within(screen.getByRole("region", { name: "首页" })).getByRole("button", {
        name: "语音输入",
      }),
    );
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
  fireEvent.click(
    within(screen.getByRole("region", { name: "首页" })).getByRole("button", { name: "语音输入" }),
  );
  expect(screen.getByText("语音快捷键")).toBeTruthy();
  expect(screen.getByText(/连接实体键盘后/)).toBeTruthy();
  expect(screen.queryByText("语音输入弹出条主题")).toBeNull();
});

// A route value reaches the page from a host menu or restored history. A name that exists only on Object.prototype (`toString`, `constructor`) once passed the alias lookup and resolved to a function, which a phone then pushed into history: jsdom keeps it, but a WebView's pushState structured-clones the state and throws, taking the settings page down with it. An unknown id has to fall back to the default page like any other.
test("a route naming an Object.prototype member pushes no uncloneable history state", async () => {
  const client = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockResolvedValue(undefined),
    host: testHost({ platform: "android" }),
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

// 桌面窗口在侧栏顶部、搜索框之上写出品牌：macOS 和 HarmonyOS 2in1 没有自绘标题栏，品牌放进侧栏；Windows 的标题栏已经带着品牌，手机界面一律不画。HarmonyOS 宿主不提供窗口命令，这里也去掉。
test("the sidebar brand heads desktop sidebars and stays off phone layouts", async () => {
  const noWindowCommands = { windowControl: undefined, beginWindowDrag: undefined };
  for (const [platform, host, client] of [
    ["macos", {}, {}],
    ["harmony", { mobile_settings: false }, noWindowCommands],
  ] as const) {
    renderSettings(platform, host, undefined, client);
    await settingsFormReady();
    const sidebar = screen.getByRole("navigation", { name: "设置分类" });
    const brand = sidebar.querySelector<HTMLElement>("[data-sidebar-brand]");
    expect(brand, platform).not.toBeNull();
    expect(within(brand!).getByText("水杉输入法")).toBeTruthy();
    // 图标只是装饰，名称由文字给出，也不进 Tab 顺序。
    expect(brand!.querySelector("img")?.getAttribute("alt")).toBe("");
    expect(brand!.querySelector("a, button, [tabindex]")).toBeNull();
    const search = within(sidebar).getByRole("searchbox", { name: "搜索设置" });
    expect(brand!.compareDocumentPosition(search) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // macOS 的品牌行在红绿灯那一行下面，和那一行一样可以拖动窗口。
    expect(brand!.hasAttribute("data-window-drag")).toBe(platform === "macos");
    cleanup();
  }

  renderSettings("windows");
  await settingsFormReady();
  expect(document.querySelector("[data-sidebar-brand]")).toBeNull();
  expect(
    within(screen.getByRole("banner", { name: "窗口控制" })).getByText("水杉输入法"),
  ).toBeTruthy();
  cleanup();

  for (const [platform, host] of [
    ["android", {}],
    ["ios", {}],
    ["harmony", { mobile_settings: true }],
  ] as const) {
    renderSettings(platform, host);
    await settingsFormReady();
    expect(document.querySelector("[data-sidebar-brand]"), platform).toBeNull();
    cleanup();
  }
});

// ---- HarmonyOS 手机外壳 ----

const autumnLight: ResolvedAppTheme = {
  id: "siji",
  season: "autumn",
  accent: "#B5562B",
  accent_soft: "#B5562B22",
  on_accent: "#FFFFFF",
  background: "#F6E9DC",
  card: "#FFFBF6",
  hair: "#B5562B33",
};
const autumnDark: ResolvedAppTheme = {
  ...autumnLight,
  accent: "#F0975F",
  accent_soft: "#F0975F40",
  on_accent: "#3C2618",
  background: "#21150F",
  card: "#2E1E15",
  hair: "#FFFFFF1A",
};

/** 宿主的应用主题，季节可以在页面显示期间改变，就像月份更替或在 我的 里做出选择时那样。 */
function appThemeClient() {
  let theme = autumnLight;
  const listeners = new Set<() => void>();
  const client = {
    load: () => "siji" as const,
    save: () => true,
    catalog: () => [],
    resolve: vi.fn((dark: boolean) => (dark ? autumnDark : theme)),
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const change = (next: ResolvedAppTheme) => {
    theme = next;
    for (const listener of listeners) listener();
  };
  return { client, change };
}

const typingStatistics = {
  load: vi.fn().mockResolvedValue({
    availability: "neverWritten",
    statistics: { enabled: false, total: 0, days: {}, detail: {} },
  }),
  setEnabled: vi.fn(),
  reset: vi.fn(),
};

const shell = () => document.querySelector<HTMLElement>("[data-settings-shell]")!;

// 设计稿给每个 HarmonyOS 手机页面都加标题，标签页根页面也不例外；其他触屏宿主的 统计 和 我的 不加标题，它们的内容已经表明了自己是什么页面。
test("a HarmonyOS phone titles the 设置, 统计 and 我的 roots", async () => {
  renderSettings("harmony", {}, undefined, { typingStatistics, account: {} });
  await settingsFormReady();
  const root = screen.getByRole("heading", { level: 1, name: "设置" });
  expect(root.parentElement?.className).not.toContain("max-phone:sr-only");

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  fireEvent.click(within(bar).getByRole("button", { name: "统计" }));
  const statistics = await screen.findByRole("heading", { level: 1, name: "统计" });
  expect(statistics.parentElement?.className).not.toContain("max-phone:sr-only");
  cleanup();

  renderSettings("android", {}, "typing-statistics", { typingStatistics });
  const untitled = await screen.findByRole("heading", { level: 1, name: "统计" });
  expect(untitled.parentElement?.className).toContain("max-phone:sr-only");
});

// 设计稿把手机页面叫作 皮肤、表达、开发者选项 和 反馈，桌面侧栏则叫 主题、标点与翻译、维护与诊断 和 帮助与反馈。
test("a touch host uses the design's page names", async () => {
  renderSettings("harmony", {}, "expression");
  await settingsFormReady();
  expect(screen.getByRole("heading", { level: 1, name: "表达" })).toBeTruthy();
  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  for (const name of ["皮肤", "表达", "反馈"]) {
    expect(within(sidebar).getByRole("button", { name })).toBeTruthy();
  }
  for (const name of ["主题", "标点与翻译", "帮助与反馈", "首页"]) {
    expect(within(sidebar).queryByRole("button", { name })).toBeNull();
  }
});

// 推入标签页的页面在标题前放设计稿里的圆形返回按钮，取代回到上级页面的文字链接；标签页根页面没有这个按钮。它和边缘滑动一样沿 WebView 历史返回。
test("a HarmonyOS phone page pushed onto a tab has a back button", async () => {
  renderSettings("harmony");
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "返回" })).toBeNull();

  fireEvent.click(
    within(screen.getByRole("region", { name: "首页" })).getByRole("button", { name: "输入" }),
  );
  const header = screen.getByRole("heading", { level: 1, name: "输入" }).parentElement!;
  const back = within(header).getByRole("button", { name: "返回" });
  expect(back.querySelector("svg")?.getAttribute("width")).toBe("22");
  fireEvent.click(back);
  expect(await screen.findByRole("heading", { level: 1, name: "设置" })).toBeTruthy();
  cleanup();
  window.history.replaceState({}, "");

  // 帮助 是 反馈 的子页面：返回按钮取代了「‹ 反馈」链接。
  renderSettings("harmony", {}, "help");
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "返回反馈" })).toBeNull();
  expect(screen.getByRole("button", { name: "返回" })).toBeTruthy();
  cleanup();

  renderSettings("android", {}, "help");
  await settingsFormReady();
  expect(screen.getByRole("button", { name: "返回反馈" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "返回" })).toBeNull();
});

// 设置打开时所在的页面（启动时的深链接）在 WebView 历史里身后没有设置页面，所以它的返回按钮向上回到所在标签页的根页面，而不是退到宿主放在那里的任何内容。
test("the back button of the page the settings opened on goes up to its tab's root", async () => {
  const back = vi.spyOn(window.history, "back");
  renderSettings("harmony", {}, "dictionary");
  await settingsFormReady();
  const push = vi.spyOn(window.history, "pushState");
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(back).not.toHaveBeenCalled();
  expect(screen.getByRole("heading", { level: 1, name: "设置" })).toBeTruthy();
  // 根页面替换深链接页面的历史条目，而不是叠在它上面，这样在 设置 上做系统返回手势会直接离开，不会重新打开 词库。
  expect(push).not.toHaveBeenCalled();
  expect(window.history.state).toEqual(expect.objectContaining({ page: "home" }));
  push.mockRestore();

  // 从那里推入的页面沿历史返回。
  fireEvent.click(
    within(screen.getByRole("region", { name: "首页" })).getByRole("button", { name: "输入" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(back).toHaveBeenCalledTimes(1);
});

// 推入标签页的页面会滑入，标签页根页面不会。社区、统计 和 我的 带有标记，让样式表画出更紧凑的标签页列表。
test("a HarmonyOS phone pushes pages in and marks the tab pages", async () => {
  renderSettings("harmony", {}, undefined, { typingStatistics, account: {} });
  await settingsFormReady();
  const columnOf = () =>
    [...document.getElementById("settings-content")!.children].find((node) =>
      node.className.includes("max-w-[900px]"),
    )!;
  expect(columnOf().className).not.toContain("animate-ms-push-in");
  expect(columnOf().hasAttribute("data-tab-page")).toBe(false);

  fireEvent.click(
    within(screen.getByRole("region", { name: "首页" })).getByRole("button", { name: "输入" }),
  );
  expect(columnOf().className).toContain("animate-ms-push-in");
  expect(columnOf().className).toContain("motion-reduce:animate-none");
  expect(columnOf().hasAttribute("data-tab-page")).toBe(false);

  fireEvent.click(
    within(screen.getByRole("navigation", { name: "主要功能" })).getByRole("button", {
      name: "统计",
    }),
  );
  await screen.findByRole("heading", { level: 1, name: "统计" });
  expect(columnOf().hasAttribute("data-tab-page")).toBe(true);
  expect(columnOf().className).not.toContain("animate-ms-push-in");
  cleanup();

  renderSettings("android", {}, "input");
  await settingsFormReady();
  expect(columnOf().className).not.toContain("animate-ms-push-in");
});

// 应用主题为 HarmonyOS 手机重新着色：强调色系以及该季节的页面、卡片和细线颜色以内联方式写在带 `data-platform` 的元素上，并跟随外观和宿主的主题变化。
test("a HarmonyOS phone takes the season's colours from the host app theme", async () => {
  // jsdom 没有 `matchMedia`，设置会把这种情况当成深色系统；这次渲染打桩成浅色。
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: (query: string) => ({
      matches: query === "(prefers-color-scheme: light)",
      media: query,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
    }),
  });
  try {
    const { client: appTheme, change } = appThemeClient();
    const setSystemBars = vi.fn();
    renderSettings("harmony", {}, undefined, { appTheme, chrome: { setSystemBars } });
    await settingsFormReady();

    const root = shell();
    expect(root.getAttribute("data-platform")).toBe("harmony");
    expect(root.getAttribute("data-season")).toBe("autumn");
    expect(appTheme.resolve).toHaveBeenLastCalledWith(false);
    expect(root.style.getPropertyValue("--accent-color")).toBe(autumnLight.accent);
    expect(root.style.getPropertyValue("--p-bg")).toBe(autumnLight.background);
    expect(root.style.getPropertyValue("--p-group-bg")).toBe(autumnLight.card);
    expect(root.style.getPropertyValue("--p-hair")).toBe(autumnLight.hair);
    // 状态栏取页面背景色，导航栏取标签栏的固定颜色。
    expect(setSystemBars).toHaveBeenLastCalledWith({
      background: autumnLight.background,
      navigationBar: "#F1F3F5",
      dark: false,
    });

    act(() => change({ ...autumnLight, season: "winter", background: "#EEF2F6" }));
    expect(root.getAttribute("data-season")).toBe("winter");
    expect(root.style.getPropertyValue("--p-bg")).toBe("#EEF2F6");
    expect(setSystemBars).toHaveBeenLastCalledWith({
      background: "#EEF2F6",
      navigationBar: "#F1F3F5",
      dark: false,
    });
  } finally {
    Reflect.deleteProperty(window, "matchMedia");
  }
  cleanup();

  // 深色外观下季节解析为深色，系统栏请求浅色内容。
  const { client: appTheme } = appThemeClient();
  const setSystemBars = vi.fn();
  renderSettings("harmony", {}, undefined, { appTheme, chrome: { setSystemBars } });
  await settingsFormReady();
  expect(appTheme.resolve).toHaveBeenLastCalledWith(true);
  expect(shell().style.getPropertyValue("--p-bg")).toBe(autumnDark.background);
  expect(setSystemBars).toHaveBeenLastCalledWith({
    background: autumnDark.background,
    navigationBar: "#141414",
    dark: true,
  });
});

// 2-in-1 只取强调色系，保留自己的中性表面色；它没有手机系统栏。其他平台忽略应用主题。
test("the 2-in-1 takes the season's accent only and other hosts none of it", async () => {
  const { client: appTheme } = appThemeClient();
  const setSystemBars = vi.fn();
  renderSettings("harmony", { mobile_settings: false }, undefined, {
    appTheme,
    chrome: { setSystemBars },
  });
  await settingsFormReady();
  expect(shell().getAttribute("data-platform")).toBe("hm2");
  expect(shell().style.getPropertyValue("--accent-color")).not.toBe("");
  expect(shell().style.getPropertyValue("--p-bg")).toBe("");
  expect(setSystemBars).not.toHaveBeenCalled();
  cleanup();

  renderSettings("android", {}, undefined, { appTheme, chrome: { setSystemBars } });
  await settingsFormReady();
  expect(shell().getAttribute("style")).toBeNull();
  expect(shell().hasAttribute("data-season")).toBe(false);
  expect(setSystemBars).not.toHaveBeenCalled();
});

// 没有应用主题时，手机仍然给系统栏着色，用的是 token 表自己的页面背景色。
test("a HarmonyOS phone without an app theme colours its bars in the default background", async () => {
  const setSystemBars = vi.fn();
  renderSettings("harmony", {}, undefined, { chrome: { setSystemBars } });
  await settingsFormReady();
  // jsdom 没有 `matchMedia`，设置会把这种情况当成深色系统。
  expect(setSystemBars).toHaveBeenLastCalledWith({
    background: "#000000",
    navigationBar: "#141414",
    dark: true,
  });
  expect(shell().hasAttribute("data-season")).toBe(false);
});

// 外壳为每个页面承载设计稿的 toast：一个始终挂载在带主题的根元素内的 polite live region。
test("the settings shell carries the toast live region", async () => {
  renderSettings("harmony");
  await settingsFormReady();
  const region = [...shell().children].find(
    (node) => node.getAttribute("role") === "status" && node.getAttribute("aria-live") === "polite",
  );
  expect(region).toBeTruthy();
});

// 2-in-1 没有设置引导卡片，所以 输入 和 关于 会说明还缺什么；其他页面和手机不会。
test("the 2-in-1 warns on 输入 and 关于 while the input method is not set up", async () => {
  const setup = {
    read: () => ({ enabled: false, current: false }),
    subscribe: () => () => {},
  };
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  renderSettings("harmony", { mobile_settings: false }, undefined, {
    home: { setup, openSystemKeyboardSettings },
  });
  await settingsFormReady();
  expect(screen.getByText("水杉输入法尚未在「系统 → 输入法」中添加")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "去添加" }));
  expect(openSystemKeyboardSettings).toHaveBeenCalledTimes(1);

  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(screen.getByText("水杉输入法尚未在「系统 → 输入法」中添加")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  expect(screen.queryByText("水杉输入法尚未在「系统 → 输入法」中添加")).toBeNull();
  cleanup();

  renderSettings("harmony", {}, "input", { home: { setup, openSystemKeyboardSettings } });
  await settingsFormReady();
  expect(screen.queryByText("水杉输入法尚未在「系统 → 输入法」中添加")).toBeNull();
});
