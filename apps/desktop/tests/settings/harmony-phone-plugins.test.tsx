// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  SettingsPage,
  type CommunityPlugin,
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
  extra: Partial<PluginPackage> = {},
): PluginPackage {
  return {
    id,
    name,
    version: "1.0.0",
    license: "CC0-1.0",
    author: null,
    description: null,
    builtin: false,
    kind,
    ...extra,
  };
}

// 触屏键盘用得上的四类各一个，再加五个手机上用不上的：旋律包、特效包、音乐包，以及进不了 K 和 / 模式的指令表和短语表。
const catalogPackages: PluginPackage[] = [
  pack("default", "清脆键盘", "sound", { builtin: true, mode: "keys" }),
  pack("user-keys", "合成按键", "sound", { mode: "keys" }),
  pack("twinkle", "小星星", "sound", { builtin: true, mode: "sequence" }),
  pack("spark", "合成火花", "effect", { style: "sparks" }),
  pack("lofi", "合成音乐", "music", { tracks: ["a.ogg"] }),
  pack("work", "合成指令", "command_table", {
    commands: [{ trigger: "sig", title: "签名", template: "合成签名" }],
  }),
  pack("office", "合成短语", "phrase_table", { phrases: [{ key: "dz", text: "合成地址" }] }),
  pack("codes", "合成辅助码", "helpcode", { entries: 1, preview: [] }),
  pack("words", "合成单词本", "wordbook", { word_count: 1, first_words: ["apple"] }),
  pack("marks", "合成符号", "symbol_set", {
    groups: [{ tab: "symbols", title: "箭头", keywords: "", items: ["→"] }],
  }),
];

const feedback: MobileKeyboardFeedback = {
  soundEnabled: true,
  hapticsEnabled: false,
  hapticStrength: "medium",
};

function communityPlugin(id: string, kind: CommunityPlugin["kind"], name: string): CommunityPlugin {
  return {
    id,
    kind,
    plugin_id: id,
    name,
    description: "",
    author: "合成作者",
    version: "1.0.0",
    license: "CC0-1.0",
    size: 1,
    sha256: "",
    downloads: 0,
    rating_count: 0,
    rating_average: 0,
    owned: false,
    my_rating: 0,
    created_at: "2026-10-26T00:00:00Z",
  };
}

function renderSettings({
  mobile = true,
  capability = mobile,
  sound = true,
  preferences,
}: {
  mobile?: boolean;
  /** 宿主报告 `touch_plugin_packs`；鸿蒙的设置桥只在手机和平板上报告。 */
  capability?: boolean;
  sound?: boolean;
  preferences?: Record<string, unknown>;
} = {}) {
  const save = vi.fn().mockImplementation(async (_revision: number, next: unknown) => ({
    ...snapshot(),
    revision: 4,
    preferences: next,
  }));
  const saveFeedback = vi.fn(async (next: MobileKeyboardFeedback) => next);
  const importPack = vi.fn().mockResolvedValue(null);
  const list = vi.fn().mockResolvedValue({
    plugins: [
      communityPlugin("c-music", "music", "社区音乐"),
      communityPlugin("c-phrase", "phrase_table", "社区短语"),
      communityPlugin("c-effect", "effect", "社区特效"),
      communityPlugin("c-words", "wordbook", "社区单词本"),
    ],
    has_more: false,
  });
  render(
    <SettingsPage
      initialPage="plugins"
      client={{
        load: vi.fn().mockResolvedValue(snapshot(preferences)),
        save,
        host: testHost({
          platform: "harmony",
          mobile_settings: mobile,
          panel_windows: !mobile,
          touch_key_sound_packs: mobile,
          touch_plugin_packs: capability,
          // 鸿蒙每种形态都声明单词本和符号集；2in1 另外路由 V、/ 和 @，播放音效、音乐并画特效。
          plugin_triggers: !mobile,
          wordbook_packs: true,
          symbol_set_packs: true,
          key_sound: !mobile,
          music: !mobile,
          typing_effects: !mobile,
        }),
        openSystemKeyboardSettings: vi.fn(),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
        mobileKeyboardFeedback: {
          load: vi.fn().mockResolvedValue({ ...feedback, soundEnabled: sound }),
          save: saveFeedback,
        },
        plugins: {
          catalog: vi.fn().mockResolvedValue({ packages: catalogPackages, issues: [] }),
          importPack,
          remove: vi.fn(),
          loadMentions: vi.fn().mockResolvedValue([]),
          saveMentions: vi.fn(),
          loadAuthoredPack: vi.fn(),
          saveAuthoredPack: vi.fn(),
        },
        communityPlugins: {
          list,
          detail: vi.fn(async (id: string) => communityPlugin(id, "phrase_table", "社区短语")),
          packPreview: vi.fn(),
          publish: vi.fn(),
          install: vi.fn(),
          rate: vi.fn(),
          delete: vi.fn(),
        },
      }}
    />,
  );
  return { save, saveFeedback, importPack, list };
}

