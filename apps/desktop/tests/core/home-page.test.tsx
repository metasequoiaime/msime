// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { HomePage, SettingsPage, type Snapshot } from "@msime/ui";
import type {
  ImeSetupClient,
  ImeSetupState,
} from "../../../../packages/ui/src/core/host-contracts";
import { ToastProvider } from "../../../../packages/ui/src/core/toast";
import { SetupWarningStrip } from "../../../../packages/ui/src/keyboard/setup-status-card";
import {
  voiceLanguageTitle,
  type RootPage,
} from "../../../../packages/ui/src/keyboard/root-settings-list";

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

const preferences = {
  ...initial.preferences,
  voice_input: { enabled: true, language: "yue" },
} as Snapshot["preferences"];

// 外壳交给「设置」根页的各页面，已带上手机端标题，按外壳自己的（导航分组）顺序排列，而不是根页的顺序。
const rootPages: RootPage[] = [
  { id: "input", title: "输入" },
  { id: "expression", title: "表达" },
  { id: "shortcuts", title: "外接键盘快捷键" },
  { id: "dictionary", title: "词库" },
  { id: "skin", title: "皮肤" },
  { id: "appearance", title: "候选栏" },
  { id: "floating-toolbar", title: "悬浮工具栏" },
  { id: "screen-keyboard", title: "键盘" },
  { id: "voice", title: "语音输入" },
  { id: "handwriting", title: "手写输入" },
  { id: "tools", title: "剪贴板" },
  { id: "ai", title: "AI 辅助" },
  { id: "developer", title: "开发者选项" },
];

/** 宿主设置客户端，测试像宿主的变更通知那样向它推送状态。 */
function fakeSetup(first: ImeSetupState | null) {
  const listeners = new Set<(state: ImeSetupState) => void>();
  const client: ImeSetupClient = {
    read: vi.fn(() => first),
    subscribe: vi.fn((listener: (state: ImeSetupState) => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    }),
  };
  const push = (state: ImeSetupState) =>
    act(() => listeners.forEach((listener) => listener(state)));
  return { client, push, listeners };
}

function renderTouchHome(
  options: {
    actions?: Parameters<typeof HomePage>[0]["actions"];
    onOpenPage?: (page: string) => void;
    pages?: RootPage[];
  } = {},
) {
  const onOpenPage = options.onOpenPage ?? vi.fn();
  render(
    <ToastProvider>
      <HomePage
        preferences={preferences}
        actions={options.actions}
        onOpenPage={onOpenPage}
        touchLayout
        rootPages={options.pages ?? rootPages}
      />
    </ToastProvider>,
  );
  return { onOpenPage, home: screen.getByRole("region", { name: "首页" }) };
}

/** 根页的导航行标题，按出现顺序排列。 */
function rowTitles(container: HTMLElement) {
  return [...container.querySelectorAll("[data-row-title]")].map((title) => title.textContent);
}

function statusCard() {
  return screen.getByRole("group", { name: "水杉输入法状态" });
}

// ---- 触屏首页：「设置」根页 ----

test("the touch home is the 设置 root: search, status card and grouped rows, without the old home surface", () => {
  const { home } = renderTouchHome();

  expect(within(home).getByRole("searchbox", { name: "搜索设置" })).toBeTruthy();
  expect(statusCard()).toBeTruthy();
  for (const gone of ["让输入，更像你", "我的键盘", "高情商回复", "全部设置", "选择输入法"]) {
    expect(within(home).queryByText(gone)).toBeNull();
  }
  expect(within(home).queryByRole("img", { name: "屏幕键盘完整布局预览" })).toBeNull();
});

