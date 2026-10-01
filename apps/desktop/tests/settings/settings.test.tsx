// @vitest-environment jsdom
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import minimizeIcon from "../../../../packages/ui/src/assets/minimize.svg";
import maximizeIcon from "../../../../packages/ui/src/assets/maximize.svg";
import restoreIcon from "../../../../packages/ui/src/assets/restore.svg";
import closeIcon from "../../../../packages/ui/src/assets/close.svg";
import keyboardCapability from "../../src-tauri/capabilities/keyboard.json";
import {
  AI_PROVIDER_OPTIONS,
  CloudCandidatesPanel,
  CloudClipboardPanel,
  CloudDictionaryApplyPanel,
  CloudDictionaryCatalogPanel,
  CloudDictionaryFilesPanel,
  CloudDictionaryPanel,
  EmojiPanel,
  HandwritingPanel,
  KeyboardPanel,
  VoicePanel,
  SettingsPage,
  aiCredentialOrigin,
  aiProviderUpdate,
  type AiAssistantPreferences,
  type CustomSkinLibraryAction,
  type FloatingToolbarPreferences,
  type HostCapabilities,
  type SavedTouchKeyboardSkin,
  type SettingsClient,
  type Snapshot,
  type TouchKeyboardSkinDesign,
  themeEntry,
} from "@msime/ui";
import {
  describeInstallerTrust,
  selectPlatformRelease,
  validateGitHubRelease,
} from "../../../../packages/ui/src/settings/update-manifest";
import { answerConfirm } from "../support/confirm";

afterEach(cleanup);

/**
 * Wait until the settings page has finished its initial load.
 *
 * Clicking a category in the sidebar before then is a race: the page is still resolving the
 * snapshot, and the selection it makes when that resolves replaces whatever was clicked. It
 * passes whenever the mocked load happens to settle first, and fails when the machine is busy
 * -- which is exactly the shape of a test that reports a product regression that is not there.
 */
async function settingsReady() {
  await settingsFormReady();
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

test("macOS voice shortcuts use native key names and space-lock semantics", async () => {
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  await screen.findByRole("switch", { name: "按住右 Option 录音" });
  expect(screen.getByRole("switch", { name: "按住右 Control+右 Option 录音" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "按住 Control+Command 录音" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "空格锁定语音" })).toBeTruthy();
  expect(screen.queryByRole("switch", { name: "Ctrl+Win 切换语音" })).toBeNull();
  expect(screen.getByText(/首次授权后请重新按键/)).toBeTruthy();
});

// The Windows host records while a modifier shortcut is held, the two-key chord takes the right Ctrl specifically, and Space locks a held recording. The labels used to read as toggles on a plain Ctrl.
test("Windows voice shortcuts describe hold-to-record, the right Ctrl chord and space lock", async () => {
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: { platform: "windows" } as HostCapabilities,
      }}
    />,
  );
  await screen.findByRole("switch", { name: "长按右 Alt 录音" });
  expect(screen.getByRole("switch", { name: "长按右 Ctrl+右 Alt 录音" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "长按 Ctrl+Win 录音" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "长按录音时按空格锁定" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "Ctrl+F9 切换语音" })).toBeTruthy();
  expect(screen.queryByRole("switch", { name: "Ctrl+右 Alt 切换语音" })).toBeNull();
  expect(screen.queryByRole("switch", { name: "右 Alt 切换语音" })).toBeNull();
  expect(screen.getByText(/按住期间按空格锁定录音/)).toBeTruthy();
});

// Both Linux hosts (IBus ClientEngine voice_hotkey, Fcitx5 FcitxEngine) record while a modifier shortcut is held and lock on Space, as Windows does. Only IBus requires the right Ctrl in the two-key chord; Fcitx5 also accepts left Ctrl+Right Alt and stops only when Right Alt or Right Ctrl is released, but the right-Ctrl label is true on both. The labels used to read 切换语音, a toggle, which only Ctrl+F9 is.
test("Linux voice shortcuts describe hold-to-record like Windows and keep Ctrl+F9 a toggle", async () => {
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: { platform: "linux", panel_windows: true } as HostCapabilities,
      }}
    />,
  );
  await screen.findByRole("switch", { name: "长按右 Alt 录音" });
  expect(screen.getByRole("switch", { name: "长按右 Ctrl+右 Alt 录音" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "长按 Ctrl+Win 录音" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "长按录音时按空格锁定" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "Ctrl+F9 切换语音" })).toBeTruthy();
  for (const toggle of [
    "右 Alt 切换语音",
    "Ctrl+右 Alt 切换语音",
    "Ctrl+Win 切换语音",
    "空格锁定语音",
  ])
    expect(screen.queryByRole("switch", { name: toggle })).toBeNull();
  expect(screen.getByText(/长按快捷键录音，松开结束/)).toBeTruthy();
  expect(screen.getByText(/没有 provider 时快捷键不会拦截编辑器输入/)).toBeTruthy();
  expect(screen.queryByText(/切换语音录音/)).toBeNull();
});

test("macOS exposes the non-activating input-mode HUD preference", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save,
        // 中英文切换提示在所有平台都放在输入页「中英文」组，macOS 也不例外。
        host: { platform: "macos", mode_switch_shortcuts: true } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  const toggle = screen.getByRole("switch", {
    name: "中英文切换提示",
  }) as HTMLInputElement;
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, expect.objectContaining({ input_mode_hud: false }));
});

test("macOS persists Wubi unique-candidate auto-commit outside shared preferences", async () => {
  const wubiInitial = {
    ...initial,
    preferences: { ...initial.preferences, scheme: "wubi" as const },
  };
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...wubiInitial,
    revision: 8,
    preferences,
  }));
  const loadWubiAutoCommit = vi.fn().mockResolvedValue(false);
  const saveWubiAutoCommit = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: async () => wubiInitial,
        save,
        host: { platform: "macos" } as HostCapabilities,
        loadMacosWubiAutoCommitUnique: loadWubiAutoCommit,
        saveMacosWubiAutoCommitUnique: saveWubiAutoCommit,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  const toggle = (await screen.findByRole("switch", {
    name: "五笔四码唯一候选自动上屏",
  })) as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  fireEvent.click(toggle);
  saveSettingsNow();
  await screen.findByText("已保存");
  // Only the native preference changed, so the shared document is left alone.
  expect(save).not.toHaveBeenCalled();
  expect(loadWubiAutoCommit).toHaveBeenCalled();
  expect(saveWubiAutoCommit).toHaveBeenCalledWith(true);
});

test("titlebar sits above the shared sidebar and content body", async () => {
  const mounted = render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        windowControl: vi.fn().mockResolvedValue(undefined),
      }}
    />,
  );
  await settingsFormReady();
  const body = mounted.container.querySelector("[data-settings-body]")!;
  expect(body.contains(screen.getByRole("navigation", { name: "设置分类" }))).toBe(true);
  expect(body.contains(screen.getByRole("main"))).toBe(true);
  expect(body.contains(screen.getByRole("banner", { name: "窗口控制" }))).toBe(false);
  expect(body.previousElementSibling).toBe(screen.getByRole("banner", { name: "窗口控制" }));
  expect(screen.getByRole("button", { name: "关闭" }).classList.contains("window-close")).toBe(
    true,
  );
  expect(
    within(screen.getByRole("banner", { name: "窗口控制" })).getByText("水杉输入法").textContent,
  ).toBe("水杉输入法");
});

test("Android fuzzy-pinyin settings preserve rules while disabled and reset explicitly", async () => {
  const save = vi.fn().mockResolvedValue(initial);
  render(<SettingsPage client={{ load: async () => initial, save, fuzzyPinyin: true }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const enabled = screen.getByRole("switch", { name: "启用模糊音" }) as HTMLInputElement;
  // 总开关关着时规则列表收起，只留总开关。
  expect(enabled.checked).toBe(false);
  expect(screen.queryByRole("checkbox", { name: "模糊音规则 z-zh" })).toBeNull();
  fireEvent.click(enabled);
  const rule = screen.getByRole("checkbox", { name: "模糊音规则 z-zh" }) as HTMLInputElement;
  expect(rule.checked).toBe(true);
  fireEvent.click(rule);
  expect(rule.checked).toBe(false);
  fireEvent.click(rule);
  expect(rule.checked).toBe(true);
  fireEvent.click(enabled);
  expect(screen.queryByRole("checkbox", { name: "模糊音规则 z-zh" })).toBeNull();
  // 关掉再打开，之前选的规则原样还在。
  fireEvent.click(enabled);
  expect(rule.checked).toBe(true);
  expect(rule.disabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "重置模糊音配置" }));
  expect((await screen.findByRole("alertdialog")).textContent).toContain("所有模糊音规则会被清空");
  await answerConfirm("confirm");
  expect(enabled.checked).toBe(false);
  expect(rule.checked).toBe(false);
});

test("Android fuzzy-pinyin first enable seeds every rule once", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(<SettingsPage client={{ load: async () => initial, save, fuzzyPinyin: true }} />);
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  const enabled = screen.getByRole("switch", { name: "启用模糊音" }) as HTMLInputElement;
  fireEvent.click(enabled);
  for (const id of [
    "z-zh",
    "c-ch",
    "s-sh",
    "n-l",
    "f-h",
    "r-l",
    "an-ang",
    "en-eng",
    "in-ing",
    "ian-iang",
    "uan-uang",
  ]) {
    expect(
      (screen.getByRole("checkbox", { name: `模糊音规则 ${id}` }) as HTMLInputElement).checked,
    ).toBe(true);
  }
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({
      fuzzy_pinyin: {
        enabled: true,
        rules: [
          "z-zh",
          "c-ch",
          "s-sh",
          "n-l",
          "f-h",
          "r-l",
          "an-ang",
          "en-eng",
          "in-ing",
          "ian-iang",
          "uan-uang",
        ],
        seeded: true,
      },
    }),
  );
});

const touchSchemeLabels = [
  "全拼 26 键",
  "全拼 9 键",
  "小鹤双拼",
  "自然码双拼",
  "微软双拼",
  "首道双拼",
  "86 五笔",
  "日语 9 键",
  "日语 26 键",
  "手写",
  "高情商回复",
  "韩语 26 键",
];
const touchSchemeIds = [
  "quanpin",
  "nine_key",
  "xiaohe",
  "ziranma",
  "microsoft",
  "shoudao",
  "wubi",
  "japanese_nine_key",
  "japanese",
  "handwriting",
  "thoughtful_reply",
  "korean",
];

test("Android touch schemes follow Apple order and stay absent on hosts without the capability", async () => {
  const enabled = render(
    <SettingsPage
      client={{ load: async () => initial, save: vi.fn(), touchKeyboardSchemes: true }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  const group = screen.getByRole("group", { name: "输入方案" });
  expect(
    within(group)
      .getAllByRole("button")
      .map((button) => button.textContent?.replace("✓", "")),
  ).toEqual(touchSchemeLabels);
  expect(within(group).getAllByRole("switch")).toHaveLength(12);
  enabled.unmount();
  render(<SettingsPage client={{ load: async () => initial, save: vi.fn() }} />);
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  expect(screen.queryByRole("switch", { name: "显示输入方案 全拼 26 键" })).toBeNull();
});

test("offline candidate gloss is host-enabled, defaults off and persists", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  const enabled = render(
    <SettingsPage client={{ load: async () => initial, save, candidateEnglishGloss: true }} />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  const toggle = screen.getByRole("switch", { name: "显示英文释义" }) as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  expect(screen.getByText(/释义来自随键盘打包的离线词库，不联网/)).toBeDefined();
  fireEvent.click(toggle);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    candidate_english_gloss: true,
  });
  enabled.unmount();
  render(<SettingsPage client={{ load: async () => initial, save: vi.fn() }} />);
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  expect(screen.queryByRole("switch", { name: "显示英文释义" })).toBeNull();
});

test("Android English suggestions default on and persist independently", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save,
        host: { platform: "android" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  const toggle = screen.getByRole("switch", { name: "英文建议" }) as HTMLInputElement;
  expect(toggle.checked).toBe(true);
  expect(screen.getByText(/英文 26 键直接输入时/)).toBeDefined();
  fireEvent.click(toggle);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, expect.objectContaining({ english_suggestions: false }));
});

test("Linux can expose the shared offline candidate gloss setting", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: { platform: "linux" } as HostCapabilities,
        candidateEnglishGloss: true,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  expect(screen.getByRole("switch", { name: "显示英文释义" })).toBeTruthy();
});

test("iOS exposes the shared offline candidate gloss setting", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save,
        openExternalUrl,
        host: { platform: "ios" } as HostCapabilities,
        candidateEnglishGloss: true,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "手写输入" }));
  expect(screen.getByText(/首次在键盘中使用手写时下载中文模型/)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "手写 SDK 隐私说明" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith("https://developers.google.com/ml-kit/terms"),
  );
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  const toggle = screen.getByRole("switch", { name: "显示英文释义" }) as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  fireEvent.click(toggle);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, expect.objectContaining({ candidate_english_gloss: true }));
});

test("Android exposes handwriting model privacy and system settings", async () => {
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        openExternalUrl,
        openSystemKeyboardSettings,
        host: { platform: "android" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "手写输入" }));
  expect(screen.getByText(/Android 键盘中切换到手写时/)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "手写 SDK 隐私说明" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith("https://developers.google.com/ml-kit/terms"),
  );
  expect(screen.getByText("Android 键盘手写")).toBeDefined();
  expect(screen.getByText(/Android 系统输入法设置中启用水杉键盘/)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "打开系统输入法设置" }));
  expect(openSystemKeyboardSettings).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("button", { name: "打开手写识别板" })).toBeNull();
});

test("mobile input settings expose the keyboard AI entry", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: { platform: "android" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  expect(screen.getByText(/切换到高情商回复键盘/)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "配置键盘 AI" }));
  expect(await screen.findByText("启用 AI 辅助")).toBeDefined();
});

test("Android touch scheme selection, fallback, last-visible guard and save payload match Apple", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(<SettingsPage client={{ load: async () => initial, save, touchKeyboardSchemes: true }} />);
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  fireEvent.click(screen.getByRole("button", { name: "设为当前输入方案 全拼 9 键" }));
  expect(
    screen.getByRole("button", { name: "设为当前输入方案 全拼 9 键" }).getAttribute("aria-pressed"),
  ).toBe("true");
  fireEvent.click(screen.getByRole("switch", { name: "显示输入方案 全拼 9 键" }));
  expect(
    screen
      .getByRole("button", { name: "设为当前输入方案 全拼 26 键" })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  for (const label of touchSchemeLabels.slice(1)) {
    const toggle = screen.getByRole("switch", {
      name: `显示输入方案 ${label}`,
    }) as HTMLInputElement;
    if (toggle.checked) fireEvent.click(toggle);
  }
  const last = screen.getByRole("switch", {
    name: "显示输入方案 全拼 26 键",
  }) as HTMLInputElement;
  expect(last.checked).toBe(true);
  expect(last.disabled).toBe(true);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({
      scheme: "quanpin",
      last_chinese_scheme: "quanpin",
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes: { enabled: ["quanpin"], selected: "quanpin" },
    }),
  );
});

test("Android selecting nine-key saves the shared selected scheme and matching engine layout", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(<SettingsPage client={{ load: async () => initial, save, touchKeyboardSchemes: true }} />);
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  fireEvent.click(screen.getByRole("button", { name: "设为当前输入方案 全拼 9 键" }));
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({
      scheme: "quanpin",
      touch_keyboard_layout: "nine_key",
      touch_keyboard_schemes: { enabled: touchSchemeIds, selected: "nine_key" },
    }),
  );
});

test("Android touch schemes display the first enabled fallback for a valid selection-less snapshot", async () => {
  const snapshot = {
    ...initial,
    preferences: { ...initial.preferences, touch_keyboard_schemes: { enabled: ["wubi" as const] } },
  };
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), touchKeyboardSchemes: true }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "输入" }));
  expect(
    screen.getByRole("button", { name: "设为当前输入方案 86 五笔" }).getAttribute("aria-pressed"),
  ).toBe("true");
  expect(
    (screen.getByRole("switch", { name: "显示输入方案 86 五笔" }) as HTMLInputElement).disabled,
  ).toBe(true);
});

test("window SVGs follow host state and retain accessible controls", async () => {
  let publish: (maximized: boolean) => void = () => {};
  const windowControl = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        windowControl,
        onWindowStateChanged: async (listener) => {
          publish = listener;
          return () => {};
        },
      }}
    />,
  );
  await settingsFormReady();
  function icon(label: string, source: string) {
    const button = screen.getByRole("button", { name: label });
    const img = button.querySelector("img")!;
    expect(img).not.toBeNull();
    expect(img.getAttribute("src")).toBe(source);
    expect(img.alt).toBe("");
    expect(img.draggable).toBe(false);
    // The glyph ships white and is inverted on a light theme; that is the contract, not a class name.
    expect(img.className).toContain("light-theme:[filter:invert(1)_brightness(0.2)]");
    expect(img.className).toContain("object-contain");
    expect(button.textContent).toBe("");
    return button;
  }
  fireEvent.click(icon("最小化", minimizeIcon));
  expect(windowControl).toHaveBeenLastCalledWith("minimize");
  fireEvent.click(icon("最大化", maximizeIcon));
  expect(windowControl).toHaveBeenLastCalledWith("maximize");
  act(() => publish(true));
  expect(screen.queryByRole("button", { name: "最大化" })).toBeNull();
  fireEvent.click(icon("还原", restoreIcon));
  expect(windowControl).toHaveBeenLastCalledWith("restore");
  act(() => publish(false));
  expect(screen.queryByRole("button", { name: "还原" })).toBeNull();
  icon("最大化", maximizeIcon);
  fireEvent.click(icon("关闭", closeIcon));
  expect(windowControl).toHaveBeenLastCalledWith("close");
});

test("resize starts on edge press, not pointer movement", async () => {
  const resizeWindow = vi.fn().mockResolvedValue(undefined);
  const mounted = render(
    <SettingsPage client={{ load: async () => initial, save: vi.fn(), resizeWindow }} />,
  );
  await settingsFormReady();
  const shell = mounted.container.querySelector("[data-settings-shell]")!;
  vi.spyOn(shell, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    right: 800,
    bottom: 600,
    width: 800,
    height: 600,
    x: 0,
    y: 0,
    toJSON() {},
  });
  fireEvent(
    shell,
    new MouseEvent("pointermove", { bubbles: true, buttons: 1, clientX: 1, clientY: 1 }),
  );
  expect(resizeWindow).not.toHaveBeenCalled();
  fireEvent(
    shell,
    new MouseEvent("pointerdown", { bubbles: true, button: 0, clientX: 799, clientY: 599 }),
  );
  expect(resizeWindow).toHaveBeenCalledWith("se");
  fireEvent(
    shell,
    new MouseEvent("pointerdown", { bubbles: true, button: 2, clientX: 1, clientY: 1 }),
  );
  expect(resizeWindow).toHaveBeenCalledTimes(1);
});

function titlebarPointer(
  target: Element,
  type: string,
  x: number,
  y: number,
  detail = 1,
  buttons = 1,
) {
  fireEvent(
    target,
    new MouseEvent(type, { bubbles: true, button: 0, buttons, clientX: x, clientY: y, detail }),
  );
}

test("titlebar drag waits for upstream two-pixel threshold and starts only once", async () => {
  const beginWindowDrag = vi.fn().mockResolvedValue(undefined);
  render(<SettingsPage client={{ load: async () => initial, save: vi.fn(), beginWindowDrag }} />);
  await settingsFormReady();
  const titlebar = screen.getByRole("banner", { name: "窗口控制" });
  titlebarPointer(titlebar, "pointerdown", 100, 16);
  expect(beginWindowDrag).not.toHaveBeenCalled();
  titlebarPointer(titlebar, "pointermove", 101, 16);
  expect(beginWindowDrag).not.toHaveBeenCalled();
  titlebarPointer(titlebar, "pointermove", 101, 17);
  expect(beginWindowDrag).toHaveBeenCalledTimes(1);
  titlebarPointer(titlebar, "pointermove", 110, 17);
  expect(beginWindowDrag).toHaveBeenCalledTimes(1);
});

test.each(["pointerup", "pointercancel", "pointerout", "blur", "released", "double-press"])(
  "%s cancels or excludes a pending titlebar drag",
  async (reason) => {
    const beginWindowDrag = vi.fn().mockResolvedValue(undefined);
    render(<SettingsPage client={{ load: async () => initial, save: vi.fn(), beginWindowDrag }} />);
    await settingsFormReady();
    const titlebar = screen.getByRole("banner", { name: "窗口控制" });
    titlebarPointer(titlebar, "pointerdown", 100, 16, reason === "double-press" ? 2 : 1);
    if (reason === "blur") fireEvent(window, new Event("blur"));
    else if (reason === "released") titlebarPointer(titlebar, "pointermove", 100, 16, 1, 0);
    else if (reason !== "double-press") titlebarPointer(titlebar, reason, 100, 16);
    titlebarPointer(titlebar, "pointermove", 110, 16);
    expect(beginWindowDrag).not.toHaveBeenCalled();
  },
);

test("resize edges do not drag or double-click maximize the titlebar", async () => {
  const beginWindowDrag = vi.fn().mockResolvedValue(undefined);
  const resizeWindow = vi.fn().mockResolvedValue(undefined);
  const windowControl = vi.fn().mockResolvedValue(undefined);
  const mounted = render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        beginWindowDrag,
        resizeWindow,
        windowControl,
      }}
    />,
  );
  await settingsFormReady();
  vi.spyOn(
    mounted.container.querySelector("[data-settings-shell]")!,
    "getBoundingClientRect",
  ).mockReturnValue({
    left: 0,
    top: 0,
    right: 800,
    bottom: 780,
    width: 800,
    height: 780,
    x: 0,
    y: 0,
    toJSON() {},
  });
  const titlebar = screen.getByRole("banner", { name: "窗口控制" });
  titlebarPointer(titlebar, "pointerdown", 100, 2);
  titlebarPointer(titlebar, "pointermove", 110, 16);
  expect(resizeWindow).toHaveBeenCalledWith("n");
  expect(beginWindowDrag).not.toHaveBeenCalled();
  fireEvent.doubleClick(titlebar, { button: 0, clientX: 100, clientY: 2 });
  fireEvent.doubleClick(titlebar, { button: 2, clientX: 100, clientY: 16 });
  expect(windowControl).not.toHaveBeenCalled();
  fireEvent.doubleClick(titlebar, { button: 0, clientX: 100, clientY: 16 });
  expect(windowControl).toHaveBeenCalledWith("maximize");
});

test.each([false, true])(
  "titlebar drag handles host failure (synchronous=%s)",
  async (synchronous) => {
    const beginWindowDrag = vi.fn(() => {
      if (synchronous) throw new Error("host unavailable");
      return Promise.reject(new Error("host unavailable"));
    });
    render(<SettingsPage client={{ load: async () => initial, save: vi.fn(), beginWindowDrag }} />);
    await settingsFormReady();
    const titlebar = screen.getByRole("banner", { name: "窗口控制" });
    titlebarPointer(titlebar, "pointerdown", 100, 16);
    titlebarPointer(titlebar, "pointermove", 110, 16);
    expect(await screen.findByText("无法移动窗口，请重试。")).toBeTruthy();
  },
);

test("window state subscription failures are handled", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        onWindowStateChanged: async () => {
          throw new Error("unavailable");
        },
      }}
    />,
  );
  expect(await screen.findByText("无法读取窗口状态，请重试。")).toBeTruthy();
});

test("window state update errors are shown and detached hosts cannot report errors", async () => {
  let reportError = () => {};
  const client: SettingsClient = {
    load: async () => initial,
    save: vi.fn(),
    onWindowStateChanged: async (_listener, onError) => {
      reportError = onError!;
      return () => {};
    },
  };
  const mounted = render(<SettingsPage client={client} />);
  await settingsFormReady();
  act(() => reportError());
  expect(screen.getByText("无法读取窗口状态，请重试。")).toBeTruthy();
  mounted.rerender(<SettingsPage client={{ load: async () => initial, save: vi.fn() }} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
  await waitFor(() => expect(screen.queryByText("无法读取窗口状态，请重试。")).toBeNull());
  act(() => reportError());
  expect(screen.queryByText("无法读取窗口状态，请重试。")).toBeNull();
});

test("late window subscriptions are disposed and old callbacks ignored", async () => {
  let publish: (value: boolean) => void = () => {};
  let finish: (cleanup: () => void) => void = () => {};
  const unsubscribe = vi.fn();
  const windowControl = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: async () => initial,
    save: vi.fn(),
    windowControl,
    onWindowStateChanged: (listener) => {
      publish = listener;
      return new Promise((resolve) => {
        finish = resolve;
      });
    },
  };
  const mounted = render(<SettingsPage client={client} />);
  await settingsFormReady();
  mounted.rerender(
    <SettingsPage client={{ load: async () => initial, save: vi.fn(), windowControl }} />,
  );
  finish(unsubscribe);
  await waitFor(() => expect(unsubscribe).toHaveBeenCalledTimes(1));
  publish(true);
  expect(screen.queryByRole("button", { name: "还原" })).toBeNull();
  expect(screen.getByRole("button", { name: "最大化" })).toBeTruthy();
});

test("drag-only hosts do not expose unavailable window controls", async () => {
  render(
    <SettingsPage
      client={{ load: async () => initial, save: vi.fn(), beginWindowDrag: vi.fn() }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "关闭" })).toBeNull();
  expect(screen.queryByRole("button", { name: "最大化" })).toBeNull();
});

test("window buttons do not bubble drag or double-click maximize", async () => {
  const windowControl = vi.fn().mockResolvedValue(undefined);
  const beginWindowDrag = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        windowControl,
        beginWindowDrag,
        onWindowStateChanged: async (listener) => {
          listener(true);
          return () => {};
        },
      }}
    />,
  );
  const restore = await screen.findByRole("button", { name: "还原" });
  fireEvent.pointerDown(restore, { button: 0 });
  fireEvent.doubleClick(restore);
  expect(beginWindowDrag).not.toHaveBeenCalled();
  expect(windowControl).not.toHaveBeenCalled();
  fireEvent.click(restore);
  expect(windowControl).toHaveBeenLastCalledWith("restore");
  fireEvent.doubleClick(screen.getByRole("banner", { name: "窗口控制" }));
  expect(windowControl).toHaveBeenLastCalledWith("restore");
});

test("mixed candidate defaults, independent switches and threshold persist", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  const english = (await screen.findByRole("switch", { name: /^中英混输/ })) as HTMLInputElement;
  const emoji = screen.getByRole("switch", { name: /^emoji 混输/ }) as HTMLInputElement;
  const kaomoji = screen.getByRole("switch", { name: /^颜文字混输/ }) as HTMLInputElement;
  const threshold = screen.getByLabelText("触发字符数") as HTMLSelectElement;
  expect(english.checked).toBe(true);
  expect(emoji.checked).toBe(false);
  expect(kaomoji.checked).toBe(false);
  expect(threshold.value).toBe("5");
  expect(threshold.options.length).toBe(8);
  fireEvent.change(threshold, { target: { value: "8" } });
  fireEvent.click(english);
  expect(threshold.disabled).toBe(true);
  expect(threshold.value).toBe("8");
  fireEvent.click(emoji);
  fireEvent.click(kaomoji);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    mixed_input: { english: false, minimum_prefix: 8, emoji: true, kaomoji: true },
  });
});

test("traditional Chinese output toggle persists", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const toggle = (await screen.findByRole("switch", {
    name: "繁体输出",
  })) as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  fireEvent.click(toggle);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    traditional_chinese_output: true,
  });
});

test("voice settings persist under the shared voice_input contract", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  const enabled = (await screen.findByRole("switch", {
    name: "启用语音输入",
  })) as HTMLInputElement;
  expect(enabled.checked).toBe(true);
  fireEvent.click(enabled);
  fireEvent.change(screen.getByRole("combobox", { name: "识别服务" }), {
    target: { value: "doubao" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "豆包鉴权方式" }), {
    target: { value: "legacy" },
  });
  fireEvent.change(screen.getByLabelText("识别语言"), { target: { value: "en-US" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    // Picking a provider now also writes that provider's endpoint and model.
    // That is the point of the change: the shipped endpoint default is Doubao's
    // websocket URL, and leaving it behind routed other providers' tokens to
    // ByteDance.
    voice_input: {
      enabled: false,
      asr_provider: "doubao",
      language: "en-US",
      doubao_auth_mode: "legacy",
      asr_resource_id: "volc.seedasr.sauc.duration",
      asr_endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
      asr_model: "",
      // Tokens are kept per provider, so switching also moves the credential
      // into the slot being left rather than carrying it to the new endpoint.
      asr_token: "",
      asr_tokens: {},
    },
  });
});

test("voice settings default to the single API Key mode", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  const client: SettingsClient = { load: vi.fn().mockResolvedValue(initial), save };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  const authMode = (await screen.findByRole("combobox", {
    name: "豆包鉴权方式",
  })) as HTMLSelectElement;
  expect(authMode.value).toBe("api_key");
  fireEvent.change(authMode, { target: { value: "legacy" } });
  fireEvent.change(authMode, { target: { value: "api_key" } });
  saveSettingsNow();
  await vi.waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls[0][1].voice_input.doubao_auth_mode).toBe("api_key");
});

test("voice capture backend choices follow the host platform", async () => {
  const options = async (platform: HostCapabilities["platform"]) => {
    const mounted = render(
      <SettingsPage
        initialPage="voice"
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host: { platform, voice_capture_devices: true } as HostCapabilities,
          listVoiceCaptureDevices: vi.fn().mockResolvedValue([]),
        }}
      />,
    );
    const select = (await screen.findByRole("combobox", { name: "录音后端" })) as HTMLSelectElement;
    const values = Array.from(select.options).map((option) => option.value);
    mounted.unmount();
    return values;
  };
  expect(await options("linux")).toEqual(["", "auto", "pulse", "pipewire", "alsa"]);
  expect(await options("macos")).toEqual(["", "auto", "macos"]);
  expect(await options("windows")).toEqual(["", "auto", "windows"]);
});

test("AI credentials stay scoped to the normalized HTTPS origin", async () => {
  expect(aiCredentialOrigin("https://Fixture.Invalid/v1/chat/completions")).toBe(
    "https://fixture.invalid:443",
  );
  expect(aiCredentialOrigin("https://fixture.invalid:444/v1/chat/completions")).toBe(
    "https://fixture.invalid:444",
  );
  expect(aiCredentialOrigin("http://fixture.invalid/v1/chat/completions")).toBeNull();
  const firstOrigin = "https://fixture.invalid:443";
  const snapshot: Snapshot = {
    ...initial,
    preferences: {
      ...initial.preferences,
      ai_assistant: {
        enabled: true,
        provider: "openai",
        model: "fixture-model",
        endpoint: "https://fixture.invalid/v1/chat/completions",
        candidate_limit: 3,
        tokens: { [firstOrigin]: "first-origin-fixture" },
        prompt_id: "polish",
        prompt: "保持原意",
        prompt_custom_1: "",
        prompt_custom_2: "",
        prompt_custom_3: "",
      },
    },
  };
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(snapshot),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...snapshot,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("button", { name: "AI 辅助" }));
  const endpoint = (await screen.findByLabelText("AI 接口地址")) as HTMLInputElement;
  const token = screen.getByLabelText("AI API Token") as HTMLInputElement;
  expect(token.value).toBe("first-origin-fixture");
  fireEvent.change(endpoint, { target: { value: "https://fixture.invalid/v2/chat/completions" } });
  expect(token.value).toBe("first-origin-fixture");
  fireEvent.change(endpoint, { target: { value: "https://other.invalid/v1/chat/completions" } });
  expect(token.value).toBe("");
  fireEvent.change(token, { target: { value: "second-origin-fixture" } });
  fireEvent.change(endpoint, { target: { value: "https://fixture.invalid/v1/chat/completions" } });
  expect(token.value).toBe("first-origin-fixture");
  saveSettingsNow();
  await screen.findByText("已保存");
  const saved = vi.mocked(client.save).mock.calls[0][1].ai_assistant!;
  expect(saved.token).toBe("");
  expect(saved.tokens).toEqual({
    [firstOrigin]: "first-origin-fixture",
    "https://other.invalid:443": "second-origin-fixture",
  });
});

test("mobile AI settings expose the Apple provider catalog and preserve custom edits", async () => {
  expect(AI_PROVIDER_OPTIONS.map((option) => option.id)).toEqual([
    "everyapi",
    "openai",
    "anthropic",
    "gemini",
    "deepseek",
    "qwen",
    "kimi",
    "zhipu",
    "siliconflow",
    "groq",
    "openrouter",
    "custom",
  ]);
  const base: AiAssistantPreferences = {
    enabled: true,
    provider: "deepseek",
    model: "deepseek-v4-flash",
    endpoint: "https://api.deepseek.com/chat/completions",
    candidate_limit: 3,
    prompt: "保持原意",
    prompt_custom_1: "",
    prompt_custom_2: "",
    prompt_custom_3: "",
  };
  expect(aiProviderUpdate("anthropic", base)).toMatchObject({
    provider: "anthropic",
    endpoint: "https://api.anthropic.com/v1/chat/completions",
    model: "claude-sonnet-4-6",
  });
  expect(
    aiProviderUpdate("openai", {
      ...base,
      endpoint: "https://private.invalid/v1/chat/completions",
      model: "private-model",
    }),
  ).toMatchObject({
    provider: "openai",
    endpoint: "https://private.invalid/v1/chat/completions",
    model: "private-model",
  });
});

