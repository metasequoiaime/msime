// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const initial: Snapshot = {
  format_version: 1,
  revision: 7,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: false },
    shuangpin_helpcode: { enabled: true, schema: "lantian", show_in_candidate_window: true },
  },
};

function renderSettings(platform: string, save = vi.fn().mockResolvedValue(undefined)) {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save,
        host: testHost({ platform }),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );
  return save;
}

// 触屏首页把各设置页列为根行，每行是一个以其标题命名的按钮。
async function openMoreSetting(title: string) {
  await settingsFormReady();
  const home = screen.getByRole("region", { name: "首页" });
  fireEvent.click(within(home).getByRole("button", { name: title }));
}

// The helper-code settings are a group of the 输入 page; the former `helpcode` route opens that page.
async function openHelpcode() {
  await openMoreSetting("输入");
  return screen.getByRole("region", { name: "辅助码" });
}

// The Android keyboard sends helper codes -- Shift during a quanpin or shuangpin
// composition -- and the Engine reads the schema from these preferences, so the
// page has to be reachable there.
test("Android reaches the helper-code settings from the 键盘 tab", async () => {
  renderSettings("android");

  const helpcode = await openHelpcode();
  expect(screen.getByRole("heading", { name: "输入" })).toBeTruthy();
  expect(within(helpcode).getByText(/按 Shift 再输入的字母作为辅助码/)).toBeTruthy();
});

test("Android saves a helper-code schema into shared preferences", async () => {
  const save = renderSettings("android");

  await openHelpcode();
  const schema = screen.getByRole("combobox", { name: /全拼辅助码方案/ }) as HTMLSelectElement;
  expect(schema.value).toBe("ziranma");
  fireEvent.change(schema, { target: { value: "xiaohe" } });

  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        quanpin_helpcode: expect.objectContaining({ schema: "xiaohe" }),
      }),
    ),
  );
});

// A touch host has a candidate row, not a candidate window.
test("mobile names the candidate row rather than a window", async () => {
  renderSettings("android");

  await openHelpcode();
  expect(screen.getByLabelText("在候选栏中显示双拼辅助码")).toBeTruthy();
  expect(screen.getByLabelText("在候选栏中显示全拼辅助码")).toBeTruthy();
  expect(screen.queryByLabelText("在候选窗口中显示双拼辅助码")).toBeNull();
});

// The iOS keyboard extension marks a helper code with Shift like Android, and its host says so; the page follows that capability.
test("iOS reaches the helper-code settings when its keyboard marks helper codes", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "ios", helpcode_shift_entry: true }),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );

  await openHelpcode();
  expect(screen.getByText(/按 Shift\s*再输入的字母作为辅助码/)).toBeTruthy();
  expect(screen.getByLabelText("在候选栏中显示全拼辅助码")).toBeTruthy();
});

// A host that does not report the Shift helper-code gesture keeps the group hidden.
test("an iOS host without the capability keeps the helper-code settings hidden", async () => {
  renderSettings("ios");

  await openMoreSetting("输入");
  expect(screen.queryByRole("region", { name: "辅助码" })).toBeNull();
});

test("the desktop input page keeps the helper-code settings and their window wording", async () => {
  renderSettings("windows");

  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(screen.getByRole("region", { name: "辅助码" })).toBeTruthy();
  expect(screen.getByLabelText("在候选窗口中显示双拼辅助码")).toBeTruthy();
  expect(screen.getByLabelText("在候选窗口中显示全拼辅助码")).toBeTruthy();
});

// HarmonyOS ships the same helper-code input: its ChineseHelpcodePolicy is the Android one,
// ported, and the session calls it on every shifted key during a quanpin or shuangpin
// composition. The page was hidden there anyway, which left a shipping feature with no way to
// pick a schema or turn it off — the state this file's Android tests exist to prevent.
test("HarmonyOS reaches the helper-code settings from the 键盘 tab", async () => {
  renderSettings("harmony");

  await openHelpcode();
  expect(screen.getByRole("heading", { name: "输入" })).toBeTruthy();
});

test("HarmonyOS saves a helper-code schema into shared preferences", async () => {
  const save = renderSettings("harmony");

  await openHelpcode();
  const schema = screen.getByRole("combobox", { name: /全拼辅助码方案/ }) as HTMLSelectElement;
  expect(schema.value).toBe("ziranma");
  fireEvent.change(schema, { target: { value: "xiaohe" } });

  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        quanpin_helpcode: expect.objectContaining({ schema: "xiaohe" }),
      }),
    ),
  );
});

// The gesture is the host's, not the platform's: Windows appends a helper code to a finished
// spelling and needs none, while the hosts running the ported ChineseHelpcodePolicy mark it with
// Shift. HarmonyOS reached the page before it could read this sentence.
test("the Shift explanation follows the capability, not the platform name", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "harmony", helpcode_shift_entry: true }),
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );
  await openHelpcode();
  expect(screen.getByText(/按 Shift\s*再输入的字母作为辅助码/)).toBeTruthy();
});

test("a host that appends helper codes is not told to hold Shift", async () => {
  render(
    <SettingsPage
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: testHost({ platform: "windows", helpcode_shift_entry: false }),
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(screen.getByRole("region", { name: "辅助码" })).toBeTruthy();
  expect(screen.queryByText(/按 Shift\s*再输入的字母作为辅助码/)).toBeNull();
});