test("root rows follow the design's grouping and order, with every other offered page in a last group", () => {
  const { home } = renderTouchHome();

  expect(rowTitles(home)).toEqual([
    "皮肤",
    "键盘",
    "候选栏",
    "输入",
    "表达",
    "词库",
    "外接键盘快捷键",
    "语音输入",
    "手写输入",
    "剪贴板",
    "AI 辅助",
    "开发者选项",
    "悬浮工具栏",
  ]);
  const groupOf = (title: string) => screen.getByRole("button", { name: title }).parentElement;
  expect(groupOf("键盘")).toBe(groupOf("皮肤"));
  expect(groupOf("候选栏")).toBe(groupOf("皮肤"));
  expect(groupOf("输入")).not.toBe(groupOf("皮肤"));
  expect(groupOf("外接键盘快捷键")).toBe(groupOf("输入"));
  expect(groupOf("手写输入")).toBe(groupOf("语音输入"));
  expect(groupOf("AI 辅助")).toBe(groupOf("剪贴板"));
  expect(groupOf("开发者选项")).not.toBe(groupOf("剪贴板"));
  expect(groupOf("悬浮工具栏")).not.toBe(groupOf("开发者选项"));
});

test("root rows show only the pages the shell offers", () => {
  const { home } = renderTouchHome({
    pages: rootPages.filter((page) => !["shortcuts", "developer", "handwriting"].includes(page.id)),
  });

  const titles = rowTitles(home);
  expect(titles).not.toContain("外接键盘快捷键");
  expect(titles).not.toContain("开发者选项");
  expect(titles).not.toContain("手写输入");
  expect(titles).toContain("语音输入");
});

test("root rows carry the current values read from the preferences", () => {
  renderTouchHome();

  expect(screen.getByRole("button", { name: "皮肤", description: "夜青" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "输入", description: "小鹤双拼" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "表达", description: "中文标点" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "词库", description: "记忆新词已开" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "语音输入", description: "粤语" })).toBeTruthy();
  for (const plain of ["键盘", "候选栏", "手写输入", "剪贴板", "AI 辅助", "开发者选项"]) {
    expect(screen.getByRole("button", { name: plain }).getAttribute("aria-describedby")).toBeNull();
  }
});

test("root row values follow the opposite preferences and a custom skin", () => {
  render(
    <HomePage
      preferences={{
        ...preferences,
        global_theme: "custom",
        learning: false,
        chinese_punctuation: false,
      }}
      onOpenPage={vi.fn()}
      touchLayout
      rootPages={rootPages}
    />,
  );

  expect(screen.getByRole("button", { name: "皮肤", description: "我的皮肤" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "表达", description: "英文标点" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "词库", description: "记忆新词已关" })).toBeTruthy();
});

test("voice languages are named for both locale ids and provider codes, and unknown codes are shown as they are", () => {
  expect(voiceLanguageTitle(undefined)).toBe("普通话");
  expect(voiceLanguageTitle("zh-CN")).toBe("普通话");
  expect(voiceLanguageTitle("zh-cn")).toBe("普通话");
  expect(voiceLanguageTitle("yue")).toBe("粤语");
  expect(voiceLanguageTitle("zh-HK")).toBe("粤语");
  expect(voiceLanguageTitle("en-US")).toBe("英语");
  expect(voiceLanguageTitle("en")).toBe("英语");
  expect(voiceLanguageTitle("ja-JP")).toBe("日语");
  expect(voiceLanguageTitle("auto")).toBe("普通话 + 英语");
  expect(voiceLanguageTitle("ko-KR")).toBe("ko-KR");
});

test("root rows open their pages", () => {
  const { onOpenPage } = renderTouchHome();

  for (const title of ["皮肤", "表达", "语音输入", "剪贴板", "AI 辅助", "悬浮工具栏"]) {
    fireEvent.click(screen.getByRole("button", { name: title }));
  }

  expect(vi.mocked(onOpenPage).mock.calls).toEqual([
    ["skin"],
    ["expression"],
    ["voice"],
    ["tools"],
    ["ai"],
    ["floating-toolbar"],
  ]);
});

test("a shell that passes no root pages still reaches every page through 全部设置", () => {
  const onOpenPage = vi.fn();
  render(<HomePage preferences={preferences} onOpenPage={onOpenPage} touchLayout />);

  fireEvent.click(screen.getByRole("button", { name: "全部设置" }));
  expect(onOpenPage).toHaveBeenCalledWith("more");
});

// ---- 搜索 ----