test("Android AI settings fetch models and run a native-hosted polish test", async () => {
  const fetchModels = vi.fn().mockResolvedValue(["fixture-model", "fixture-fast"]);
  const testAi = vi.fn().mockResolvedValue("fixture-polished");
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        aiAssistant: { fetchModels, test: testAi },
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  fireEvent.click(await screen.findByRole("button", { name: "AI 辅助" }));
  fireEvent.change(screen.getByLabelText("AI API Token"), { target: { value: "fixture-token" } });
  fireEvent.click(screen.getByRole("button", { name: "获取模型列表" }));
  await screen.findByText("已获取 2 个可用模型。");
  fireEvent.change(screen.getByRole("combobox", { name: "已获取的 AI 模型" }), {
    target: { value: "fixture-fast" },
  });
  fireEvent.change(screen.getByLabelText("AI 测试输入"), { target: { value: "fixture input" } });
  fireEvent.click(screen.getByRole("button", { name: "发送并润色" }));
  await screen.findByText("fixture-polished");
  // `provider` travels with both calls now: the hosts whose provider service
  // holds the credential check their private configuration against it, and the
  // hosts that hold the token themselves ignore it.
  expect(fetchModels).toHaveBeenCalledWith({
    endpoint: "https://api.deepseek.com/chat/completions",
    token: "fixture-token",
    provider: "deepseek",
  });
  expect(testAi).toHaveBeenCalledWith({
    endpoint: "https://api.deepseek.com/chat/completions",
    model: "fixture-fast",
    prompt: "请润色以下文字，保持原意，只返回修改后的文字。",
    token: "fixture-token",
    text: "fixture input",
    provider: "deepseek",
  });
});

test("a provider-credential host runs the AI service controls without a token", async () => {
  // On this host the credential is in the provider service's owner-only file, so
  // the page offers no token field at all. Both service controls still have to
  // work: they reach the service through that provider. Before the capability
  // existed, the model listing was hidden by platform name and the polish test
  // refused for want of a token the page is not allowed to hold.
  const fetchModels = vi.fn().mockResolvedValue(["fixture-model", "fixture-fast"]);
  const testAi = vi.fn().mockResolvedValue("fixture-polished");
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: {
          platform: "linux",
          ai_provider_credentials: true,
        } as HostCapabilities,
        aiAssistant: { fetchModels, test: testAi },
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  fireEvent.click(await screen.findByRole("button", { name: "AI 辅助" }));
  expect(screen.queryByLabelText("AI API Token")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "获取模型列表" }));
  await screen.findByText("已获取 2 个可用模型。");
  fireEvent.change(screen.getByLabelText("AI 测试输入"), { target: { value: "fixture input" } });
  fireEvent.click(screen.getByRole("button", { name: "发送并润色" }));
  await screen.findByText("fixture-polished");
  // An empty token goes out because there is none to send; the provider ignores
  // it and authenticates from its own configuration.
  expect(fetchModels.mock.calls[0][0]).toMatchObject({ provider: "deepseek", token: "" });
  expect(testAi.mock.calls[0][0]).toMatchObject({
    provider: "deepseek",
    text: "fixture input",
    token: "",
  });
  cleanup();
});

test("AI settings explain the platform-specific keyboard surface", async () => {
  const client = {
    load: async () => initial,
    save: vi.fn(),
    host: { platform: "ios" } as HostCapabilities,
  };
  render(<SettingsPage initialPage="ai" client={client} />);
  expect(await screen.findByText("为键盘 AI 联想、回复与润色提供共享配置")).toBeTruthy();
  cleanup();
  render(
    <SettingsPage
      initialPage="ai"
      client={{ ...client, host: { platform: "android" } as HostCapabilities }}
    />,
  );
  expect(await screen.findByText("为拼音联想和 Android 选中文字润色提供共享配置")).toBeTruthy();
});

test("input parity controls persist cloud, translation and punctuation settings", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(((await screen.findByLabelText("默认中英文")) as HTMLSelectElement).value).toBe("chinese");
  // Anchored: the emoji and kaomoji toggles mention 云候选 in their own descriptions.
  fireEvent.click(await screen.findByRole("switch", { name: /^云候选/ }));
  // Translation and punctuation are on the 标点与翻译 page; both pages edit one draft.
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("switch", { name: /候选词翻译/ }));
  fireEvent.change(screen.getByLabelText("候选词翻译目标语言"), { target: { value: "ja" } });
  // Anchored too: 重复标点转中文 names 智能标点 in its description, because the reference's wording
  // for it states the precondition rather than leaving the pair's relationship to be guessed.
  fireEvent.click(screen.getByRole("switch", { name: /^智能标点/ }));
  fireEvent.change(screen.getByLabelText("固定标点"), { target: { value: "english" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    cloud_candidates: false,
    candidate_translations: false,
    translation_target_language: "ja",
    smart_punctuation: true,
    punctuation_lock: "english",
  });
});

test("Android candidate translations persist an optional second language", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save,
        host: { platform: "android" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  const secondary = screen.getByRole("combobox", {
    name: "候选词翻译第二种语言",
  }) as HTMLSelectElement;
  expect(secondary.value).toBe("");
  fireEvent.change(secondary, { target: { value: "ja" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({
      translation_secondary_language: "ja",
    }),
  );
});

test("macOS candidate translations expose the shared second language", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save,
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  const secondary = screen.getByRole("combobox", {
    name: "候选词翻译第二种语言",
  }) as HTMLSelectElement;
  expect(secondary.value).toBe("");
  expect([...secondary.options].map((option) => option.value)).toEqual([
    "",
    "en",
    "fr",
    "ja",
    "es",
    "ru",
    "de",
    "ko",
  ]);
  fireEvent.change(secondary, { target: { value: "ko" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({
      translation_secondary_language: "ko",
    }),
  );
});

test("mobile translation languages stay editable for offline English glosses", async () => {
  const snapshot: Snapshot = {
    ...initial,
    preferences: {
      ...initial.preferences,
      candidate_translations: false,
      candidate_english_gloss: true,
    },
  };
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: { platform: "android" } as HostCapabilities,
        candidateEnglishGloss: true,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  const primary = screen.getByRole("combobox", { name: "候选词翻译目标语言" }) as HTMLSelectElement;
  const secondary = screen.getByRole("combobox", {
    name: "候选词翻译第二种语言",
  }) as HTMLSelectElement;
  expect([...primary.options].map((option) => option.value)).not.toContain("ru");
  expect([...secondary.options].map((option) => option.value)).not.toContain("ru");
  expect(primary.disabled).toBe(false);
  expect(secondary.disabled).toBe(false);
  fireEvent.click(screen.getByRole("switch", { name: "显示英文释义" }));
  expect(primary.disabled).toBe(true);
  expect(secondary.disabled).toBe(true);
});

test("mobile preserves legacy Russian gloss values without leaking them to new pages", async () => {
  const legacy: Snapshot = {
    ...initial,
    preferences: {
      ...initial.preferences,
      translation_target_language: "ru",
      translation_secondary_language: "ru",
    },
  };
  const client = {
    load: async () => legacy,
    save: vi.fn(),
    host: { platform: "ios" } as HostCapabilities,
  };
  const first = render(<SettingsPage client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  expect(
    (screen.getByRole("combobox", { name: "候选词翻译目标语言" }) as HTMLSelectElement)
      .selectedOptions[0].textContent,
  ).toContain("已保存");
  expect(
    (screen.getByRole("combobox", { name: "候选词翻译第二种语言" }) as HTMLSelectElement)
      .selectedOptions[0].textContent,
  ).toContain("已保存");
  first.unmount();
  render(<SettingsPage client={{ ...client, load: async () => initial }} />);
  fireEvent.click(await screen.findByRole("button", { name: "标点与翻译" }));
  expect(
    [
      ...(screen.getByRole("combobox", { name: "候选词翻译目标语言" }) as HTMLSelectElement)
        .options,
    ].map((option) => option.value),
  ).not.toContain("ru");
});

test("frequency values above the upstream dropdown range remain visible", async () => {
  const snapshot: Snapshot = {
    ...initial,
    preferences: {
      ...initial.preferences,
      frequency: { mode: "halve", trigger_count: 10, linear_step: 7 },
    },
  };
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(snapshot),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...snapshot,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const trigger = await screen.findByRole("combobox", { name: "触发频次(第几次上屏触发)" });
  expect(trigger.textContent).toContain("10");
  expect(screen.getByRole("combobox", { name: "线性调频步长" }).textContent).toContain("7");
  fireEvent.change(screen.getByRole("combobox", { name: "调频方式" }), {
    target: { value: "pin" },
  });
  fireEvent.click(screen.getByRole("option", { name: "一次置顶" }));
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...snapshot.preferences,
    frequency: { mode: "pin", trigger_count: 10, linear_step: 7 },
  });
});

test("frequency modes, threshold and step persist independently", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const mode = await screen.findByRole("combobox", { name: "调频方式" });
  expect(mode.textContent).toContain("一次置前");
  fireEvent.change(mode, { target: { value: "linear" } });
  fireEvent.change(screen.getByRole("combobox", { name: "触发频次(第几次上屏触发)" }), {
    target: { value: "3" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "线性调频步长" }), {
    target: { value: "2" },
  });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    frequency: { mode: "linear", trigger_count: 3, linear_step: 2 },
  });
});

test("frequency settings remain editable independently of learning and mode", async () => {
  const client: SettingsClient = { load: vi.fn().mockResolvedValue(initial), save: vi.fn() };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const learning = (await screen.findByRole("switch", {
    name: /学习选词习惯/,
  })) as HTMLInputElement;
  const mode = screen.getByRole("combobox", { name: "调频方式" });
  const trigger = screen.getByRole("combobox", { name: "触发频次(第几次上屏触发)" });
  const step = screen.getByRole("combobox", { name: "线性调频步长" });
  fireEvent.change(mode, { target: { value: "linear" } });
  fireEvent.click(learning);
  expect(mode).toBeDefined();
  expect(trigger).toBeDefined();
  expect(step).toBeDefined();
  fireEvent.change(mode, { target: { value: "disabled" } });
  expect(mode.textContent).toContain("关闭");
});

test("word-to-character and paging disable each other while preserving the chosen keys", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const word = (await screen.findByRole("switch", { name: /以词定字/ })) as HTMLInputElement;
  const minus = screen.getByRole("radio", { name: "- / =" }) as HTMLInputElement;
  expect(word.checked).toBe(true);
  expect(minus.disabled).toBe(true);
  fireEvent.click(word);
  // 翻页方式和以词定字同在输入页「选词与翻页」组，在一处勾选会同屏改掉另一处。
  fireEvent.click(screen.getByRole("checkbox", { name: "[ / ]" }));
  expect(word.checked).toBe(false);
  fireEvent.click(word);
  expect((screen.getByRole("checkbox", { name: "[ / ]" }) as HTMLInputElement).checked).toBe(false);
  fireEvent.click(screen.getByRole("checkbox", { name: "- / =" }));
  expect(minus.disabled).toBe(false);
  fireEvent.click(minus);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    word_character: { enabled: true, keys: "minus_equal" },
    navigation: {
      minus_equal: false,
      comma_period: true,
      brackets: false,
      tab: true,
      page_up_down: true,
      arrows: true,
    },
  });
});

test("paging defaults match Windows and individual edits persist", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const brackets = (await screen.findByRole("checkbox", { name: "[ / ]" })) as HTMLInputElement;
  expect(brackets.checked).toBe(false);
  for (const name of [
    "- / =",
    ", / .",
    "Shift+Tab / Tab",
    "PageUp / PageDown",
    "上 / 下（移动候选项）",
  ]) {
    expect((screen.getByRole("checkbox", { name }) as HTMLInputElement).checked).toBe(true);
  }
  fireEvent.click(brackets);
  fireEvent.click(screen.getByRole("checkbox", { name: "Shift+Tab / Tab" }));
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    word_character: { enabled: false, keys: "brackets" },
    navigation: {
      minus_equal: true,
      comma_period: true,
      brackets: true,
      tab: false,
      page_up_down: true,
      arrows: true,
    },
  });
});

test("candidate-panel mouse-wheel paging is opt-in and persists", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const wheel = (await screen.findByRole("checkbox", {
    name: "鼠标滚轮（候选窗口支持时翻页）",
  })) as HTMLInputElement;
  expect(wheel.checked).toBe(false);
  fireEvent.click(wheel);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({
      navigation: expect.objectContaining({ mouse_wheel: true }),
    }),
  );
});

// On Linux the switch reaches the IBus panel's wheel directly but Fcitx5 classic UI only through its own desktop-wide WheelForPaging option (platforms/linux/README.md), so Linux explains both; other hosts do not get the note.
test("Linux explains what the mouse-wheel paging switch does on IBus and Fcitx5", async () => {
  for (const [platform, shown] of [
    ["linux", true],
    ["windows", false],
  ] as const) {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host: { platform } as HostCapabilities,
        }}
      />,
    );
    await settingsReady();
    fireEvent.click(screen.getByRole("button", { name: "输入" }));
    await screen.findByRole("checkbox", { name: "鼠标滚轮（候选窗口支持时翻页）" });
    const note = screen.queryByText(/在 IBus 候选窗口上滚动即翻页/);
    if (shown) {
      expect(note?.textContent).toContain("关闭时滚轮不做任何事");
      expect(note?.textContent).toContain("对 Fcitx5 中的所有输入法生效");
    } else {
      expect(note).toBeNull();
    }
    cleanup();
  }
});
test("helpcode schemes save independently and retain disabled selections", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const quanpin = (await screen.findByRole("combobox", {
    name: "全拼辅助码方案",
  })) as HTMLSelectElement;
  const shuangpin = screen.getByRole("combobox", { name: "双拼辅助码方案" }) as HTMLSelectElement;
  const displays = [
    screen.getByRole("switch", { name: "在候选窗口中显示双拼辅助码" }),
    screen.getByRole("switch", { name: "在候选窗口中显示全拼辅助码" }),
  ] as HTMLInputElement[];
  expect(quanpin.value).toBe("ziranma");
  expect(shuangpin.value).toBe("lantian");
  expect(displays.map((display) => display.checked)).toEqual([true, false]);
  fireEvent.change(quanpin, { target: { value: "xiaohe" } });
  fireEvent.click(screen.getByRole("switch", { name: "全拼辅助码" }));
  expect(quanpin.disabled).toBe(true);
  expect(quanpin.textContent).toContain("小鹤");
  fireEvent.change(shuangpin, { target: { value: "shouyou2_0" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    quanpin_helpcode: { enabled: false, schema: "xiaohe", show_in_candidate_window: false },
    shuangpin_helpcode: { enabled: true, schema: "shouyou2_0", show_in_candidate_window: true },
  });
});

test("shortcut page reflects enabled navigation shortcuts", async () => {
  render(<SettingsPage client={{ load: vi.fn().mockResolvedValue(initial), save: vi.fn() }} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(await screen.findByText("候选操作")).toBeDefined();
  expect(screen.getAllByText("- / =").length).toBeGreaterThan(0);
  // 开着的几组翻页键合在同一行里，不再各占一行同名的「向前 / 向后翻页」。
  expect(screen.getAllByText("向前 / 向后翻页")).toHaveLength(1);
  // 以词定字默认开着，用 [ / ] 上屏首字和末字。
  expect(screen.getByText("以词定字（上屏首字 / 末字）")).toBeDefined();
  expect(screen.getByText("↑ / ↓")).toBeDefined();
  expect(screen.getByText("Home / End")).toBeDefined();
  // Home/End move across the whole candidate list, as on Windows, not within the current page.
  expect(screen.getByText("移动到候选列表首项 / 末项（页码随之切换）")).toBeDefined();
  expect(screen.getByText("Ctrl+Shift+Alt+C")).toBeDefined();
});

function candidateThemeNote() {
  return screen.getByText(/^预览跟随颜色模式/).textContent;
}

test("Linux appearance and maintenance copy names both hosts and the Fcitx5 reload", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        restartInputMethod: vi.fn().mockResolvedValue(undefined),
        host: { platform: "linux", panel_windows: true, restart_input_method: true } as never,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  expect(await screen.findByRole("combobox", { name: "候选窗口主题" })).toBeDefined();
  expect(candidateThemeNote()).toBe(
    "预览跟随颜色模式；IBus 候选窗口与 Fcitx5 经典界面按此明暗着色",
  );
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(
    await screen.findByText("在当前 IBus 或 Fcitx5 输入上下文中维护候选与重启服务"),
  ).toBeDefined();
  // Fcitx5 hosts MSIME in process, so the Fcitx5 half must read as a plugin reset that spares other input methods, not a restart.
  // The chord sits in the row's control slot; the row itself carries the title that explains it.
  const restartRow =
    screen.getByText("Ctrl+Shift+Alt+R").parentElement?.parentElement?.textContent ?? "";
  expect(restartRow).toContain("IBus 执行 ibus restart");
  expect(restartRow).toContain("Fcitx5 重置水杉插件，不影响其他输入法");
  // 重启输入法服务在「维护与诊断」页。
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  const service = screen.getByRole("region", { name: "输入法服务" }).textContent ?? "";
  expect(service).toContain("重启 IBus 输入法服务");
  expect(service).toContain("使用 Fcitx5 时重载水杉插件");
});

test.each(["windows", "macos"] as const)(
  "%s candidate theme note does not mention the Linux hosts",
  async (platform) => {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host: { platform, panel_windows: true } as never,
        }}
      />,
    );
    await settingsReady();
    fireEvent.click(screen.getByRole("button", { name: "主题" }));
    expect(await screen.findByRole("combobox", { name: "候选窗口主题" })).toBeDefined();
    expect(candidateThemeNote()).toBe("预览跟随颜色模式");
  },
);

test("macOS maintenance shortcuts use the current input context and Option", async () => {
  const restartInputMethod = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        restartInputMethod,
        host: {
          platform: "macos",
          restart_input_method: true,
          panel_windows: true,
          mode_switch_shortcuts: true,
          panel_shortcuts: true,
        } as never,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(await screen.findByText("输入上下文维护快捷键")).toBeDefined();
  expect(
    screen.getByText("仅在水杉输入法当前输入上下文生效；Option 对应 Windows 基线中的 Alt。"),
  ).toBeDefined();
  for (const key of ["1–8", "C", "R", "T"])
    expect(screen.getByText(`Ctrl+Shift+Option+${key}`)).toBeDefined();
  expect(screen.queryByText("Ctrl+Shift+Alt+C")).toBeNull();
  expect(screen.getByText("重新注册并重启当前输入法")).toBeDefined();
  expect(screen.getByText("立即退出当前输入法进程")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  fireEvent.click(screen.getByRole("button", { name: "重新注册" }));
  await waitFor(() => expect(restartInputMethod).toHaveBeenCalledOnce());
  expect(await screen.findByText("已重新注册输入源。")).toBeDefined();
});

test("Linux restart copy covers both input method frameworks", async () => {
  // The page cannot tell whether IBus or Fcitx5 is running, so the copy has to be true for both: IBus restarts its service, Fcitx5 resets the MSIME addon in process.
  const restartInputMethod = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        restartInputMethod,
        host: { platform: "linux", restart_input_method: true } as never,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  expect(
    await screen.findByText(
      "重启 IBus 输入法服务；使用 Fcitx5 时重载水杉插件，关闭并重建所有输入会话，不影响其他输入法。",
    ),
  ).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "重启" }));
  await waitFor(() => expect(restartInputMethod).toHaveBeenCalledOnce());
  expect(await screen.findByText("已请求重启输入法服务。")).toBeDefined();
});

test("macOS service page exposes installation separately from re-registration", async () => {
  const installInputSource = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        restartInputMethod: vi.fn().mockResolvedValue(undefined),
        installInputSource,
        host: { platform: "macos", restart_input_method: true, panel_windows: true } as never,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  expect(await screen.findByText("安装或更新水杉输入源")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "安装 / 更新" }));
  await waitFor(() => expect(installInputSource).toHaveBeenCalledOnce());
  expect(await screen.findByText("输入源已安装并注册。")).toBeDefined();
});

test("macOS reports the start-time input method refresh and a source that still needs enabling", async () => {
  const openSettings = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        inputSourceStartup: {
          status: vi.fn().mockResolvedValue({
            action: "updated",
            enabled: false,
            bundled_version: "0.51.0 (7300)",
            installed_version: "0.51.0 (7300)",
          }),
          openSettings,
        },
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  const banner = await screen.findByRole("status", { name: "水杉输入法安装状态" });
  expect(within(banner).getByText("把水杉输入法加入输入法列表")).toBeDefined();
  expect(
    within(banner).getByText(/^已更新到 0\.51\.0 \(7300\)。macOS 只允许你自己把输入法加入列表/),
  ).toBeDefined();
  expect(
    within(banner).getByText(/在左侧选「简体中文」，再选「水杉输入法」，然后点「添加」/),
  ).toBeDefined();
  expect(within(banner).getByText(/系统对所有第三方输入法都会显示的标准提示/)).toBeDefined();
  // macOS 27 does not let a process enable the source, so nothing offers to do it for the user.
  expect(within(banner).queryByRole("button", { name: /启用/ })).toBeNull();
  fireEvent.click(within(banner).getByRole("button", { name: "打开键盘设置" }));
  await waitFor(() => expect(openSettings).toHaveBeenCalledOnce());
  fireEvent.click(within(banner).getByRole("button", { name: "知道了" }));
  expect(screen.queryByRole("status", { name: "水杉输入法安装状态" })).toBeNull();
});

test("macOS names a system-wide copy of the input method even when everything else is current", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        inputSourceStartup: {
          status: vi.fn().mockResolvedValue({
            action: "up_to_date",
            enabled: true,
            bundled_version: "0.50.0 (1)",
            installed_version: "0.50.0 (1)",
            system_bundles: ["/Library/Input Methods/水杉输入法.app"],
          }),
          openSettings: vi.fn(),
        },
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  const banner = await screen.findByRole("status", { name: "水杉输入法安装状态" });
  expect(within(banner).getByText(/\/Library\/Input Methods\/水杉输入法\.app/)).toBeDefined();
  expect(within(banner).queryByRole("button", { name: "打开键盘设置" })).toBeNull();
});

test("macOS stays quiet when the input method is current and enabled, and points to the manual button on failure", async () => {
  const quiet = vi.fn().mockResolvedValue({
    action: "up_to_date",
    enabled: true,
    bundled_version: "0.50.0 (1)",
    installed_version: "0.50.0 (2)",
  });
  const { unmount } = render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        inputSourceStartup: { status: quiet, openSettings: vi.fn() },
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  await waitFor(() => expect(quiet).toHaveBeenCalledOnce());
  expect(screen.queryByLabelText("水杉输入法安装状态")).toBeNull();
  unmount();

  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        inputSourceStartup: {
          status: vi.fn().mockResolvedValue({
            action: "failed",
            enabled: null,
            bundled_version: "0.50.0 (1)",
            installed_version: null,
          }),
          openSettings: vi.fn(),
        },
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  const banner = await screen.findByRole("alert", { name: "水杉输入法安装状态" });
  expect(within(banner).getByText(/点「安装 \/ 更新」重试/)).toBeDefined();
  expect(within(banner).queryByRole("button", { name: "打开键盘设置" })).toBeNull();
});

test("macOS asks for a new login when a first install waits for the input source list", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        inputSourceStartup: {
          status: vi.fn().mockResolvedValue({
            action: "login_required",
            enabled: false,
            bundled_version: "0.50.0 (1)",
            installed_version: "0.50.0 (1)",
          }),
          openSettings: vi.fn(),
        },
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  const banner = await screen.findByRole("status", { name: "水杉输入法安装状态" });
  expect(within(banner).getByText(/请注销并重新登录/)).toBeDefined();
  expect(within(banner).queryByRole("button", { name: "打开键盘设置" })).toBeNull();
});

test("macOS keeps reading the input source list while the notice waits for the user", async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  try {
    const status = vi
      .fn()
      .mockResolvedValueOnce({
        action: "up_to_date",
        enabled: false,
        bundled_version: "0.51.0 (7300)",
        installed_version: "0.51.0 (7300)",
      })
      .mockResolvedValue({
        action: "up_to_date",
        enabled: true,
        bundled_version: "0.51.0 (7300)",
        installed_version: "0.51.0 (7300)",
      });
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          inputSourceStartup: { status, openSettings: vi.fn() },
          host: { platform: "macos" } as HostCapabilities,
        }}
      />,
    );
    await screen.findByRole("status", { name: "水杉输入法安装状态" });
    // System Settings sits beside the window, so no focus event arrives; the interval still notices the added source.
    await vi.advanceTimersByTimeAsync(3000);
    await waitFor(() => expect(screen.queryByLabelText("水杉输入法安装状态")).toBeNull());
    expect(status).toHaveBeenCalledTimes(2);
    // Nothing is left to wait for, so the reads stop.
    await vi.advanceTimersByTimeAsync(9000);
    expect(status).toHaveBeenCalledTimes(2);
  } finally {
    vi.useRealTimers();
  }
});

test("macOS reads the input source again when the window regains focus, but not after a dismissal", async () => {
  const status = vi
    .fn()
    .mockResolvedValueOnce({
      action: "up_to_date",
      enabled: false,
      bundled_version: "0.51.0 (7300)",
      installed_version: "0.51.0 (7300)",
    })
    .mockResolvedValue({
      action: "up_to_date",
      enabled: true,
      bundled_version: "0.51.0 (7300)",
      installed_version: "0.51.0 (7300)",
    });
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        inputSourceStartup: { status, openSettings: vi.fn() },
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  await screen.findByRole("status", { name: "水杉输入法安装状态" });
  // The user added the source in System Settings and came back.
  fireEvent.focus(window);
  await waitFor(() => expect(screen.queryByLabelText("水杉输入法安装状态")).toBeNull());
  expect(status).toHaveBeenCalledTimes(2);
});

test("macOS keeps a dismissed input source notice hidden on later focus", async () => {
  const status = vi.fn().mockResolvedValue({
    action: "up_to_date",
    enabled: false,
    bundled_version: "0.51.0 (7300)",
    installed_version: "0.51.0 (7300)",
  });
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        inputSourceStartup: { status, openSettings: vi.fn() },
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  const banner = await screen.findByRole("status", { name: "水杉输入法安装状态" });
  fireEvent.click(within(banner).getByRole("button", { name: "知道了" }));
  fireEvent.focus(window);
  await settingsReady();
  expect(screen.queryByLabelText("水杉输入法安装状态")).toBeNull();
  expect(status).toHaveBeenCalledOnce();
});

test("the start-time input method report is macOS only", async () => {
  const status = vi.fn().mockResolvedValue({
    action: "installed",
    enabled: false,
    bundled_version: "0.50.0 (1)",
    installed_version: "0.50.0 (1)",
  });
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        inputSourceStartup: { status, openSettings: vi.fn() },
        host: { platform: "windows" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  expect(status).not.toHaveBeenCalled();
  expect(screen.queryByLabelText("水杉输入法安装状态")).toBeNull();
});

test("macOS about page exposes reversible uninstall with explicit data removal", async () => {
  const uninstallInputSource = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        restartInputMethod: vi.fn().mockResolvedValue(undefined),
        uninstallInputSource,
        host: { platform: "macos", restart_input_method: true, panel_windows: true } as never,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(await screen.findByText("卸载水杉输入法")).toBeDefined();
  expect(screen.getByText("© 2026 Metasequoia IME")).toBeDefined();
  const remove = screen.getByRole("checkbox", { name: /同时删除词库/ }) as HTMLInputElement;
  expect(remove.checked).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "卸载…" }));
  expect(uninstallInputSource).not.toHaveBeenCalled();
  expect(await screen.findByRole("alertdialog", { name: "确认卸载水杉输入法" })).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(uninstallInputSource).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "卸载…" }));
  fireEvent.click(screen.getByRole("button", { name: "确认卸载" }));
  await waitFor(() => expect(uninstallInputSource).toHaveBeenCalledWith(false));
  expect(await screen.findByText("输入法已移到废纸篓。")).toBeDefined();
  fireEvent.click(remove);
  fireEvent.click(screen.getByRole("button", { name: "卸载…" }));
  fireEvent.click(screen.getByRole("button", { name: "确认卸载" }));
  await waitFor(() => expect(uninstallInputSource).toHaveBeenCalledWith(true));
});

test("macOS developer page moves the shared data root only after an explicit confirmation", async () => {
  const status = vi.fn().mockResolvedValue({ path: "/synthetic/default-state", isDefault: true });
  const pick = vi.fn().mockResolvedValue("/synthetic/second-volume/MetasequoiaIME");
  const move = vi.fn().mockResolvedValue({
    path: "/synthetic/second-volume/MetasequoiaIME",
    isDefault: false,
    retainedOldData: false,
  });
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        dataDirectory: { status, pick, move },
        host: { platform: "macos", panel_windows: true } as never,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  expect(await screen.findByText("/synthetic/default-state")).toBeDefined();
  expect(screen.getByText("（默认）")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "选择位置…" }));
  expect(await screen.findByText(/词库、学习记录、皮肤、剪贴板历史和设置将移动到/)).toBeDefined();
  expect(move).not.toHaveBeenCalled();
  await answerConfirm("confirm");
  await waitFor(() => expect(move).toHaveBeenCalledWith());
  expect(await screen.findByText(/数据已移动。设置窗口即将关闭/)).toBeDefined();
});

test("Linux developer page moves the data root and says the fixed configuration stays behind", async () => {
  const status = vi
    .fn()
    .mockResolvedValue({ path: "/synthetic/home/.config/msime-client", isDefault: true });
  const pick = vi
    .fn()
    .mockResolvedValueOnce("/synthetic/data/msime")
    .mockRejectedValueOnce({ code: "data_directory_picker_unavailable" });
  const move = vi.fn().mockResolvedValue({
    path: "/synthetic/data/msime",
    isDefault: false,
    retainedOldData: false,
  });
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        dataDirectory: { status, pick, move },
        host: { platform: "linux", panel_windows: true } as never,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  expect(await screen.findByText("/synthetic/home/.config/msime-client")).toBeDefined();
  expect(screen.getByRole("group", { name: "数据目录" }).textContent).toContain(
    "凭据固定保存在 ~/.config/msime-client",
  );
  fireEvent.click(screen.getByRole("button", { name: "选择位置…" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(move).toHaveBeenCalledWith());
  expect(await screen.findByText(/数据已移动。设置窗口即将关闭/)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "选择位置…" }));
  expect(await screen.findByText(/请安装 zenity 或 kdialog/)).toBeDefined();
  expect(move).toHaveBeenCalledTimes(1);
});

test("Linux data move reports busy input sessions and a restart it could not do", async () => {
  const status = vi
    .fn()
    .mockResolvedValue({ path: "/synthetic/home/.config/msime-client", isDefault: true });
  const pick = vi.fn().mockResolvedValue("/synthetic/data/msime");
  const move = vi
    .fn()
    .mockRejectedValueOnce({ code: "data_directory_busy" })
    .mockResolvedValueOnce({
      path: "/synthetic/data/msime",
      isDefault: false,
      retainedOldData: false,
      inputMethodRestarted: false,
    });
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        dataDirectory: { status, pick, move },
        host: { platform: "linux", panel_windows: true } as never,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  expect(await screen.findByText("/synthetic/home/.config/msime-client")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "选择位置…" }));
  await answerConfirm("confirm");
  expect(await screen.findByText("输入法仍在使用数据目录，请稍后重试。")).toBeDefined();
  expect(screen.getByText("/synthetic/home/.config/msime-client")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "选择位置…" }));
  await answerConfirm("confirm");
  expect(
    await screen.findByText(
      "数据已移动。输入法未能自动重启，请手动重启输入法后再继续输入。设置窗口即将关闭，请重新打开后继续使用。",
    ),
  ).toBeDefined();
  expect(move).toHaveBeenCalledTimes(2);
});

test("shortcut page reflects enabled candidate mouse-wheel paging", async () => {
  const preferences = {
    ...initial.preferences,
    navigation: {
      minus_equal: true,
      comma_period: true,
      brackets: false,
      tab: true,
      page_up_down: true,
      mouse_wheel: true,
      arrows: true,
    },
  };
  render(
    <SettingsPage
      client={{ load: vi.fn().mockResolvedValue({ ...initial, preferences }), save: vi.fn() }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(await screen.findByText("鼠标滚轮")).toBeDefined();
});

test("utility mode switches preserve defaults and drafts across pages", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const unicode = (await screen.findByRole("switch", {
    name: /^Unicode 便捷录入/,
  })) as HTMLInputElement;
  expect(unicode.checked).toBe(true);
  fireEvent.click(unicode);
  fireEvent.click(screen.getByRole("button", { name: "标点与翻译" }));
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(unicode.checked).toBe(false);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    local_modes: {
      unicode: false,
      date_time: true,
      quick_phrase: true,
      emoji: true,
      kaomoji: true,
      super_jianpin: true,
      temporary_english: true,
      temporary_japanese: true,
    },
  });
});

