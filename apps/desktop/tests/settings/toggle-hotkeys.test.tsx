// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  SettingsPage,
  type SettingsClient,
  type Snapshot,
  type ToggleHotkeyCheckResult,
} from "@msime/ui";
import { saveSettingsNow, selectSettingsTab, settingsFormReady } from "../support/settings-form";
import { testHost } from "../support/host";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    keybindings: {
      toggle_english_candidates: "Ctrl+Shift+KeyE",
      toggle_full_width: "Ctrl+Shift+Space",
      toggle_punctuation: "Ctrl+Period",
    },
  } as Snapshot["preferences"],
};

function mount(client: Partial<SettingsClient>) {
  return render(<SettingsPage client={{ load: async () => initial, save: vi.fn(), ...client }} />);
}

const host = () =>
  testHost({ platform: "macos", mode_switch_shortcuts: true, toggle_hotkey_chords: true });

async function openShortcuts(client: Partial<SettingsClient>) {
  mount({ host: host(), ...client });
  await settingsFormReady();
  await act(async () => {
    selectSettingsTab("快捷键");
  });
}

const ok = (chord: string): ToggleHotkeyCheckResult => ({ ok: true, chord });

test("三个切换热键在快捷键页各有一个录键控件，显示当前绑定", async () => {
  await openShortcuts({ checkToggleHotkey: async () => ok("") });
  for (const title of ["中英切换（英文候选模式）", "全角 / 半角切换", "中英文标点切换"]) {
    expect(screen.getByText(title)).toBeTruthy();
  }
  expect(screen.getByText("Ctrl+Shift+KeyE")).toBeTruthy();
  expect(screen.getByText("Ctrl+Shift+Space")).toBeTruthy();
  expect(screen.getByText("Ctrl+Period")).toBeTruthy();
});

test("录下合法组合键经宿主校验后写入规范拼写", async () => {
  const check = vi.fn(async () => ok("Ctrl+Alt+KeyR"));
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  await openShortcuts({ checkToggleHotkey: check, save });
  const row = screen.getByText("中英切换（英文候选模式）").closest("div")!;
  const button = row.querySelector("button")!;
  fireEvent.click(button);
  fireEvent.keyDown(button, { code: "KeyR", ctrlKey: true, altKey: true });
  await act(async () => {});
  expect(check).toHaveBeenCalledWith(
    expect.objectContaining({ field: "toggle_english_candidates", chord: "Ctrl+Alt+KeyR" }),
  );
});

test("Esc 取消录制不写入", async () => {
  const check = vi.fn(async () => ok("Ctrl+Alt+KeyR"));
  await openShortcuts({ checkToggleHotkey: check });
  const row = screen.getByText("中英文标点切换").closest("div")!;
  const button = row.querySelector("button")!;
  fireEvent.click(button);
  fireEvent.keyDown(button, { code: "Escape" });
  expect(check).not.toHaveBeenCalled();
});

test("物理录键请求布局归一并显示宿主返回的主键", async () => {
  const check = vi.fn(async () => ok("Ctrl+Alt+Period"));
  await openShortcuts({ checkToggleHotkey: check });
  const button = screen.getByText("中英文标点切换").closest("div")!.querySelector("button")!;
  fireEvent.click(button);
  // Dvorak 物理 E 键输出句号，Option 修改后的字符不能作为绑定身份。
  fireEvent.keyDown(button, { code: "KeyE", key: "…", ctrlKey: true, altKey: true });
  await act(async () => {});
  expect(check).toHaveBeenCalledWith(
    expect.objectContaining({
      chord: "Ctrl+Alt+KeyE",
      physical_recording: true,
    }),
  );
  expect(screen.getByText("Ctrl+Alt+Period")).toBeTruthy();
});

test("Backspace 解绑并把行显示成未绑定", async () => {
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  await openShortcuts({ checkToggleHotkey: async () => ok(""), save });
  const row = screen.getByText("中英文标点切换").closest("div")!;
  const button = row.querySelector("button")!;
  fireEvent.click(button);
  fireEvent.keyDown(button, { code: "Backspace" });
  await act(async () => {});
  expect(within(row).getByText("未绑定")).toBeTruthy();
});

