// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { HomePage, SettingsPage, type HostCapabilities, type Snapshot } from "@msime/ui";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 7,
  preferences: {
    scheme: "shuangpin",
    shuangpin_profile: "xiaohe",
    global_theme: "night",
    touch_keyboard_schemes: { enabled: ["xiaohe"], selected: "xiaohe" },
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

test("renders the keyboard home surface with the current skin and scheme", () => {
  render(<HomePage preferences={initial.preferences} onOpenPage={vi.fn()} />);

  expect(screen.getByRole("region", { name: "首页" })).toBeTruthy();
  expect(screen.getByText("我的键盘")).toBeTruthy();
  expect(screen.getByText("夜青 · 小鹤双拼")).toBeTruthy();
  expect(screen.getByRole("img", { name: "屏幕键盘完整布局预览" })).toBeTruthy();
  expect(screen.getByText("高情商回复")).toBeTruthy();
});

// Each shortcut is meant to be recognisable by its own colour rather than by reading the label, so
// the six icons must not collapse onto one palette. Found through the grid that holds them: the
// tiles are styled with utilities now, and a class name there is no longer a stable handle.
test("home shortcuts expose a distinct visual tile for each function", () => {
  render(<HomePage preferences={initial.preferences} onOpenPage={vi.fn()} />);
  const grid = screen.getByRole("button", { name: /皮肤/ }).parentElement!;
  const tiles = [...grid.querySelectorAll("button")];
  expect(tiles).toHaveLength(6);

  const icons = tiles.map((tile) => tile.querySelector<HTMLElement>('span[aria-hidden="true"]')!);
  expect(icons.every(Boolean)).toBe(true);
  expect(new Set(icons.map((icon) => icon.className)).size).toBe(6);
});

test("routes home shortcuts to the shared settings pages", () => {
  const onOpenPage = vi.fn();
  render(<HomePage preferences={initial.preferences} onOpenPage={onOpenPage} />);

  fireEvent.click(screen.getByRole("button", { name: /皮肤/ }));
  fireEvent.click(screen.getByRole("button", { name: /输入方案/ }));
  fireEvent.click(screen.getByRole("button", { name: /按键/ }));
  // The row used to land on 外观 alone; it opens the 全部设置 list now, which is where every page
  // without a tab of its own is reached.
  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  fireEvent.click(screen.getByRole("button", { name: /高情商回复/ }));

  expect(onOpenPage.mock.calls).toEqual([
    ["skin"],
    ["input"],
    ["screen-keyboard"],
    ["more"],
    ["input"],
  ]);
});

test("exposes Apple home shortcuts for dictionary, AI and system settings", () => {
  const onOpenPage = vi.fn();
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  render(
    <HomePage
      preferences={initial.preferences}
      actions={{ openSystemKeyboardSettings }}
      onOpenPage={onOpenPage}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "词库个人词与同步" }));
  fireEvent.click(screen.getByRole("button", { name: "AI回复与润色" }));
  fireEvent.click(screen.getByRole("button", { name: "系统设置启用与完全访问" }));

  expect(onOpenPage.mock.calls).toEqual([["dictionary"], ["ai"]]);
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();
});

test("routes the keyboard card to the shared tryout when a mobile host cannot open a window", () => {
  const onOpenPage = vi.fn();
  const onOpenChat = vi.fn();
  render(
    <HomePage preferences={initial.preferences} onOpenPage={onOpenPage} onOpenChat={onOpenChat} />,
  );

  fireEvent.click(screen.getByRole("button", { name: /试用键盘/ }));

  expect(onOpenChat).toHaveBeenCalledOnce();
  expect(onOpenPage).not.toHaveBeenCalled();
});