test("macOS offers every local mode, because every catalog ships", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save,
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(await screen.findByRole("switch", { name: /^快捷短语/ })).toBeDefined();
  expect(screen.getByRole("switch", { name: /^日期与时间/ })).toBeDefined();
  expect(screen.getByRole("switch", { name: /^Unicode/ })).toBeDefined();
  expect(screen.getByRole("switch", { name: /^超级简拼/ })).toBeDefined();
  expect(screen.getByRole("switch", { name: /^临时英文/ })).toBeDefined();
  // others.db and dict_japanese.dat are in the pinned resource set the macOS app bundles, so these three
  // work and hiding their switches only hid working features. Temporary English, gated the same way on
  // english.db, was never hidden.
  expect(screen.getByRole("switch", { name: /^Emoji/ })).toBeDefined();
  expect(screen.getByRole("switch", { name: /^颜文字/ })).toBeDefined();
  expect(screen.getByRole("switch", { name: /^临时日语/ })).toBeDefined();
  fireEvent.click(screen.getByRole("switch", { name: /^Unicode/ }));
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    local_modes: {
      unicode: false,
      date_time: true,
      quick_phrase: true,
      emoji: true,
      kaomoji: true,
      super_jianpin: true,
      temporary_english: true,
      temporary_japanese: true,
    },
  });
});

test("clipboard history defaults off, clears when disabled, and saves independently", async () => {
  const clear = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
    clipboard: { clear },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "剪贴板" }));
  const clipboard = (await screen.findByRole("switch", {
    name: "剪贴板管理",
  })) as HTMLInputElement;
  expect(clipboard.checked).toBe(false);
  fireEvent.click(clipboard);
  expect(clipboard.checked).toBe(true);
  fireEvent.click(clipboard);
  expect(clear).toHaveBeenCalledTimes(1);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, { ...initial.preferences, clipboard_history: false });
});

test("clipboard history exposes timestamps, pinning, deletion and two-step clearing", async () => {
  let entries = [
    { text: "synthetic pinned", timestampMs: 1_789_000_000_000, pinned: true },
    { text: "synthetic recent", timestampMs: 1_788_000_000_000, pinned: false },
  ];
  const list = vi.fn().mockImplementation(async () => entries);
  const setPinned = vi.fn().mockImplementation(async (text: string, pinned: boolean) => {
    entries = entries.map((entry) => (entry.text === text ? { ...entry, pinned } : entry));
  });
  const remove = vi.fn().mockImplementation(async (text: string) => {
    entries = entries.filter((entry) => entry.text !== text);
  });
  const clear = vi.fn().mockImplementation(async () => {
    entries = [];
  });
  const snapshot = { ...initial, preferences: { ...initial.preferences, clipboard_history: true } };
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(snapshot),
    save: vi.fn(),
    clipboard: { list, setPinned, remove, clear },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "剪贴板" }));
  expect(await screen.findByText("synthetic pinned")).toBeDefined();
  expect(screen.getByText(/已固定/)).toBeDefined();

  fireEvent.click(screen.getByRole("button", { name: "固定剪贴板记录" }));
  await waitFor(() => expect(setPinned).toHaveBeenCalledWith("synthetic recent", true));
  expect(await screen.findAllByRole("button", { name: "取消固定剪贴板记录" })).toHaveLength(2);

  const recentRow = screen
    .getByText("synthetic recent")
    .closest("[data-clipboard-entry-row]") as HTMLElement;
  fireEvent.click(within(recentRow).getByRole("button", { name: "删除剪贴板记录" }));
  await waitFor(() => expect(remove).toHaveBeenCalledWith("synthetic recent"));
  expect(screen.queryByText("synthetic recent")).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "清空历史" }));
  expect(clear).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "确认清空" }));
  await waitFor(() => expect(clear).toHaveBeenCalledTimes(1));
  expect(await screen.findByText("暂无历史记录")).toBeDefined();
});

test("iOS clipboard history follows keyboard permission instead of the desktop preference", async () => {
  const list = vi
    .fn()
    .mockResolvedValue([
      { text: "synthetic mobile", timestampMs: 1_789_000_000_000, pinned: false },
    ]);
  const sync = vi.fn();
  const clear = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    host: { platform: "ios" } as HostCapabilities,
    clipboard: { clear, list, sync },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "剪贴板" }));
  expect(await screen.findByText("synthetic mobile")).toBeDefined();
  expect(screen.queryByRole("switch", { name: "剪贴板管理" })).toBeNull();
  expect(screen.queryByRole("button", { name: "从系统剪贴板同步" })).toBeNull();
  expect(screen.getAllByText(/允许完全访问/).length).toBeGreaterThan(0);
});

test("dictionary manager queries, edits and removes Engine entries", async () => {
  const quick = { kind: "quick_phrase" as const, key: "x", value: "fixture", weight: 100000 };
  const list = vi.fn().mockResolvedValue({
    entries: [quick, { kind: "pinyin" as const, key: "ni", value: "你好", weight: 100 }],
    has_more: false,
  });
  const edit = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    dictionary: { list, edit },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "查询" }));
  expect(await screen.findByText("fixture")).toBeDefined();
  expect(
    within(screen.getByRole("region", { name: "本地词库管理" })).queryByText("你好"),
  ).toBeNull();
  // The host now selects the kind and code prefix instead of the client
  // filtering a page it had already fetched.
  expect(list).toHaveBeenCalledWith(0, 100, "quick_phrase", "");
  fireEvent.click(screen.getByRole("button", { name: "编辑" }));
  fireEvent.change(screen.getByLabelText("短语"), { target: { value: "updated" } });
  // The entry editor has its own 保存; 保存设置 in the footer writes the preferences document and
  // never touches the dictionary. A rename of the footer button swept this one up with it.
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(edit).toHaveBeenCalledWith(
      quick,
      { ...quick, value: "updated" },
      expect.stringMatching(/^ui-edit-/),
    ),
  );
  // Deletion is not undoable, so it asks first.
  fireEvent.click(await screen.findByRole("button", { name: "删除" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(edit).toHaveBeenCalledWith(quick, null, expect.stringMatching(/^ui-remove-/)),
  );
});

test("a pinyin prefix typed without separators keeps the rows the host matched", async () => {
  const nihao = { kind: "pinyin" as const, key: "ni'hao", value: "你好", weight: 100 };
  const other = { kind: "pinyin" as const, key: "zai'jian", value: "再见", weight: 100 };
  // The first answer is the shared host's (it already matched `nihao` to `ni'hao`); the second is a host that ignores the query, like the mobile personal dictionary.
  const list = vi
    .fn()
    .mockResolvedValueOnce({ entries: [], has_more: false })
    .mockResolvedValueOnce({ entries: [nihao], has_more: false })
    .mockResolvedValueOnce({ entries: [nihao, other], has_more: false });
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    dictionary: { list, edit: vi.fn() },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.change(await screen.findByLabelText("本地词库类型"), { target: { value: "pinyin" } });
  await waitFor(() => expect(list).toHaveBeenCalledTimes(1));
  fireEvent.change(screen.getByRole("textbox", { name: "编码前缀" }), {
    target: { value: "NiHao" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询" }));
  const results = await screen.findByRole("list", { name: "词库查询结果" });
  expect(list).toHaveBeenLastCalledWith(0, 100, "pinyin", "NiHao");
  expect(await within(results).findByText("你好")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "查询" }));
  await waitFor(() => expect(list).toHaveBeenCalledTimes(3));
  await waitFor(() => expect(within(results).queryByText("再见")).toBeNull());
  expect(within(results).getByText("你好")).toBeDefined();
});

test("a refused pinyin entry says why instead of asking for a retry", async () => {
  const edit = vi.fn().mockRejectedValue({ code: "dictionary_invalid_entry" });
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
      edit,
    },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.change(await screen.findByLabelText("本地词库类型"), { target: { value: "pinyin" } });
  fireEvent.click(await screen.findByRole("button", { name: "新增词条" }));
  fireEvent.change(screen.getByRole("textbox", { name: /^编码 / }), {
    target: { value: "nhao" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "词条" }), {
    target: { value: "你好" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("完整音节");
  expect(alert.textContent).toContain("音节数需与汉字数一致");
  expect(alert.textContent).not.toContain("稍后重试");
});

test("dictionary manager creates entries with the Windows settings default weight", async () => {
  const edit = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
      edit,
    },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "新增词条" }));
  expect(screen.getByRole("spinbutton", { name: "权重" })).toHaveProperty("value", "10");
  fireEvent.change(screen.getByRole("textbox", { name: /^编码 / }), {
    target: { value: "fixture-code" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "短语" }), {
    target: { value: "synthetic phrase" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(edit).toHaveBeenCalledWith(
      null,
      {
        kind: "quick_phrase",
        key: "fixture-code",
        value: "synthetic phrase",
        weight: 10,
      },
      expect.stringMatching(/^ui-add-/),
    ),
  );
});

function exportingDictionaryClient(saveExport?: SettingsClient["saveExport"]): SettingsClient {
  return {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    ...(saveExport ? { saveExport } : {}),
    dictionary: {
      list: vi.fn().mockImplementation(async (_offset: number, _limit: number, kind?: string) => ({
        entries:
          kind === "quick_phrase"
            ? [{ kind: "quick_phrase", key: "fixture", value: "synthetic phrase", weight: 10 }]
            : [],
        has_more: false,
      })),
      edit: vi.fn(),
      export: vi
        .fn()
        .mockResolvedValue({ text: "synthetic phrase\tfixture\t10\n", has_more: false }),
    },
  };
}

test("a host-saved dictionary export reports the path only once the host has written it", async () => {
  let written: (path: string) => void = () => undefined;
  const saveExport = vi.fn(
    () =>
      new Promise<string>((resolve) => {
        written = resolve;
      }),
  );
  const createObjectURL = vi.fn(() => "blob:fixture");
  Object.defineProperty(URL, "createObjectURL", { configurable: true, value: createObjectURL });
  render(<SettingsPage client={exportingDictionaryClient(saveExport)} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "导出当前类型" }));
  await waitFor(() =>
    expect(saveExport).toHaveBeenCalledWith(
      "水杉IME-快捷短语用户词库.txt",
      "\ufeffsynthetic phrase\tfixture\t10\n",
    ),
  );
  expect(screen.queryByText(/已导出/)).toBeNull();
  await act(async () => written("/Users/fixture/Downloads/水杉IME-快捷短语用户词库 (2).txt"));
  expect(
    await screen.findByText(
      "已导出 1 条用户词条到 /Users/fixture/Downloads/水杉IME-快捷短语用户词库 (2).txt。",
    ),
  ).toBeDefined();

  const exportAll = screen.getByRole<HTMLButtonElement>("button", { name: "导出全部" });
  // The previous export holds the dictionary buttons until it settles.
  await waitFor(() => expect(exportAll.disabled).toBe(false));
  fireEvent.click(exportAll);
  await waitFor(() =>
    expect(saveExport).toHaveBeenLastCalledWith(
      "水杉用户词库.txt",
      "# 类别\t编码\t词条\t权重\n快捷短语\tfixture\tsynthetic phrase\t10\n",
    ),
  );
  expect(screen.queryByText(/已导出全部/)).toBeNull();
  await act(async () => written("/Users/fixture/Downloads/水杉用户词库.txt"));
  expect(
    await screen.findByText(
      "已导出全部 1 条用户词条到 /Users/fixture/Downloads/水杉用户词库.txt。",
    ),
  ).toBeDefined();
  // The host wrote the file, so the page must not also start a download the webview would drop.
  expect(createObjectURL).not.toHaveBeenCalled();
});

test("a refused host-saved dictionary export shows an error and never claims success", async () => {
  const saveExport = vi.fn().mockRejectedValue({ code: "storage" });
  render(<SettingsPage client={exportingDictionaryClient(saveExport)} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "导出当前类型" }));
  expect((await screen.findByRole("alert")).textContent).toBe("无法写入“下载”文件夹，词库未导出。");
  const exportAll = screen.getByRole<HTMLButtonElement>("button", { name: "导出全部" });
  // The previous export holds the dictionary buttons until it settles.
  await waitFor(() => expect(exportAll.disabled).toBe(false));
  fireEvent.click(exportAll);
  await waitFor(() => expect(saveExport).toHaveBeenCalledTimes(2));
  expect((await screen.findByRole("alert")).textContent).toBe("无法写入“下载”文件夹，词库未导出。");
  expect(screen.queryByText(/已导出/)).toBeNull();
  expect(screen.queryByText("正在读取全部用户词库…")).toBeNull();
});

test("a host save picker the user closes cancels the export without an error or a success", async () => {
  const saveExport = vi.fn().mockResolvedValue(null);
  render(<SettingsPage client={exportingDictionaryClient(saveExport)} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "导出当前类型" }));
  expect(await screen.findByText("已取消导出。")).toBeDefined();
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.queryByText(/已导出/)).toBeNull();
});

test("a host that names its own export failure has that message shown", async () => {
  const saveExport = vi.fn().mockRejectedValue(new Error("无法保存导出文件，词库未导出。"));
  render(<SettingsPage client={exportingDictionaryClient(saveExport)} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "导出当前类型" }));
  expect((await screen.findByRole("alert")).textContent).toBe("无法保存导出文件，词库未导出。");
  expect(screen.queryByText(/已导出/)).toBeNull();
});

test("a host without saveExport keeps the download link", async () => {
  const createObjectURL = vi.fn(() => "blob:fixture");
  Object.defineProperty(URL, "createObjectURL", { configurable: true, value: createObjectURL });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: vi.fn() });
  const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
  render(<SettingsPage client={exportingDictionaryClient()} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "导出当前类型" }));
  expect(await screen.findByText("已导出 1 条用户词条。")).toBeDefined();
  expect(createObjectURL).toHaveBeenCalledTimes(1);
  expect(click).toHaveBeenCalledTimes(1);
  click.mockRestore();
});

test("dictionary fallback import uses 10000 by default and preserves an explicit zero", async () => {
  const edit = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
      edit,
    },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  const manager = screen.getByRole("region", { name: "本地词库管理" });
  fireEvent.change(within(manager).getByLabelText("导入"), {
    target: {
      files: [
        new File(["synthetic default\tdefault-code\nsynthetic zero\tzero-code\t0\n"], "words.tsv"),
      ],
    },
  });
  await waitFor(() => expect(edit).toHaveBeenCalledTimes(2));
  expect(edit).toHaveBeenNthCalledWith(
    1,
    null,
    {
      kind: "quick_phrase",
      key: "default-code",
      value: "synthetic default",
      weight: 10000,
    },
    expect.stringMatching(/^ui-import-/),
  );
  expect(edit).toHaveBeenNthCalledWith(
    2,
    null,
    {
      kind: "quick_phrase",
      key: "zero-code",
      value: "synthetic zero",
      weight: 0,
    },
    expect.stringMatching(/^ui-import-/),
  );
});

test("dictionary manager pages through entries instead of loading the whole dictionary", async () => {
  const page = (offset: number, count: number, has_more: boolean) => ({
    entries: Array.from({ length: count }, (_, index) => ({
      kind: "pinyin" as const,
      key: `k${offset + index}`,
      value: `词${offset + index}`,
      weight: 100,
    })),
    has_more,
  });
  const list = vi
    .fn()
    .mockResolvedValueOnce(page(0, 100, true))
    .mockResolvedValueOnce(page(100, 20, false));
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    dictionary: { list, edit: vi.fn() },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  fireEvent.change(await screen.findByLabelText("本地词库类型"), { target: { value: "pinyin" } });
  fireEvent.click(screen.getByRole("button", { name: "查询" }));
  expect(await screen.findByText("第 1–100 条，后面还有结果")).toBeDefined();
  // This case selected 全拼, so that is the kind the host is asked for.
  expect(list).toHaveBeenCalledWith(0, 100, "pinyin", "");
  // The first page must not be followed by a second request on its own.
  expect(list).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("button", { name: "上一页" })).toHaveProperty("disabled", true);
  const results = screen.getByRole("list", { name: "词库查询结果" });
  results.scrollTop = 480;
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  expect(results.scrollTop).toBe(0);
  expect(await screen.findByText("第 101–120 条")).toBeDefined();
  expect(list).toHaveBeenLastCalledWith(100, 100, "pinyin", "");
  expect(screen.getByRole("button", { name: "下一页" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "上一页" })).toHaveProperty("disabled", false);
});

test("dictionary manager ignores a stale page response", async () => {
  let resolveFirst: ((value: { entries: never[]; has_more: boolean }) => void) | undefined;
  let resolveSecond:
    | ((value: {
        entries: { kind: "pinyin"; key: string; value: string; weight: number }[];
        has_more: boolean;
      }) => void)
    | undefined;
  const list = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve as typeof resolveFirst;
        }),
    )
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveSecond = resolve as typeof resolveSecond;
        }),
    );
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    dictionary: { list, edit: vi.fn() },
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  const query = await screen.findByRole("button", { name: "查询" });
  const kind = screen.getByLabelText("本地词库类型");
  act(() => {
    fireEvent.click(query);
    fireEvent.change(kind, { target: { value: "pinyin" } });
  });
  expect(list).toHaveBeenCalledTimes(2);
  resolveSecond?.({
    entries: [{ kind: "pinyin", key: "new", value: "新结果", weight: 100 }],
    has_more: false,
  });
  await screen.findByText("新结果");
  resolveFirst?.({ entries: [], has_more: false });
  await act(async () => {
    await Promise.resolve();
  });
  expect(screen.getByText("新结果")).toBeDefined();
  expect(screen.queryByText("查询失败，请重试")).toBeNull();
});

test("diagnostic logging starts off and each host is saved separately", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  const server = (await screen.findByLabelText("Server 端日志")) as HTMLInputElement;
  const tsf = screen.getByLabelText("TSF 端日志") as HTMLInputElement;
  // A configuration that never mentioned diagnostics must not start logging.
  expect(server.checked).toBe(false);
  expect(tsf.checked).toBe(false);
  fireEvent.click(server);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    diagnostic_log: { server: true, tsf: false },
  });
  expect(server.checked).toBe(true);
  expect(tsf.checked).toBe(false);
});

test("Linux diagnostics expose the IBus host logger without a TSF switch", async () => {
  const host: HostCapabilities = {
    platform: "linux",
    restart_input_method: true,
    panel_windows: true,
    ime_mode_scope: true,
    typing_statistics: false,
    fuzzy_pinyin: true,
    system_fonts: true,
    window_chrome: true,
    floating_toolbar: true,
    floating_toolbar_appearance: false,
    floating_toolbar_components: false,
    mode_switch_shortcuts: true,
    panel_shortcuts: true,
    voice_capture_devices: true,
    candidate_font_controls: false,
    candidate_row_colors: true,
    candidate_selection_appearance: false,
    candidate_follow_cursor: false,
  };
  render(
    <SettingsPage client={{ load: vi.fn().mockResolvedValue(initial), save: vi.fn(), host }} />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  const log = await screen.findByLabelText("输入法宿主日志");
  // Both Linux frameworks write the log, and the user needs to know which file to send.
  // The switch is named by its row and described by the row's text.
  const description =
    document.getElementById(log.getAttribute("aria-describedby") ?? "")?.textContent ?? "";
  expect(description).toContain("Fcitx5");
  expect(description).toContain("diagnostic.log");
  // The Linux hosts log focus, preference, menu-save and failure stages only; they time nothing and have no server link to trace, so the copy must not promise either.
  expect(description).toContain("操作失败的阶段");
  // IBus refreshes the dictionary generation before the log is configured, so only the maintenance release is promised for both hosts.
  expect(description).toContain("词库维护时释放会话");
  expect(description).not.toContain("词库刷新");
  expect(description).not.toContain("延迟");
  expect(description).not.toContain("通信");
  expect(screen.queryByLabelText("TSF 端日志")).toBeNull();
});

test("macOS exposes its native server logger without a Windows TSF switch", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(await screen.findByRole("heading", { name: "关于" })).toBeDefined();
  // macOS has no Server process, so the switch is named for what it actually logs here.
  expect(screen.getByLabelText("输入法日志")).toBeDefined();
  expect(screen.queryByLabelText("Server 端日志")).toBeNull();
  expect(screen.queryByLabelText("TSF 端日志")).toBeNull();
  expect(screen.queryByLabelText("输入法宿主日志")).toBeNull();
});

test("macOS reveals the diagnostic log in Finder and names what it records", async () => {
  const openDiagnosticLogDirectory = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "macos" } as HostCapabilities,
        openDiagnosticLogDirectory,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  const log = await screen.findByLabelText("输入法日志");
  // The macOS log covers key latency, candidate placement and statistics failures as well as focus and preferences, and the copy points at the action instead of a path the Finder hides.
  // The switch is named by its row and described by the row's text.
  const description =
    document.getElementById(log.getAttribute("aria-describedby") ?? "")?.textContent ?? "";
  expect(description).toContain("超过 8 毫秒的按键处理耗时");
  expect(description).toContain("候选窗口的显示位置");
  expect(description).toContain("输入统计写入失败");
  expect(description).toContain("不记录按键、输入内容或候选文本");
  expect(description).toContain("在 Finder 中显示");
  fireEvent.click(screen.getByRole("button", { name: "在 Finder 中显示" }));
  expect(openDiagnosticLogDirectory).toHaveBeenCalledTimes(1);
  expect(openDiagnosticLogDirectory).toHaveBeenCalledWith();
  expect(screen.queryByRole("alert")).toBeNull();

  openDiagnosticLogDirectory.mockRejectedValueOnce(new Error("storage"));
  fireEvent.click(screen.getByRole("button", { name: "在 Finder 中显示" }));
  expect((await screen.findByRole("alert")).textContent).toBe(
    "无法在 Finder 中显示诊断日志，请稍后重试。",
  );
});

test("the diagnostic log action needs a host that can reveal the file", async () => {
  const { unmount } = render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  expect(await screen.findByLabelText("输入法日志")).toBeDefined();
  // Without the host callback, as on the phones, there is no button that would fail when pressed.
  expect(screen.queryByRole("button", { name: "在 Finder 中显示" })).toBeNull();
  expect(screen.queryByRole("button", { name: "打开日志目录" })).toBeNull();
  unmount();

  const openDiagnosticLogDirectory = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "linux" } as HostCapabilities,
        openDiagnosticLogDirectory,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  // Outside macOS the action opens the folder and says so.
  fireEvent.click(await screen.findByRole("button", { name: "打开日志目录" }));
  expect(openDiagnosticLogDirectory).toHaveBeenCalledTimes(1);
});

test.each(["windows", "linux", "macos", "android", "ios", "harmony", undefined])(
  "the usage reporting switch is offered on every platform, starts on and says what it sends (%s)",
  async (platform) => {
    const client: SettingsClient = {
      load: vi.fn().mockResolvedValue(initial),
      save: vi.fn().mockImplementation(async (_revision, preferences) => ({
        ...initial,
        revision: 8,
        preferences,
      })),
      host: platform ? ({ platform } as HostCapabilities) : undefined,
    };
    render(<SettingsPage client={client} />);
    await settingsReady();
    fireEvent.click(screen.getByRole("button", { name: "关于" }));
    const toggle = (await screen.findByRole("switch", {
      name: "匿名使用统计",
    })) as HTMLInputElement;
    // A configuration that never mentioned usage reporting reports by default.
    expect(toggle.checked).toBe(true);
    const description = screen.getByText(/^默认开启，可随时关闭。/).textContent ?? "";
    expect(description).toContain("https://api.msime.app/v1/telemetry/events");
    expect(description).toContain("安装 id");
    expect(description).toContain("不含输入内容");
    expect(description).toContain("清空尚未发送的记录");
    fireEvent.click(toggle);
    saveSettingsNow();
    await screen.findByText("已保存");
    expect(client.save).toHaveBeenCalledWith(7, {
      ...initial.preferences,
      usage_reporting: false,
    });
  },
);

const initial: Snapshot = {
  format_version: 1,
  revision: 7,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

test("macOS exposes learning data reset and keeps its confirmation flow", async () => {
  const resetLearnedData = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="dictionary"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        resetLearnedData,
        dictionary: {} as never,
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );

  await screen.findByRole("region", { name: "学习数据" });
  // The destructive action wears the shared `danger-button` utility (see learning-data-section.test.tsx).
  expect(
    screen.getByRole("button", { name: "清除全部学习数据" }).classList.contains("danger-button"),
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "清除全部学习数据" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(resetLearnedData).toHaveBeenCalledTimes(1));
  expect(await screen.findByText("已清除所有学习数据；输入方案和设置保持不变。")).toBeTruthy();
});

test("mobile hosts use Apple-style primary navigation and retain secondary settings", async () => {
  const host: HostCapabilities = {
    platform: "ios",
    restart_input_method: false,
    panel_windows: false,
    ime_mode_scope: false,
    typing_statistics: true,
    fuzzy_pinyin: false,
    system_fonts: false,
    window_chrome: false,
    floating_toolbar: false,
    floating_toolbar_appearance: false,
    floating_toolbar_components: false,
    mode_switch_shortcuts: false,
    panel_shortcuts: false,
    voice_capture_devices: false,
    candidate_font_controls: false,
    candidate_row_colors: false,
    candidate_selection_appearance: false,
    candidate_follow_cursor: false,
  };
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host,
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        typingStatistics: {
          load: vi.fn().mockResolvedValue({
            availability: "neverWritten",
            statistics: { enabled: false, total: 0, days: {}, detail: {} },
          }),
          setEnabled: vi.fn(),
          reset: vi.fn(),
        },
        account: {
          status: vi.fn().mockResolvedValue({ available: false }),
          providers: vi.fn().mockResolvedValue({ apple: false, email: false, phone: false }),
          requestCode: vi.fn(),
          login: vi.fn(),
          profile: vi.fn(),
          rename: vi.fn(),
          logout: vi.fn(),
          deleteAccount: vi.fn(),
          clearExpired: vi.fn(),
        },
        communitySkins: {
          list: vi.fn(),
          detail: vi.fn(),
          download: vi.fn(),
          rate: vi.fn(),
          publish: vi.fn(),
          unpublish: vi.fn(),
          setCategory: vi.fn(),
          finishTrial: vi.fn(),
        },
        communityResources: {
          list: vi.fn(),
          detail: vi.fn(),
          publish: vi.fn(),
          apply: vi.fn(),
          save: vi.fn(),
          rate: vi.fn(),
          unpublish: vi.fn(),
          storeReply: vi.fn(),
          removeReply: vi.fn(),
        },
      }}
    />,
  );
  await settingsFormReady();
  const primary = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(primary).getByRole("button", { name: "设置" })).toBeTruthy();
  expect(within(primary).getByRole("button", { name: "社区" })).toBeTruthy();
  expect(within(primary).getByRole("button", { name: "统计" })).toBeTruthy();
  // The source names this tab 我的, which is also the page's own title; the bar said 账号 against it.
  expect(within(primary).getByRole("button", { name: "我的" })).toBeTruthy();
  // The bar holds those four and nothing else. Every other page is a row on the 全部设置 page, one
  // level down inside the 设置 tab.
  fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
  const rows = [...screen.getByRole("region", { name: "全部设置" }).querySelectorAll("button")];
  const secondaryLabels = rows.map((row) => row.querySelector("strong")?.textContent ?? "");
  expect(secondaryLabels).toContain("输入");
  expect(secondaryLabels).toContain("剪贴板");
  expect(secondaryLabels).not.toContain("辅助码");
  // A touch host calls the shortcut page 外接键盘快捷键, and this one routes no hardware chords.
  expect(secondaryLabels).not.toContain("外接键盘快捷键");
  expect(secondaryLabels).not.toContain("悬浮工具栏");
  fireEvent.click(rows[secondaryLabels.indexOf("输入")]);
  expect(screen.getByRole("heading", { name: "输入" })).toBeTruthy();
});

test("mobile input settings expose native keyboard sound and haptic feedback", async () => {
  const load = vi.fn().mockResolvedValue({
    soundEnabled: true,
    hapticsEnabled: false,
    hapticStrength: "medium",
    englishSuggestions: true,
  });
  const save = vi.fn().mockImplementation(async (settings) => settings);
  const preview = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save,
        host: { platform: "ios" } as HostCapabilities,
        home: { openKeyboard: vi.fn() },
        mobileKeyboardFeedback: { load, save, preview },
      }}
    />,
  );
  expect(await screen.findByRole("heading", { name: "键盘" })).toBeTruthy();
  const feedback = await screen.findByRole("group", { name: "按键反馈" });
  expect(within(feedback).getByLabelText("按键音")).toBeTruthy();
  const haptics = within(feedback).getByLabelText("按键振动") as HTMLInputElement;
  fireEvent.click(haptics);
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ hapticsEnabled: true })),
  );
  expect(within(feedback).getByLabelText("振动强度")).toBeTruthy();
  fireEvent.click(within(feedback).getByRole("button", { name: "试一下振动" }));
  await waitFor(() => expect(preview).toHaveBeenCalledWith("medium"));
  // 「英文建议」管的是候选，在输入页「候选与联想」组，不在按键反馈里重复出现。
  expect(within(feedback).queryByLabelText("英文建议")).toBeNull();
});

test("iOS English suggestions sit with the candidate settings on the input page", async () => {
  const load = vi.fn().mockResolvedValue({
    soundEnabled: true,
    hapticsEnabled: false,
    hapticStrength: "medium",
    englishSuggestions: true,
  });
  const save = vi.fn().mockImplementation(async (settings) => settings);
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "ios" } as HostCapabilities,
        home: { openKeyboard: vi.fn() },
        mobileKeyboardFeedback: { load, save },
      }}
    />,
  );
  const candidates = await screen.findByRole("region", { name: "候选与联想" });
  const toggle = (await within(candidates).findByRole("switch", {
    name: "英文建议",
  })) as HTMLInputElement;
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ englishSuggestions: false })),
  );
});

test("an iPad keeps key sounds but hides vibration it cannot produce", async () => {
  const load = vi.fn().mockResolvedValue({
    soundEnabled: true,
    hapticsEnabled: true,
    hapticStrength: "strong",
    englishSuggestions: true,
    hapticsAvailable: false,
  });
  const save = vi.fn().mockImplementation(async (settings) => settings);
  render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn().mockImplementation(async (value) => value),
        host: { platform: "ios" } as HostCapabilities,
        home: { openKeyboard: vi.fn() },
        mobileKeyboardFeedback: { load, save, preview: vi.fn() },
      }}
    />,
  );
  const feedback = await screen.findByRole("group", { name: "按键反馈" });
  fireEvent.click(within(feedback).getByLabelText("按键音"));
  // The stored vibration choice travels untouched, so it still reaches the user's iPhone through settings sync.
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        soundEnabled: false,
        hapticsEnabled: true,
        hapticStrength: "strong",
      }),
    ),
  );
  expect(within(feedback).queryByLabelText("按键振动")).toBeNull();
  expect(within(feedback).queryByLabelText("振动强度")).toBeNull();
  expect(within(feedback).queryByRole("button", { name: "试一下振动" })).toBeNull();
});

test("the iPad digit row and Tab key switch appears only where the plugin reports it", async () => {
  const feedback = {
    soundEnabled: true,
    hapticsEnabled: false,
    hapticStrength: "medium",
    englishSuggestions: true,
  };
  const phone = render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn().mockImplementation(async (value) => value),
        host: { platform: "ios" } as HostCapabilities,
        home: { openKeyboard: vi.fn() },
        mobileKeyboardFeedback: { load: vi.fn().mockResolvedValue(feedback), save: vi.fn() },
      }}
    />,
  );
  await screen.findByLabelText("键盘高度", undefined, { timeout: 3000 });
  await waitFor(() => expect(screen.queryByLabelText("按键音")).not.toBeNull());
  expect(screen.queryByLabelText("数字行与 Tab 键")).toBeNull();
  phone.unmount();

  const save = vi.fn().mockImplementation(async (settings) => settings);
  const saveDocument = vi.fn().mockImplementation(async (value) => value);
  render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: saveDocument,
        host: { platform: "ios" } as HostCapabilities,
        home: { openKeyboard: vi.fn() },
        mobileKeyboardFeedback: {
          load: vi
            .fn()
            .mockResolvedValue({ ...feedback, hapticsAvailable: false, tabletFullKeys: true }),
          save,
        },
      }}
    />,
  );
  const fullKeys = (await screen.findByLabelText("数字行与 Tab 键", undefined, {
    timeout: 3000,
  })) as HTMLInputElement;
  expect(fullKeys.checked).toBe(true);
  fireEvent.click(fullKeys);
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ tabletFullKeys: false })),
  );
  expect(saveDocument).not.toHaveBeenCalled();
});

test("the touch toolbar switches appear only on a host that reads them and save into the document", async () => {
  const android = render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "android" } as HostCapabilities,
      }}
    />,
  );
  await screen.findByLabelText("键盘高度", undefined, { timeout: 3000 });
  expect(screen.queryByLabelText("工具栏：剪贴板历史")).toBeNull();
  android.unmount();

  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save,
        host: { platform: "ios", touch_toolbar_components: true } as HostCapabilities,
        home: { openKeyboard: vi.fn() },
      }}
    />,
  );
  const clipboard = (await screen.findByLabelText("工具栏：剪贴板历史", undefined, {
    timeout: 3000,
  })) as HTMLInputElement;
  const skin = screen.getByLabelText("工具栏：切换皮肤") as HTMLInputElement;
  // A document that has never said anything keeps the bar the keyboard always had.
  expect(clipboard.checked).toBe(false);
  expect(skin.checked).toBe(true);
  fireEvent.click(clipboard);
  fireEvent.click(skin);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    touch_toolbar: {
      layout: true,
      emoji: true,
      skin: false,
      clipboard: true,
      ai: false,
      character_set: false,
      fullwidth: false,
      punctuation: false,
    },
  });
});

