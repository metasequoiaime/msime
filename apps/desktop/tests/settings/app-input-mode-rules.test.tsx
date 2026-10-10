// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type Snapshot } from "@msime/ui";
import {
  appInputModeRuleProblem,
  normalizeAppInputModeRuleId,
  sortedAppInputModeRules,
} from "../../../../packages/ui/src/settings/app-input-mode-rules";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
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
  },
};

function mount(client: Partial<SettingsClient>) {
  return render(<SettingsPage client={{ load: async () => initial, save: vi.fn(), ...client }} />);
}

test("Windows rule identifiers are lower-cased executable base names", () => {
  expect(
    normalizeAppInputModeRuleId("  C:\\Program Files\\Microsoft VS Code\\Code.EXE ", true),
  ).toBe("code.exe");
  // 非 ASCII 字母保持原样，和 Server 只折叠 ASCII 的比较一致。
  expect(normalizeAppInputModeRuleId("微信Ä.exe", true)).toBe("微信Ä.exe");
  // 资源管理器「复制文件地址」给出的路径带一对双引号。
  expect(
    normalizeAppInputModeRuleId(' "C:\\Program Files\\Microsoft VS Code\\Code.exe" ', true),
  ).toBe("code.exe");
  expect(normalizeAppInputModeRuleId('"', true)).toBe('"');
  // macOS 的 bundle id 保留大小写，只去掉两侧空白。
  expect(normalizeAppInputModeRuleId(" com.apple.Terminal ", false)).toBe("com.apple.Terminal");
});

test("rule identifiers are checked the way the preferences store checks them", () => {
  expect(appInputModeRuleProblem("", {}, true)).toBe("empty");
  expect(appInputModeRuleProblem("code", {}, true)).toBe("not_exe");
  expect(appInputModeRuleProblem("com.apple.Terminal", {}, false)).toBeNull();
  expect(appInputModeRuleProblem("bad\u0007.exe", {}, true)).toBe("invalid_character");
  expect(appInputModeRuleProblem("apps/code", {}, false)).toBe("invalid_character");
  expect(appInputModeRuleProblem(`${"a".repeat(61)}.exe`, {}, true)).toBe("too_long");
  expect(appInputModeRuleProblem(`${"a".repeat(60)}.exe`, {}, true)).toBeNull();
  expect(appInputModeRuleProblem("code.exe", { "Code.exe": "english" }, true)).toBe("duplicate");
  const full = Object.fromEntries(
    Array.from({ length: 32 }, (_, index) => [`app${index}.exe`, "chinese" as const]),
  );
  expect(appInputModeRuleProblem("another.exe", full, true)).toBe("too_many");
  expect(sortedAppInputModeRules({ "z.exe": "english", "a.exe": "chinese" })).toEqual([
    ["a.exe", "chinese"],
    ["z.exe", "english"],
  ]);
});

test("a Windows host edits the rule table in the 中英文 group and saves it to the shared preferences", async () => {
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  mount({ host: testHost({ platform: "windows", app_input_mode_rules: true }), save });
  await settingsFormReady();
  const table = await screen.findByRole("group", { name: "应用例外" });
  expect(table.textContent).toContain("还没有应用例外");

  const input = screen.getByLabelText("程序文件名");
  fireEvent.change(input, { target: { value: "notepad" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect((await screen.findByRole("alert")).textContent).toContain(".exe");

  fireEvent.change(input, { target: { value: "C:\\Tools\\Code.EXE" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  const mode = (await screen.findByLabelText("code.exe 的输入模式")) as HTMLSelectElement;
  expect(mode.value).toBe("chinese");
  // 先把新加的这一条存下来：机器忙时自动保存可能已经在添加和改模式之间触发，这里让两次保存的顺序固定下来，而不是取决于计时。
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(3, {
    ...initial.preferences,
    app_input_mode_rules: { "code.exe": "chinese" },
  });
  fireEvent.change(mode, { target: { value: "english" } });
  saveSettingsNow();
  await vi.waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(4, {
      ...initial.preferences,
      app_input_mode_rules: { "code.exe": "english" },
    }),
  );
  expect(save).toHaveBeenCalledTimes(2);

  fireEvent.click(screen.getByRole("button", { name: "移除 code.exe 的应用例外" }));
  expect(screen.queryByLabelText("code.exe 的输入模式")).toBeNull();
});

test("the Enter that commits an IME composition does not add a rule", async () => {
  const save = vi.fn();
  mount({ host: testHost({ platform: "windows", app_input_mode_rules: true }), save });
  await settingsFormReady();
  const input = screen.getByLabelText("程序文件名");
  fireEvent.change(input, { target: { value: "code.exe" } });
  fireEvent.keyDown(input, { key: "Enter", keyCode: 229 });
  fireEvent.keyDown(input, { key: "Enter", isComposing: true });
  expect((input as HTMLInputElement).value).toBe("code.exe");
  expect(screen.getByRole("group", { name: "应用例外" }).textContent).toContain("还没有应用例外");

  fireEvent.keyDown(input, { key: "Enter", keyCode: 13 });
  expect(await screen.findByLabelText("code.exe 的输入模式")).toBeTruthy();
});

test("a Windows host offers the input mode badge and the Alt+Shift+H switch", async () => {
  mount({
    host: testHost({
      platform: "windows",
      input_mode_hud: true,
      fullwidth_chord: true,
      mode_switch_shortcuts: true,
    }),
  });
  await settingsFormReady();
  expect(screen.getByLabelText("中英文切换提示")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(await screen.findByRole("switch", { name: "Alt+Shift+H 切换全半角" })).toBeTruthy();
});

test("hosts that do not apply rules show no rule table", async () => {
  mount({ host: testHost({ platform: "linux", ime_mode_scope: true }) });
  await settingsFormReady();
  expect(screen.queryByRole("group", { name: "应用例外" })).toBeNull();
});