test("search lists the rows whose title or value matches, case-insensitively, and hides the card and groups", () => {
  const { home, onOpenPage } = renderTouchHome();
  const search = screen.getByRole("searchbox", { name: "搜索设置" });

  fireEvent.change(search, { target: { value: "ai" } });
  expect(rowTitles(home)).toEqual(["AI 辅助"]);
  expect(screen.queryByRole("group", { name: "水杉输入法状态" })).toBeNull();

  // 「中文标点」是「表达」行的值，不是它的标题。
  fireEvent.change(search, { target: { value: "标点" } });
  expect(rowTitles(home)).toEqual(["表达"]);

  fireEvent.change(search, { target: { value: "  键盘 " } });
  expect(rowTitles(home)).toEqual(["键盘", "外接键盘快捷键"]);

  fireEvent.click(screen.getByRole("button", { name: "外接键盘快捷键" }));
  expect(onOpenPage).toHaveBeenCalledWith("shortcuts");
});

test("search says so when nothing matches", () => {
  const { home } = renderTouchHome();

  fireEvent.change(screen.getByRole("searchbox", { name: "搜索设置" }), {
    target: { value: "不存在的设置" },
  });

  expect(screen.getByText("没有匹配的设置")).toBeTruthy();
  expect(rowTitles(home)).toEqual([]);
});

test("Escape clears the search and brings back the card and groups", () => {
  const { home } = renderTouchHome();
  const search = screen.getByRole("searchbox", { name: "搜索设置" }) as HTMLInputElement;

  fireEvent.change(search, { target: { value: "词库" } });
  expect(rowTitles(home)).toEqual(["词库"]);
  fireEvent.keyDown(search, { key: "Escape" });

  expect(search.value).toBe("");
  expect(statusCard()).toBeTruthy();
  expect(rowTitles(home)).toHaveLength(rootPages.length);
});

// ---- 状态卡片 ----

test("the status card says 检查中 and offers no step until the host reports, then follows its notifications", () => {
  const setup = fakeSetup(null);
  const actions = {
    setup: setup.client,
    openSystemKeyboardSettings: vi.fn().mockResolvedValue(undefined),
    showInputMethodPicker: vi.fn().mockResolvedValue(undefined),
  };
  renderTouchHome({ actions });
  const card = statusCard();

  expect(within(card).getByText("检查中")).toBeTruthy();
  expect(within(card).queryByRole("button", { name: "去开启" })).toBeNull();
  expect(within(card).queryByRole("button", { name: "设为默认" })).toBeNull();
  expect(setup.client.read).toHaveBeenCalledOnce();

  setup.push({ enabled: false, current: false });
  expect(within(card).getByText("尚未完成设置 · 小鹤双拼")).toBeTruthy();
  fireEvent.click(within(card).getByRole("button", { name: "去开启" }));
  // 选择器只列出已启用的输入法，所以在第一步之前，「设为默认」还会打开系统列表。
  fireEvent.click(within(card).getByRole("button", { name: "设为默认" }));
  expect(actions.openSystemKeyboardSettings).toHaveBeenCalledTimes(2);
  expect(actions.showInputMethodPicker).not.toHaveBeenCalled();

  setup.push({ enabled: true, current: false });
  expect(within(card).getByText("尚未完成设置 · 小鹤双拼")).toBeTruthy();
  expect(within(card).queryByRole("button", { name: "去开启" })).toBeNull();
  fireEvent.click(within(card).getByRole("button", { name: "设为默认" }));
  expect(actions.showInputMethodPicker).toHaveBeenCalledOnce();

  setup.push({ enabled: true, current: true });
  expect(within(card).getByText("已启用 · 小鹤双拼")).toBeTruthy();
  expect(within(card).queryByRole("button", { name: "去开启" })).toBeNull();
  expect(within(card).queryByRole("button", { name: "设为默认" })).toBeNull();
});

test("the status card reads the host's state on mount and unsubscribes on unmount", () => {
  const setup = fakeSetup({ enabled: true, current: true });
  renderTouchHome({ actions: { setup: setup.client } });

  expect(within(statusCard()).getByText("已启用 · 小鹤双拼")).toBeTruthy();
  expect(setup.listeners.size).toBe(1);
  cleanup();
  expect(setup.listeners.size).toBe(0);
});

test("a check the host has not answered stays unknown while the other is shown", () => {
  const setup = fakeSetup({ enabled: true, current: null });
  renderTouchHome({ actions: { setup: setup.client, showInputMethodPicker: vi.fn() } });
  const card = statusCard();

  expect(within(card).getByText("检查中")).toBeTruthy();
  expect(within(card).queryByRole("button", { name: "设为默认" })).toBeNull();
});