test("the iOS skin page hands the candidate strip to the desktop candidate skin", async () => {
  const load = vi.fn().mockResolvedValue({
    soundEnabled: true,
    hapticsEnabled: false,
    hapticStrength: "medium",
    englishSuggestions: true,
    candidatePaletteFollowsDesktop: false,
  });
  const saveFeedback = vi.fn().mockImplementation(async (settings) => settings);
  const client = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (value) => value),
    host: { platform: "ios", candidate_row_colors: true } as HostCapabilities,
    home: { openKeyboard: vi.fn() },
    mobileKeyboardFeedback: { load, save: saveFeedback },
  };
  const { unmount } = render(<SettingsPage initialPage="appearance" client={client} />);
  expect(
    await screen.findByText(/候选栏正在使用键盘皮肤的颜色/, undefined, { timeout: 3000 }),
  ).toBeTruthy();
  unmount();

  render(<SettingsPage initialPage="skin" client={client} />);
  const follow = (await screen.findByLabelText("使用桌面候选皮肤")) as HTMLInputElement;
  expect(follow.checked).toBe(false);
  fireEvent.click(follow);
  await waitFor(() =>
    expect(saveFeedback).toHaveBeenCalledWith(
      expect.objectContaining({ candidatePaletteFollowsDesktop: true, englishSuggestions: true }),
    ),
  );
});

test("iOS offers 行内预编辑 as its own keyboard switch instead of the shared preedit style", async () => {
  const load = vi.fn().mockResolvedValue({
    soundEnabled: true,
    hapticsEnabled: false,
    hapticStrength: "medium",
    englishSuggestions: true,
    candidatePaletteFollowsDesktop: false,
    inlinePreedit: false,
  });
  const saveFeedback = vi.fn().mockImplementation(async (settings) => settings);
  const save = vi.fn().mockImplementation(async (value) => value);
  const client = {
    load: vi.fn().mockResolvedValue(initial),
    save,
    host: { platform: "ios" } as HostCapabilities,
    home: { openKeyboard: vi.fn() },
    mobileKeyboardFeedback: { load, save: saveFeedback },
  };
  render(<SettingsPage initialPage="appearance" client={client} />);
  const inline = (await screen.findByLabelText("行内预编辑", undefined, {
    timeout: 3000,
  })) as HTMLInputElement;
  expect(inline.type).toBe("checkbox");
  expect(inline.checked).toBe(false);
  expect(screen.queryByRole("option", { name: "原始按键" })).toBeNull();
  fireEvent.click(inline);
  await waitFor(() =>
    expect(saveFeedback).toHaveBeenCalledWith(
      expect.objectContaining({ inlinePreedit: true, candidatePaletteFollowsDesktop: false }),
    ),
  );
  expect(save).not.toHaveBeenCalled();
});

test("iOS describes local modes and 以词定字 the way its keyboard reaches them", async () => {
  const client = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    host: { platform: "ios" } as HostCapabilities,
    home: { openKeyboard: vi.fn() },
  };
  // Both live on the 输入 page.
  render(<SettingsPage initialPage="input" client={client} />);
  const input = await screen.findByRole("group", { name: "输入" }, { timeout: 3000 });
  expect(within(input).getByText(/「更多 → 本地输入」里选「快捷短语」/)).toBeTruthy();
  expect(within(input).getByText(/「更多 → 本地输入」里选「英文补全」/)).toBeTruthy();
  // 只看快捷模式组：同页「选词与翻页」组的翻页方式里有 Shift+Tab 这样的键名。
  expect(
    within(within(input).getByRole("region", { name: "快捷模式" })).queryByText(/Shift\+[A-Z]/),
  ).toBeNull();
  expect(within(input).getByText(/长按两个字以上的候选/)).toBeTruthy();
  expect(within(input).queryByText("以词定字快捷键")).toBeNull();
});

test("desktop hosts keep the chord wording for local modes and 以词定字", async () => {
  const client = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    host: { platform: "windows" } as HostCapabilities,
  };
  render(<SettingsPage initialPage="input" client={client} />);
  const input = await screen.findByRole("group", { name: "输入" }, { timeout: 3000 });
  expect(within(input).getByText(/中文模式下按 Shift\+K/)).toBeTruthy();
  expect(within(input).getByText("以词定字快捷键")).toBeTruthy();
});

test("a mobile host without the candidate palette switch shows neither the switch nor the hint", async () => {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "android" } as HostCapabilities,
        home: { openKeyboard: vi.fn() },
        mobileKeyboardFeedback: {
          load: vi.fn().mockResolvedValue({
            soundEnabled: true,
            hapticsEnabled: true,
            hapticStrength: "medium",
          }),
          save: vi.fn(),
        },
      }}
    />,
  );
  expect(await screen.findByRole("group", { name: "主题" }, { timeout: 3000 })).toBeTruthy();
  expect(screen.queryByLabelText("使用桌面候选皮肤")).toBeNull();
  expect(screen.queryByText(/候选栏正在使用键盘皮肤的颜色/)).toBeNull();
});

test("mobile settings pages follow the WebView back stack", async () => {
  const previous = window.history.state;
  window.history.replaceState(null, "");
  try {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host: { platform: "ios" } as HostCapabilities,
          home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        }}
      />,
    );
    await settingsFormReady();
    fireEvent.click(screen.getByRole("button", { name: /全部设置/ }));
    const rows = [...screen.getByRole("region", { name: "全部设置" }).querySelectorAll("button")];
    fireEvent.click(rows.find((row) => row.querySelector("strong")?.textContent === "输入")!);
    expect(window.history.state).toEqual(
      expect.objectContaining({ msimeSettings: true, page: "input" }),
    );
    act(() => {
      const state = { msimeSettings: true, page: "appearance" };
      window.history.replaceState(state, "");
      window.dispatchEvent(new PopStateEvent("popstate", { state }));
    });
    expect(await screen.findByRole("heading", { name: "候选栏" })).toBeTruthy();
  } finally {
    window.history.replaceState(previous, "");
  }
});

test("mobile settings reload shared preferences after returning to foreground", async () => {
  let hidden = false;
  vi.spyOn(document, "hidden", "get").mockImplementation(() => hidden);
  const load = vi.fn().mockResolvedValue(initial);
  render(
    <SettingsPage
      client={{
        load,
        save: vi.fn(),
        host: { platform: "android" } as HostCapabilities,
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );
  await settingsFormReady();
  expect(load).toHaveBeenCalledTimes(1);
  hidden = true;
  fireEvent(document, new Event("visibilitychange"));
  hidden = false;
  fireEvent(document, new Event("visibilitychange"));
  await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
});

test("mobile account deep links participate in the back stack", async () => {
  const previous = window.history.state;
  window.history.replaceState(null, "");
  try {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host: { platform: "ios" } as HostCapabilities,
          account: {
            status: vi.fn().mockResolvedValue({ available: false }),
            providers: vi.fn().mockResolvedValue({ apple: false, email: false, phone: false }),
            requestCode: vi.fn(),
            login: vi.fn(),
            profile: vi.fn(),
            rename: vi.fn(),
            logout: vi.fn(),
            deleteAccount: vi.fn(),
            clearExpired: vi.fn(),
          },
        }}
      />,
    );
    await settingsFormReady();
    // The tab and the sidebar entry share the page's title, so reach for the one in the bar.
    const bar = screen.getByRole("navigation", { name: "主要功能" });
    fireEvent.click(within(bar).getByRole("button", { name: "我的" }));
    // 反馈 and 关于 live in 我的 on a touch host, so the 全部设置 list does not repeat them.
    expect(screen.queryByRole("button", { name: "帮助与反馈" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "关于" }));
    expect(window.history.state).toEqual(
      expect.objectContaining({ msimeSettings: true, page: "about" }),
    );
    act(() => {
      const state = { msimeSettings: true, page: "account" };
      window.history.replaceState(state, "");
      window.dispatchEvent(new PopStateEvent("popstate", { state }));
    });
    expect(await screen.findByRole("heading", { name: "我的" })).toBeTruthy();
  } finally {
    window.history.replaceState(previous, "");
  }
});

test("candidate appearance settings persist and use Windows baseline defaults", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage initialPage="appearance" client={client} />);
  // The layout is a segmented control: one radio per arrangement.
  const layout = await screen.findByRole("radiogroup", { name: "候选项排列方式" });
  expect((within(layout).getByRole("radio", { name: "纵向" }) as HTMLInputElement).checked).toBe(
    true,
  );
  expect((screen.getByLabelText("候选窗字号") as HTMLSelectElement).value).toBe("18");
  expect((screen.getByLabelText("候选窗预编辑字号") as HTMLSelectElement).value).toBe("15");
  fireEvent.click(within(layout).getByRole("radio", { name: "横向" }));
  fireEvent.change(screen.getByLabelText("候选窗字号"), { target: { value: "20" } });
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  expect(screen.getByRole("switch", { name: /跟随系统/ }).getAttribute("aria-checked")).toBe(
    "true",
  );
  fireEvent.click(screen.getByRole("switch", { name: /夜青/ }));
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    candidate_layout: "horizontal",
    candidate_font_size: 20,
    global_theme: "night",
  });
});

test("macOS exposes the shuangpin preedit presentation and persists the expanded pinyin choice", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save,
    host: { platform: "macos" } as HostCapabilities,
  };
  render(<SettingsPage initialPage="appearance" client={client} />);
  const preedit = (await screen.findByRole("combobox", {
    name: "双拼预编辑",
  })) as HTMLSelectElement;
  expect(preedit.value).toBe("raw");
  fireEvent.change(preedit, { target: { value: "pinyin" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    shuangpin_preedit_uses_raw: false,
  });
});

test("font family controls validate drafts and save Unicode without touching the fallback list", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  const mounted = render(<SettingsPage client={{ load: async () => initial, save }} />);
  const primary = await screen.findByLabelText("候选窗主字体");
  expect((primary as HTMLInputElement).value).toBe("Noto Sans SC");
  // An empty family holds the automatic save back, and submitting the form does not force one.
  fireEvent.change(primary, { target: { value: "" } });
  saveSettingsNow();
  fireEvent.submit(mounted.container.querySelector("form")!);
  expect(save).not.toHaveBeenCalled();
  fireEvent.change(primary, { target: { value: "示例主字体" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(7, {
    ...initial.preferences,
    candidate_font_family: "示例主字体",
  });
  const saves = save.mock.calls.length;
  fireEvent.change(primary, { target: { value: "字".repeat(43) } });
  saveSettingsNow();
  expect(save).toHaveBeenCalledTimes(saves);
  fireEvent.change(primary, { target: { value: "有效示例" } });
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(8, {
      ...initial.preferences,
      candidate_font_family: "有效示例",
    }),
  );
});

// The fallback list is written by the 候选字体 presets and has no editor of its own: a stored list, even a full one, is carried through a save unchanged.
test("the candidate window page has no fallback font editor and keeps the stored list", async () => {
  const fallbacks = Array.from({ length: 32 }, (_, i) => `示例${i}`);
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => ({
          ...initial,
          preferences: { ...initial.preferences, candidate_fallback_fonts: fallbacks },
        }),
        save,
      }}
    />,
  );
  const primary = await screen.findByLabelText("候选窗主字体");
  expect(screen.queryByRole("button", { name: "添加补充字体" })).toBeNull();
  expect(screen.queryByLabelText(/^补充字体/)).toBeNull();
  expect(screen.queryByText(/补充字体/)).toBeNull();
  fireEvent.change(primary, { target: { value: "示例主字体" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(7, {
    ...initial.preferences,
    candidate_font_family: "示例主字体",
    candidate_fallback_fonts: fallbacks,
  });
});

// The Windows renderer reads no candidate_font_family: Chinese text comes from the fallback chain, so there the main font also leads that chain, and the names typed on the way to it do not pile up behind it.
test("on Windows the main font leads the fallback chain", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save,
        host: { platform: "windows", candidate_font_controls: true } as HostCapabilities,
      }}
    />,
  );
  const primary = await screen.findByLabelText("候选窗主字体");
  for (const typed of ["L", "LX", "LXGW WenKai"])
    fireEvent.change(primary, { target: { value: typed } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(7, {
    ...initial.preferences,
    candidate_font_family: "LXGW WenKai",
    candidate_fallback_fonts: ["LXGW WenKai", "Microsoft YaHei"],
  });
});

test("automatic color swatch follows candidate theme without persisting a color override", async () => {
  const save = vi
    .fn()
    .mockImplementation(async (_revision, preferences) => ({ ...initial, preferences }));
  render(<SettingsPage client={{ load: async () => initial, save }} />);
  const color = (await screen.findByLabelText("候选文字颜色")) as HTMLInputElement;
  const change = (label: string, value: string) =>
    fireEvent.change(screen.getByLabelText(label), { target: { value } });
  // The pickers and the theme selects are on 主题; the full candidate window preview stays on 候选窗口.
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  expect(color.value).toBe("#e9e8e8");
  fireEvent.click(
    within(screen.getByRole("radiogroup", { name: "颜色模式" })).getByRole("radio", {
      name: "浅色",
    }),
  );
  expect(color.value).toBe("#1a1a1a");
  change("设置界面主题", "dark");
  expect(color.value).toBe("#1a1a1a");
  change("候选窗口主题", "dark");
  expect(color.value).toBe("#e9e8e8");
  change("候选文字颜色", "#123456");
  change("候选窗口主题", "light");
  expect(color.value).toBe("#123456");
  fireEvent.click(screen.getByRole("button", { name: "跟随主题" }));
  expect(color.value).toBe("#1a1a1a");
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  const preview = screen
    .getByRole("region", { name: "候选窗口预览" })
    .querySelector<HTMLElement>(".appearance-candidate-preview")!;
  // Picking a colour selected the custom theme; resetting it leaves that selection alone. No override is left, so the preview draws the platform's own text colour rather than the #123456 that was typed and then dropped.
  expect(preview.getAttribute("data-global-theme")).toBe("custom");
  expect(preview.style.getPropertyValue("--cand-text")).toBe("");
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        global_theme: "custom",
        // Picking from the system theme customizes that theme: it becomes the base and no package is layered over it.
        custom_theme: { base: "system", candidate_skin: null, candidate_colors: { text: null } },
      }),
    ),
  );
});

test("screen keyboard header drag is bounded and separate from close and keys", async () => {
  expect(keyboardCapability.windows).toEqual(["keyboard-panel"]);
  expect(keyboardCapability.permissions).toEqual([
    "core:window:allow-start-dragging",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
  ]);
  const beginWindowDrag = vi.fn().mockResolvedValue(undefined);
  const close = vi.fn().mockResolvedValue(undefined);
  const sendKey = vi.fn().mockResolvedValue(undefined);
  const view = render(<KeyboardPanel client={{ close, beginWindowDrag, sendKey }} />);
  const header = view.container.querySelector(".native-panel-header")!;
  titlebarPointer(header, "pointerdown", 100, 14);
  titlebarPointer(header, "pointermove", 101, 14);
  expect(beginWindowDrag).not.toHaveBeenCalled();
  titlebarPointer(header, "pointermove", 103, 14);
  titlebarPointer(header, "pointermove", 110, 14);
  expect(beginWindowDrag).toHaveBeenCalledTimes(1);
  for (const target of [
    screen.getByRole("button", { name: "关闭" }),
    screen.getByRole("button", { name: "a" }),
  ]) {
    titlebarPointer(target, "pointerdown", 100, 14);
    titlebarPointer(target, "pointermove", 110, 14);
  }
  expect(beginWindowDrag).toHaveBeenCalledTimes(1);
  expect(close).not.toHaveBeenCalled();
  expect(sendKey).not.toHaveBeenCalled();
  for (const reason of ["pointerup", "pointercancel", "pointerout", "blur"]) {
    titlebarPointer(header, "pointerdown", 100, 14);
    if (reason === "blur") fireEvent(window, new Event("blur"));
    else titlebarPointer(header, reason, 100, 14);
    titlebarPointer(header, "pointermove", 110, 14);
    expect(beginWindowDrag).toHaveBeenCalledTimes(1);
  }
});

test.each([false, true])(
  "keyboard drag reports host failure (synchronous=%s)",
  async (synchronous) => {
    const view = render(
      <KeyboardPanel
        client={{
          close: async () => {},
          beginWindowDrag: () => {
            if (synchronous) throw new Error("synthetic");
            return Promise.reject(new Error("synthetic"));
          },
        }}
      />,
    );
    const header = view.container.querySelector(".native-panel-header")!;
    titlebarPointer(header, "pointerdown", 100, 14);
    titlebarPointer(header, "pointermove", 110, 14);
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe("无法移动窗口，请重试。"),
    );
  },
);

test("screen keyboard matches upstream Shift and Caps posting combinations", async () => {
  for (const caps of [false, true])
    for (const shift of [false, true]) {
      const sendKey = vi.fn().mockResolvedValue(undefined);
      const panel = render(<KeyboardPanel client={{ close: async () => {}, sendKey }} />);
      if (caps) fireEvent.click(screen.getByRole("button", { name: "Caps Lock" }));
      if (shift) fireEvent.click(screen.getAllByRole("button", { name: "Shift" })[0]);
      // Find the key by what it types: the preview's row layout is presentation
      // and has already been rearranged once.
      const letter = Array.from(
        panel.container.querySelectorAll<HTMLButtonElement>("[data-keyboard-row] button"),
      ).find((button) => button.textContent === (shift ? "A" : "a"))!;
      expect(letter).toBeDefined();
      expect(screen.getByRole("button", { name: "Space" })).toBeDefined();
      fireEvent.click(letter);
      expect(sendKey).toHaveBeenLastCalledWith(
        expect.objectContaining({
          virtual_key: 0x41,
          shift: caps !== shift,
          include_sticky_modifiers: true,
        }),
      );
      await waitFor(() => expect(screen.getByRole("status").textContent).toContain("已发送"));
      expect(letter.textContent).toBe("a");
      panel.unmount();
    }
});

test("screen keyboard serializes rapid host commands and drops queued keys after failure", async () => {
  const deliveries: Array<{ resolve(): void; reject(): void }> = [];
  const sendKey = vi.fn().mockImplementation(
    () =>
      new Promise<void>((resolve, reject) => {
        deliveries.push({ resolve, reject });
      }),
  );
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} />);

  fireEvent.click(screen.getByRole("button", { name: "a" }));
  fireEvent.click(screen.getByRole("button", { name: "b" }));
  fireEvent.click(screen.getByRole("button", { name: "c" }));

  expect(sendKey).toHaveBeenCalledTimes(1);
  expect(sendKey.mock.calls[0][0].virtual_key).toBe(0x41);
  deliveries[0].resolve();
  await waitFor(() => expect(sendKey).toHaveBeenCalledTimes(2));
  expect(sendKey.mock.calls[1][0].virtual_key).toBe(0x42);

  deliveries[1].reject();
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("后续排队按键已取消"),
  );
  expect(sendKey).toHaveBeenCalledTimes(2);

  fireEvent.click(screen.getByRole("button", { name: "d" }));
  await waitFor(() => expect(sendKey).toHaveBeenCalledTimes(3));
  expect(sendKey.mock.calls[2][0].virtual_key).toBe(0x44);
  deliveries[2].resolve();
  await waitFor(() => expect(screen.getByRole("status").textContent).toContain("已发送：d"));
});

test("screen keyboard refreshes the external target before every queued key", async () => {
  const events: string[] = [];
  const rememberInputTarget = vi.fn(async () => {
    events.push("remember");
  });
  const sendKey = vi.fn(async () => {
    events.push("send");
  });
  render(<KeyboardPanel client={{ close: async () => {}, rememberInputTarget, sendKey }} />);
  await waitFor(() => expect(rememberInputTarget).toHaveBeenCalledTimes(1));
  events.length = 0;

  fireEvent.click(screen.getByRole("button", { name: "a" }));
  fireEvent.click(screen.getByRole("button", { name: "b" }));
  await waitFor(() => expect(sendKey).toHaveBeenCalledTimes(2));
  expect(events).toEqual(["remember", "send", "remember", "send"]);
});

test("screen keyboard sends every digit as an unmodified IME selection key", async () => {
  const sendKey = vi.fn().mockResolvedValue(undefined);
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} />);
  for (const modifier of ["Ctrl", "Alt", "Win"]) {
    fireEvent.click(screen.getAllByRole("button", { name: modifier })[0]);
  }
  for (const [index, digit] of [..."1234567890"].entries()) {
    fireEvent.click(screen.getAllByRole("button", { name: "Shift" })[0]);
    fireEvent.click(screen.getByRole("button", { name: [..."!@#$%^&*()"][index] }));
    await waitFor(() => expect(sendKey).toHaveBeenCalledTimes(index + 1));
    expect(sendKey).toHaveBeenLastCalledWith({
      virtual_key: digit.charCodeAt(0),
      shift: false,
      modifiers: { ctrl: true, alt: true, win: true },
      include_sticky_modifiers: false,
    });
    expect(screen.getAllByRole("button", { name: "Shift" })[0].getAttribute("aria-pressed")).toBe(
      "false",
    );
  }
  await waitFor(() => expect(screen.getByRole("status").textContent).toContain("已发送"));
});

test("screen keyboard Shift key faces match punctuation and preserve virtual keys", async () => {
  const sendKey = vi.fn().mockResolvedValue(undefined);
  render(<KeyboardPanel client={{ close: async () => {}, sendKey }} />);
  const keys: [string, string, number][] = [
    ["`", "~", 0xc0],
    ["-", "_", 0xbd],
    ["=", "+", 0xbb],
    ["[", "{", 0xdb],
    ["]", "}", 0xdd],
    ["\\", "|", 0xdc],
    [";", ":", 0xba],
    ["'", '"', 0xde],
    [",", "<", 0xbc],
    [".", ">", 0xbe],
    ["/", "?", 0xbf],
  ];
  for (const [index, [normal, shifted, code]] of keys.entries()) {
    fireEvent.click(screen.getAllByRole("button", { name: "Shift" })[0]);
    fireEvent.click(screen.getByRole("button", { name: shifted }));
    await waitFor(() => expect(sendKey).toHaveBeenCalledTimes(index + 1));
    expect(sendKey).toHaveBeenLastCalledWith(
      expect.objectContaining({ virtual_key: code, shift: true, include_sticky_modifiers: true }),
    );
    expect(screen.getByRole("button", { name: normal })).toBeDefined();
  }
  await waitFor(() => expect(screen.getByRole("status").textContent).toContain("已发送"));
});

test("screen keyboard theme and Apple skin load, save independently and reload", async () => {
  let snapshot: Snapshot = {
    ...initial,
    preferences: {
      ...initial.preferences,
      theme: "light",
      screen_keyboard_theme: "light",
      toolbar_theme: "dark",
      global_theme: "paper",
    },
  };
  const save = vi.fn().mockImplementation(async (_revision, preferences) => {
    snapshot = { ...snapshot, revision: 8, preferences };
    return snapshot;
  });
  render(<SettingsPage client={{ load: async () => snapshot, save }} />);
  const select = (await screen.findByLabelText("屏幕键盘主题")) as HTMLSelectElement;
  fireEvent.click(screen.getByRole("button", { name: "屏幕键盘" }));
  expect(select.value).toBe("light");
  // The override sits with the other surfaces' under 主题 › 高级; the keyboard page only links there.
  expect(
    within(screen.getByRole("group", { name: "屏幕键盘" })).queryByLabelText("屏幕键盘主题"),
  ).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "屏幕键盘外观" }));
  expect(
    within(screen.getByRole("group", { name: "主题" })).getByRole("combobox", {
      name: "屏幕键盘主题",
    }),
  ).toBe(select);
  fireEvent.click(screen.getByRole("button", { name: "屏幕键盘" }));
  const preview = screen.getByRole("img", { name: "屏幕键盘完整布局预览" });
  expect(preview.getAttribute("data-preview-theme")).toBe("light");
  expect(preview.getAttribute("data-preview-skin")).toBe("paper");
  // The keyboard follows the global theme, which is picked from the cards on 主题; the keyboard page keeps the full preview.
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const cards = screen
    .getAllByRole("switch")
    .filter((button) => button.closest("article")?.getAttribute("aria-label"));
  expect(cards.map((button) => button.getAttribute("aria-label"))).toEqual([
    "跟随系统",
    "水杉",
    "浅色",
    "纸白",
    "夜青",
    "墨",
    "自定义",
  ]);
  fireEvent.click(screen.getByRole("switch", { name: "夜青" }));
  fireEvent.click(screen.getByRole("button", { name: "屏幕键盘" }));
  expect(preview.getAttribute("data-preview-skin")).toBe("night");
  expect(preview.querySelectorAll("[data-keyboard-key]")).toHaveLength(61);
  expect(
    [...preview.querySelectorAll("[data-keyboard-row]")].map((row) => row.children.length),
  ).toEqual([14, 14, 13, 12, 8]);
  expect(preview.querySelectorAll("button, [tabindex], a")).toHaveLength(0);
  for (const row of preview.querySelectorAll("[data-keyboard-row]")) {
    let right = 0;
    for (const rect of row.querySelectorAll("rect")) {
      const x = Number(rect.getAttribute("x"));
      const y = Number(rect.getAttribute("y"));
      const width = Number(rect.getAttribute("width"));
      const height = Number(rect.getAttribute("height"));
      expect(x).toBeGreaterThanOrEqual(right);
      expect(x + width).toBeLessThanOrEqual(1100);
      expect(y + height).toBeLessThanOrEqual(400);
      expect(width).toBeGreaterThan(0);
      right = x + width;
    }
  }
  fireEvent.change(select, { target: { value: "dark" } });
  expect(preview.getAttribute("data-preview-theme")).toBe("dark");
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        screen_keyboard_theme: "dark",
        toolbar_theme: "dark",
        global_theme: "night",
      }),
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("switch", { name: "水杉" }));
  fireEvent.click(screen.getByRole("button", { name: "屏幕键盘" }));
  expect(preview.getAttribute("data-preview-skin")).toBe("shuishan");
  fireEvent.change(select, { target: { value: "follow" } });
  expect(preview.getAttribute("data-preview-theme")).toBe("light");
  await waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(
      8,
      expect.objectContaining({ screen_keyboard_theme: "follow", global_theme: "shuishan" }),
    ),
  );
});

test("Android custom skin editor applies Apple templates, undo, materials and shared selection", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(
    <SettingsPage client={{ load: async () => initial, save, customTouchKeyboardSkins: true }} />,
  );
  await settingsFormReady();
  // 我的皮肤 and its editor sit in the 自定义主题 group of 主题, beside the theme cards it customizes.
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  expect(screen.getAllByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" })).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "设计我的皮肤" }));
  const editor = screen.getByLabelText("自定义皮肤编辑器");
  fireEvent.click(within(editor).getByRole("tab", { name: "设计" }));
  expect(within(editor).getAllByRole("button", { name: /皮肤模板/ })).toHaveLength(15);
  expect(
    within(editor)
      .getAllByRole("button", { name: /皮肤模板/ })[0]
      .getAttribute("aria-label"),
  ).toBe("皮肤模板 薄荷晨光");
  fireEvent.click(within(editor).getByRole("button", { name: "皮肤模板 奶油桃桃" }));
  const preview = within(editor).getByRole("img", { name: "屏幕键盘完整布局预览" });
  expect(preview.getAttribute("data-key-shape")).toBe("pebble");
  expect(preview.getAttribute("data-key-material")).toBe("raised");
  fireEvent.click(within(editor).getByRole("button", { name: "撤销设计" }));
  expect(preview.getAttribute("data-key-shape")).toBe("rounded");
  fireEvent.click(within(editor).getByRole("button", { name: "重做" }));
  expect(preview.getAttribute("data-key-shape")).toBe("pebble");
  fireEvent.click(within(editor).getByRole("button", { name: "皮肤模板 工程蓝图" }));
  expect(preview.getAttribute("data-key-material")).toBe("glass");
  fireEvent.click(within(editor).getByRole("button", { name: "撤销设计" }));
  expect(preview.getAttribute("data-key-shape")).toBe("pebble");
  fireEvent.click(within(editor).getByRole("tab", { name: "按键" }));
  fireEvent.change(within(editor).getByLabelText("键帽不透明度"), { target: { value: ".45" } });
  fireEvent.click(within(editor).getByRole("button", { name: "使用皮肤" }));
  expect(
    screen.getByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" }).getAttribute("aria-checked"),
  ).toBe("true");
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        global_theme: "custom",
        custom_theme: expect.objectContaining({
          keyboard: expect.objectContaining({
            background: 0xffe0d0,
            keyShape: "pebble",
            keyMaterial: "raised",
            keyOpacity: 0.45,
            cornerRadius: 18,
            pattern: 3,
          }),
        }),
      }),
    ),
  );
});

test("choosing the custom keyboard customizes the selected theme instead of reviving a stale package", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  const load = async () => ({
    ...initial,
    preferences: {
      ...initial.preferences,
      global_theme: "night" as const,
      custom_theme: { base: "paper" as const, candidate_skin: "sakura" },
    },
  });
  render(<SettingsPage client={{ load, save, customTouchKeyboardSkins: true }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const card = screen.getByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" });
  expect(card.getAttribute("aria-checked")).toBe("false");
  fireEvent.click(card);
  expect(card.getAttribute("aria-checked")).toBe("true");
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        global_theme: "custom",
        custom_theme: expect.objectContaining({
          base: "night",
          candidate_skin: null,
          keyboard: expect.objectContaining({ background: 0xe8f0eb }),
        }),
      }),
    ),
  );
});

test("a custom theme without a keyboard design does not mark the custom keyboard selected", async () => {
  const load = async () => ({
    ...initial,
    preferences: {
      ...initial.preferences,
      global_theme: "custom" as const,
      custom_theme: {
        base: "ink" as const,
        candidate_colors: { accent: "#2C7A4B" },
      },
    },
  });
  render(<SettingsPage client={{ load, save: vi.fn(), customTouchKeyboardSkins: true }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const card = screen.getByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" });
  expect(card.getAttribute("aria-checked")).toBe("false");
  expect(card.textContent).not.toContain("✓");
});

const savedSkinDesign = (
  patch: Partial<TouchKeyboardSkinDesign> = {},
): TouchKeyboardSkinDesign => ({
  background: 0xe8f0eb,
  keyBackground: 0xffffff,
  keyForeground: 0x17251d,
  accent: 0x185c47,
  actionBackground: 0x185c47,
  cornerRadius: 8,
  borderWidth: 0,
  shadow: 0,
  pattern: 0,
  monospaced: false,
  ...patch,
});

test("Android named skin library loads and completes create, apply, update, rename and delete", async () => {
  let saved: SavedTouchKeyboardSkin[] = [
    {
      id: "11111111-1111-4111-8111-111111111111",
      name: "雾光样例",
      design: savedSkinDesign({ keyShape: "ticket", keyMaterial: "paper" }),
    },
  ];
  const load = vi.fn(async () => saved);
  const mutate = vi.fn(async (action: CustomSkinLibraryAction) => {
    if (action.operation === "create")
      saved = [
        ...saved,
        { id: "22222222-2222-4222-8222-222222222222", name: action.name, design: action.design },
      ];
    if (action.operation === "rename")
      saved = saved.map((item) => (item.id === action.id ? { ...item, name: action.name } : item));
    if (action.operation === "update")
      saved = saved.map((item) =>
        item.id === action.id ? { ...item, design: action.design } : item,
      );
    if (action.operation === "delete") saved = saved.filter((item) => item.id !== action.id);
    return saved;
  });
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        customTouchKeyboardSkins: true,
        customSkinLibrary: { load, mutate },
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("button", { name: "设计我的皮肤" }));
  const editor = screen.getByLabelText("自定义皮肤编辑器");
  await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
  fireEvent.click(within(editor).getByRole("tab", { name: "我的" }));
  await within(editor).findByRole("button", { name: "应用已保存皮肤 雾光样例" });

  fireEvent.click(within(editor).getByRole("button", { name: "应用已保存皮肤 雾光样例" }));
  expect(
    within(editor)
      .getByRole("img", { name: "屏幕键盘完整布局预览" })
      .getAttribute("data-key-shape"),
  ).toBe("ticket");

  fireEvent.click(within(editor).getByRole("tab", { name: "设计" }));
  fireEvent.click(within(editor).getByRole("button", { name: "皮肤模板 奶油桃桃" }));
  fireEvent.click(within(editor).getByRole("tab", { name: "我的" }));
  fireEvent.click(within(editor).getByRole("button", { name: "用当前设计更新 雾光样例" }));
  fireEvent.click(within(editor).getByRole("button", { name: "确认更新" }));
  await waitFor(() =>
    expect(mutate).toHaveBeenCalledWith(
      expect.objectContaining({
        operation: "update",
        id: "11111111-1111-4111-8111-111111111111",
        design: expect.objectContaining({ keyShape: "pebble", keyMaterial: "raised" }),
      }),
    ),
  );

  fireEvent.click(within(editor).getByRole("button", { name: "重命名 雾光样例" }));
  fireEvent.change(within(editor).getByLabelText("皮肤名称"), { target: { value: "桃色样例" } });
  fireEvent.click(within(editor).getByRole("button", { name: "确认重命名" }));
  await within(editor).findByRole("button", { name: "应用已保存皮肤 桃色样例" });
  expect(mutate).toHaveBeenCalledWith({
    operation: "rename",
    id: "11111111-1111-4111-8111-111111111111",
    name: "桃色样例",
  });

  fireEvent.click(within(editor).getByRole("button", { name: "删除 桃色样例" }));
  fireEvent.click(within(editor).getByRole("button", { name: "确认删除" }));
  await waitFor(() =>
    expect(within(editor).queryByRole("button", { name: "应用已保存皮肤 桃色样例" })).toBeNull(),
  );
  expect(mutate).toHaveBeenCalledWith({
    operation: "delete",
    id: "11111111-1111-4111-8111-111111111111",
  });

  fireEvent.click(within(editor).getByRole("button", { name: "保存设计" }));
  fireEvent.change(within(editor).getByLabelText("皮肤名称"), { target: { value: "新建样例" } });
  fireEvent.click(within(editor).getByRole("button", { name: "确认保存" }));
  await within(editor).findByRole("button", { name: "应用已保存皮肤 新建样例" });
  expect(mutate).toHaveBeenCalledWith(
    expect.objectContaining({ operation: "create", name: "新建样例" }),
  );
  expect(
    screen.getByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" }).getAttribute("aria-checked"),
  ).toBe("true");
});

test("Android named skin library reports duplicate names and concurrent twelve-item limits", async () => {
  const mutate = vi
    .fn()
    .mockRejectedValueOnce({ code: "custom_skin_duplicate_name" })
    .mockRejectedValueOnce({ code: "custom_skin_full" });
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        customTouchKeyboardSkins: true,
        customSkinLibrary: { load: async () => [], mutate },
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("button", { name: "设计我的皮肤" }));
  const editor = screen.getByLabelText("自定义皮肤编辑器");
  const saveDesign = within(editor).getByRole("button", { name: "保存设计" }) as HTMLButtonElement;
  await waitFor(() => expect(saveDesign.disabled).toBe(false));
  fireEvent.click(saveDesign);
  fireEvent.change(within(editor).getByLabelText("皮肤名称"), { target: { value: "重复样例" } });
  fireEvent.click(within(editor).getByRole("button", { name: "确认保存" }));
  await within(editor).findByText("已经有同名皮肤，请换一个名称。");
  fireEvent.change(within(editor).getByLabelText("皮肤名称"), { target: { value: "容量样例" } });
  fireEvent.click(within(editor).getByRole("button", { name: "确认保存" }));
  await within(editor).findByText("最多保存 12 套皮肤，请先删除不需要的设计。");
  expect(mutate).toHaveBeenCalledTimes(2);
});