test("iOS home keyboard card opens and focuses the shared keyboard tryout", async () => {
  const client = {
    load: async () => initial,
    save: vi.fn(),
    host: { platform: "ios" } as HostCapabilities,
    home: { openSystemKeyboardSettings: vi.fn() },
    chat: {
      models: async () => ({ data: [{ id: "fixture-chat" }], defaultModel: "fixture-chat" }),
      complete: async () => "fixture reply",
    },
  };
  render(<SettingsPage client={client} />);
  await screen.findByRole("region", { name: "首页" });
  fireEvent.click(screen.getByRole("button", { name: /试用键盘/ }));
  const composer = await screen.findByRole("textbox", { name: "聊天消息" });
  await waitFor(() => expect(document.activeElement).toBe(composer));
});

test("selects thoughtful reply from the home feature entry", () => {
  const onOpenPage = vi.fn();
  const onSelectScheme = vi.fn();
  render(
    <HomePage
      preferences={initial.preferences}
      onOpenPage={onOpenPage}
      onSelectScheme={onSelectScheme}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: /高情商回复/ }));

  expect(onSelectScheme).toHaveBeenCalledWith("thoughtful_reply");
  expect(onOpenPage).toHaveBeenCalledWith("input");
});

test("invokes Android keyboard and system input actions", () => {
  const actions = {
    openKeyboard: vi.fn().mockResolvedValue(undefined),
    openSystemKeyboardSettings: vi.fn().mockResolvedValue(undefined),
    showInputMethodPicker: vi.fn().mockResolvedValue(undefined),
  };
  render(<HomePage preferences={initial.preferences} actions={actions} onOpenPage={vi.fn()} />);

  fireEvent.click(screen.getByRole("button", { name: /试用键盘/ }));
  fireEvent.click(screen.getByRole("button", { name: "系统键盘设置" }));
  fireEvent.click(screen.getByRole("button", { name: "选择输入法" }));

  expect(actions.openKeyboard).toHaveBeenCalledOnce();
  expect(actions.openSystemKeyboardSettings).toHaveBeenCalledOnce();
  expect(actions.showInputMethodPicker).toHaveBeenCalledOnce();
});

test("opens the Android emoji and clipboard tools inside the mobile shell", () => {
  const openEmojiPanel = vi.fn().mockResolvedValue(undefined);
  const openClipboardPanel = vi.fn().mockResolvedValue(undefined);
  render(
    <HomePage
      preferences={initial.preferences}
      actions={{ openEmojiPanel, openClipboardPanel }}
      onOpenPage={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "表情与符号" }));
  fireEvent.click(screen.getByRole("button", { name: "剪贴板历史" }));

  expect(openEmojiPanel).toHaveBeenCalledOnce();
  expect(openClipboardPanel).toHaveBeenCalledOnce();
});

test("opens Android on home and preserves the appearance fallback without home capability", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        home: {
          openKeyboard: vi.fn().mockResolvedValue(undefined),
        },
      }}
    />,
  );
  await screen.findByRole("region", { name: "首页" });
  expect(screen.getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBe("page");

  cleanup();
  render(<SettingsPage client={{ load: async () => initial, save: vi.fn() }} />);
  await settingsFormReady();
  expect(screen.getByRole("button", { name: "候选窗口" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(screen.queryByRole("region", { name: "首页" })).toBeNull();
});

test("opens iOS system keyboard settings from the shared home", async () => {
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        home: {
          openSystemKeyboardSettings,
        },
      }}
    />,
  );

  await screen.findByRole("region", { name: "首页" });
  fireEvent.click(screen.getByRole("button", { name: "系统键盘设置" }));
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();
  expect(screen.queryByRole("button", { name: "选择输入法" })).toBeNull();
});

test("enables and selects thoughtful reply when opened from Android home", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        touchKeyboardSchemes: true,
        home: {
          openKeyboard: vi.fn().mockResolvedValue(undefined),
        },
      }}
    />,
  );
  await screen.findByRole("region", { name: "首页" });
  fireEvent.click(screen.getByRole("button", { name: /高情商回复/ }));

  expect(
    screen
      .getByRole("button", { name: "设为当前输入方案 高情商回复" })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  expect(
    (screen.getByRole("switch", { name: "显示输入方案 高情商回复" }) as HTMLInputElement).checked,
  ).toBe(true);
});