test("without a setup client the card offers both steps and shows the scheme alone", () => {
  const actions = {
    openSystemKeyboardSettings: vi.fn().mockResolvedValue(undefined),
    showInputMethodPicker: vi.fn().mockResolvedValue(undefined),
  };
  renderTouchHome({ actions });
  const card = statusCard();

  expect(within(card).getByText("小鹤双拼")).toBeTruthy();
  expect(within(card).queryByText("检查中")).toBeNull();
  fireEvent.click(within(card).getByRole("button", { name: "去开启" }));
  fireEvent.click(within(card).getByRole("button", { name: "设为默认" }));
  expect(actions.openSystemKeyboardSettings).toHaveBeenCalledOnce();
  expect(actions.showInputMethodPicker).toHaveBeenCalledOnce();
});

test("a setup step the host could not take is reported as a toast", async () => {
  const setup = fakeSetup({ enabled: true, current: false });
  renderTouchHome({
    actions: {
      setup: setup.client,
      showInputMethodPicker: vi.fn().mockRejectedValue(new Error("无法打开输入法选择器")),
    },
  });

  fireEvent.click(within(statusCard()).getByRole("button", { name: "设为默认" }));

  expect(await screen.findByText("无法打开输入法选择器")).toBeTruthy();
});

// ---- 试用键盘 ----

test("试用键盘 opens the tryout sheet with a focused text area when the host has no keyboard window", () => {
  renderTouchHome({ actions: { setup: fakeSetup({ enabled: true, current: true }).client } });
  const tryButton = within(statusCard()).getByRole("button", { name: "试用键盘" });
  tryButton.focus();

  fireEvent.click(tryButton);

  const sheet = screen.getByRole("dialog", { name: "试用键盘" });
  const field = within(sheet).getByRole("textbox", { name: "试用键盘输入框" });
  expect(field.getAttribute("placeholder")).toBe("在这里打字试试");
  expect(document.activeElement).toBe(field);
  // 已经是当前输入法：无需提醒。
  expect(within(sheet).queryByText(/先完成上面的设置/)).toBeNull();

  fireEvent.click(within(sheet).getByRole("button", { name: "完成" }));
  expect(screen.queryByRole("dialog", { name: "试用键盘" })).toBeNull();
  expect(document.activeElement).toBe(tryButton);
});

test("the tryout sheet warns while this is not the current input method and closes on Escape", () => {
  const showInputMethodPicker = vi.fn().mockResolvedValue(undefined);
  renderTouchHome({
    actions: { setup: fakeSetup({ enabled: true, current: false }).client, showInputMethodPicker },
  });

  fireEvent.click(within(statusCard()).getByRole("button", { name: "试用键盘" }));
  const sheet = screen.getByRole("dialog", { name: "试用键盘" });
  expect(within(sheet).getByText(/先完成上面的设置，键盘才会是水杉/)).toBeTruthy();
  fireEvent.click(within(sheet).getByRole("button", { name: "设为默认" }));
  expect(showInputMethodPicker).toHaveBeenCalledOnce();

  fireEvent.keyDown(within(sheet).getByRole("textbox"), { key: "Escape" });
  expect(screen.queryByRole("dialog", { name: "试用键盘" })).toBeNull();
});

test("试用键盘 keeps a host's own keyboard window, and falls back to the sheet when it fails", async () => {
  const openKeyboard = vi.fn().mockResolvedValue(undefined);
  renderTouchHome({ actions: { openKeyboard } });

  fireEvent.click(screen.getByRole("button", { name: "试用键盘" }));
  expect(openKeyboard).toHaveBeenCalledOnce();
  await act(async () => {});
  expect(screen.queryByRole("dialog", { name: "试用键盘" })).toBeNull();

  cleanup();
  renderTouchHome({ actions: { openKeyboard: vi.fn().mockRejectedValue(new Error("no window")) } });
  fireEvent.click(screen.getByRole("button", { name: "试用键盘" }));
  expect(await screen.findByRole("dialog", { name: "试用键盘" })).toBeTruthy();
});

// ---- 2in1 设置提醒条 ----