test("Android AI skin draw prepares artwork, saves a proposal and continues editing", async () => {
  const aiSkins = {
    generate: vi.fn().mockResolvedValue(
      [1, 2, 3].map((index) => ({
        name: `AI 测试 ${index}`,
        description: "仅用于界面自动化的合成设计",
        artworkPrompt: "原创背景场景，角色位于边缘，柔和插画，中央安静留白",
        design: savedSkinDesign({
          keyShape: index === 1 ? "rounded" : index === 2 ? "capsule" : "ticket",
          keyMaterial: index === 1 ? "flat" : index === 2 ? "raised" : "paper",
        }),
        artwork: {
          b64_json:
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
          mime_type: "image/png" as const,
          width: 1,
          height: 1,
        },
      })),
    ),
    cancel: vi.fn().mockResolvedValue(undefined),
    onProgress: vi.fn().mockResolvedValue(() => {}),
  };
  const mutate = vi.fn().mockImplementation(async (action: CustomSkinLibraryAction) => [
    {
      id: "33333333-3333-4333-8333-333333333333",
      name: action.operation === "create" ? action.name : "AI 测试 1",
      design: savedSkinDesign(),
    },
  ]);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        customTouchKeyboardSkins: true,
        aiSkins,
        customSkinLibrary: { load: async () => [], mutate },
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("button", { name: "设计我的皮肤" }));
  const editor = screen.getByLabelText("自定义皮肤编辑器");
  await waitFor(() =>
    expect(
      (within(editor).getByRole("button", { name: "AI 皮肤抽卡" }) as HTMLButtonElement).disabled,
    ).toBe(false),
  );
  fireEvent.click(within(editor).getByRole("button", { name: "AI 皮肤抽卡" }));
  const draw = screen.getByRole("button", { name: "抽三张皮肤" });
  act(() => {
    fireEvent.click(draw);
    fireEvent.click(draw);
  });
  await screen.findByRole("heading", { name: "AI 测试 1" });
  fireEvent.click(screen.getAllByRole("button", { name: "保存到我的皮肤" })[0]);
  await screen.findByText("已保存到“我的皮肤”。");
  expect(aiSkins.generate).toHaveBeenCalledTimes(1);
  expect(mutate).toHaveBeenCalledWith(
    expect.objectContaining({
      operation: "create",
      name: "AI 测试 1",
      design: expect.objectContaining({ photo: expect.any(String), keyOpacity: 0.92 }),
    }),
  );
  fireEvent.click(screen.getAllByRole("button", { name: "使用并继续编辑" })[0]);
  expect(screen.queryByRole("dialog", { name: "AI 皮肤抽卡" })).toBeNull();
});

test("AI skin publish ignores a same-tick duplicate submission", async () => {
  const pendingPublish = deferred<void>();
  const publish = vi.fn().mockReturnValue(pendingPublish.promise);
  const aiSkins = {
    generate: vi.fn().mockResolvedValue([
      {
        name: "AI 重复测试",
        description: "合成设计说明",
        artworkPrompt: "原创背景场景，中央留白",
        design: savedSkinDesign(),
        artwork: {
          b64_json:
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
          mime_type: "image/png" as const,
          width: 1,
          height: 1,
        },
      },
    ]),
    cancel: vi.fn().mockResolvedValue(undefined),
  };
  const saved = {
    id: "44444444-4444-4444-8444-444444444444",
    name: "AI 重复测试",
    design: savedSkinDesign(),
  };
  const client = {
    customSkinLibrary: {
      load: vi.fn().mockResolvedValue([]),
      mutate: vi.fn().mockResolvedValue([saved]),
    },
    aiSkins,
    communitySkins: { publish } as never,
  };
  render(
    <SettingsPage
      client={{
        ...client,
        load: async () => initial,
        save: vi.fn(),
        customTouchKeyboardSkins: true,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("button", { name: "设计我的皮肤" }));
  const editor = screen.getByLabelText("自定义皮肤编辑器");
  await waitFor(() =>
    expect(
      (within(editor).getByRole("button", { name: "AI 皮肤抽卡" }) as HTMLButtonElement).disabled,
    ).toBe(false),
  );
  fireEvent.click(within(editor).getByRole("button", { name: "AI 皮肤抽卡" }));
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  await screen.findByRole("heading", { name: "AI 重复测试" });
  fireEvent.click(screen.getByRole("button", { name: "发布到社区" }));
  await screen.findByRole("dialog", { name: "发布 AI 皮肤" });
  fireEvent.click(screen.getByRole("checkbox", { name: /拥有发布所用素材/ }));
  const submit = screen.getByRole("button", { name: "公开发布" });
  act(() => {
    fireEvent.click(submit);
    fireEvent.click(submit);
  });
  expect(publish).toHaveBeenCalledOnce();
  pendingPublish.resolve();
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "发布 AI 皮肤" })).toBeNull());
});

test("AI skin save ignores a same-tick duplicate submission", async () => {
  const pendingMutate = deferred<SavedTouchKeyboardSkin[]>();
  const mutate = vi.fn().mockReturnValue(pendingMutate.promise);
  const aiSkins = {
    generate: vi.fn().mockResolvedValue([
      {
        name: "AI 保存重复测试",
        description: "合成设计说明",
        artworkPrompt: "原创背景场景，中央留白",
        design: savedSkinDesign(),
        artwork: {
          b64_json:
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
          mime_type: "image/png" as const,
          width: 1,
          height: 1,
        },
      },
    ]),
    cancel: vi.fn().mockResolvedValue(undefined),
  };
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        customTouchKeyboardSkins: true,
        aiSkins,
        customSkinLibrary: { load: vi.fn().mockResolvedValue([]), mutate },
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("button", { name: "设计我的皮肤" }));
  const editor = screen.getByLabelText("自定义皮肤编辑器");
  await waitFor(() =>
    expect(
      (within(editor).getByRole("button", { name: "AI 皮肤抽卡" }) as HTMLButtonElement).disabled,
    ).toBe(false),
  );
  fireEvent.click(within(editor).getByRole("button", { name: "AI 皮肤抽卡" }));
  fireEvent.click(screen.getByRole("button", { name: "抽三张皮肤" }));
  await screen.findByRole("heading", { name: "AI 保存重复测试" });
  const save = screen.getByRole("button", { name: "保存到我的皮肤" });
  act(() => {
    fireEvent.click(save);
    fireEvent.click(save);
  });
  await waitFor(() => expect(mutate).toHaveBeenCalled());
  expect(mutate).toHaveBeenCalledOnce();
  pendingMutate.resolve([
    {
      id: "55555555-5555-4555-8555-555555555555",
      name: "AI 保存重复测试",
      design: savedSkinDesign(),
    },
  ]);
  await screen.findByText("已保存到“我的皮肤”。");
});

test("toolbar theme loads, previews independently, saves and reloads", async () => {
  let snapshot: Snapshot = {
    ...initial,
    preferences: {
      ...initial.preferences,
      theme: "dark",
      settings_theme: "dark",
      candidate_theme: "dark",
      toolbar_theme: "light",
    },
  };
  const save = vi.fn().mockImplementation(async (_revision, preferences) => {
    snapshot = { ...snapshot, revision: 8, preferences };
    return snapshot;
  });
  render(<SettingsPage client={{ load: async () => snapshot, save }} />);
  const select = (await screen.findByLabelText("悬浮工具栏主题")) as HTMLSelectElement;
  expect(select.value).toBe("light");
  fireEvent.click(screen.getByRole("button", { name: "悬浮工具栏" }));
  const preview = screen.getByLabelText("悬浮工具栏预览").querySelector("[data-toolbar-preview]")!;
  expect(preview.getAttribute("data-preview-theme")).toBe("light");
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  fireEvent.change(select, { target: { value: "follow" } });
  expect(preview.getAttribute("data-preview-theme")).toBe("dark");
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        toolbar_theme: "follow",
        candidate_theme: "dark",
        settings_theme: "dark",
      }),
    ),
  );
  fireEvent.change(select, { target: { value: "dark" } });
  await waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(8, expect.objectContaining({ toolbar_theme: "dark" })),
  );
});

test("candidate text colour loads, previews, saves and resets to theme", async () => {
  const saved = {
    ...initial,
    preferences: {
      ...initial.preferences,
      global_theme: "custom" as const,
      custom_theme: { candidate_colors: { text: "#123456" } },
    },
  };
  const save = vi
    .fn()
    .mockImplementation(async (_revision, preferences) => ({ ...saved, revision: 8, preferences }));
  render(<SettingsPage client={{ load: async () => saved, save }} />);
  const color = (await screen.findByLabelText("候选文字颜色")) as HTMLInputElement;
  expect(color.value).toBe("#123456");
  // The picker is on 主题 and the full candidate window preview on 候选窗口; the preview element stays mounted while its page is hidden.
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  expect(screen.getByRole("button", { name: "跟随主题" }).getAttribute("aria-pressed")).toBe(
    "false",
  );
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  const preview = screen
    .getByRole("region", { name: "候选窗口预览" })
    .querySelector<HTMLElement>(".appearance-candidate-preview")!;
  expect(preview.style.getPropertyValue("--cand-text")).toBe("#123456");
  fireEvent.change(color, { target: { value: "#abcdef" } });
  expect(preview.style.getPropertyValue("--cand-num")).toBe("#ABCDEF9D");
  expect(save).not.toHaveBeenCalled();
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(7, {
    ...saved.preferences,
    custom_theme: { candidate_colors: { text: "#abcdef" } },
  });
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("button", { name: "跟随主题" }));
  // Dropping the override hands the tokens back to the stylesheet: a custom theme without a package or a picker colour has no candidate slots of its own, so the preview draws the platform defaults.
  expect(preview.getAttribute("data-global-theme")).toBe("custom");
  expect(preview.style.getPropertyValue("--cand-text")).toBe("");
  expect(preview.style.getPropertyValue("--cand-num")).toBe("");
  expect(color.value).toBe("#e9e8e8");
  expect(screen.getByRole("button", { name: "跟随主题" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(8, {
      ...saved.preferences,
      custom_theme: { candidate_colors: { text: null } },
    }),
  );
});

test("a picker used over a built-in theme customizes that theme, and the package can be removed", async () => {
  const saved = {
    ...initial,
    preferences: {
      ...initial.preferences,
      global_theme: "night" as const,
      custom_theme: { candidate_skin: "sample" },
    },
  };
  const save = vi
    .fn()
    .mockImplementation(async (_revision, preferences) => ({ ...saved, revision: 8, preferences }));
  render(<SettingsPage initialPage="appearance" client={{ load: async () => saved, save }} />);
  const color = (await screen.findByLabelText("候选文字颜色")) as HTMLInputElement;
  fireEvent.change(color, { target: { value: "#ff0000" } });
  const preview = screen
    .getByRole("region", { name: "候选窗口预览" })
    .querySelector<HTMLElement>(".appearance-candidate-preview")!;
  // The night palette stays underneath the picked text: a package's own base would have replaced it, so it is dropped.
  expect(preview.getAttribute("data-global-theme")).toBe("custom");
  expect(preview.getAttribute("data-preview-theme")).toBe("dark");
  expect(preview.style.getPropertyValue("--cand-bg")).toBe(themeEntry("night").candidate!.surface);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(7, {
    ...saved.preferences,
    global_theme: "custom",
    custom_theme: { base: "night", candidate_skin: null, candidate_colors: { text: "#ff0000" } },
  });
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const card = screen.getByRole("article", { name: "自定义" });
  expect(within(card).getByRole("switch", { name: "自定义" }).getAttribute("aria-checked")).toBe(
    "true",
  );
  expect(card.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme")).toBe(
    "dark",
  );
});

test("choosing the custom theme card drops its package and keeps the rest of the custom theme", async () => {
  const saved = {
    ...initial,
    preferences: {
      ...initial.preferences,
      global_theme: "paper" as const,
      custom_theme: { candidate_skin: "sample", candidate_colors: { text: "#123456" } },
    },
  };
  const save = vi
    .fn()
    .mockImplementation(async (_revision, preferences) => ({ ...saved, revision: 8, preferences }));
  render(<SettingsPage client={{ load: async () => saved, save }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const card = screen.getByRole("article", { name: "自定义" });
  // The package has its own card in the carousel, so the custom card no longer names it.
  expect(within(card).getByText("外部皮肤、候选颜色与自定义键盘")).not.toBeNull();
  fireEvent.click(within(card).getByRole("switch", { name: "自定义" }));
  expect(within(card).getByRole("switch", { name: "自定义" }).getAttribute("aria-checked")).toBe(
    "true",
  );
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(7, {
    ...saved.preferences,
    global_theme: "custom",
    custom_theme: { candidate_skin: null, candidate_colors: { text: "#123456" } },
  });
});

test("complete candidate and preedit font sizes load, preview independently and save", async () => {
  const saved = {
    ...initial,
    preferences: {
      ...initial.preferences,
      candidate_font_size: 19,
      candidate_preedit_font_size: 27,
    },
  };
  const save = vi
    .fn()
    .mockImplementation(async (_revision, preferences) => ({ ...saved, revision: 8, preferences }));
  render(<SettingsPage initialPage="appearance" client={{ load: async () => saved, save }} />);
  const size = (await screen.findByLabelText("候选窗字号")) as HTMLSelectElement;
  const preedit = screen.getByLabelText("候选窗预编辑字号") as HTMLSelectElement;
  expect(size.value).toBe("19");
  expect(preedit.value).toBe("27");
  expect([...size.options].map((option) => option.value)).toEqual(
    Array.from({ length: 21 }, (_, index) => String(index + 12)),
  );
  expect([...preedit.options].map((option) => option.value)).toEqual(
    [...size.options].map((option) => option.value),
  );
  const preview = screen
    .getByRole("region", { name: "候选窗口预览" })
    .querySelector<HTMLElement>(".appearance-candidate-preview")!;
  // Both ends and a middle value. Every selection runs the same two assignments, and the two
  // assertions above already pin the complete option list, so walking all 21 values only bought
  // 42 further full re-renders of the settings page -- enough to push this past the 20s timeout
  // on its own, which reports as a product failure rather than a slow test.
  for (const value of [12, 22, 32]) {
    fireEvent.change(size, { target: { value: String(value) } });
    fireEvent.change(preedit, { target: { value: String(44 - value) } });
    expect(preview.style.getPropertyValue("--appearance-font-size")).toBe(`${value}px`);
    expect(preview.style.getPropertyValue("--appearance-preedit-font-size")).toBe(
      `${44 - value}px`,
    );
  }
  expect(save).not.toHaveBeenCalled();
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, {
    ...saved.preferences,
    candidate_font_size: 32,
    candidate_preedit_font_size: 12,
  });
});

test("appearance preview follows drafts and skin selection before they are saved", async () => {
  const save = vi.fn();
  render(<SettingsPage initialPage="appearance" client={{ load: async () => initial, save }} />);
  const preview = await screen.findByRole("region", { name: "候选窗口预览" });
  expect(preview.querySelectorAll(".cand")).toHaveLength(5);
  expect(preview.querySelector('[data-preview-layout="vertical"]')).not.toBeNull();
  expect(preview.querySelector('[data-font-size="18"]')).not.toBeNull();
  fireEvent.click(
    within(screen.getByRole("radiogroup", { name: "候选项排列方式" })).getByRole("radio", {
      name: "横向",
    }),
  );
  fireEvent.change(screen.getByLabelText("候选窗字号"), { target: { value: "20" } });
  fireEvent.change(screen.getByLabelText("每页候选项数量"), { target: { value: "9" } });
  // The brand mark leads the top row, ahead of the reading.
  expect(preview.querySelector(".pinyin > .candidate-brand + .text")).not.toBeNull();
  fireEvent.change(screen.getByLabelText("候选窗预编辑"), { target: { value: "empty" } });
  // With the reading hidden the row stays for the mark alone.
  expect(
    preview.querySelector(".container.preedit-hidden > .candidate-brand-row > .candidate-brand"),
  ).not.toBeNull();
  expect(preview.querySelectorAll(".cand")).toHaveLength(9);
  expect(preview.querySelector('[data-preview-layout="horizontal"]')).not.toBeNull();
  expect(preview.querySelector('[data-font-size="20"]')).not.toBeNull();
  expect(preview.querySelector<HTMLElement>(".pinyin")?.hidden).toBe(true);
  expect(
    preview.querySelector(".container.preedit-hidden > .pinyin + .row-wrapper > .first"),
  ).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("switch", { name: /夜青/ }));
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  expect(preview.querySelector('[data-global-theme="night"]')).not.toBeNull();
  // The preview follows the draft at once; the draft itself is written once the edits pause.
  expect(save).not.toHaveBeenCalled();
  await waitFor(() => expect(save).toHaveBeenCalledOnce());
  expect(save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({ candidate_page_size: 9, global_theme: "night" }),
  );
});

test("appearance preview identifies external skins instead of showing a false built-in match", async () => {
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => ({
          ...initial,
          preferences: {
            ...initial.preferences,
            global_theme: "custom",
            custom_theme: { candidate_skin: "external.sample" },
          },
        }),
        save: vi.fn(),
      }}
    />,
  );
  const preview = await screen.findByRole("region", { name: "候选窗口预览" });
  expect(preview.textContent).toContain("当前宿主不支持扫描外部皮肤");
  expect(preview.querySelector(".candidate")).toBeNull();
});

test.each(["quanpin", "shuangpin", "wubi", "japanese"] as const)(
  "appearance preview honors helpcode visibility for %s",
  async (scheme) => {
    render(
      <SettingsPage
        initialPage="appearance"
        client={{
          load: async () => ({
            ...initial,
            preferences: {
              ...initial.preferences,
              scheme,
              quanpin_helpcode: { enabled: false, schema: "ziranma" },
              shuangpin_helpcode: { enabled: true, schema: "ziranma" },
            },
          }),
          save: vi.fn(),
        }}
      />,
    );
    const preview = await screen.findByRole("region", { name: "候选窗口预览" });
    expect(preview.querySelectorAll(".cand-helpcode")).toHaveLength(scheme === "shuangpin" ? 5 : 0);
  },
);

test("touch keyboard geometry mirrors Apple defaults and persists height and spacing", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(<SettingsPage client={{ load: async () => initial, save }} />);
  fireEvent.click(await screen.findByRole("button", { name: "屏幕键盘" }));
  const height = screen.getByRole("slider", { name: "键盘高度" }) as HTMLInputElement;
  const keys = screen.getByRole("slider", { name: "按键间距" }) as HTMLInputElement;
  const rows = screen.getByRole("slider", { name: "行间距" }) as HTMLInputElement;
  expect(height.value).toBe("0");
  expect(keys.value).toBe("60");
  expect(rows.value).toBe("70");
  expect(screen.getByText("0 dp")).toBeDefined();
  expect(screen.getByText("6.0 dp")).toBeDefined();
  expect(screen.getByText("7.0 dp")).toBeDefined();
  const preview = screen.getByRole("img", { name: "屏幕键盘完整布局预览" });
  expect(preview.getAttribute("data-key-spacing")).toBe("6.0");
  expect(preview.getAttribute("data-row-spacing")).toBe("7.0");
  expect(preview.getAttribute("data-keyboard-height")).toBe("400");
  const voice = screen.getByRole("switch", { name: "顶部语音入口" }) as HTMLInputElement;
  expect(voice.checked).toBe(false);
  fireEvent.change(height, { target: { value: "24" } });
  fireEvent.change(keys, { target: { value: "35" } });
  fireEvent.change(rows, { target: { value: "95" } });
  fireEvent.click(voice);
  expect(screen.getByText("+24 dp")).toBeDefined();
  expect(screen.getByText("3.5 dp")).toBeDefined();
  expect(screen.getByText("9.5 dp")).toBeDefined();
  expect(preview.getAttribute("data-key-spacing")).toBe("3.5");
  expect(preview.getAttribute("data-row-spacing")).toBe("9.5");
  expect(preview.getAttribute("data-keyboard-height")).toBe("424");
  const dragSurface = screen.getByLabelText("拖动预览调整键盘间距");
  fireEvent.pointerDown(dragSurface, {
    pointerId: 1,
    pointerType: "touch",
    clientX: 100,
    clientY: 100,
  });
  fireEvent.pointerMove(dragSurface, {
    pointerId: 1,
    pointerType: "touch",
    clientX: 136,
    clientY: 100,
  });
  fireEvent.pointerUp(dragSurface, {
    pointerId: 1,
    pointerType: "touch",
    clientX: 136,
    clientY: 100,
  });
  expect(preview.getAttribute("data-key-spacing")).toBe("5.5");
  fireEvent.pointerDown(dragSurface, {
    pointerId: 2,
    pointerType: "touch",
    clientX: 100,
    clientY: 100,
  });
  fireEvent.pointerMove(dragSurface, {
    pointerId: 2,
    pointerType: "touch",
    clientX: 100,
    clientY: 118,
  });
  fireEvent.pointerUp(dragSurface, {
    pointerId: 2,
    pointerType: "touch",
    clientX: 100,
    clientY: 118,
  });
  expect(preview.getAttribute("data-row-spacing")).toBe("10.0");
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    touch_keyboard_height_adjustment: 24,
    touch_key_spacing_tenths: 55,
    touch_row_spacing_tenths: 100,
    touch_voice_shortcut: true,
  });
});

test("skin preview switches are independent, reversible and do not change saved selection", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  render(<SettingsPage client={{ load: async () => initial, save }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const cards = screen.getAllByRole("article");
  expect(cards).toHaveLength(7);
  const preview = (card: HTMLElement) =>
    card.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme");
  // A built-in theme is one fixed palette, so only the system card and the custom card over the system base offer a light/dark preview.
  const toggled = cards.filter((card) => within(card).queryByRole("button", { name: "预览浅色" }));
  expect(toggled).toEqual([
    screen.getByRole("article", { name: "跟随系统" }),
    screen.getByRole("article", { name: "自定义" }),
  ]);
  for (const [title, appearance] of [
    ["水杉", "dark"],
    ["浅色", "light"],
    ["纸白", "light"],
    ["夜青", "dark"],
    ["墨", "dark"],
  ] as const)
    expect(preview(screen.getByRole("article", { name: title }))).toBe(appearance);
  const system = toggled[0];
  expect(preview(system)).toBe("dark");
  fireEvent.click(within(system).getByRole("button", { name: "预览浅色" }));
  expect(preview(system)).toBe("light");
  expect(screen.getByRole("switch", { name: /跟随系统/ }).getAttribute("aria-checked")).toBe(
    "true",
  );
  expect(preview(screen.getByRole("article", { name: "水杉" }))).toBe("dark");
  fireEvent.click(within(system).getByRole("button", { name: "预览深色" }));
  expect(preview(system)).toBe("dark");
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("switch", { name: /夜青/ }));
  fireEvent.click(within(system).getByRole("button", { name: "预览浅色" }));
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  expect(preview(system)).toBe("light");
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, { ...initial.preferences, global_theme: "night" });
});

test("each skin card includes both six-candidate previews without duplicate IDs", async () => {
  const mounted = render(<SettingsPage client={{ load: async () => initial, save: vi.fn() }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const cards = screen.getAllByRole("article");
  expect(cards).toHaveLength(7);
  for (const card of cards) {
    const previews = card.querySelectorAll("[data-preview-layout]");
    expect(previews).toHaveLength(2);
    expect(card.querySelectorAll(".ftb-preview-host .status-bar")).toHaveLength(1);
    for (const layout of ["horizontal", "vertical"]) {
      const preview = card.querySelector(`[data-preview-layout="${layout}"]`)!;
      expect(preview.querySelectorAll(".row-wrapper")).toHaveLength(6);
      expect(preview.querySelectorAll(".first")).toHaveLength(1);
      expect(preview.querySelector(".pinyin .text")?.textContent).toBe("ni'mf");
      // Theme cards draw the core default, 全拼 with its helper codes hidden.
      expect(preview.querySelectorAll(".cand-helpcode")).toHaveLength(0);
      expect(preview.querySelector(".first .text")?.textContent).toBe("1你们");
      expect(
        Array.from(
          preview.querySelectorAll(layout === "horizontal" ? ".num" : ".cand-no"),
          (node) => node.textContent,
        ),
      ).toEqual(["1", "2", "3", "4", "5", "6"]);
    }
  }
  expect(mounted.container.querySelectorAll("#realContainer")).toHaveLength(0);
  const ids = Array.from(mounted.container.querySelectorAll("[id]"), (element) => element.id);
  expect(new Set(ids).size).toBe(ids.length);
});

test("skin header controls precede previews and always keep one selected skin", async () => {
  const save = vi.fn();
  render(<SettingsPage client={{ load: async () => initial, save }} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const cards = screen.getAllByRole("article");
  for (const card of cards) {
    const header = card.querySelector("[data-skin-card-header]")!;
    expect(header.nextElementSibling).toBe(card.querySelector("[data-skin-preview]"));
    const control = within(card).getByRole("switch");
    expect(control.tagName).toBe("BUTTON");
    expect(header.contains(control)).toBe(true);
    expect(card.querySelectorAll("[data-skin-stage]")).toHaveLength(3);
    fireEvent.click(control);
    fireEvent.click(control);
    expect(control.getAttribute("aria-checked")).toBe("true");
    expect(
      screen.getAllByRole("switch").filter((item) => item.getAttribute("aria-checked") === "true"),
    ).toHaveLength(1);
    // Static samples cannot select a different skin by clicking their labels.
    const other = cards.find((item) => item !== card)!;
    fireEvent.click(other.querySelector("[data-skin-preview]")!);
    expect(control.getAttribute("aria-checked")).toBe("true");
  }
  expect(save).not.toHaveBeenCalled();
});

test("floating toolbar settings use Windows defaults and persist independently", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "悬浮工具栏" }));
  const enabled = (await screen.findByRole("switch", {
    name: "在桌面显示悬浮工具栏",
  })) as HTMLInputElement;
  expect(enabled.checked).toBe(true);
  expect((screen.getByLabelText("工具栏缩放") as HTMLSelectElement).value).toBe("100");
  expect((screen.getByLabelText("图标尺寸") as HTMLSelectElement).value).toBe("24");
  expect((screen.getByRole("checkbox", { name: "英文输入模式" }) as HTMLInputElement).checked).toBe(
    true,
  );
  expect((screen.getByRole("checkbox", { name: "全角 / 半角" }) as HTMLInputElement).checked).toBe(
    true,
  );
  expect((screen.getByRole("checkbox", { name: "屏幕键盘" }) as HTMLInputElement).checked).toBe(
    false,
  );
  fireEvent.change(screen.getByLabelText("工具栏缩放"), { target: { value: "125" } });
  fireEvent.change(screen.getByLabelText("图标尺寸"), { target: { value: "28" } });
  fireEvent.click(screen.getByRole("switch", { name: "在桌面显示悬浮工具栏" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "全角 / 半角" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "屏幕键盘" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "英文输入模式" }));
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    floating_toolbar: {
      enabled: false,
      english_mode: false,
      fullwidth: false,
      punctuation: true,
      character_set: true,
      // Opt-in components, at their defaults: this host offers no handwriting or voice switch, and
      // emoji was not touched here. The default toolbar is the compact one.
      emoji: false,
      handwriting: false,
      screen_keyboard: true,
      voice: false,
      settings: true,
      scale_percent: 125,
      font_size: 28,
    },
  });
});

// The handwriting and voice buttons are this client's own additions to the toolbar and only one host
// draws them. A switch for them anywhere else would turn off something that is not there, and having
// no switch at all - which is how they shipped - leaves two buttons the user cannot remove. Both are
// opt-in now, so the switch a new profile sees is off and the button is not on its toolbar.
test("the toolbar's handwriting and voice switches follow the host that draws them", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
    host: macosHostCapabilities as HostCapabilities,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "悬浮工具栏" }));
  const handwriting = (await screen.findByRole("checkbox", {
    name: "手写识别板",
  })) as HTMLInputElement;
  const voice = screen.getByRole("checkbox", { name: "语音输入" }) as HTMLInputElement;
  // Both are opt-in, so a profile that has not chosen sees them off, and the toolbar it gets is the
  // compact one. Turning one on must move only that one.
  expect(handwriting.checked).toBe(false);
  expect(voice.checked).toBe(false);

  fireEvent.click(handwriting);
  saveSettingsNow();
  await waitFor(() => expect(client.save).toHaveBeenCalled());
  const [, saved] = (client.save as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
    number,
    { floating_toolbar: FloatingToolbarPreferences },
  ];
  expect(saved.floating_toolbar.handwriting).toBe(true);
  expect(saved.floating_toolbar.voice).toBe(false);
});

test("a host without those toolbar buttons is not offered their switches", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: {
          ...macosHostCapabilities,
          floating_toolbar_handwriting: false,
          floating_toolbar_voice: false,
        } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "悬浮工具栏" }));
  // The rest of the component list is still there, so this is about those two and not about the
  // section having failed to render.
  expect(await screen.findByRole("checkbox", { name: "表情与符号" })).toBeTruthy();
  expect(screen.queryByRole("checkbox", { name: "手写识别板" })).toBeNull();
  expect(screen.queryByRole("checkbox", { name: "语音输入" })).toBeNull();
});

test("help, about and feedback pages expose their Windows content and actions", async () => {
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  const copyText = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    openExternalUrl,
    copyText,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(screen.getByRole("button", { name: "帮助" }));
  expect(await screen.findByText("快速上手")).toBeDefined();
  expect(screen.getByText(/Win \+ Space/)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "完整文档（网页）" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledWith("https://msime.app/docs/"));

  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(await screen.findByText("Metasequoia IME")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "开源许可协议" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://github.com/metasequoiaime/msime/blob/develop/LICENSE",
    ),
  );

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  expect(await screen.findByText("GitHub Issues")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "复制群号" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledWith("829919142"));
  fireEvent.click(screen.getByRole("button", { name: "查看 Issues" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith("https://github.com/metasequoiaime/msime/issues"),
  );
});

test("Linux help quick start covers both Fcitx5 and IBus", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    host: { platform: "linux" } as never,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(screen.getByRole("button", { name: "帮助" }));
  const intro = await screen.findByText(/Linux 桌面环境下的中文输入法/);
  expect(intro.textContent).toContain("Fcitx5");
  expect(intro.textContent).toContain("IBus");
  const quickStart = screen.getByText(/fcitx5-configtool/);
  // First-run setup adds the input method on its own; the manual steps are the fallback.
  expect(quickStart.textContent).toContain("自动加入正在运行的 Fcitx5 或 IBus 的输入法列表");
  expect(quickStart.textContent).toContain(
    "「水杉输入法」（英文界面显示为「MSIME」）加入当前输入法组",
  );
  // The name IBus lists is the component's longname.
  expect(quickStart.textContent).toContain("「Metasequoia 水杉输入法」");
  expect(quickStart.textContent).not.toContain("MSIME Client");
  expect(screen.queryByText(/Win \+ Space/)).toBeNull();
});

