// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  SettingsPage,
  type MobileKeyboardFeedback,
  type PluginPackage,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

function snapshot(preferences: Record<string, unknown> = {}): Snapshot {
  return {
    format_version: 1,
    revision: 3,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      ...preferences,
    },
  } as Snapshot;
}

function pack(
  id: string,
  name: string,
  kind: PluginPackage["kind"],
  mode?: PluginPackage["mode"],
): PluginPackage {
  return {
    id,
    name,
    version: "1.0.0",
    license: "CC0-1.0",
    author: null,
    description: null,
    builtin: true,
    kind,
    ...(mode ? { mode } : {}),
  };
}

// 内置的五个按键包、一个没写模式的包（按按键包算），再加一个旋律包和一个音乐包：后两个不能当按键音。
const catalogPackages: PluginPackage[] = [
  pack("default", "清脆键盘", "sound", "keys"),
  pack("msime-8bit", "8 位游戏机", "sound", "keys"),
  pack("msime-bubble", "气泡", "sound", "keys"),
  pack("msime-typewriter", "打字机", "sound", "keys"),
  pack("msime-woodblock", "木鱼", "sound"),
  pack("twinkle", "小星星", "sound", "sequence"),
  pack("msime-music-lofi", "Lo-fi", "music"),
];

const feedback: MobileKeyboardFeedback = {
  soundEnabled: true,
  hapticsEnabled: false,
  hapticStrength: "medium",
};

function renderSettings({
  mobile = true,
  capability = mobile,
  sound = true,
  preferences,
}: {
  mobile?: boolean;
  /** 宿主报告 `touch_key_sound_packs`；鸿蒙的设置桥只在手机和平板上报告。 */
  capability?: boolean;
  sound?: boolean;
  preferences?: Record<string, unknown>;
} = {}) {
  const save = vi.fn().mockImplementation(async (_revision: number, next: unknown) => ({
    ...snapshot(),
    revision: 4,
    preferences: next,
  }));
  const catalog = vi.fn().mockResolvedValue({ packages: catalogPackages, issues: [] });
  const previewKeySound = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(snapshot(preferences)),
        save,
        host: testHost({
          platform: "harmony",
          mobile_settings: mobile,
          panel_windows: !mobile,
          touch_toolbar_tools: mobile,
          touch_key_sound_packs: capability,
          // 2in1 照旧在插件页播放音效包，手机不声明。
          key_sound: !mobile,
        }),
        openSystemKeyboardSettings: vi.fn(),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        mobileKeyboardFeedback: {
          load: vi.fn().mockResolvedValue({ ...feedback, soundEnabled: sound }),
          save: vi.fn(async (next: MobileKeyboardFeedback) => next),
          previewKeySound,
        },
        plugins: {
          catalog,
          importPack: vi.fn(),
          remove: vi.fn(),
          loadMentions: vi.fn().mockResolvedValue([]),
          saveMentions: vi.fn(),
        },
      }}
    />,
  );
  return { save, catalog, previewKeySound };
}

/** HarmonyOS 选择器行：标题为 `title` 的那个按钮。 */
function pickerRow(title: string) {
  return screen
    .queryAllByRole("button")
    .find(
      (button) =>
        button.getAttribute("aria-haspopup") === "dialog" &&
        button.querySelector("[data-row-title]")?.textContent === title,
    );
}

function openSheet(title: string) {
  const row = pickerRow(title);
  if (!row) throw new Error(`no picker row ${title}`);
  fireEvent.click(row);
  return screen.getByRole("dialog", { name: title });
}

async function feedbackGroup() {
  return screen.findByRole("group", { name: "按键反馈" });
}

test("a Harmony phone picks the key sound pack right under 按键音 and writes it to the shared preferences", async () => {
  const { save, previewKeySound } = renderSettings();
  await settingsFormReady();
  const group = await feedbackGroup();
  await waitFor(() => expect(pickerRow("音效")).toBeTruthy());
  // 「音效」紧跟在「按键音」下面。
  const titles = [...group.querySelectorAll("[data-row-title]")].map((node) => node.textContent);
  expect(titles.slice(0, 2)).toEqual(["按键音", "音效"]);
  // 没有写过 `plugins` 时是内置的「清脆键盘」。
  expect(pickerRow("音效")?.textContent).toContain("清脆键盘");

  const sheet = openSheet("音效");
  expect(
    within(sheet)
      .getAllByRole("button")
      .filter((button) => button.hasAttribute("data-sheet-option"))
      .map((button) => button.textContent),
  ).toEqual(["清脆键盘", "8 位游戏机", "气泡", "打字机", "木鱼"]);
  fireEvent.click(within(sheet).getByRole("button", { name: "气泡" }));
  expect(pickerRow("音效")?.textContent).toContain("气泡");
  // 选中即试听一声，音量取共享偏好里的音效音量（缺省 50）。
  expect(previewKeySound).toHaveBeenCalledExactlyOnceWith("msime-bubble", 50);

  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1];
  expect(saved.plugins.key_sound.pack).toBe("msime-bubble");
  // 只改了包：2in1 的开关、模式和音量都保持原样。
  expect(saved.plugins.key_sound).toMatchObject({ enabled: false, mode: "keys", volume: 50 });
});

test("the phone keyboard page offers no melody, commit, achievement or music sound settings", async () => {
  renderSettings();
  await settingsFormReady();
  await feedbackGroup();
  await waitFor(() => expect(pickerRow("音效")).toBeTruthy());
  const page = screen.getByRole("group", { name: "屏幕键盘" });
  for (const title of ["发声方式", "旋律", "音效音量", "上屏音", "成就音效", "背景音乐"]) {
    expect(within(page).queryByText(title)).toBeNull();
  }
});

test("the 音效 row is disabled while 按键音 is off", async () => {
  renderSettings({ sound: false });
  await settingsFormReady();
  await feedbackGroup();
  await waitFor(() => expect(pickerRow("音效")).toBeTruthy());
  expect((pickerRow("音效") as HTMLButtonElement).disabled).toBe(true);
});

test("a selected pack the catalog no longer lists is shown as missing", async () => {
  renderSettings({
    preferences: {
      plugins: { key_sound: { enabled: false, mode: "keys", pack: "user-gone", volume: 70 } },
    },
  });
  await settingsFormReady();
  await feedbackGroup();
  await waitFor(() => expect(pickerRow("音效")).toBeTruthy());
  expect(pickerRow("音效")?.textContent).toContain("user-gone（未找到）");
});

test("a Harmony phone without the capability shows no 音效 row and reads no catalog for it", async () => {
  const { catalog } = renderSettings({ capability: false });
  await settingsFormReady();
  await feedbackGroup();
  expect(pickerRow("音效")).toBeUndefined();
  expect(catalog).not.toHaveBeenCalled();
});

test("a Harmony 2in1 keeps its screen keyboard page without the touch 音效 row", async () => {
  renderSettings({ mobile: false });
  await settingsFormReady();
  expect(screen.queryByRole("group", { name: "按键反馈" })).toBeNull();
  expect(screen.queryByText("音效")).toBeNull();
});