async function installedList() {
  return screen.findByLabelText("已安装的插件");
}

async function savedPreferences(save: ReturnType<typeof vi.fn>) {
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalled());
  return save.mock.calls.at(-1)?.[1];
}

test("a Harmony phone lists only the packs its touch keyboard uses and counts the rest", async () => {
  renderSettings();
  await settingsFormReady();
  expect(await screen.findByRole("group", { name: "插件" })).toBeTruthy();
  const list = await installedList();
  await within(list).findByText("合成按键");
  for (const name of ["清脆键盘", "合成按键", "合成辅助码", "合成单词本", "合成符号"]) {
    expect(within(list).getByText(name)).toBeTruthy();
  }
  for (const name of ["小星星", "合成火花", "合成音乐", "合成指令", "合成短语"]) {
    expect(within(list).queryByText(name)).toBeNull();
  }
  expect(within(list).queryByRole("group", { name: "特效包" })).toBeNull();
  expect(within(list).queryByRole("group", { name: "音乐包" })).toBeNull();
  expect(
    within(list).getByText(
      "另有 5 个已安装的插件（音效包·旋律、特效包、音乐包、指令表、短语表）在这台设备上用不到，没有列出。",
    ),
  ).toBeTruthy();
  // 手机上没有声音与效果：开关在「键盘 → 按键反馈」。
  expect(screen.queryByText("声音与效果")).toBeNull();
  expect(screen.getByRole("button", { name: "新建插件" })).toBeTruthy();
});

test("the phone imports a .zip only: its file picker cannot pick a folder", async () => {
  const { importPack } = renderSettings();
  await settingsFormReady();
  await installedList();
  expect(screen.queryByRole("button", { name: "导入文件夹" })).toBeNull();
  fireEvent.click(await screen.findByRole("button", { name: "导入 .zip" }));
  await waitFor(() => expect(importPack).toHaveBeenCalledExactlyOnceWith("archive"));
});

test("a phone without touch_plugin_packs keeps the plugin page hidden", async () => {
  renderSettings({ capability: false });
  await settingsFormReady();
  // 页面仍挂载在表单里，只是隐藏：按角色找不到它。
  expect(screen.queryByRole("group", { name: "插件" })).toBeNull();
});

test("the 2-in-1 keeps the desktop plugin page with every kind and folder import", async () => {
  renderSettings({ mobile: false });
  await settingsFormReady();
  const list = await installedList();
  await within(list).findByText("合成火花");
  expect(within(list).getByText("合成音乐")).toBeTruthy();
  expect(within(list).getByText("小星星")).toBeTruthy();
  expect(within(list).queryByText(/在这台设备上用不到/)).toBeNull();
  expect(screen.getByRole("button", { name: "导入文件夹" })).toBeTruthy();
});