test("Linux help network section says what goes online and where credentials live", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    host: { platform: "linux" } as never,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(screen.getByRole("button", { name: "帮助" }));
  const network = await screen.findByText(/日常拼音输入无需联网/);
  const text = network.textContent ?? "";
  // Cloud candidates are on after first-run setup unless declined, and they send the spelling being typed.
  expect(text).toContain("云候选默认开启");
  expect(text).toContain("Google input-tools");
  expect(text).toContain("msime-linux-online-provider");
  expect(text).toContain("msime-linux-voice-provider");
  // Fresh Linux installs translate candidates through the MSIME account, so the copy says what it sends and how to switch away; voice and AI still wait for a configured service.
  expect(text).toContain("候选词翻译默认用水杉账号，会把当前页的中文候选词发送到 api.msime.app");
  expect(text).toContain("可在翻译服务里改选自己的服务或不使用在线翻译");
  expect(text).toContain("语音识别和 AI 功能只在启用并配置好对应服务后联网");
  expect(text).not.toContain("填好凭据后联网");
  // The provider credentials are private files; NiuTrans and custom translation keys are the exception and the copy says so.
  expect(text).toContain("ai-provider.json、tencent-provider.json 和 voice-provider.json");
  expect(text).toContain("不进入共享设置");
  expect(text).toContain("小牛翻译和自定义翻译服务的密钥则保存在共享设置中");
  expect(text).not.toContain("不保存或转发 provider 的凭据");
});

test("Android help and about pages use mobile instructions and project links", async () => {
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    openExternalUrl,
    host: { platform: "android", floating_toolbar: false } as never,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(screen.getByRole("button", { name: "帮助" }));
  expect(await screen.findByText(/Android 平台的中文输入法/)).toBeDefined();
  expect(screen.getByText(/语言和输入法/)).toBeDefined();
  expect(screen.queryByText(/Win \+ Space/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "完整文档（网页）" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledWith("https://msime.app/docs/"));

  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(await screen.findByText(/Android 触屏输入体验/)).toBeDefined();
  expect(screen.queryByRole("button", { name: "悬浮工具栏" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "开源许可协议" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://github.com/metasequoiaime/msime/blob/develop/LICENSE",
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "隐私政策" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledWith("https://msime.app/privacy/"));

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(screen.getByRole("button", { name: "查看 Issues" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith("https://github.com/metasequoiaime/msime/issues"),
  );
});

// The Linux section of msime.app/privacy/ does not match this host (it has an update check and keeps provider credentials in 0600 files), so Linux opens the PRIVACY.md that ships with this code, as the Windows reference opens its own. Every other host keeps msime.app/privacy/, which a looser Linux check would break.
test("the privacy link opens PRIVACY.md on Linux and msime.app/privacy/ elsewhere", async () => {
  const expected: Record<string, string> = {
    linux: "https://github.com/metasequoiaime/msime/blob/develop/PRIVACY.md",
    windows: "https://msime.app/privacy/",
    macos: "https://msime.app/privacy/",
    android: "https://msime.app/privacy/",
    harmony: "https://msime.app/privacy/",
    ios: "https://msime.app/privacy/",
  };
  for (const [platform, url] of Object.entries(expected)) {
    const openExternalUrl = vi.fn().mockResolvedValue(undefined);
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          openExternalUrl,
          host: { platform } as HostCapabilities,
        }}
      />,
    );
    await settingsReady();
    fireEvent.click(screen.getByRole("button", { name: "关于" }));
    fireEvent.click(await screen.findByRole("button", { name: "隐私政策" }));
    await waitFor(() => expect(openExternalUrl).toHaveBeenCalledWith(url));
    expect(openExternalUrl).toHaveBeenCalledTimes(1);
    cleanup();
  }
});

test("iOS help opens keyboard settings and feedback builds a visible report", async () => {
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  const openSystemKeyboardSettings = vi.fn().mockResolvedValue(undefined);
  const copyText = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        openExternalUrl,
        openSystemKeyboardSettings,
        copyText,
        host: { platform: "ios" } as HostCapabilities,
      }}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  // 帮助 is an entry on the 反馈 page, which is only drawn once the settings have loaded.
  fireEvent.click(await screen.findByRole("button", { name: "帮助" }));
  expect(await screen.findByText("允许完全访问")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "打开系统键盘设置" }));
  await waitFor(() => expect(openSystemKeyboardSettings).toHaveBeenCalledOnce());

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.change(screen.getByRole("combobox", { name: "反馈类型" }), {
    target: { value: "候选词不对" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "反馈描述" }), {
    target: { value: "synthetic repro" },
  });
  fireEvent.click(screen.getByRole("button", { name: "复制报告" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledWith(expect.stringContaining("候选词不对")));
  fireEvent.click(screen.getByRole("button", { name: "在 GitHub 提交" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(expect.stringContaining("/issues/new?")),
  );
  expect(new URL(openExternalUrl.mock.calls.at(-1)?.[0] ?? "").searchParams.get("body")).toContain(
    "synthetic repro",
  );
});

test("macOS support pages use client project and privacy links", async () => {
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  const openThirdPartyLicenses = vi.fn().mockResolvedValue(undefined);
  const copyText = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        openExternalUrl,
        openThirdPartyLicenses,
        copyText,
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  await screen.findByText("Metasequoia IME");
  fireEvent.click(screen.getByRole("button", { name: "开源许可协议" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://github.com/metasequoiaime/msime/blob/develop/LICENSE",
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "隐私政策" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledWith("https://msime.app/privacy/"));
  fireEvent.click(screen.getByRole("button", { name: "查看许可全文" }));
  await waitFor(() => expect(openThirdPartyLicenses).toHaveBeenCalledOnce());

  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.change(screen.getByRole("combobox", { name: "反馈类型" }), {
    target: { value: "候选词不对" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "反馈描述" }), {
    target: { value: "synthetic macOS repro" },
  });
  fireEvent.click(screen.getByRole("button", { name: "复制报告" }));
  await waitFor(() =>
    expect(copyText).toHaveBeenCalledWith(expect.stringContaining("synthetic macOS repro")),
  );
  fireEvent.click(screen.getByRole("button", { name: "在 GitHub 提交" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(expect.stringContaining("/issues/new?")),
  );
  fireEvent.click(screen.getByRole("button", { name: "查看 Issues" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith("https://github.com/metasequoiaime/msime/issues"),
  );
});

test("macOS and iOS help pages use their native host instructions", async () => {
  const macos = render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  // 帮助 is an entry on the 反馈 page, which is only drawn once the settings have loaded.
  fireEvent.click(await screen.findByRole("button", { name: "帮助" }));
  // macOS answers the three questions as term/description rows, not prose, so the page carries the
  // reference window's own terms rather than the shared platform intro.
  expect(await screen.findByRole("group", { name: "开始输入" })).toBeDefined();
  expect(screen.getByRole("group", { name: "候选词释义" })).toBeDefined();
  expect(screen.getByRole("group", { name: "遇到问题" })).toBeDefined();
  expect(screen.getByText("数字键 1–9")).toBeDefined();
  expect(screen.getByText("Option / Control + 数字")).toBeDefined();
  expect(screen.getByText(/Shift\+Tab 反向/)).toBeDefined();
  expect(screen.getByText(/系统设置 › 键盘 › 文字输入 › 输入法/)).toBeDefined();
  expect(screen.queryByText(/macOS 平台的中文输入法/)).toBeNull();
  expect(screen.queryByText(/Win \+ Space/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(await screen.findByText(/现代 macOS 桌面体验/)).toBeDefined();
  macos.unmount();

  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "ios" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  // 帮助 is an entry on the 反馈 page, which is only drawn once the settings have loaded.
  fireEvent.click(await screen.findByRole("button", { name: "帮助" }));
  expect(await screen.findByText(/iOS 平台的中文输入法/)).toBeDefined();
  expect(screen.getByText(/应用的输入源按钮/)).toBeDefined();
  expect(screen.getByText(/键盘扩展的日常拼音输入无需联网/)).toBeDefined();
  expect(screen.queryByText(/Win \+ Space/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(await screen.findByText(/iPhone 与 iPad 触屏输入体验/)).toBeDefined();
});

// 设置导航（`settingsNavGroups`）：六组，按顺序排列，每组带组名。宿主不提供的页从所在组里消失而不留空位，子页从父页里进入。
test("the sidebar follows the six titled navigation groups", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "windows", floating_toolbar: true } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  const groups = [...sidebar.querySelectorAll("[data-sidebar-section]")].map((section) =>
    [...section.querySelectorAll("button")].map((item) => item.textContent ?? ""),
  );
  expect(groups).toEqual([
    ["输入", "标点与翻译", "快捷键", "词库"],
    ["主题", "候选窗口", "悬浮工具栏"],
    ["屏幕键盘", "语音输入", "手写输入"],
    ["剪贴板"],
    ["维护与诊断", "帮助与反馈", "关于"],
  ]);
  // 组名对辅助技术可见；只剩一项的「工具」组也能看出它属于哪一类。
  expect(
    within(sidebar)
      .getAllByRole("group")
      .map((group) => group.getAttribute("aria-label")),
  ).toEqual(["打字", "外观", "更多输入方式", "工具", "支持"]);
  for (const title of ["AI 辅助", "AI 对话", "背单词", "帮助", "辅助码", "其他平台下载"]) {
    expect(groups.flat()).not.toContain(title);
  }
});

test("macOS shortcut page owns the full-width chord and the input page the mode HUD", async () => {
  const save = vi.fn().mockResolvedValue({ ...initial, revision: 8 });
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save,
        host: { platform: "macos", mode_switch_shortcuts: true } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  // The chords are named for the keys a Mac keyboard actually has.
  expect(await screen.findByText("单击 Control 切换中英文")).toBeDefined();
  expect(screen.getByText("Control+Option+Space 切换中英文")).toBeDefined();
  // 中英文切换提示不在快捷键页重复出现。
  expect(screen.queryByRole("switch", { name: "切换中英文时显示提示" })).toBeNull();
  const fullWidth = screen.getByRole("switch", { name: "Option+Shift+H 切换全半角" });
  expect((fullWidth as HTMLInputElement).checked).toBe(true);
  fireEvent.click(fullWidth);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({
      keybindings: expect.objectContaining({ toggle_fullwidth_option_shift_h: false }),
    }),
  );
  // 它和其他平台一样在输入页「中英文」组。
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const hud = within(screen.getByRole("region", { name: "中英文" })).getByRole("switch", {
    name: "中英文切换提示",
  });
  expect((hud as HTMLInputElement).checked).toBe(true);
});

test("the feedback report leads with the release and the scheme", async () => {
  const copyText = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        copyText,
        host: { platform: "macos", os_version: "27.0" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(await screen.findByRole("button", { name: "复制报告" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledOnce());
  const report = copyText.mock.calls[0][0] as string;
  expect(report).toContain("macOS 27.0");
  expect(report).toContain("输入方案：全拼");
  // The user agent only names the web view; with a real release to report it is noise.
  expect(report).not.toContain("User-Agent");
});

test("a host that cannot name its release still reports something", async () => {
  const copyText = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        copyText,
        host: { platform: "linux" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(await screen.findByRole("button", { name: "复制报告" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledOnce());
  const report = copyText.mock.calls[0][0] as string;
  expect(report).toContain("平台：linux");
  expect(report).toContain("User-Agent");
});

test("the full-width chord row is macOS only", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "windows", mode_switch_shortcuts: true } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(await screen.findByText("单击 Ctrl 切换中英文")).toBeDefined();
  expect(screen.queryByRole("switch", { name: "Option+Shift+H 切换全半角" })).toBeNull();
});

test("restore defaults applies the host's defaults and saves them", async () => {
  const save = vi.fn().mockResolvedValue({ ...initial, revision: 8 });
  // What the host hands back: settings at their defaults, the key it was told to keep still there.
  const restored = {
    ...initial.preferences,
    candidate_page_size: 9,
    voice_input: { ...initial.preferences.voice_input, asr_token: "kept-by-the-host" },
  };
  const loadDefaultPreferences = vi.fn().mockResolvedValue(restored);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue({
          ...initial,
          preferences: { ...initial.preferences, candidate_page_size: 5 },
        }),
        save,
        loadDefaultPreferences,
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "恢复默认设置" }));
  await answerConfirm("confirm");
  // Restoring takes effect at once: the defaults become the draft, and the draft saves itself.
  await screen.findByText("所有设置已恢复默认。");
  expect(loadDefaultPreferences).toHaveBeenCalledOnce();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, expect.objectContaining({ candidate_page_size: 9 }));
  expect(save).toHaveBeenCalledWith(
    7,
    expect.objectContaining({
      voice_input: expect.objectContaining({ asr_token: "kept-by-the-host" }),
    }),
  );
});

test("restore defaults declined leaves the draft alone", async () => {
  const loadDefaultPreferences = vi.fn();
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        loadDefaultPreferences,
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "恢复默认设置" }));
  await answerConfirm("cancel");
  expect(loadDefaultPreferences).not.toHaveBeenCalled();
});

test("restore defaults sits in the footer of preference pages only", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        loadDefaultPreferences: vi.fn(),
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  expect(screen.getByRole("button", { name: "恢复默认设置" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(await screen.findByRole("group", { name: "关于" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "恢复默认设置" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  expect(await screen.findByRole("button", { name: "恢复默认设置" })).toBeTruthy();
});

test("a host without the defaults command shows no restore button", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "windows" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  expect(screen.queryByRole("button", { name: "恢复默认设置" })).toBeNull();
});

// The reference window's 外观 page, in its order. Only the sections it also has are pinned, and only their relative order, so a host that hides a section (no candidate font control) does not fail this. Its colour and theme sections are on the 主题 page now, as the design groups them.
// 输入页不再沿用参考窗口的顺序，改按「基础 → 进阶」排（见下面的输入页顺序用例），规则和外观页那条相同：只钉列出的那些节，只钉它们的相对顺序。列表之外的设置（全角输入、整句联想、英文建议等）可以在所属组里自由移动，不用改列表。
// A section title is the element's own text plus a nested <small> description, so read only the
// direct text nodes: "中文标点" has to stay distinguishable from "中文标点后按空格转换".
// A row built from the platform primitives marks its name with `data-row-title` instead, and a group of them its own with `data-group-title`: the reference's 拼音方案调频 is a section of three settings, which is a group here.
const sectionTitles = (scope: HTMLElement) =>
  [...scope.querySelectorAll(".section-title, [data-row-title], [data-group-title]")].map((node) =>
    [...node.childNodes]
      .filter((child) => child.nodeType === 3)
      .map((child) => child.textContent ?? "")
      .join("")
      .replace(/\s+/g, " ")
      .trim(),
  );

/**
 * The reference window's settings sections, page by page, as verified against its partials in
 * `ui-html/webview2/settings/ime-settings/src/partials`.
 *
 * Four slices of this comparison were done by reading both trees, and each one cost time to
 * re-derive because a naive search under-reports both sides: the reference combines class names
 * (`class="section-title ai-heading"`) and this UI renders many titles from an expression rather
 * than literal text. This table is that work, written down so it is checked rather than repeated,
 * and so dropping or renaming one of these sections fails here instead of silently diverging.
 *
 * Where a name differs deliberately it is recorded with the reason, not silently omitted.
 */
const referenceSections: {
  page: string;
  button: string;
  // 子页从父页里进入；与别的组共用一页的组（输入页上的辅助码）按组名找它的 region。
  via?: string;
  group?: string;
  titles: string[];
}[] = [
  {
    page: "appearance",
    button: "候选窗口",
    titles: [
      "候选窗口跟随光标",
      "候选窗主字体",
      "候选窗字号",
      "候选窗预编辑字号",
      "每页候选项数量",
      "候选项排列方式",
      "行内预编辑",
      "候选窗预编辑",
    ],
  },
  {
    page: "input",
    button: "输入",
    titles: [
      "输入模式",
      "输入方案",
      "双拼方案",
      "五笔方案",
      "日语方案",
      "以词定字",
      // 参考窗口也把翻页放在输入页；这里放在「选词与翻页」组，和与它互斥的以词定字同屏。
      "翻页方式",
      "默认中英文",
      "中英文状态",
      "繁体输出",
      "云候选",
      "拼音方案调频",
      // The reference's 实用功能 modes; the design keeps them with the other ways of typing.
      "快捷短语(K 模式)",
      "日期与时间快捷输入(T 模式)",
      "Unicode 便捷录入(U 模式)",
      "Emoji 快捷输入(E 模式)",
      "颜文字快捷输入(M 模式)",
      "超级简拼(J 模式)",
      "临时英文(Y 模式)",
      "临时日语(R 模式)",
    ],
  },
  {
    // The reference's 输入 sections that shape what is written rather than how it is typed.
    page: "expression",
    button: "标点与翻译",
    titles: [
      "候选词翻译",
      // 中文标点 is the reference's 始终使用英文标点 with the opposite polarity on the same `chinese_punctuation` preference, so the reference's wording would mislabel the toggle.
      "中文标点",
      "智能标点",
      "重复标点转中文",
      "成对标点自动补全",
      "固定标点",
      "中英混输",
    ],
  },
  {
    page: "helpcode",
    button: "输入",
    group: "辅助码",
    titles: [
      "双拼辅助码",
      "双拼辅助码方案",
      "全拼辅助码",
      "全拼辅助码方案",
      // The display rows name their own scheme, as the reference does. Desktop says 候选窗口,
      // the touch hosts say 候选栏, which is the surface they actually have.
      "在候选窗口中显示双拼辅助码",
      "在候选窗口中显示全拼辅助码",
    ],
  },
  {
    page: "tools",
    button: "剪贴板",
    titles: ["剪贴板管理"],
  },
  {
    page: "floating-toolbar",
    button: "悬浮工具栏",
    titles: ["在桌面显示悬浮工具栏", "工具栏缩放", "图标尺寸", "工具栏组件"],
  },
  {
    page: "screen-keyboard",
    button: "屏幕键盘",
    titles: ["打开屏幕键盘"],
  },
  {
    page: "handwriting",
    button: "手写输入",
    titles: ["打开手写识别板"],
  },
  {
    page: "voice",
    button: "语音输入",
    titles: [
      // 来源的「基础设置」与「启用语音输入」在这里合成一节。
      "语音输入",
      // 来源：ASR API。
      "识别服务",
      "豆包识别选项",
      // 来源：文本润色 API。
      "文本润色 provider",
      "录音时静音其他声音",
      // 来源：语音输入快捷键。三个长按组合的键名按平台改写（Option/Command 对 Alt/Win），
      // 所以这里只钉小节本身，键名由各自平台的用例覆盖。
      "语音快捷键",
    ],
  },
  {
    page: "skin",
    button: "主题",
    titles: [
      // 来源把四套内置皮肤各自做成一节；这里是一个皮肤选择器，四套在同一个卡片列表里，
      // 所以只有「外部皮肤」是小节。皮肤本身四套都在（CandidateSkin.cpp 与皮肤页的名字）。
      "外部皮肤",
      // The reference's 外观 sections that pick colours rather than lay out the window; the design gathers them on 主题. 主题模式 is 颜色模式 here, beside the theme cards it would otherwise read as a second picker of.
      "颜色模式",
      "候选文字颜色",
      "设置界面主题",
      "候选窗口主题",
      "悬浮工具栏主题",
      "菜单主题",
      "表情面板主题",
      "手写识别板主题",
      "语音输入弹出条主题",
    ],
  },
  {
    page: "dictionary",
    button: "词库",
    titles: [
      // 来源分成「批量导入纯汉字词组」与「导出词库」两节；这里是一节带类别选择的词库管理，
      // 查询、新增、编辑、导入、导出都在其中，导入说明里写明支持纯汉字自动注音。
      "本地词库管理",
    ],
  },
  {
    page: "ai",
    button: "AI 辅助",
    via: "标点与翻译",
    titles: [
      // 来源：启用 AI 联想。这里的开关还管 iOS 键盘的 AI 回复与 Android 的选中文字润色，
      // 所以名字不按来源收窄到候选联想。
      "启用 AI 辅助",
      // 来源把这些放在「API 配置」一节里，这里各自成行。
      "服务提供商",
      "模型",
      "接口地址",
      "API Token",
    ],
  },
  {
    page: "shortcuts",
    button: "快捷键",
    titles: [
      // 来源：中英文切换。简繁切换在来源是并列的一节，这里是这一节里的一行，且键名按平台
      // 改写（Control 对 Ctrl），所以不在这里钉。
      "输入模式切换",
      "候选操作",
      // 来源：全局快捷键。
      "面板快捷键",
    ],
  },
  {
    // The reference keeps its logs on 关于; the design moves them to 维护与诊断.
    page: "developer",
    button: "维护与诊断",
    titles: [
      // 来源另有「TSF 端日志」，那是 Windows 的 TIP 进程，按平台门控。
      "Server 端日志",
    ],
  },
  {
    page: "help",
    button: "帮助",
    via: "帮助与反馈",
    titles: ["快速上手", "基本功能"],
  },
];

test.each(referenceSections)(
  "the $page page still carries the reference window's sections",
  async ({ button, via, group, titles }) => {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          // The dictionary and skin pages render their sections only once the host offers them
          // anything to show; without these two the pages would be empty and the table would be
          // asserting nothing about them.
          dictionary: {
            list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
            edit: vi.fn(),
          },
          scanSkinCatalog: vi.fn().mockResolvedValue({ directory: "", packages: [], issues: [] }),
          // The capabilities `host_surface.rs` gives the Windows host, since these are the
          // reference's own sections: several of them are behind a capability and a bare fixture
          // would assert they are missing when the host simply never declared it.
          host: {
            platform: "windows",
            floating_toolbar: true,
            floating_toolbar_components: true,
            floating_toolbar_appearance: true,
            candidate_font_controls: true,
            candidate_follow_cursor: true,
            ime_mode_scope: true,
            // `for_platform(Windows)` declares both, and the shortcut page's sections are behind
            // them; without these the table would pass by asserting a page that rendered nothing.
            mode_switch_shortcuts: true,
            panel_shortcuts: true,
          } as HostCapabilities,
        }}
      />,
    );
    await settingsReady();
    if (via) fireEvent.click(screen.getByRole("button", { name: via }));
    fireEvent.click(screen.getByRole("button", { name: button }));
    const page = group
      ? await screen.findByRole("region", { name: group })
      : await screen.findByRole("group", { name: button });
    const present = sectionTitles(page);
    expect(titles.filter((title) => !present.includes(title))).toEqual([]);
  },
);

/**
 * The same sections, asked of the macOS host.
 *
 * The table above renders with Windows' capabilities because the sections are the reference
 * window's. That says nothing about the host this client is migrating them to: a section behind a
 * capability macOS does not declare, or behind a platform name, would be missing there and the
 * assertion above would still pass. This is the same table against `for_platform(Macos)`.
 *
 * A section macOS legitimately does not show is named here with the reason, and an entry that stops
 * being needed fails, so the list cannot outlive what it explains.
 */
/** What `host_surface.rs` answers for HostPlatform::Macos, field for field. */
const macosHostCapabilities = {
  platform: "macos",
  restart_input_method: true,
  panel_windows: true,
  ime_mode_scope: true,
  typing_statistics: true,
  fuzzy_pinyin: true,
  system_fonts: true,
  window_chrome: true,
  floating_toolbar: true,
  floating_toolbar_appearance: true,
  floating_toolbar_components: true,
  // Only this host's toolbar carries these two buttons, so only here are their switches offered.
  floating_toolbar_handwriting: true,
  floating_toolbar_voice: true,
  mode_switch_shortcuts: true,
  panel_shortcuts: true,
  number_row_selection: false,
  voice_capture_devices: true,
  candidate_font_controls: true,
  candidate_row_colors: true,
  candidate_selection_appearance: true,
  candidate_border_color: true,
  candidate_follow_cursor: true,
  input_mode_hud: true,
  voice_commit_mode: true,
  shuangpin_preedit: true,
  candidate_english_font: true,
} as HostCapabilities;

const macosAbsentSections: Record<string, string> = {
  // The Windows font row carries this note; macOS applies the family without a restart, and its
  // font controls are the shared three (main, supplementary, English) rather than Windows' pair.
  保存后自动应用: "a Windows-only note on its own font row",
  // The handwriting panel needs the input method process's IMK session, and the settings window is
  // a different process. macOS shows a card saying to open it from the toolbar or the input menu
  // instead of a button that could not work from here.
  打开手写识别板: "the panel is opened from the input method, not from this window",
  // The reference answers a new user's questions as prose. macOS answers the same ones as term and
  // description rows (`macosHelpCards`), because the three that actually come up there - how to
  // switch to English, how to select a candidate, why the input source is missing from the menu -
  // want to be findable rather than read through.
  快速上手: "macOS answers the same questions as help cards",
  // macOS runs no Server process; the same switch is 输入法日志 there, named for the focus and
  // preference events it actually records.
  "Server 端日志": "macOS names this 输入法日志, having no Server process",
  基本功能: "macOS answers the same questions as help cards",
};

test.each(referenceSections)(
  "the $page page carries the reference window's sections on macOS too",
  async ({ button, via, group, titles }) => {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          // What `host_surface.rs` gives HostPlatform::Macos, so a missing section is a difference
          // in the page rather than a capability the fixture forgot to declare.
          host: macosHostCapabilities,
          dictionary: {
            list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
            edit: vi.fn(),
          },
          scanSkinCatalog: vi.fn().mockResolvedValue({ directory: "", packages: [], issues: [] }),
        }}
      />,
    );
    await settingsReady();
    if (via) fireEvent.click(screen.getByRole("button", { name: via }));
    fireEvent.click(screen.getByRole("button", { name: button }));
    const page = group
      ? await screen.findByRole("region", { name: group })
      : await screen.findByRole("group", { name: button });
    const present = sectionTitles(page);
    const wanted = titles.filter((title) => !(title in macosAbsentSections));
    expect(wanted.filter((title) => !present.includes(title))).toEqual([]);
    for (const [title, reason] of Object.entries(macosAbsentSections)) {
      if (!titles.includes(title)) continue;
      expect(present.includes(title), `${title} is shown on macOS after all: ${reason}`).toBe(
        false,
      );
    }
  },
);

/**
 * The reference's 反馈 page, which the section table above cannot reach.
 *
 * Its three channels are cards rather than sections - the reference draws them that way and so does
 * this client - so `.section-title` finds none of them and the page would sit outside the table
 * unnoticed. The channels are the whole point of that page: an address that quietly disappears is a
 * user who cannot report anything.
 *
 * Asked of both hosts, because the page is behind no capability and a platform branch that hid it
 * on macOS would otherwise pass here.
 */
test.each([
  ["windows", { platform: "windows" } as HostCapabilities],
  ["macos", macosHostCapabilities as HostCapabilities],
])("the feedback page keeps the reference's channels on %s", async (_name, host) => {
  render(
    <SettingsPage client={{ load: vi.fn().mockResolvedValue(initial), save: vi.fn(), host }} />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  const page = await screen.findByRole("group", { name: "帮助与反馈" });
  for (const channel of ["GitHub Issues", "QQ 交流群", "Telegram 群组"]) {
    expect(within(page).getByText(channel)).toBeTruthy();
  }
  // The addresses themselves, not just the headings: a card with the wrong group number is worse
  // than no card.
  expect(within(page).getByText("群号：829919142")).toBeTruthy();
  expect(within(page).getByText("t.me/msimegroup")).toBeTruthy();
  // The reference closes the page by saying what to attach to a report.
  expect(within(page).getByText("提交问题时建议附上")).toBeTruthy();
});

/**
 * The option lists the reference window offers for a given control, value and label both.
 *
 * Section titles are pinned above; these are the choices inside them, which drifted separately --
 * 候选项排列方式 read 竖排/横排 where the reference names the axis, and 候选窗预编辑 read
 * 显示拼音/隐藏 for the same pinyin/empty pair that 行内预编辑 right above it already called
 * 拼音分词/不显示.
 */
const referenceOptions: {
  page: string;
  button: string;
  control: string;
  // A choice drawn as a segmented control is a radio group rather than a select.
  role?: "radiogroup";
  options: string[];
}[] = [
  {
    // A segmented control in the design's 窗口布局 group, with the reference's two choices.
    page: "appearance",
    button: "候选窗口",
    control: "候选项排列方式",
    role: "radiogroup",
    options: ["横向", "纵向"],
  },
  {
    page: "appearance",
    button: "候选窗口",
    control: "行内预编辑",
    options: ["原始按键", "拼音分词", "不显示"],
  },
  {
    page: "appearance",
    button: "候选窗口",
    control: "候选窗预编辑",
    options: ["拼音分词", "不显示"],
  },
  {
    page: "appearance",
    button: "候选窗口",
    control: "候选窗字号",
    options: Array.from({ length: 21 }, (_, index) => String(index + 12)),
  },
  {
    page: "appearance",
    button: "候选窗口",
    control: "候选窗预编辑字号",
    options: Array.from({ length: 21 }, (_, index) => String(index + 12)),
  },
  {
    // The reference's 主题模式 select, a segmented control named 颜色模式 on the design's 主题 page: next to a picker of themes, 主题模式 read as a second one. The choices are the reference's, in the design's order.
    page: "skin",
    button: "主题",
    control: "颜色模式",
    role: "radiogroup",
    options: ["跟随系统", "浅色", "深色"],
  },
  {
    page: "skin",
    button: "主题",
    control: "设置界面主题",
    options: ["跟随全局", "深色", "浅色"],
  },
  {
    // This one read 跟随 where every other surface theme - and the reference - says 跟随全局.
    page: "skin",
    button: "主题",
    control: "候选窗口主题",
    options: ["跟随全局", "深色", "浅色"],
  },
  {
    page: "input",
    button: "输入",
    control: "双拼方案",
    options: ["小鹤双拼", "自然码双拼", "首道双拼", "微软双拼"],
  },
  {
    page: "expression",
    button: "标点与翻译",
    control: "触发字符数",
    options: ["1", "2", "3", "4", "5", "6", "7", "8"],
  },
  {
    page: "input",
    button: "输入",
    control: "调频方式",
    options: ["关闭", "一次置顶", "折半调频", "线性调频", "一次置前"],
  },
  {
    page: "input",
    button: "输入",
    control: "触发频次(第几次上屏触发)",
    options: ["1", "2", "3", "4", "5", "6"],
  },
  {
    page: "input",
    button: "输入",
    control: "线性调频步长",
    options: ["1", "2", "3", "4", "5", "6"],
  },
  {
    page: "helpcode",
    button: "输入",
    control: "双拼辅助码方案",
    options: ["蓝天小雨点", "自然码", "首右2.0", "首右plus", "小鹤", "加加"],
  },
  {
    page: "helpcode",
    button: "输入",
    control: "全拼辅助码方案",
    options: ["蓝天小雨点", "自然码", "首右2.0", "首右plus", "小鹤", "加加"],
  },
  {
    page: "floating-toolbar",
    button: "悬浮工具栏",
    control: "工具栏缩放",
    options: ["75%", "100%", "125%", "150%"],
  },
  {
    page: "floating-toolbar",
    button: "悬浮工具栏",
    control: "图标尺寸",
    options: ["16", "18", "20", "22", "24", "26", "28"],
  },
  {
    // The four built-in prompts, named as the reference names them in the same table the prompts
    // themselves were copied from. The three custom slots are this client's.
    page: "voice",
    button: "语音输入",
    control: "润色方案",
    options: ["精炼整理", "忠实校对", "中翻英", "口语整理", "自定义一", "自定义二", "自定义三"],
  },
  {
    page: "expression",
    button: "标点与翻译",
    control: "固定标点",
    options: ["跟随中英文状态", "始终使用中文标点", "始终使用英文标点"],
  },
  {
    page: "input",
    button: "输入",
    control: "默认中英文",
    options: ["中文", "英文"],
  },
  {
    page: "input",
    button: "输入",
    control: "中英文状态",
    options: ["按应用记忆", "全局统一"],
  },
];

const optionHosts: [string, HostCapabilities][] = [
  [
    "windows",
    {
      platform: "windows",
      floating_toolbar: true,
      floating_toolbar_components: true,
      floating_toolbar_appearance: true,
      candidate_font_controls: true,
      candidate_follow_cursor: true,
      ime_mode_scope: true,
      mode_switch_shortcuts: true,
      panel_shortcuts: true,
    } as HostCapabilities,
  ],
  // The same lists, asked of macOS. A choice that exists on one host and not the other is a
  // difference in the page, and this is the layer where one hid: the candidate page sizes were
  // 5/7/9 here and 3-9 in the reference until the host stopped rewriting them.
  ["macos", macosHostCapabilities],
];

test.each(
  referenceOptions.flatMap((entry) =>
    optionHosts.map(([platform, host]) => ({ ...entry, platform, host })),
  ),
)(
  "$control offers the reference window's choices on $platform",
  async ({ button, control, role, options, host }) => {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host,
          dictionary: {
            list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
            edit: vi.fn(),
          },
          scanSkinCatalog: vi.fn().mockResolvedValue({ directory: "", packages: [], issues: [] }),
        }}
      />,
    );
    await settingsReady();
    fireEvent.click(screen.getByRole("button", { name: button }));
    if (role === "radiogroup") {
      const group = await screen.findByRole("radiogroup", { name: control });
      expect(
        within(group)
          .getAllByRole("radio")
          .map((radio) => radio.closest("label")?.textContent),
      ).toEqual(options);
      return;
    }
    const select = (await screen.findByRole("combobox", { name: control })) as HTMLSelectElement;
    expect([...select.options].map((option) => option.textContent)).toEqual(options);
  },
);

test.each(optionHosts)(
  "the input page goes from the basic groups to the advanced ones on %s",
  async (_platform, host) => {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host,
          fuzzyPinyin: true,
        }}
      />,
    );
    await settingsReady();
    fireEvent.click(screen.getByRole("button", { name: "输入" }));
    const input = await screen.findByRole("group", { name: "输入" });
    // 组的顺序：方案 → 中英文 → 选词与翻页 → 候选与联想 → 输出 → 快捷模式 → 模糊音 → 辅助码 → 拼音方案调频。不再沿用参考窗口的顺序。
    const groups = [...input.querySelectorAll("[data-group-title]")].map(
      (node) => node.textContent ?? "",
    );
    expect(groups).toEqual([
      "方案",
      "中英文",
      "选词与翻页",
      "候选与联想",
      "输出",
      "快捷模式",
      "模糊音",
      "辅助码",
      "拼音方案调频",
    ]);
    const present = sectionTitles(input);
    const expected = [
      "输入模式",
      "输入方案",
      "双拼方案",
      // 五笔、日语只有一个方案，只在对应方案下显示，但仍在页面里。
      "五笔方案",
      "日语方案",
      "默认中英文",
      "中英文状态",
      "以词定字",
      "翻页方式",
      "云候选",
      "繁体输出",
      "拼音方案调频",
    ];
    const ordered = present.filter((text) => expected.includes(text));
    // 输入方案 has a touch variant and a desktop variant; only one is ever shown, but both can be in
    // the tree, so collapse a repeat rather than reading it as a move.
    const collapsed = ordered.filter((title, index) => title !== ordered[index - 1]);
    expect(collapsed).toEqual(expected);
  },
);