test("the 2in1 warning strip names the missing step and takes it", () => {
  const setup = fakeSetup({ enabled: false, current: false });
  const actions = {
    setup: setup.client,
    openSystemKeyboardSettings: vi.fn().mockResolvedValue(undefined),
    showInputMethodPicker: vi.fn().mockResolvedValue(undefined),
  };
  const { container } = render(<SetupWarningStrip actions={actions} />);

  expect(screen.getByText("水杉输入法尚未在「系统 → 输入法」中添加")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "去添加" }));
  expect(actions.openSystemKeyboardSettings).toHaveBeenCalledOnce();

  setup.push({ enabled: true, current: false });
  expect(screen.getByText("水杉输入法还不是默认输入法")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "设为默认" }));
  expect(actions.showInputMethodPicker).toHaveBeenCalledOnce();

  setup.push({ enabled: true, current: true });
  expect(container.childElementCount).toBe(0);
});

test("the 2in1 warning strip stays away while the state is unknown", () => {
  const { container } = render(
    <SetupWarningStrip actions={{ setup: fakeSetup({ enabled: false, current: null }).client }} />,
  );
  expect(container.childElementCount).toBe(0);

  cleanup();
  const empty = render(<SetupWarningStrip actions={{ setup: fakeSetup(null).client }} />);
  expect(empty.container.childElementCount).toBe(0);

  cleanup();
  const none = render(<SetupWarningStrip />);
  expect(none.container.childElementCount).toBe(0);
});

// ---- 外壳里的触屏首页 ----

test("the HarmonyOS phone shell opens on the 设置 root with its rows", async () => {
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony" }),
        home: {
          openSystemKeyboardSettings,
          setup: fakeSetup({ enabled: false, current: false }).client,
        },
      }}
    />,
  );

  const home = await screen.findByRole("region", { name: "首页" });
  expect(within(home).getByRole("button", { name: "输入" })).toBeTruthy();
  fireEvent.click(within(statusCard()).getByRole("button", { name: "去开启" }));
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();
});

test("opening a root row from the shell lands on that page", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "harmony" }),
        home: { openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );

  const home = await screen.findByRole("region", { name: "首页" });
  fireEvent.click(within(home).getByRole("button", { name: "表达" }));
  await waitFor(() => expect(screen.queryByRole("region", { name: "首页" })).toBeNull());
});

// ---- 桌面首页：保持不变 ----

test("renders the keyboard home surface with the current skin and scheme", () => {
  render(<HomePage preferences={initial.preferences} onOpenPage={vi.fn()} />);

  expect(screen.getByRole("region", { name: "首页" })).toBeTruthy();
  expect(screen.getByText("我的键盘")).toBeTruthy();
  expect(screen.getByText("夜青 · 小鹤双拼")).toBeTruthy();
  expect(screen.getByRole("img", { name: "屏幕键盘完整布局预览" })).toBeTruthy();
  expect(screen.getByText("高情商回复")).toBeTruthy();
  // 磁贴与主题页同名；「全部设置」的副标题按导航分组概括。
  expect(screen.getByRole("button", { name: /^主题/ })).toBeTruthy();
  expect(screen.getByText("打字、外观、语音与词库")).toBeTruthy();
  // 桌面首页没有「设置」根页的那些界面。
  expect(screen.queryByRole("searchbox")).toBeNull();
  expect(screen.queryByRole("group", { name: "水杉输入法状态" })).toBeNull();
});

// 每个快捷入口都应凭自己的颜色就能认出，不必读标签，所以六个图标不能落到同一套配色上。通过容纳它们的网格来查找：图块现在用工具类设置样式，那里的类名已不再是稳定的定位手段。
test("home shortcuts expose a distinct visual tile for each function", () => {
  render(<HomePage preferences={initial.preferences} onOpenPage={vi.fn()} />);
  const grid = screen.getByRole("button", { name: /^主题/ }).parentElement!;
  const tiles = [...grid.querySelectorAll("button")];
  expect(tiles).toHaveLength(6);

  const icons = tiles.map((tile) => tile.querySelector<HTMLElement>('span[aria-hidden="true"]')!);
  expect(icons.every(Boolean)).toBe(true);
  expect(new Set(icons.map((icon) => icon.className)).size).toBe(6);
});