test("宿主报冲突时行内指出冲突项且不写入", async () => {
  const check = vi.fn(async (): Promise<ToggleHotkeyCheckResult> => ({
    ok: false,
    kind: "conflict",
    owner: "toggle_full_width",
    chord: "Ctrl+Shift+Space",
  }));
  await openShortcuts({ checkToggleHotkey: check });
  const row = screen.getByText("中英文标点切换").closest("div")!;
  const button = row.querySelector("button")!;
  fireEvent.click(button);
  fireEvent.keyDown(button, { code: "Space", ctrlKey: true, shiftKey: true });
  await act(async () => {});
  expect(screen.getByText(/与「全角 \/ 半角切换」相同/)).toBeTruthy();
});

test("宿主报非法写法时行内报错且不写入", async () => {
  const check = vi.fn(async (): Promise<ToggleHotkeyCheckResult> => ({
    ok: false,
    kind: "invalid",
    detail: "key chord has no main key",
  }));
  await openShortcuts({ checkToggleHotkey: check });
  const row = screen.getByText("中英文标点切换").closest("div")!;
  const button = row.querySelector("button")!;
  fireEvent.click(button);
  fireEvent.keyDown(button, { code: "Tab", ctrlKey: true });
  await act(async () => {});
  expect(screen.getByText("无法识别的组合键")).toBeTruthy();
});

async function openVoiceHotkey(client: Partial<SettingsClient>) {
  mount({ host: host(), ...client });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  return within(screen.getByRole("region", { name: "自定义语音快捷键" }));
}

test.each(["macos", "linux"] as const)(
  "%s 语音页录入组合键经共享校验归一，保存时保留其他绑定",
  async (platform) => {
    const check = vi.fn(async () => ok("Ctrl+Alt+F8"));
    const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
      ...initial,
      revision: 4,
      preferences,
    }));
    const group = await openVoiceHotkey({
      host: testHost({ platform }),
      checkToggleHotkey: check,
      save,
    });
    const button = group.getByRole("button", { name: "录制快捷键" });
    expect(group.getByText("未绑定")).toBeTruthy();
    fireEvent.click(button);
    fireEvent.keyDown(button, { code: "F8", ctrlKey: true, altKey: true });
    await waitFor(() => expect(group.getByText("Ctrl+Alt+F8")).toBeTruthy());
    expect(check).toHaveBeenCalledWith(
      expect.objectContaining({
        field: "toggle_voice_input",
        chord: "Ctrl+Alt+F8",
        voice_ctrl_f9: true,
      }),
    );
    saveSettingsNow();
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith(
        3,
        expect.objectContaining({
          keybindings: expect.objectContaining({
            ...initial.preferences.keybindings,
            toggle_voice_input: "Ctrl+Alt+F8",
          }),
        }),
      ),
    );
  },
);

test("语音录键取消不校验，冲突拒绝后仍为未绑定", async () => {
  const check = vi.fn(async (): Promise<ToggleHotkeyCheckResult> => ({
    ok: false,
    kind: "conflict",
    owner: "toggle_punctuation",
    chord: "Ctrl+Period",
  }));
  const group = await openVoiceHotkey({ checkToggleHotkey: check });
  const button = group.getByRole("button", { name: "录制快捷键" });
  fireEvent.click(button);
  fireEvent.keyDown(button, { code: "Escape" });
  expect(check).not.toHaveBeenCalled();
  fireEvent.click(button);
  fireEvent.keyDown(button, { code: "Period", ctrlKey: true });
  await waitFor(() => expect(group.getByText(/与「中英文标点切换」相同/)).toBeTruthy());
  expect(group.getByText("未绑定")).toBeTruthy();
});

test("语音页没有校验桥时不显示录入入口", async () => {
  mount({ host: host() });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  expect(screen.queryByRole("region", { name: "自定义语音快捷键" })).toBeNull();
});

test("语音录键校验桥失败时显示重试提示并保留原绑定", async () => {
  const save = vi.fn();
  const group = await openVoiceHotkey({
    checkToggleHotkey: async () => {
      throw new Error("synthetic unavailable");
    },
    save,
  });
  fireEvent.click(group.getByRole("button", { name: "录制快捷键" }));
  fireEvent.keyDown(group.getByRole("button", { name: "正在录制快捷键" }), {
    code: "F8",
    key: "F8",
    ctrlKey: true,
    altKey: true,
  });
  expect(await screen.findByText("无法检查快捷键，请稍后重试")).toBeTruthy();
  expect(group.getByText("未绑定")).toBeTruthy();
  expect(save).not.toHaveBeenCalled();
});