test("setting a key sound pack on the phone writes only the pack and turns 按键音 on when it is off", async () => {
  const { save, saveFeedback } = renderSettings({ sound: false });
  await settingsFormReady();
  const list = await installedList();
  // 选中的包按触屏「按键音」开关标记。
  await waitFor(() => expect(within(list).getByText("已选 · 按键音未开")).toBeTruthy());
  fireEvent.click(within(list).getByText("合成按键"));
  expect(screen.getByText(/按键音现在没有打开。设为按键音效会同时打开/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "设为按键音效" }));
  await waitFor(() =>
    expect(saveFeedback).toHaveBeenCalledExactlyOnceWith({ ...feedback, soundEnabled: true }),
  );
  const saved = await savedPreferences(save);
  expect(saved.plugins.key_sound.pack).toBe("user-keys");
  // 2in1 的开关、模式和音量不动。
  expect(saved.plugins.key_sound).toMatchObject({ enabled: false, mode: "keys" });
});

test("the phone neither lists nor offers to create command and phrase tables: it has no way into K or /", async () => {
  renderSettings({ preferences: { local_modes: { quick_phrase: false, command: false } } });
  await settingsFormReady();
  const list = await installedList();
  await within(list).findByText("合成按键");
  // 手机和平板没有「快捷模式」组和「本地输入」磁贴，列出的表只会指向不存在的入口。
  expect(screen.queryByText(/「输入 → 快捷模式」/)).toBeNull();
  expect(screen.queryByText(/「本地输入」/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "新建插件" }));
  const kinds = await screen.findByRole("region", { name: "想做哪一种插件？" });
  expect(within(kinds).getByText("符号集")).toBeTruthy();
  expect(within(kinds).getByText("单词本")).toBeTruthy();
  expect(within(kinds).queryByText("短语表")).toBeNull();
  expect(within(kinds).queryByText("指令表")).toBeNull();
});

test("the 2-in-1 keeps command and phrase tables and offers to create them", async () => {
  renderSettings({ mobile: false });
  await settingsFormReady();
  const list = await installedList();
  await within(list).findByText("合成指令");
  expect(within(list).getByText("合成短语")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "新建插件" }));
  const kinds = await screen.findByRole("region", { name: "想做哪一种插件？" });
  expect(within(kinds).getByText("短语表")).toBeTruthy();
  expect(within(kinds).getByText("指令表")).toBeTruthy();
});

test("a pack detail on the phone has its own history entry, so system back returns to the list", async () => {
  renderSettings();
  await settingsFormReady();
  const list = await installedList();
  const before = window.history.length;
  fireEvent.click(await within(list).findByText("合成单词本"));
  expect(screen.getByRole("button", { name: "返回我的插件" })).toBeTruthy();
  expect(window.history.length).toBe(before + 1);
  expect(window.history.state.pluginView).toEqual({
    kind: "pack",
    packKind: "wordbook",
    id: "words",
  });
  act(() => {
    window.history.back();
  });
  await waitFor(() => expect(screen.queryByRole("button", { name: "返回我的插件" })).toBeNull());
  expect(await installedList()).toBeTruthy();
});

test("the phone's community gallery offers and lists only the kinds its keyboard uses", async () => {
  const { list } = renderSettings();
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("tab", { name: "社区插件" }));
  await waitFor(() => expect(list).toHaveBeenCalled());
  const filters = screen.getByRole("group", { name: "插件类型" });
  const labels = within(filters)
    .getAllByRole("button")
    .map((button) => button.textContent);
  expect(labels).toEqual(["全部", "音效包", "辅助码表", "单词本", "符号集"]);
  await screen.findByText("社区单词本");
  expect(screen.queryByText("社区短语")).toBeNull();
  expect(screen.queryByText("社区音乐")).toBeNull();
  expect(screen.queryByText("社区特效")).toBeNull();
});