test("routes home shortcuts to the shared settings pages", () => {
  const onOpenPage = vi.fn();
  render(<HomePage preferences={initial.preferences} onOpenPage={onOpenPage} />);

  fireEvent.click(screen.getByRole("button", { name: /^主题/ }));
  fireEvent.click(screen.getByRole("button", { name: /输入方案/ }));
  fireEvent.click(screen.getByRole("button", { name: /按键/ }));
  // 这一行过去只跳到「外观」；现在它打开「全部设置」列表，所有没有独立标签页的页面都从那里进入。
  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  fireEvent.click(screen.getByRole("button", { name: /高情商回复/ }));

  expect(onOpenPage.mock.calls).toEqual([
    ["skin"],
    ["input"],
    ["screen-keyboard"],
    ["more"],
    ["ai"],
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
      ios
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "词库个人词与同步" }));
  fireEvent.click(screen.getByRole("button", { name: "AI回复与润色" }));
  fireEvent.click(screen.getByRole("button", { name: "系统设置启用与完全访问" }));

  expect(onOpenPage.mock.calls).toEqual([["dictionary"], ["ai"]]);
  expect(openSystemKeyboardSettings).toHaveBeenCalledOnce();
});

test("routes the keyboard card to the shared tryout when a host cannot open a window", () => {
  const onOpenPage = vi.fn();
  const onOpenChat = vi.fn();
  render(
    <HomePage preferences={initial.preferences} onOpenPage={onOpenPage} onOpenChat={onOpenChat} />,
  );

  fireEvent.click(screen.getByRole("button", { name: /试用键盘/ }));

  expect(onOpenChat).toHaveBeenCalledOnce();
  expect(onOpenPage).not.toHaveBeenCalled();
});

test("the home reply entry opens AI settings instead of selecting a scheme", () => {
  const onOpenPage = vi.fn();
  render(<HomePage preferences={initial.preferences} onOpenPage={onOpenPage} />);

  fireEvent.click(screen.getByRole("button", { name: /高情商回复/ }));

  expect(onOpenPage.mock.calls).toEqual([["ai"]]);
  expect(screen.getByText(/点键盘工具栏上的回复/)).toBeTruthy();
});

test("invokes Android keyboard and system input actions", () => {
  const actions = {
    openKeyboard: vi.fn().mockResolvedValue(undefined),
    openSystemKeyboardSettings: vi.fn().mockResolvedValue(undefined),
    showInputMethodPicker: vi.fn().mockResolvedValue(undefined),
  };
  render(<HomePage preferences={initial.preferences} actions={actions} onOpenPage={vi.fn()} />);

  // 完全访问 and 系统键盘设置 are the iOS keyboard extension's words; Android opens the system input method settings.
  expect(screen.getByRole("button", { name: "系统设置启用与设为默认" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /试用键盘/ }));
  fireEvent.click(screen.getByRole("button", { name: "系统输入法设置" }));
  fireEvent.click(screen.getByRole("button", { name: "选择输入法" }));

  expect(actions.openKeyboard).toHaveBeenCalledOnce();
  expect(actions.openSystemKeyboardSettings).toHaveBeenCalledOnce();
  expect(actions.showInputMethodPicker).toHaveBeenCalledOnce();
});

test("opens the emoji and clipboard tools from the desktop home", () => {
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

test("opens on home with home capability and falls back to the first navigation page without it", async () => {
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
  expect(screen.getByRole("button", { name: "输入" }).getAttribute("aria-current")).toBe("page");
  expect(screen.queryByRole("region", { name: "首页" })).toBeNull();
});

test("the desktop home reply entry opens AI settings and leaves the touch schemes alone", async () => {
  const save = vi.fn();
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save,
        touchKeyboardSchemes: true,
        home: {
          openKeyboard: vi.fn().mockResolvedValue(undefined),
        },
      }}
    />,
  );
  await screen.findByRole("region", { name: "首页" });
  fireEvent.click(screen.getByRole("button", { name: /高情商回复/ }));

  expect(await screen.findByText("启用 AI 辅助")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "设为当前输入方案 高情商回复" })).toBeNull();
  expect(save).not.toHaveBeenCalled();
});
