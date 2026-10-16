// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
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

async function openThemePage(platform: string, host: Record<string, unknown> = {}) {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform, ...host }),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );
  // 等待主题页实际挂载，再检查各宿主提供的界面主题。
  await screen.findByLabelText("颜色模式");
}

// The Android keyboard resolves both of these against its own surface setting.
test("Android offers the emoji and handwriting panel themes", async () => {
  await openThemePage("android");

  expect(screen.getByLabelText("表情面板主题")).toBeTruthy();
  expect(screen.getByLabelText("手写识别板主题")).toBeTruthy();
});

// Only the native desktop menus read menu_theme; a touch host draws no menu it applies to.
test("mobile hosts do not offer a menu theme", async () => {
  await openThemePage("android");
  expect(screen.queryByLabelText("菜单主题")).toBeNull();

  cleanup();
  await openThemePage("ios");
  expect(screen.queryByLabelText("菜单主题")).toBeNull();
});

// 不跟随颜色模式的桌面宿主（`surface_themes_follow` 为假，现在只有测试宿主会这样报告）仍给菜单主题。
test("a desktop host whose surfaces do not follow keeps the menu theme", async () => {
  await openThemePage("windows", { surface_themes_follow: false });

  expect(screen.getByLabelText("菜单主题")).toBeTruthy();
});

// 桌面宿主的各界面一律跟随颜色模式（`surface_themes_follow`）：只剩候选窗口主题，放在「明暗」组里，主题页没有「高级」组。
test.each(["macos", "windows", "linux"])(
  "%s keeps only the candidate window's own light/dark, beside the colour mode",
  async (platform) => {
    await openThemePage(platform, { floating_toolbar: true });
    for (const label of [
      "设置界面主题",
      "屏幕键盘主题",
      "悬浮工具栏主题",
      "菜单主题",
      "表情面板主题",
      "手写识别板主题",
      "语音输入弹出条主题",
    ])
      expect(screen.queryByLabelText(label)).toBeNull();
    const candidate = screen.getByLabelText("候选窗口主题");
    const colourMode = screen.getByRole("region", { name: "明暗", hidden: true });
    expect(colourMode.contains(candidate)).toBe(true);
    expect(screen.queryByRole("region", { name: "高级", hidden: true })).toBeNull();
  },
);

// 宿主报告各界面不跟随时，同一份共享页面照旧给出各界面的单独明暗。
test("a desktop host whose surfaces do not follow keeps the per-surface themes in the 高级 group", async () => {
  await openThemePage("windows", { floating_toolbar: true, surface_themes_follow: false });
  const advanced = screen.getByRole("region", { name: "高级", hidden: true });
  for (const label of ["设置界面主题", "候选窗口主题", "悬浮工具栏主题", "菜单主题"])
    expect(advanced.contains(screen.getByLabelText(label))).toBe(true);
});

// The IBus property menu and the Fcitx5 status menu are drawn by the desktop panel, which applies its own theme.
test("Linux does not offer a menu theme it cannot apply", async () => {
  await openThemePage("linux");
  expect(screen.queryByLabelText("菜单主题")).toBeNull();
});

// Linux stands its toolbar up as the IBus property menu and the Fcitx5 status menu, drawn by the desktop panel; no Linux host reads toolbar_theme.
test("Linux does not offer a floating-toolbar theme it cannot apply", async () => {
  await openThemePage("linux", { floating_toolbar: true });
  expect(screen.queryByLabelText("悬浮工具栏主题")).toBeNull();
});

test("a desktop host that draws the toolbar and does not follow keeps its theme", async () => {
  // 跟随颜色模式的桌面宿主不给这一行，见上面的用例。
  await openThemePage("windows", { floating_toolbar: true, surface_themes_follow: false });
  expect(screen.getByLabelText("悬浮工具栏主题")).toBeTruthy();
});

// 跟随颜色模式的桌面宿主上工具栏和屏幕键盘没有自己的明暗、主题页也没有「高级」组，两页指向「主题」页的那一行要照实说它们跟随颜色模式和主题；颜色模式也不再是可被各界面覆盖的「默认」。Linux 不画工具栏那一行（见 floating-toolbar-page.tsx）。宿主报告不跟随时照旧。
const following = {
  colourMode: "设置窗口和各界面的明暗模式；候选窗口可在下面单独设置",
  toolbar: "悬浮工具栏跟随「主题」页的颜色模式和主题",
  keyboard: "屏幕键盘跟随「主题」页的颜色模式和主题",
};
test.each([
  ["macos", true, following],
  ["windows", true, following],
  ["linux", true, { ...following, toolbar: undefined }],
  [
    "windows",
    false,
    {
      colourMode: "设置窗口和各界面的默认明暗模式",
      toolbar: "悬浮工具栏的明暗在「主题」页设置",
      keyboard: "主题与明暗外观在「主题」页的「高级」中设置",
    },
  ],
] as const)(
  "on %s (surfaces follow: %s) the colour mode and the toolbar and keyboard pointers describe what the theme page offers",
  async (platform, follow, { colourMode, toolbar, keyboard }) => {
    await openThemePage(platform, {
      floating_toolbar: true,
      floating_toolbar_appearance: true,
      surface_themes_follow: follow,
    });
    expect(screen.getByText(colourMode)).toBeTruthy();
    const toolbarLink = screen.queryByRole("button", { name: "颜色与明暗", hidden: true });
    if (toolbar) expect(toolbarLink?.textContent).toContain(toolbar);
    else expect(toolbarLink).toBeNull();
    const keyboardLink = screen.getByRole("button", { name: "屏幕键盘外观", hidden: true });
    expect(keyboardLink.textContent).toContain(keyboard);
  },
);