test.each(optionHosts)(
  "the appearance page follows the reference window's order on %s",
  async (_platform, host) => {
    render(
      <SettingsPage
        initialPage="appearance"
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host,
        }}
      />,
    );
    await settingsReady();
    const appearance = screen.getByRole("group", { name: "候选窗口" });
    const present = sectionTitles(appearance);
    // The reference's 外观 page less its colour and theme sections, which the design moves to 主题.
    const reference = [
      "候选窗口跟随光标",
      "候选窗主字体",
      "候选窗字号",
      "候选窗预编辑字号",
      "每页候选项数量",
      "候选项排列方式",
      "行内预编辑",
      "候选窗预编辑",
    ];
    const ordered = present.filter((text) => reference.includes(text));
    expect(ordered).toEqual(reference.filter((title) => ordered.includes(title)));
    // Every host shows 候选窗主字体 now, Windows included, so none may go missing.
    expect(ordered.length).toBe(reference.length);
  },
);

test("macOS enables the shuangpin profile menu only under shuangpin", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "macos" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  // The fixture is quanpin, so the choice would change nothing yet.
  const menu = (await screen.findByRole("combobox", { name: "双拼方案" })) as HTMLSelectElement;
  expect(menu.disabled).toBe(true);
  fireEvent.click(screen.getByRole("radio", { name: "双拼" }));
  expect(menu.disabled).toBe(false);
  fireEvent.click(screen.getByRole("radio", { name: "全拼" }));
  expect(menu.disabled).toBe(true);
});

test("other hosts keep the shuangpin profile menu editable", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "windows" } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  const menu = (await screen.findByRole("combobox", { name: "双拼方案" })) as HTMLSelectElement;
  expect(menu.disabled).toBe(false);
});

test("macOS sidebar uses the same six groups", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "macos", floating_toolbar: true } as HostCapabilities,
      }}
    />,
  );
  await settingsReady();
  const sidebar = screen.getByRole("navigation", { name: "设置分类" });
  const groups = [...sidebar.querySelectorAll("[data-sidebar-section]")].map((section) =>
    [...section.querySelectorAll("button")].map((item) => item.textContent ?? ""),
  );
  expect(groups).toEqual([
    ["输入", "标点与翻译", "快捷键", "词库"],
    ["主题", "候选窗口", "悬浮工具栏"],
    ["屏幕键盘", "语音输入", "手写输入"],
    ["剪贴板"],
    ["维护与诊断", "帮助与反馈", "关于"],
  ]);
});

test("about page validates a newer release before offering its URL", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        version: "v1.2.0",
        releaseUrl: "https://github.com/metasequoiaime/msime/releases",
        signed: true,
      }),
    }),
  );
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    openExternalUrl,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("发现新版本 v1.2.0")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "前往下载" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://github.com/metasequoiaime/msime/releases",
    ),
  );
  vi.unstubAllGlobals();
});

test("Linux checks the client release feed and treats no release as a normal result", async () => {
  const fetch = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    json: async () => [
      {
        tag_name: "macos-v9.0.0",
        html_url: "https://github.com/metasequoiaime/msime/releases/tag/macos-v9.0.0",
      },
    ],
  });
  vi.stubGlobal("fetch", fetch);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "linux" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("暂无可用发行版")).toBeDefined();
  expect(fetch).toHaveBeenCalledWith(
    expect.stringMatching(
      /^https:\/\/api\.github\.com\/repos\/metasequoiaime\/msime\/releases\?per_page=100&t=\d+$/,
    ),
    expect.objectContaining({ cache: "no-store", signal: expect.any(AbortSignal) }),
  );
  vi.unstubAllGlobals();
});

test("Linux offers its own newest published release, not another platform's", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => [
        {
          tag_name: "macos-v9.0.0",
          html_url: "https://github.com/metasequoiaime/msime/releases/tag/macos-v9.0.0",
        },
        {
          tag_name: "linux-v1.3.0",
          html_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v1.3.0",
          prerelease: true,
        },
        {
          tag_name: "linux-v1.2.0",
          html_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v1.2.0",
        },
      ],
    }),
  );
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        openExternalUrl,
        host: { platform: "linux" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("发现新版本 v1.2.0")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "前往下载" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://github.com/metasequoiaime/msime/releases/tag/linux-v1.2.0",
    ),
  );
  // A release without assets has no digest to show, so the notice falls back to SHA256SUMS instead of inventing one.
  expect(screen.queryByText(/下载后请核对 SHA256/)).toBeNull();
  expect(
    screen.getByText(/该软件包未签名。.*sha256sum -c SHA256SUMS --ignore-missing/),
  ).toBeDefined();
  vi.unstubAllGlobals();
});

test("Linux update notice shows the .deb digest GitHub computed and the sha256sum command", async () => {
  const digest = "0123456789abcdef".repeat(4);
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => [
        {
          tag_name: "linux-v1.2.0",
          html_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v1.2.0",
          assets: [
            {
              name: "msime-linux-1.2.0-linux-x86_64.tar.gz",
              digest: `sha256:${"f".repeat(64)}`,
              browser_download_url:
                "https://github.com/metasequoiaime/msime/releases/download/linux-v1.2.0/msime-linux-1.2.0-linux-x86_64.tar.gz",
            },
            {
              name: "msime-linux_1.2.0_amd64.deb",
              digest: `sha256:${digest}`,
              browser_download_url:
                "https://github.com/metasequoiaime/msime/releases/download/linux-v1.2.0/msime-linux_1.2.0_amd64.deb",
            },
            { name: "SHA256SUMS", digest: `sha256:${"e".repeat(64)}` },
          ],
        },
      ],
    }),
  );
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform: "linux" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("发现新版本 v1.2.0")).toBeDefined();
  expect(screen.getByText("该软件包未签名，请务必核对下面的校验值。")).toBeDefined();
  expect(screen.getByText(digest)).toBeDefined();
  expect(screen.getByText("sha256sum msime-linux_1.2.0_amd64.deb")).toBeDefined();
  expect(screen.queryByText(/Get-FileHash/)).toBeNull();
  vi.unstubAllGlobals();
});

test("Linux release assets yield a digest only when it is well-formed and unambiguous", () => {
  const page = "https://github.com/metasequoiaime/msime/releases";
  const digest = "a".repeat(64);
  const release = (assets: unknown) => [
    { tag_name: "linux-v1.2.0", html_url: `${page}/tag/linux-v1.2.0`, assets },
  ];
  const pick = (assets: unknown) => {
    const update = selectPlatformRelease(release(assets), "linux", page);
    return (
      update && {
        name: update.installerName,
        sha256: update.installerSha256,
        signed: update.signed,
      }
    );
  };
  expect(pick([{ name: "msime-linux_1.2.0_amd64.deb", digest: `sha256:${digest}` }])).toEqual({
    name: "msime-linux_1.2.0_amd64.deb",
    sha256: digest,
    signed: false,
  });
  // The tarball is the fallback when no .deb was uploaded.
  expect(
    pick([{ name: "msime-linux-1.2.0-linux-x86_64.tar.gz", digest: `sha256:${digest}` }]),
  ).toEqual({ name: "msime-linux-1.2.0-linux-x86_64.tar.gz", sha256: digest, signed: false });
  // Older API responses omit the digest or return null; a wrong algorithm, uppercase hex or a short value is not trusted either.
  for (const bad of [
    undefined,
    null,
    `sha512:${digest}`,
    `sha256:${digest.toUpperCase()}`,
    `sha256:${digest.slice(1)}`,
    digest,
    42,
  ]) {
    expect(pick([{ name: "msime-linux_1.2.0_amd64.deb", digest: bad }])).toEqual({
      name: "msime-linux_1.2.0_amd64.deb",
      sha256: null,
      signed: false,
    });
  }
  // Two architectures would make any single digest wrong for someone.
  expect(
    pick([
      { name: "msime-linux_1.2.0_amd64.deb", digest: `sha256:${digest}` },
      { name: "msime-linux_1.2.0_arm64.deb", digest: `sha256:${"b".repeat(64)}` },
    ]),
  ).toEqual({ name: null, sha256: null, signed: false });
  // A name that would need shell quoting is never put into the copyable command.
  expect(pick([{ name: "--x;rm -rf ~.deb", digest: `sha256:${digest}` }])).toEqual({
    name: null,
    sha256: null,
    signed: false,
  });
  for (const assets of [undefined, null, "x", [null, 3, { digest: `sha256:${digest}` }]]) {
    expect(pick(assets)).toEqual({ name: null, sha256: null, signed: false });
  }
  // Windows never takes a Linux package for its installer.
  expect(
    selectPlatformRelease(
      [
        {
          tag_name: "windows-v1.2.0",
          html_url: `${page}/tag/windows-v1.2.0`,
          assets: [{ name: "msime-linux_1.2.0_amd64.deb", digest: `sha256:${digest}` }],
        },
      ],
      "windows",
      page,
    ),
  ).toMatchObject({ installerName: null, installerSha256: null, signed: false });
  // Other platforms keep ignoring assets.
  expect(
    selectPlatformRelease(
      [
        {
          tag_name: "macos-v1.2.0",
          html_url: `${page}/tag/macos-v1.2.0`,
          assets: [{ name: "MetasequoiaIME_Setup_v1.2.0.exe", digest: `sha256:${digest}` }],
        },
      ],
      "macos",
      page,
    ),
  ).toMatchObject({ installerName: null, installerSha256: null, signed: null });
});

test("Windows release assets yield the installer digest and mark the build unsigned", () => {
  const page = "https://github.com/metasequoiaime/msime/releases";
  const digest = "d".repeat(64);
  const pick = (assets: unknown) => {
    const update = selectPlatformRelease(
      [{ tag_name: "windows-v1.2.0", html_url: `${page}/tag/windows-v1.2.0`, assets }],
      "windows",
      page,
    );
    return (
      update && {
        name: update.installerName,
        sha256: update.installerSha256,
        signed: update.signed,
      }
    );
  };
  // What release-windows.yml uploads: the installer and its .sha256 file.
  expect(
    pick([
      { name: "MetasequoiaIME_Setup_v1.2.0.exe", digest: `sha256:${digest}` },
      { name: "MetasequoiaIME_Setup_v1.2.0.exe.sha256", digest: `sha256:${"e".repeat(64)}` },
    ]),
  ).toEqual({ name: "MetasequoiaIME_Setup_v1.2.0.exe", sha256: digest, signed: false });
  // An older API response without digests keeps the name, so the notice can point at the .sha256 file.
  expect(pick([{ name: "MetasequoiaIME_Setup_v1.2.0.exe", digest: null }])).toEqual({
    name: "MetasequoiaIME_Setup_v1.2.0.exe",
    sha256: null,
    signed: false,
  });
  // Two installers are ambiguous; a name needing quoting never reaches the command.
  expect(
    pick([
      { name: "MetasequoiaIME_Setup_v1.2.0.exe", digest: `sha256:${digest}` },
      { name: "MetasequoiaIME_Setup_v1.2.0-x86.exe", digest: `sha256:${digest}` },
    ]),
  ).toEqual({ name: null, sha256: null, signed: false });
  expect(pick([{ name: "Setup v1.2.0;calc.exe", digest: `sha256:${digest}` }])).toEqual({
    name: null,
    sha256: null,
    signed: false,
  });
  expect(pick(undefined)).toEqual({ name: null, sha256: null, signed: false });
});

test("installer trust uses sha256sum on Linux and keeps Get-FileHash on Windows", () => {
  const digest = "c".repeat(64);
  const version = { display: "1.2.0", parts: [1, 2, 0] };
  const releaseUrl = "https://github.com/metasequoiaime/msime/releases/tag/linux-v1.2.0";
  expect(
    describeInstallerTrust(
      {
        version,
        releaseUrl,
        installerName: "msime-linux_1.2.0_amd64.deb",
        installerSha256: digest,
        signed: false,
      },
      "linux",
    ),
  ).toEqual({
    warning: "该软件包未签名，请务必核对下面的校验值。",
    verify: { command: "sha256sum msime-linux_1.2.0_amd64.deb", sha256: digest },
  });
  expect(
    describeInstallerTrust(
      { version, releaseUrl, installerName: null, installerSha256: null, signed: false },
      "linux",
    ).verify,
  ).toBeNull();
  const windows = {
    version,
    releaseUrl: "https://github.com/metasequoiaime/msime/releases",
    installerName: "MetasequoiaIME_Setup_v1.2.0.exe",
    installerSha256: digest,
    signed: false,
  };
  const expected = {
    warning:
      "该版本未经代码签名，SmartScreen 会拦截，且 uiAccess 失效（候选窗口无法浮在以管理员身份运行的程序之上）。请务必核对下面的校验值。",
    verify: {
      command: "Get-FileHash .\\MetasequoiaIME_Setup_v1.2.0.exe -Algorithm SHA256",
      sha256: digest,
    },
  };
  expect(describeInstallerTrust(windows, "windows")).toEqual(expected);
  expect(describeInstallerTrust(windows, null)).toEqual(expected);
  // Without a digest the unsigned warning points at the .sha256 file the release carries.
  expect(describeInstallerTrust({ ...windows, installerSha256: null }, "windows")).toEqual({
    warning:
      "该版本未经代码签名，SmartScreen 会拦截，且 uiAccess 失效（候选窗口无法浮在以管理员身份运行的程序之上）。请从发行页一并下载 MetasequoiaIME_Setup_v1.2.0.exe.sha256，用 Get-FileHash .\\MetasequoiaIME_Setup_v1.2.0.exe -Algorithm SHA256 核对。",
    verify: null,
  });
});

test("Windows checks this repository's Windows releases rather than the reference manifest", async () => {
  const fetch = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    json: async () => [
      {
        tag_name: "linux-v9.0.0",
        html_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v9.0.0",
      },
      {
        tag_name: "windows-v1.2.0",
        html_url: "https://github.com/metasequoiaime/msime/releases/tag/windows-v1.2.0",
        assets: [
          { name: "MetasequoiaIME_Setup_v1.2.0.exe", digest: `sha256:${"b".repeat(64)}` },
          { name: "MetasequoiaIME_Setup_v1.2.0.exe.sha256", digest: `sha256:${"c".repeat(64)}` },
        ],
      },
    ],
  });
  vi.stubGlobal("fetch", fetch);
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        openExternalUrl,
        host: { platform: "windows" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("发现新版本 v1.2.0")).toBeDefined();
  // The installer's digest and the unsigned warning reach the notice, as on the shipped settings page.
  expect(screen.getByText(/SmartScreen 会拦截，且 uiAccess 失效/)).toBeDefined();
  expect(screen.getByText("b".repeat(64))).toBeDefined();
  expect(
    screen.getByText("Get-FileHash .\\MetasequoiaIME_Setup_v1.2.0.exe -Algorithm SHA256"),
  ).toBeDefined();
  expect(fetch).toHaveBeenCalledWith(
    expect.stringMatching(/^https:\/\/api\.github\.com\/repos\/metasequoiaime\/msime\/releases\?/),
    expect.objectContaining({ cache: "no-store", signal: expect.any(AbortSignal) }),
  );
  fireEvent.click(screen.getByRole("button", { name: "前往下载" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith(
      "https://github.com/metasequoiaime/msime/releases/tag/windows-v1.2.0",
    ),
  );
  vi.unstubAllGlobals();
});

test("an update check that never answers gives up after ten seconds", async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  let signal: AbortSignal | undefined;
  vi.stubGlobal(
    "fetch",
    vi.fn((_url: string, init?: RequestInit) => {
      signal = init?.signal ?? undefined;
      return new Promise((_resolve, reject) =>
        signal?.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError"))),
      );
    }),
  );
  try {
    render(
      <SettingsPage
        client={{
          load: vi.fn().mockResolvedValue(initial),
          save: vi.fn(),
          host: { platform: "windows" } as HostCapabilities,
        }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "关于" }));
    fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
    await waitFor(() => expect(signal).toBeDefined());
    await act(async () => {
      vi.advanceTimersByTime(9_000);
    });
    expect(signal?.aborted).toBe(false);
    await act(async () => {
      vi.advanceTimersByTime(1_000);
    });
    expect(signal?.aborted).toBe(true);
    expect(await screen.findByText("检查失败，请稍后重试")).toBeDefined();
  } finally {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  }
});

test("about page uses the packaged app version for display and update comparison", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => [
        {
          tag_name: "linux-v1.2.0",
          html_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v1.2.0",
        },
      ],
    }),
  );
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("v1.2.0"),
        host: { platform: "linux" } as HostCapabilities,
      }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(await screen.findByText("v1.2.0")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("已是最新版本")).toBeDefined();
  expect(screen.queryByRole("button", { name: "前往下载" })).toBeNull();
  vi.unstubAllGlobals();
});

test("platform release selection compares versions rather than trusting list order", () => {
  const page = "https://github.com/metasequoiaime/msime/releases";
  expect(
    selectPlatformRelease(
      [
        { tag_name: "windows-v0.9.0", html_url: `${page}/tag/windows-v0.9.0` },
        { tag_name: "windows-v0.10.0", html_url: `${page}/tag/windows-v0.10.0` },
        { tag_name: "windows-v2.0.0", html_url: `${page}/tag/windows-v2.0.0`, draft: true },
        { tag_name: "windowsx-v3.0.0", html_url: `${page}/tag/windowsx-v3.0.0` },
      ],
      "windows",
      page,
    )?.version.display,
  ).toBe("0.10.0");
  expect(selectPlatformRelease([], "windows", page)).toBeNull();
});

test("client release validation rejects a release URL outside the shared repository", () => {
  expect(
    validateGitHubRelease(
      {
        tag_name: "v1.2.0",
        html_url: "https://github.com/metasequoiaime/MSIME-Windows/releases/tag/v1.2.0",
      },
      "https://github.com/metasequoiaime/msime/releases",
    ),
  ).toBeNull();
});

test("screen keyboard and handwriting pages expose the native panel actions", async () => {
  const openScreenKeyboard = vi.fn().mockResolvedValue(undefined);
  const openHandwriting = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    openScreenKeyboard,
    openHandwriting,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();

  fireEvent.click(screen.getByRole("button", { name: "屏幕键盘" }));
  expect(await screen.findByText("打开屏幕键盘")).toBeDefined();
  expect(screen.getByLabelText("屏幕键盘预览")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "打开" }));
  await waitFor(() => expect(openScreenKeyboard).toHaveBeenCalledTimes(1));

  fireEvent.click(screen.getByRole("button", { name: "手写输入" }));
  expect(await screen.findByText("打开手写识别板")).toBeDefined();
  expect(screen.getByLabelText("手写识别板预览")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "打开" }));
  await waitFor(() => expect(openHandwriting).toHaveBeenCalledTimes(1));
});

test("macOS routes input-session panels through the native input-method process", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    openHandwriting: vi.fn(),
    openCloudClipboard: vi.fn(),
    openCloudDictionary: vi.fn(),
    host: { platform: "macos" } as HostCapabilities,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();

  fireEvent.click(screen.getByRole("button", { name: "手写输入" }));
  expect(await screen.findByText("macOS 手写识别板")).toBeDefined();
  expect(screen.getByText(/需要当前输入法进程提供 IMK 输入会话/)).toBeDefined();
  expect(screen.queryByRole("button", { name: "打开" })).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "剪贴板" }));
  expect(await screen.findByText(/请从输入法菜单中的「云剪贴板…」打开云剪贴板/)).toBeDefined();
  expect(screen.queryByRole("button", { name: "打开云剪贴板" })).toBeNull();
  expect(screen.queryByRole("button", { name: "打开云词库" })).toBeNull();
  expect(client.openHandwriting).not.toHaveBeenCalled();
  expect(client.openCloudClipboard).not.toHaveBeenCalled();
  expect(client.openCloudDictionary).not.toHaveBeenCalled();
});

test("the 剪贴板 page sends a history entry through the host's cloud clipboard, macOS included", async () => {
  const snapshot = { ...initial, preferences: { ...initial.preferences, clipboard_history: true } };
  const cloudClipboardRequest = vi
    .fn()
    .mockResolvedValueOnce({ enabled: true, items: [] })
    .mockResolvedValueOnce({ items: [] });
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(snapshot),
    save: vi.fn(),
    host: { platform: "macos" } as HostCapabilities,
    clipboard: {
      clear: vi.fn(),
      list: vi
        .fn()
        .mockResolvedValue([
          { text: "synthetic cloud", timestampMs: 1_700_000_000_000, pinned: false },
        ]),
    },
    cloudClipboardRequest,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  expect(cloudClipboardRequest).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "剪贴板" }));
  expect(await screen.findByText("synthetic cloud")).toBeDefined();
  const send = screen.getByRole("button", { name: "发到云剪贴板" });
  await waitFor(() => expect(send.hasAttribute("disabled")).toBe(false));
  fireEvent.click(send);

  await waitFor(() =>
    expect(cloudClipboardRequest).toHaveBeenLastCalledWith({
      operation: "add",
      text: "synthetic cloud",
    }),
  );
  expect(await screen.findByText("已发到云剪贴板")).toBeDefined();
});

test("native panel views support close, modifier, drawing and undo interactions", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const keyboard = render(<KeyboardPanel client={{ close }} />);
  const shifts = screen.getAllByRole("button", { name: "Shift" });
  fireEvent.click(shifts[0]);
  expect(shifts[0].getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "A" }));
  expect(screen.getByRole("status").textContent).toContain("Shift+A");
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
  keyboard.unmount();

  const panel = render(<HandwritingPanel client={{ close }} />);
  const canvas = screen.getByLabelText("手写画布");
  fireEvent.pointerDown(canvas, { isPrimary: true, clientX: 20, clientY: 20, pointerId: 1 });
  fireEvent.pointerMove(canvas, { clientX: 80, clientY: 80, pointerId: 1 });
  fireEvent.pointerUp(canvas, { clientX: 100, clientY: 100, pointerId: 1 });
  expect(screen.getByRole("status").textContent).toContain("识别结果");
  fireEvent.click(screen.getByRole("button", { name: /撤销/ }));
  expect(screen.getByText("请在左侧书写，松开鼠标后自动识别")).toBeDefined();
  panel.unmount();
});

test("emoji panel searches, copies items, tracks recent use and reads clipboard history", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const copyText = vi.fn().mockResolvedValue(undefined);
  const list = vi.fn().mockResolvedValue(["fixture clipboard entry"]);
  const panel = render(<EmojiPanel client={{ close, copyText, clipboard: { list } }} />);

  expect(screen.getByRole("heading", { name: "Emoji" })).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "😀" }));
  fireEvent.click(screen.getByRole("button", { name: "😂" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledWith("😂"));
  expect(screen.getByRole("status").textContent).toContain("已复制：😂");

  fireEvent.change(screen.getByRole("textbox", { name: "搜索" }), { target: { value: "laugh" } });
  expect(screen.getByRole("button", { name: "😂" })).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "剪贴板" }));
  expect(await screen.findByText("fixture clipboard entry")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "fixture clipboard entry" }));
  await waitFor(() => expect(copyText).toHaveBeenLastCalledWith("fixture clipboard entry"));
  expect(list).toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
  panel.unmount();
});

test("voice panel requests recognition and submits the bounded result", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const recognizeVoice = vi.fn().mockResolvedValue({ text: "你好" });
  const sendText = vi.fn().mockResolvedValue(undefined);
  render(<VoicePanel client={{ close, recognizeVoice, sendText }} />);
  fireEvent.click(screen.getByRole("button", { name: "开始录音" }));
  await waitFor(() => expect(recognizeVoice).toHaveBeenCalledWith("zh-CN"));
  expect((screen.getByRole("textbox", { name: "识别结果" }) as HTMLTextAreaElement).value).toBe(
    "你好",
  );
  fireEvent.click(screen.getByRole("button", { name: "提交到当前窗口" }));
  await waitFor(() => expect(sendText).toHaveBeenCalledWith("你好"));
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
});

test("cloud clipboard panel lists, uploads, deletes and submits entries", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const sendText = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockImplementation(async (action: { operation: string }) => {
    if (action.operation === "list")
      return { items: [{ id: "entry-1", text: "云端内容" }], enabled: true };
    if (action.operation === "set_enabled") return { enabled: true };
    return { items: [{ id: "entry-1", text: "云端内容" }], enabled: true };
  });
  const panel = render(<CloudClipboardPanel client={{ close, sendText, request }} />);
  expect(await screen.findByText("云端内容")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "云端内容" }));
  await waitFor(() => expect(sendText).toHaveBeenCalledWith("云端内容"));
  fireEvent.change(screen.getByRole("textbox", { name: "待上传文本" }), {
    target: { value: "新的云端内容" },
  });
  fireEvent.click(screen.getByRole("button", { name: "上传明确选择的文本" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({ operation: "add", text: "新的云端内容" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "删除 云端内容" }));
  await waitFor(() => expect(request).toHaveBeenCalledWith({ operation: "delete", id: "entry-1" }));
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
  panel.unmount();
});

test("cloud clipboard copy-only capability never submits to an unsupported host", async () => {
  const copyText = vi.fn().mockResolvedValue(undefined);
  const sendText = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockResolvedValue({
    enabled: true,
    items: [{ id: "synthetic", text: "Synthetic clipboard" }],
  });
  render(
    <CloudClipboardPanel
      client={{ close: vi.fn(), request, copyText, sendText, canSendText: async () => false }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "Synthetic clipboard" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledWith("Synthetic clipboard"));
  expect(sendText).not.toHaveBeenCalled();
});

test("cloud clipboard rejects blank and overlong uploads before contacting the provider", async () => {
  const request = vi.fn().mockResolvedValue({ enabled: true, items: [] });
  render(<CloudClipboardPanel client={{ close: vi.fn(), request }} />);
  await screen.findByText("暂无云端历史");
  const input = screen.getByRole("textbox", { name: "待上传文本" }) as HTMLTextAreaElement;
  const submit = screen.getByRole("button", { name: "上传明确选择的文本" }) as HTMLButtonElement;
  expect(submit.disabled).toBe(true);
  fireEvent.change(input, { target: { value: "x".repeat(4001) } });
  expect(submit.disabled).toBe(true);
  expect(screen.getByText("4001 / 4000")).toBeDefined();
  fireEvent.change(input, { target: { value: "  " } });
  expect(submit.disabled).toBe(true);
  expect(request).not.toHaveBeenCalledWith({ operation: "add", text: expect.any(String) });
});

test("cloud clipboard confirms destructive disable and clears history", async () => {
  const request = vi
    .fn()
    .mockImplementation(async (action: { operation: string }) =>
      action.operation === "list"
        ? { enabled: true, items: [{ id: "synthetic", text: "Synthetic clipboard" }] }
        : { enabled: false },
    );
  render(<CloudClipboardPanel client={{ close: vi.fn(), request }} />);
  await screen.findByText("Synthetic clipboard");
  fireEvent.click(screen.getByRole("checkbox"));
  expect(screen.getByRole("alertdialog")).toBeDefined();
  expect(request).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(request).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("checkbox"));
  fireEvent.click(screen.getByRole("button", { name: "确认关闭并删除历史" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({ operation: "set_enabled", enabled: false }),
  );
  await waitFor(() => expect(screen.queryByText("Synthetic clipboard")).toBeNull());
});

test("cloud clipboard discards stale history after provider access fails", async () => {
  const request = vi
    .fn()
    .mockResolvedValueOnce({
      enabled: true,
      items: [{ id: "synthetic", text: "Synthetic clipboard" }],
    })
    .mockRejectedValue(new Error("unavailable"));
  render(<CloudClipboardPanel client={{ close: vi.fn(), request }} />);
  await screen.findByText("Synthetic clipboard");
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  await waitFor(() => expect(screen.queryByText("Synthetic clipboard")).toBeNull());
  expect((screen.getByRole("checkbox") as HTMLInputElement).disabled).toBe(true);
});

test("cloud dictionary clears old account entries when refresh fails", async () => {
  const request = vi
    .fn()
    .mockResolvedValueOnce({
      entries: [
        {
          id: "a".repeat(64),
          kind: "pinyin",
          code: "he",
          word: "合成词条",
          weight: 1,
          revision: 17,
        },
      ],
      has_more: true,
      offset: 0,
    })
    .mockRejectedValue(new Error("unavailable"));
  render(<CloudDictionaryPanel client={{ close: vi.fn(), request }} />);
  await screen.findByText("合成词条");
  fireEvent.click(screen.getByRole("button", { name: "查询" }));
  await waitFor(() => expect(screen.queryByText("合成词条")).toBeNull());
  expect((screen.getByRole("button", { name: "下一页" }) as HTMLButtonElement).disabled).toBe(true);
});

test("cloud dictionary exposes visible kind tabs for mobile layouts", async () => {
  const request = vi.fn().mockResolvedValue({ entries: [], has_more: false, offset: 0 });
  render(<CloudDictionaryPanel client={{ close: vi.fn(), request }} />);
  await screen.findByText("暂无词条");
  const tabs = screen.getByRole("tablist", { name: "云词库类型" });
  expect(tabs.querySelector("button[aria-selected='true']")?.textContent).toBe("拼音");
  fireEvent.click(screen.getByRole("tab", { name: "五笔" }));
  await waitFor(() =>
    expect(request).toHaveBeenLastCalledWith({
      operation: "list",
      kind: "wubi",
      offset: 0,
      search: "",
    }),
  );
  expect(tabs.querySelector("button[aria-selected='true']")?.textContent).toBe("五笔");
});

test("cloud dictionary panel supports paging and CRUD actions", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const request = vi
    .fn()
    .mockImplementation(async (action: { operation: string; offset?: number }) => {
      if (action.operation === "list")
        return {
          entries: [
            {
              id: "a".repeat(64),
              kind: "pinyin",
              code: "ni",
              word: "你",
              weight: 100,
              revision: 2,
            },
          ],
          has_more: true,
          offset: action.offset ?? 0,
        };
      return {};
    });
  const panel = render(<CloudDictionaryPanel client={{ close, request }} />);
  expect(await screen.findByText("你")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "list",
      kind: "pinyin",
      offset: 100,
      search: "",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "添加词条" }));
  fireEvent.change(screen.getByRole("textbox", { name: "编码" }), { target: { value: "hao" } });
  fireEvent.change(screen.getByRole("textbox", { name: "词条" }), { target: { value: "好" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "add",
      kind: "pinyin",
      code: "hao",
      word: "好",
      weight: 100000,
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
  panel.unmount();
});

test("a cloud dictionary file is decoded the same way the local import decodes it", async () => {
  const request = vi.fn().mockResolvedValue({});
  render(<CloudDictionaryFilesPanel client={{ close: vi.fn(), request }} />);
  // "你好\tni'hao\n" as GB18030, which Windows dictionary tools still write. Decoded as UTF-8
  // it becomes replacement characters and contains no NUL, so the panel's only guard passed and
  // a cloud dictionary of "\ufffd" was uploaded without a word of complaint.
  const bytes = new Uint8Array([
    0xc4, 0xe3, 0xba, 0xc3, 0x09, 0x6e, 0x69, 0x27, 0x68, 0x61, 0x6f, 0x0a,
  ]);
  fireEvent.change(screen.getByLabelText("选择文件"), {
    target: { files: [new File([bytes], "gb18030.tsv")] },
  });
  expect(await screen.findByText("gb18030.tsv")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "确认上传到云端" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "import",
      kind: "pinyin",
      format: "standard",
      text: "你好\tni'hao\n",
    }),
  );
});

test("cloud dictionary files previews, confirms and preserves bounded import/export contracts", async () => {
  const request = vi
    .fn()
    .mockImplementation(async (action: { operation: string; kind?: string; format?: string }) =>
      action.operation === "export" ? { text: "ni\t你\n", filename: "pinyin.tsv" } : {},
    );
  render(<CloudDictionaryFilesPanel client={{ close: vi.fn(), request }} />);
  const file = new File(["ni\t你\n"], "words.tsv", { type: "text/tab-separated-values" });
  fireEvent.change(screen.getByLabelText("选择文件"), { target: { files: [file] } });
  expect(await screen.findByText("words.tsv")).toBeDefined();
  expect(request).not.toHaveBeenCalledWith(expect.objectContaining({ operation: "import" }));
  fireEvent.click(screen.getByRole("button", { name: "确认上传到云端" }));
  expect((await screen.findByRole("alertdialog")).textContent).toContain(
    "按“标准 TSV”导入拼音词库",
  );
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "import",
      kind: "pinyin",
      format: "standard",
      text: "ni\t你\n",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "导出当前类型" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "export",
      kind: "pinyin",
      format: "standard",
    }),
  );
  const oversized = new File(["x".repeat(65537)], "large.tsv", { type: "text/plain" });
  fireEvent.change(screen.getByLabelText("选择文件"), { target: { files: [oversized] } });
  expect(await screen.findByText("导入文件必须大于 0 且不超过 64 KiB")).toBeDefined();
});

test("cloud dictionary entries open their editor from the row on touch layouts", async () => {
  const entry = {
    id: "a".repeat(64),
    kind: "pinyin" as const,
    code: "ni",
    word: "你",
    weight: 100,
    revision: 2,
  };
  const request = vi.fn().mockResolvedValue({ entries: [entry], has_more: false, offset: 0 });
  render(<CloudDictionaryPanel client={{ close: vi.fn(), request }} />);
  await screen.findByText("你");
  fireEvent.click(screen.getByRole("button", { name: "编辑云词条 你" }));
  expect((screen.getByRole("textbox", { name: "编码" }) as HTMLInputElement).value).toBe("ni");
  expect(screen.getByText("点按词条可编辑；窄屏下的下载和删除操作会分组显示。")).toBeTruthy();
});

test("cloud dictionary deletion asks for confirmation before changing the cloud entry", async () => {
  const entry = {
    id: "a".repeat(64),
    kind: "pinyin" as const,
    code: "ni",
    word: "你",
    weight: 100,
    revision: 2,
  };
  const request = vi.fn().mockResolvedValue({ entries: [entry], has_more: false, offset: 0 });
  render(<CloudDictionaryPanel client={{ close: vi.fn(), request }} />);
  await screen.findByText("你");
  fireEvent.click(screen.getByRole("button", { name: "删除" }));
  expect((await screen.findByRole("alertdialog")).textContent).toContain("“你”只会从云端删除");
  await answerConfirm("cancel");
  expect(request).not.toHaveBeenCalledWith(expect.objectContaining({ operation: "delete" }));
});

test("cloud dictionary panel can queue an entry for the local dictionary", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const entry = {
    id: "a".repeat(64),
    kind: "pinyin" as const,
    code: "ni",
    word: "你",
    weight: 100,
    revision: 2,
  };
  const downloadToLocal = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockResolvedValue({ entries: [entry], has_more: false, offset: 0 });
  const panel = render(<CloudDictionaryPanel client={{ close, request, downloadToLocal }} />);
  expect(await screen.findByText("你")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "下载到本机 你" }));
  expect((await screen.findByRole("alertdialog")).textContent).toContain(
    "云词条“你”会被加入本机个人词典",
  );
  await answerConfirm("confirm");
  await waitFor(() => expect(downloadToLocal).toHaveBeenCalledWith(entry));
  expect(screen.getByRole("status").textContent).toContain("云词条已加入本机词典队列");
  panel.unmount();
});

test("cloud dictionary apply requires preview and explicit confirmation", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockImplementation(async (action: { operation: string }) => {
    if (action.operation === "snapshot_status") return { localVersion: "local-v1", request: null };
    if (action.operation === "snapshot_preview")
      return {
        previewToken: "snapshot-token",
        snapshot: {
          cloudRevision: 42,
          sha256: "a".repeat(64),
          bytes: 2048,
          records: 12,
          entries: 4,
          overlays: 4,
          positions: 2,
          selections: 2,
        },
      };
    return {
      request: {
        id: "request",
        cloudRevision: 42,
        fileSha256: "a".repeat(64),
        status: action.operation === "snapshot_cancel" ? "cancelled" : "queued",
      },
    };
  });
  render(<CloudDictionaryApplyPanel client={{ close, request, snapshot: true }} />);
  await screen.findByText("已获取本机词库版本");
  fireEvent.click(screen.getByRole("button", { name: "下载云词库并预览" }));
  expect(await screen.findByText(/云端 revision 42/)).toBeDefined();
  expect(request).not.toHaveBeenCalledWith({
    operation: "snapshot_enqueue",
    token: "snapshot-token",
  });
  fireEvent.click(screen.getByRole("button", { name: "替换本机词库" }));
  expect((await screen.findByRole("alertdialog")).textContent).toContain(
    "替换本机个人词库和学习记录",
  );
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "snapshot_enqueue",
      token: "snapshot-token",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "取消待应用快照" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(request).toHaveBeenCalledWith({ operation: "snapshot_cancel" }));
});

test("cloud dictionary snapshot backup and restore stay behind validation and confirmation", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockImplementation(async (action: { operation: string }) => {
    if (action.operation === "snapshot_export")
      return { text: '{"type":"header"}\n', filename: "snapshot.ndjson" };
    if (action.operation === "snapshot_restore_preview")
      return {
        snapshot: {
          cloudRevision: 7,
          sha256: "b".repeat(64),
          bytes: 20,
          records: 0,
          entries: 0,
          overlays: 0,
          positions: 0,
          selections: 0,
        },
        expectedRevision: 12,
      };
    if (action.operation === "snapshot_restore") return { revision: 13, reset: true };
    return {};
  });
  render(<CloudDictionaryFilesPanel client={{ close, request, snapshot: true }} />);
  const file = new File(["snapshot fixture"], "snapshot.ndjson", { type: "application/x-ndjson" });
  fireEvent.change(screen.getByLabelText("选择快照恢复到云端"), { target: { files: [file] } });
  expect(await screen.findByText(/云端 revision 7/)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "确认恢复云端词库" }));
  expect((await screen.findByRole("alertdialog")).textContent).toContain(
    "替换全部云端词库和排序记录",
  );
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "snapshot_restore",
      text: "snapshot fixture",
      expected_sha256: "b".repeat(64),
      revision: 12,
    }),
  );
  if (typeof URL.createObjectURL === "function")
    vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:fixture");
  else
    Object.defineProperty(URL, "createObjectURL", {
      configurable: true,
      value: vi.fn(() => "blob:fixture"),
    });
  fireEvent.click(screen.getByRole("button", { name: "导出完整快照" }));
  await waitFor(() => expect(request).toHaveBeenCalledWith({ operation: "snapshot_export" }));
});

test("cloud dictionary catalog panel queries and edits complete directory entries", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const back = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockImplementation(async (action: { operation: string }) => {
    if (action.operation === "catalog") {
      return {
        catalog_entries: [{ kind: "pinyin", code: "ni", word: "你", weight: 100 }],
        has_more: false,
        offset: 0,
        revision: 42,
        normalized: "ni",
      };
    }
    return { revision: 43, previous: null, replacement: null };
  });
  render(<CloudDictionaryCatalogPanel client={{ close, back, request }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "完整目录编码" }), {
    target: { value: "ni" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询完整目录" }));
  expect(await screen.findByText("你")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "编辑" }));
  fireEvent.change(screen.getByRole("textbox", { name: "词条" }), { target: { value: "你们" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "edit_catalog",
      kind: "pinyin",
      code: "ni",
      word: "你",
      revision: 42,
      replacement: { code: "ni", word: "你们", weight: 100 },
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "返回云词典" }));
  await waitFor(() => expect(back).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
});

test("cloud candidates panel uses canonical pinyin and manages ranking and fixed positions", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const back = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockImplementation(async (action: { operation: string }) => {
    if (action.operation === "candidates")
      return {
        candidates: [{ code: "nihc", canonical_pinyin: "ni'hao", word: "你好", weight: 10 }],
        context: "server:context",
        revision: 42,
      };
    if (action.operation === "fixed_positions")
      return {
        positions: [{ context: "server:context", code: "ni'hao", word: "你好", position: 1 }],
        offset: 0,
        has_more: false,
      };
    if (action.operation === "rank") return { revision: 43, changed: true, selection_count: 0 };
    return { revision: 43 };
  });
  render(<CloudCandidatesPanel client={{ close, back, request }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "云端候选编码" }), {
    target: { value: "nihc" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询云端候选" }));
  expect(await screen.findByText("你好")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "调频" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      expect.objectContaining({ operation: "rank", code: "ni'hao", revision: 42 }),
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "固定" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      expect.objectContaining({ operation: "set_fixed_position", code: "ni'hao", position: 1 }),
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "取消固定" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      expect.objectContaining({ operation: "set_fixed_position", position: null }),
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "返回云词典" }));
  await waitFor(() => expect(back).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
});

test("cloud candidate rows trigger ranking and keep secondary actions grouped", async () => {
  const candidate = { code: "nihc", canonical_pinyin: "ni'hao", word: "你好", weight: 10 };
  const request = vi.fn().mockImplementation(async (action: { operation: string }) => {
    if (action.operation === "candidates")
      return { candidates: [candidate], context: "", revision: 42 };
    if (action.operation === "rank") return { changed: true, selection_count: 0 };
    return {};
  });
  render(<CloudCandidatesPanel client={{ close: vi.fn(), request }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "云端候选编码" }), {
    target: { value: "nihc" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询云端候选" }));
  await screen.findByText("你好");
  fireEvent.click(screen.getByRole("button", { name: "调频候选 你好" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      expect.objectContaining({ operation: "rank", code: "ni'hao", word: "你好" }),
    ),
  );
  // "Grouped" means the row's secondary actions sit together in one container rather than scattered
  // through the row. Asserted through the buttons themselves: they are what has to stay grouped, and
  // the container's class is a styling detail with no reason to be stable.
  const row = screen.getByRole("button", { name: "调频候选 你好" }).parentElement!;
  const group = row.querySelector("div")!;
  expect(group).not.toBeNull();
  const grouped = Array.from(group.querySelectorAll("button")).map((button) => button.textContent);
  expect(grouped).toEqual(["调频", "固定", "删除"]);
});

test("saves a shuangpin profile and retains it when switching schemes", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  await screen.findByRole("radio", { name: "全拼" });
  expect(screen.getByRole("combobox", { name: "双拼方案" })).toBeDefined();
  fireEvent.click(screen.getByRole("radio", { name: "双拼" }));
  const profile = screen.getByRole("combobox", { name: "双拼方案" }) as HTMLSelectElement;
  fireEvent.change(profile, { target: { value: "microsoft" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    scheme: "shuangpin",
    last_chinese_scheme: "shuangpin",
    shuangpin_profile: "microsoft",
  });
  fireEvent.click(screen.getByRole("radio", { name: "全拼" }));
  expect(screen.getByRole("combobox", { name: "双拼方案" })).toBeDefined();
  expect(profile.textContent).toContain("微软双拼");
});

test.each([
  ["quanpin", "全拼"],
  ["shuangpin", "双拼"],
  ["wubi", "五笔"],
] as const)("Japanese mode retains %s across save and reload", async (scheme, label) => {
  let stored: Snapshot = { ...initial, preferences: { ...initial.preferences, scheme } };
  const client: SettingsClient = {
    load: vi.fn(async () => stored),
    save: vi.fn(
      async (revision, preferences) =>
        (stored = { ...stored, revision: revision + 1, preferences }),
    ),
  };
  const mounted = render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  await screen.findByRole("radio", { name: label });
  fireEvent.click(screen.getByRole("radio", { name: "日文" }));
  expect(screen.queryByRole("radio", { name: label })).toBeNull();
  expect((screen.getByRole("radio", { name: "罗马音" }) as HTMLInputElement).checked).toBe(true);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(stored.preferences.scheme).toBe("japanese");
  expect(stored.preferences.last_chinese_scheme).toBe(scheme);
  mounted.unmount();
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  await screen.findByRole("radio", { name: "罗马音" });
  fireEvent.click(screen.getByRole("radio", { name: "中文" }));
  expect((screen.getByRole("radio", { name: label }) as HTMLInputElement).checked).toBe(true);
  expect(screen.queryByRole("radio", { name: "罗马音" })).toBeNull();
});

test("saves edited preferences against the loaded revision", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage initialPage="appearance" client={client} />);
  const size = await screen.findByRole("slider", { name: "每页候选项数量" });
  fireEvent.change(size, { target: { value: "9" } });
  // Nothing is written until the edits pause for the autosave delay.
  expect(client.save).not.toHaveBeenCalled();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledOnce();
  expect(client.save).toHaveBeenCalledWith(7, { ...initial.preferences, candidate_page_size: 9 });
});

test("a late preference save is ignored after settings unmounts", async () => {
  let finish!: (value: Snapshot) => void;
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(
      () =>
        new Promise<Snapshot>((resolve) => {
          finish = resolve;
        }),
    ),
  };
  const view = render(<SettingsPage initialPage="appearance" client={client} />);
  const size = await screen.findByRole("slider", { name: "每页候选项数量" });
  fireEvent.change(size, { target: { value: "9" } });
  saveSettingsNow();
  view.unmount();
  finish({
    ...initial,
    revision: 8,
    preferences: { ...initial.preferences, candidate_page_size: 9 },
  });
  await Promise.resolve();
});

test("macOS offers the same candidate page sizes as every other host and keeps the saved one", async () => {
  // These were 5, 7 and 9 - the Apple reference's set - while the host rewrote anything else to 9. The
  // shared default is six, so the platform displayed and saved nine for a setting nobody had touched.
  const preferences = { ...initial.preferences, candidate_page_size: 6 };
  const save = vi.fn().mockImplementation(async (_revision, next) => ({
    ...initial,
    revision: 8,
    preferences: next,
  }));
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue({ ...initial, preferences }),
    save,
    host: { platform: "macos" } as HostCapabilities,
  };
  render(<SettingsPage initialPage="appearance" client={client} />);
  const size = (await screen.findByRole("slider", {
    name: "每页候选项数量",
  })) as HTMLInputElement;
  expect([size.min, size.max, size.step]).toEqual(["3", "9", "1"]);
  expect(size.value).toBe("6");
  fireEvent.change(size, { target: { value: "4" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, { ...preferences, candidate_page_size: 4 });
});

test("a saved page size below the reference's three stays in range and selected", async () => {
  // The shared preference accepts one and two; the page offers the reference's three through nine. A
  // document carrying two must not display as three, or saving any other change would rewrite it.
  const preferences = { ...initial.preferences, candidate_page_size: 2 };
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue({ ...initial, preferences }),
    save: vi.fn(),
    host: { platform: "windows" } as HostCapabilities,
  };
  render(<SettingsPage initialPage="appearance" client={client} />);
  const size = (await screen.findByRole("slider", {
    name: "每页候选项数量",
  })) as HTMLInputElement;
  expect([size.min, size.max]).toEqual(["2", "9"]);
  expect(size.value).toBe("2");
});

test("macOS shuangpin keymap setting loads, toggles, and saves through the native preference bridge", async () => {
  const loadMacosShuangpinKeymap = vi.fn().mockResolvedValue(true);
  const saveMacosShuangpinKeymap = vi.fn().mockResolvedValue(undefined);
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 8,
    preferences,
  }));
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save,
    loadMacosShuangpinKeymap,
    saveMacosShuangpinKeymap,
    host: { platform: "macos" } as HostCapabilities,
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  fireEvent.click(await screen.findByRole("radio", { name: "双拼" }));
  const keymap = (await screen.findByRole("switch", {
    name: "输入时显示双拼键位提示",
  })) as HTMLInputElement;
  expect(loadMacosShuangpinKeymap).toHaveBeenCalledTimes(1);
  expect(keymap.checked).toBe(true);
  fireEvent.click(keymap);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    scheme: "shuangpin",
    last_chinese_scheme: "shuangpin",
  });
  expect(saveMacosShuangpinKeymap).toHaveBeenCalledWith(false);
});

test("an edit made while a save is in flight is kept and saved after it", async () => {
  const finishes: ((snapshot: Snapshot) => void)[] = [];
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(
      (revision, preferences) =>
        new Promise<Snapshot>((resolve) => {
          finishes.push(() => resolve({ ...initial, revision: revision + 1, preferences }));
        }),
    ),
  };
  render(<SettingsPage client={client} />);
  const size = (await screen.findByLabelText("每页候选项数量")) as HTMLInputElement;
  fireEvent.change(size, { target: { value: "9" } });
  saveSettingsNow();
  expect(client.save).toHaveBeenCalledTimes(1);
  expect(screen.getByText("正在保存…")).toBeTruthy();
  // Nothing is locked while the first save runs.
  fireEvent.change(screen.getByLabelText("候选窗字号"), { target: { value: "20" } });
  await act(async () => finishes[0](initial));
  expect((screen.getByLabelText("候选窗字号") as HTMLSelectElement).value).toBe("20");
  await waitFor(() => expect(client.save).toHaveBeenCalledTimes(2));
  expect(client.save).toHaveBeenLastCalledWith(8, {
    ...initial.preferences,
    candidate_page_size: 9,
    candidate_font_size: 20,
  });
  await act(async () => finishes[1](initial));
  await screen.findByText("已保存");
});

test("a conflict merges another window's change to a different setting and saves again", async () => {
  const theirs: Snapshot = {
    ...initial,
    revision: 8,
    preferences: { ...initial.preferences, learning: false },
  };
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValueOnce(initial).mockResolvedValue(theirs),
    save: vi
      .fn()
      .mockRejectedValueOnce({ code: "conflict" })
      .mockImplementation(async (revision, preferences) => ({
        ...initial,
        revision: revision + 1,
        preferences,
      })),
  };
  render(<SettingsPage client={client} />);
  const size = await screen.findByLabelText("每页候选项数量");
  fireEvent.change(size, { target: { value: "9" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenLastCalledWith(8, {
    ...theirs.preferences,
    candidate_page_size: 9,
  });
  // Different settings changed, so nothing of this window's was overridden and no merge is announced.
  expect(screen.queryByText("设置同时在其他窗口修改，已合并。")).toBeNull();
});

test("a failed save keeps the edit and 重试 saves it again", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi
      .fn()
      .mockRejectedValueOnce(new Error("磁盘已满"))
      .mockImplementation(async (revision, preferences) => ({
        ...initial,
        revision: revision + 1,
        preferences,
      })),
  };
  render(<SettingsPage client={client} />);
  const size = (await screen.findByLabelText("每页候选项数量")) as HTMLInputElement;
  fireEvent.change(size, { target: { value: "9" } });
  saveSettingsNow();
  expect((await screen.findByRole("button", { name: "重试" })).className).toBe("secondary");
  expect(screen.getByRole("button", { name: "重新读取" })).toBeTruthy();
  expect(size.value).toBe("9");
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledTimes(2);
  expect(client.save).toHaveBeenLastCalledWith(7, {
    ...initial.preferences,
    candidate_page_size: 9,
  });
  expect(screen.queryByRole("button", { name: "重试" })).toBeNull();
  expect(screen.queryByRole("button", { name: "重新读取" })).toBeNull();
});

test("a conflict that keeps recurring preserves edits and offers an explicit reload", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockRejectedValue({ code: "conflict" }),
  };
  render(<SettingsPage initialPage="appearance" client={client} />);
  const size = await screen.findByRole("slider", { name: "每页候选项数量" });
  fireEvent.change(size, { target: { value: "9" } });
  saveSettingsNow();
  expect((await screen.findByRole("alert")).textContent).toContain("其他窗口");
  expect((size as HTMLInputElement).value).toBe("9");
  // Each conflict reads the newer revision and tries again, a bounded number of times, before giving up.
  expect(client.load).toHaveBeenCalledTimes(4);
  expect(client.save).toHaveBeenCalledTimes(4);
  fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
  await answerConfirm("confirm");
  await waitFor(() => expect((size as HTMLInputElement).value).toBe("5"));
});

test.each(["windows", "macos", "linux"])(
  "%s applies external preference revisions when clean and preserves dirty edits",
  async (platform) => {
    let changed: ((snapshot: Snapshot) => void) | undefined;
    const client: SettingsClient = {
      load: vi.fn().mockResolvedValue(initial),
      save: vi.fn(async (revision, preferences) => ({
        ...initial,
        revision: revision + 1,
        preferences,
      })),
      onPreferencesChanged: vi.fn(async (listener) => {
        changed = listener;
        return () => {
          changed = undefined;
        };
      }),
      host: { platform } as never,
    };
    render(<SettingsPage initialPage="appearance" client={client} />);
    const size = (await screen.findByRole("slider", {
      name: "每页候选项数量",
    })) as HTMLInputElement;
    await waitFor(() => expect(changed).toBeDefined());
    changed?.({
      ...initial,
      revision: 8,
      preferences: { ...initial.preferences, candidate_page_size: 9 },
    });
    await waitFor(() => expect(size.value).toBe("9"));
    fireEvent.change(size, { target: { value: "7" } });
    changed?.({
      ...initial,
      revision: 9,
      preferences: { ...initial.preferences, candidate_page_size: 5 },
    });
    // Both windows changed the page size: this window's pending edit wins and is saved over the newer revision.
    expect(size.value).toBe("7");
    expect(await screen.findByText("设置同时在其他窗口修改，已合并。")).toBeDefined();
    await waitFor(() =>
      expect(client.save).toHaveBeenCalledWith(9, {
        ...initial.preferences,
        candidate_page_size: 7,
      }),
    );
  },
);

test("this window's own save echoed back by the monitor is not reported as another window's", async () => {
  let changed: ((snapshot: Snapshot) => void) | undefined;
  let finishSave: ((snapshot: Snapshot) => void) | undefined;
  const saved: Snapshot = {
    ...initial,
    revision: 8,
    preferences: { ...initial.preferences, candidate_page_size: 9 },
  };
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(
      () =>
        new Promise<Snapshot>((resolve) => {
          finishSave = resolve;
        }),
    ),
    onPreferencesChanged: vi.fn(async (listener) => {
      changed = listener;
      return () => {
        changed = undefined;
      };
    }),
    host: { platform: "linux" } as never,
  };
  render(<SettingsPage initialPage="appearance" client={client} />);
  const size = (await screen.findByRole("slider", {
    name: "每页候选项数量",
  })) as HTMLInputElement;
  await waitFor(() => expect(changed).toBeDefined());
  fireEvent.change(size, { target: { value: "9" } });
  saveSettingsNow();
  await waitFor(() => expect(finishSave).toBeDefined());
  // The echo lands between the file write and the invoke resolving, then again afterwards.
  act(() => changed?.(saved));
  await act(async () => finishSave?.(saved));
  act(() => changed?.(saved));
  // And an event older than what is on screen is ignored.
  act(() => changed?.(initial));
  expect(await screen.findByText("已保存")).toBeDefined();
  expect(screen.queryByText("设置同时在其他窗口修改，已合并。")).toBeNull();
  expect(screen.queryByText("设置已从其他窗口更新。")).toBeNull();
  expect(size.value).toBe("9");

  // A genuinely newer revision from elsewhere still applies.
  act(() =>
    changed?.({
      ...initial,
      revision: 9,
      preferences: { ...initial.preferences, candidate_page_size: 5 },
    }),
  );
  await waitFor(() => expect(size.value).toBe("5"));
  expect(await screen.findByText("设置已从其他窗口更新。")).toBeDefined();
});

test("failed initial load never enables saving fabricated defaults", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockRejectedValue({ code: "format" }),
    save: vi.fn(),
  };
  render(<SettingsPage client={client} />);
  await screen.findByRole("alert");
  expect(screen.queryByRole("form", { name: "设置" })).toBeNull();
  expect(client.save).not.toHaveBeenCalled();
});

test("autocorrect is enabled by default and its controls stay out of the settings UI", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
  };
  render(<SettingsPage client={client} />);
  await settingsReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(screen.queryByRole("group", { name: "全拼纠错" })).toBeNull();
  expect(screen.queryByLabelText("全拼纠错：字母顺序错位")).toBeNull();
  expect(screen.queryByLabelText("全拼纠错：相邻键误触")).toBeNull();
});

test("category navigation preserves one draft and saves edits across pages", async () => {
  const client: SettingsClient = {
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn().mockImplementation(async (_revision, preferences) => ({
      ...initial,
      revision: 8,
      preferences,
    })),
  };
  render(<SettingsPage client={client} />);
  // 设置窗口落在导航第一页「输入」。
  const input = screen.getByRole("button", { name: "输入" });
  expect(input.getAttribute("aria-current")).toBe("page");
  fireEvent.click(await screen.findByRole("switch", { name: "全拼辅助码" }));
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("候选窗口");
  expect(screen.queryByRole("switch", { name: "全拼辅助码" })).toBeNull();
  fireEvent.change(screen.getByRole("slider", { name: "每页候选项数量" }), {
    target: { value: "9" },
  });
  fireEvent.click(input);
  expect((screen.getByRole("switch", { name: "全拼辅助码" }) as HTMLInputElement).checked).toBe(
    false,
  );
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(client.save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    candidate_page_size: 9,
    quanpin_helpcode: { enabled: false, schema: "ziranma", show_in_candidate_window: false },
  });
  expect(client.load).toHaveBeenCalledTimes(1);
});

test("category navigation opens every shared settings page at the top", async () => {
  render(<SettingsPage client={{ load: vi.fn().mockResolvedValue(initial), save: vi.fn() }} />);
  await settingsReady();
  const content = screen.getByRole("main");
  content.scrollTop = 480;
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("候选窗口");
  expect(content.scrollTop).toBe(0);
});

test.each(["undo", "clear", "next stroke", "host replacement"])(
  "handwriting ignores delayed recognition after %s",
  async (action) => {
    let resolve!: (result: { candidates: string[] }) => void;
    const recognizeHandwriting = vi.fn(
      () =>
        new Promise<{ candidates: string[] }>((done) => {
          resolve = done;
        }),
    );
    const client = { close: vi.fn().mockResolvedValue(undefined), recognizeHandwriting };
    const panel = render(<HandwritingPanel client={client} />);
    const canvas = screen.getByLabelText("手写画布");
    fireEvent.pointerDown(canvas, { isPrimary: true, clientX: 20, clientY: 20, pointerId: 1 });
    fireEvent.pointerMove(canvas, { clientX: 80, clientY: 80, pointerId: 1 });
    fireEvent.pointerUp(canvas, { pointerId: 1 });
    expect(recognizeHandwriting).toHaveBeenCalledTimes(1);
    if (action === "undo") fireEvent.click(screen.getByRole("button", { name: /撤销/ }));
    if (action === "clear") fireEvent.click(screen.getByRole("button", { name: /重写/ }));
    if (action === "next stroke")
      fireEvent.pointerDown(canvas, { isPrimary: true, clientX: 30, clientY: 30, pointerId: 2 });
    if (action === "host replacement")
      panel.rerender(<HandwritingPanel client={{ close: client.close }} />);
    await act(async () => resolve({ candidates: ["fixture-stale"] }));
    expect(screen.queryByRole("button", { name: "fixture-stale" })).toBeNull();
    if (action === "next stroke") {
      fireEvent.pointerMove(canvas, { clientX: 90, clientY: 90, pointerId: 2 });
      fireEvent.pointerUp(canvas, { pointerId: 2 });
      await act(async () => resolve({ candidates: ["fixture-current"] }));
      expect(screen.getByRole("button", { name: "fixture-current" })).toBeDefined();
    }
  },
);

test("handwriting matches Windows candidate priority, uniqueness, and limit", async () => {
  const recognizeHandwriting = vi.fn().mockResolvedValue({
    candidates: [
      "water",
      "水",
      "water",
      "永",
      "A",
      "木",
      "B",
      "本",
      "C",
      "未",
      "D",
      "末",
      "E",
      "术",
      "F",
      "札",
      "G",
      "正",
      "H",
    ],
  });
  render(<HandwritingPanel client={{ close: vi.fn(), recognizeHandwriting }} />);
  const canvas = screen.getByLabelText("手写画布");
  fireEvent.pointerDown(canvas, { isPrimary: true, clientX: 20, clientY: 20, pointerId: 1 });
  fireEvent.pointerMove(canvas, { clientX: 80, clientY: 80, pointerId: 1 });
  fireEvent.pointerUp(canvas, { clientX: 100, clientY: 100, pointerId: 1 });
  await screen.findByRole("button", { name: "水" });
  expect(
    within(screen.getByRole("group", { name: "识别候选" }))
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(["水", "永", "木", "本", "未", "末", "术", "札", "正", "water", "A", "B"]);
});

test("a host can open the settings window on the section its menu named", async () => {
  const client: SettingsClient = { load: async () => initial, save: vi.fn() };
  render(<SettingsPage client={client} initialPage="about" />);
  expect(await screen.findByRole("heading", { name: "关于" })).toBeDefined();
  cleanup();

  // 本版本没有的页面 id 落到默认页（导航第一页「输入」），而不是打开一个空页面。
  render(<SettingsPage client={client} initialPage="not-a-page" />);
  await settingsFormReady();
  expect(screen.getByRole("heading", { name: "输入" })).toBeDefined();
  cleanup();

  // 「其他平台下载」已并入关于页：按旧 id 打开的宿主落到关于页，那里有这两行。
  render(<SettingsPage client={client} initialPage="download" />);
  expect(await screen.findByRole("heading", { name: "关于" })).toBeDefined();
  expect(screen.getByRole("button", { name: "打开下载页" })).toBeDefined();
  expect(screen.getByRole("button", { name: "查看发布记录" })).toBeDefined();
});

test("a host that fixes the candidate page size and layout does not offer them", async () => {
  const client = (host?: HostCapabilities) => ({
    load: vi.fn().mockResolvedValue(initial),
    save: vi.fn(),
    host,
  });
  const { unmount } = render(
    <SettingsPage
      initialPage="appearance"
      client={client({
        platform: "ios",
        candidate_row_colors: true,
        fixed_candidate_page_size: 9,
        fixed_candidate_layout: "horizontal",
      } as HostCapabilities)}
    />,
  );
  await screen.findByLabelText("候选栏预编辑", undefined, { timeout: 3000 });
  expect(screen.queryByLabelText("每页候选项数量")).toBeNull();
  expect(screen.queryByLabelText("候选项排列方式")).toBeNull();
  unmount();

  render(<SettingsPage initialPage="appearance" client={client()} />);
  expect(await screen.findByLabelText("每页候选项数量", undefined, { timeout: 3000 })).toBeTruthy();
  expect(screen.getByLabelText("候选项排列方式")).toBeTruthy();
});

test("an unreadable preferences document offers a repair that backs it up first", async () => {
  const recovered: Snapshot = {
    ...initial,
    revision: 12,
    preferences: { ...initial.preferences, candidate_page_size: 7 },
  };
  const recoverPreferences = vi.fn().mockResolvedValue({
    snapshot: recovered,
    backupPath: "/Users/synthetic/Library/MSIME/preferences.json.corrupt-20260923-101500",
    salvaged: true,
  });
  const openPreferencesDirectory = vi.fn().mockResolvedValue(undefined);
  const client: SettingsClient = {
    host: { platform: "macos" } as HostCapabilities,
    load: vi.fn().mockRejectedValue({ code: "format" }),
    save: vi.fn(),
    recoverPreferences,
    openPreferencesDirectory,
  };
  render(<SettingsPage client={client} />);
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("配置文件无法读取");

  // Cancelling asks nothing of the host.
  fireEvent.click(within(alert).getByRole("button", { name: "修复配置文件…" }));
  expect((await screen.findByRole("alertdialog")).textContent).toContain(
    "损坏的配置文件会先备份到同一目录",
  );
  await answerConfirm("cancel");
  expect(recoverPreferences).not.toHaveBeenCalled();

  fireEvent.click(within(alert).getByRole("button", { name: "修复配置文件…" }));
  await answerConfirm("confirm");
  expect(recoverPreferences).toHaveBeenCalledTimes(1);
  const notice = await screen.findByText(/配置文件已修复/);
  expect(notice.textContent).toContain("preferences.json.corrupt-20260923-101500");
  expect(screen.queryByRole("alert")).toBeNull();
  await settingsReady();

  fireEvent.click(within(notice).getByRole("button", { name: "在 Finder 中显示" }));
  expect(openPreferencesDirectory).toHaveBeenCalledTimes(1);
});

test("a host without a repair keeps the unreadable-document message alone", async () => {
  render(
    <SettingsPage
      client={{ load: vi.fn().mockRejectedValue({ code: "format" }), save: vi.fn() }}
    />,
  );
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("配置文件无法读取");
  expect(within(alert).queryByRole("button")).toBeNull();
});
