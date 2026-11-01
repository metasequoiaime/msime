// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsPage,
  AppInputModeRulesSection,
  type SettingsClient,
  type Snapshot,
  appInputModeRuleProblem,
  normalizeAppInputModeRuleId,
  sortedAppInputModeRules,
} from "@msime/ui";
import { selectSettingsTab, saveSettingsNow, settingsFormReady } from "../support/settings-form";
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

test("rule duplicates fold ASCII letters while keeping non-ASCII case distinct", () => {
  const rules = { "Äpp.exe": "english" } as const;
  expect(appInputModeRuleProblem("ÄPP.EXE", rules, true)).toBe("duplicate");
  expect(appInputModeRuleProblem("äpp.exe", rules, true)).toBeNull();
});

test("a Windows host edits the rule table in the 中英文 group and saves it to the shared preferences", async () => {
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  mount({ host: testHost({ platform: "windows", app_input_mode_rules: true }), save });
  await settingsFormReady();
  selectSettingsTab("中英文");
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
  selectSettingsTab("中英文");
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
  selectSettingsTab("中英文");
  expect(screen.getByLabelText("中英文切换提示")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /^(快捷键|按键设置)$/ }));
  expect(await screen.findByRole("switch", { name: "Alt+Shift+H 切换全半角" })).toBeTruthy();
});

test("Linux rule identifiers are program names, a pasted path keeps its file name", () => {
  expect(normalizeAppInputModeRuleId("  /usr/bin/Firefox ", false, true)).toBe("Firefox");
  expect(normalizeAppInputModeRuleId(" org.gnome.TextEditor ", false, true)).toBe(
    "org.gnome.TextEditor",
  );
});

test("a Linux host takes typed program names, never offers the picker and says which frameworks report them", async () => {
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  const pickApplication = vi.fn(async () => null);
  const applicationNames = vi.fn(async () => ({}));
  mount({
    host: testHost({ platform: "linux", app_input_mode_rules: true }),
    save,
    pickApplication,
    applicationNames,
  });
  await settingsFormReady();
  selectSettingsTab("中英文");
  const table = await screen.findByRole("group", { name: "应用例外" });
  // 框架说明收进了组名旁的「?」，按整组（含气泡里的说明文字）读。
  const group = table.closest("section")!;
  expect(group.textContent).toContain("Fcitx5");
  expect(group.textContent).toContain("GNOME Wayland");
  expect(screen.queryByRole("button", { name: "添加应用…" })).toBeNull();
  const input = screen.getByLabelText("程序名");
  expect((input as HTMLInputElement).placeholder).toBe("例如 firefox");
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect((await screen.findByRole("alert")).textContent).toContain("请填写程序名");
  fireEvent.change(input, { target: { value: "/usr/bin/code" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(await screen.findByLabelText("code 的输入模式")).toBeTruthy();
  saveSettingsNow();
  await vi.waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(3, {
      ...initial.preferences,
      app_input_mode_rules: { code: "chinese" },
    }),
  );
  expect(pickApplication).not.toHaveBeenCalled();
  expect(applicationNames).not.toHaveBeenCalled();
});

test("hosts that do not apply rules show no rule table", async () => {
  mount({ host: testHost({ platform: "linux", ime_mode_scope: true }) });
  await settingsFormReady();
  selectSettingsTab("中英文");
  expect(screen.queryByRole("group", { name: "应用例外" })).toBeNull();
});

test("a host with an application picker adds rules by picking an application and lists them by name", async () => {
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  const pickApplication = vi
    .fn<NonNullable<SettingsClient["pickApplication"]>>()
    .mockResolvedValueOnce(null)
    .mockRejectedValueOnce({ code: "not_application" })
    .mockResolvedValueOnce({ id: "com.microsoft.VSCode", name: "Visual Studio Code" });
  const applicationNames = vi.fn(async (ids: string[]): Promise<Record<string, string>> =>
    ids.includes("com.apple.Terminal") ? { "com.apple.Terminal": "终端" } : {},
  );
  render(
    <SettingsPage
      client={{
        load: async () => ({
          ...initial,
          preferences: {
            ...initial.preferences,
            app_input_mode_rules: {
              "com.apple.Terminal": "english",
              "org.example.gone": "chinese",
            },
          },
        }),
        save,
        host: testHost({ platform: "macos", app_input_mode_rules: true }),
        pickApplication,
        applicationNames,
      }}
    />,
  );
  await settingsFormReady();
  selectSettingsTab("中英文");
  const table = await screen.findByRole("group", { name: "应用例外" });
  // 已有规则按名称列出，Bundle ID 退到说明里；查不到名称（没装、别的机器同步来）的照旧显示 Bundle ID。
  expect(((await screen.findByLabelText("终端 的输入模式")) as HTMLSelectElement).value).toBe(
    "english",
  );
  expect(table.textContent).toContain("com.apple.Terminal");
  expect(screen.getByLabelText("org.example.gone 的输入模式")).toBeTruthy();
  // 有选择对话框时不再让人手填 Bundle ID。
  expect(screen.queryByLabelText("应用 Bundle ID")).toBeNull();

  const add = screen.getByRole("button", { name: "添加应用…" });
  fireEvent.click(add);
  await waitFor(() => expect((add as HTMLButtonElement).disabled).toBe(false));
  expect(pickApplication).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("alert")).toBeNull();

  fireEvent.click(add);
  expect((await screen.findByRole("alert")).textContent).toContain("不是应用程序");
  await waitFor(() => expect((add as HTMLButtonElement).disabled).toBe(false));

  fireEvent.click(add);
  const mode = (await screen.findByLabelText("Visual Studio Code 的输入模式")) as HTMLSelectElement;
  expect(mode.value).toBe("chinese");
  expect(screen.queryByRole("alert")).toBeNull();
  saveSettingsNow();
  await vi.waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(3, {
      ...initial.preferences,
      app_input_mode_rules: {
        "com.apple.Terminal": "english",
        "org.example.gone": "chinese",
        "com.microsoft.VSCode": "chinese",
      },
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "移除 Visual Studio Code 的应用例外" }));
  expect(screen.queryByLabelText("Visual Studio Code 的输入模式")).toBeNull();
});

test("a Windows program picked from the dialog becomes its lower-cased file name", async () => {
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  const applicationNames = vi.fn(async (): Promise<Record<string, string>> => ({}));
  mount({
    host: testHost({ platform: "windows", app_input_mode_rules: true }),
    save,
    pickApplication: async () => ({ id: "Code.exe", name: "Code" }),
    applicationNames,
  });
  await settingsFormReady();
  selectSettingsTab("中英文");
  fireEvent.click(await screen.findByRole("button", { name: "添加应用…" }));
  expect(await screen.findByLabelText("Code 的输入模式")).toBeTruthy();
  expect(screen.getByRole("group", { name: "应用例外" }).textContent).toContain("code.exe");
  saveSettingsNow();
  await vi.waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(3, {
      ...initial.preferences,
      app_input_mode_rules: { "code.exe": "chinese" },
    }),
  );
  // Windows 的规则就是程序文件名，不向宿主查名称。
  expect(applicationNames).not.toHaveBeenCalled();
});

function nameRequest() {
  let resolve!: (names: Record<string, string>) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<Record<string, string>>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

const firstApp = "org.example.first";
const secondApp = "org.example.second";

test.each(["成功", "失败"])("应用名称查询函数更换后，旧查询%s不覆盖新名称", async (outcome) => {
  const old = nameRequest();
  const current = nameRequest();
  const oldNames = vi.fn(() => old.promise);
  const currentNames = vi.fn(() => current.promise);
  const props = { windows: false, value: { [firstApp]: "english" as const }, onChange: vi.fn() };
  const view = render(<AppInputModeRulesSection {...props} applicationNames={oldNames} />);
  expect(oldNames).toHaveBeenCalledWith([firstApp]);
  view.rerender(<AppInputModeRulesSection {...props} applicationNames={currentNames} />);
  expect(currentNames).toHaveBeenCalledWith([firstApp]);
  await act(async () => {
    if (outcome === "成功") old.resolve({ [firstApp]: "旧应用" });
    else old.reject(new Error("synthetic old name query"));
  });
  expect(screen.queryByLabelText("旧应用 的输入模式")).toBeNull();
  await act(async () => current.resolve({ [firstApp]: "当前应用" }));
  expect(screen.getByLabelText("当前应用 的输入模式")).toHaveProperty("value", "english");
  expect(currentNames).toHaveBeenCalledTimes(1);
});

test.each(["成功", "失败"])("规则标识变化后，旧名称查询%s不缓存已经离开的应用", async (outcome) => {
  const old = nameRequest();
  const current = nameRequest();
  const applicationNames = vi
    .fn()
    .mockReturnValueOnce(old.promise)
    .mockReturnValueOnce(current.promise)
    .mockResolvedValueOnce({ [firstApp]: "重新查询的应用" });
  const tree = (id: string) => (
    <AppInputModeRulesSection
      windows={false}
      value={{ [id]: "chinese" }}
      applicationNames={applicationNames}
      onChange={vi.fn()}
    />
  );
  const view = render(tree(firstApp));
  view.rerender(tree(secondApp));
  await act(async () => {
    if (outcome === "成功") old.resolve({ [firstApp]: "旧应用" });
    else old.reject(new Error("synthetic old batch"));
  });
  await act(async () => current.resolve({ [secondApp]: "第二个应用" }));
  expect(screen.getByLabelText("第二个应用 的输入模式")).toBeTruthy();
  view.rerender(tree(firstApp));
  expect(await screen.findByLabelText("重新查询的应用 的输入模式")).toBeTruthy();
  expect(applicationNames.mock.calls).toEqual([[[firstApp]], [[secondApp]], [[firstApp]]]);
});

test.each(["缺失", "失败"])("应用名称%s时缓存标识回退，改模式不重复查询", async (outcome) => {
  const applicationNames =
    outcome === "缺失"
      ? vi.fn(async () => ({}))
      : vi.fn(async () => {
          throw new Error("synthetic name lookup failure");
        });
  const props = { windows: false, applicationNames, onChange: vi.fn() };
  const view = render(<AppInputModeRulesSection {...props} value={{ [firstApp]: "chinese" }} />);
  await waitFor(() => expect(applicationNames).toHaveBeenCalledTimes(1));
  await act(async () => undefined);
  view.rerender(<AppInputModeRulesSection {...props} value={{ [firstApp]: "english" }} />);
  expect(screen.getByLabelText(`${firstApp} 的输入模式`)).toHaveProperty("value", "english");
  expect(applicationNames).toHaveBeenCalledTimes(1);
});

test("StrictMode 重放后，只接受当前应用名称查询", async () => {
  const old = nameRequest();
  const current = nameRequest();
  const applicationNames = vi
    .fn()
    .mockReturnValueOnce(old.promise)
    .mockReturnValueOnce(current.promise);
  render(
    <StrictMode>
      <AppInputModeRulesSection
        windows={false}
        value={{ [firstApp]: "chinese" }}
        applicationNames={applicationNames}
        onChange={vi.fn()}
      />
    </StrictMode>,
  );
  expect(applicationNames).toHaveBeenCalledTimes(2);
  await act(async () => old.resolve({ [firstApp]: "重放前的应用" }));
  expect(screen.queryByLabelText("重放前的应用 的输入模式")).toBeNull();
  await act(async () => current.resolve({ [firstApp]: "重放后的应用" }));
  expect(screen.getByLabelText("重放后的应用 的输入模式")).toBeTruthy();
});

function pickerRequest() {
  let resolve!: (value: { id: string; name: string } | null) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<{ id: string; name: string } | null>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test("选择应用期间规则变化，完成时合并当前规则而不恢复已删除项", async () => {
  const request = pickerRequest();
  const pickApplication = vi.fn(() => request.promise);
  const onChange = vi.fn();
  const view = render(
    <AppInputModeRulesSection
      windows={false}
      value={{ "org.example.deleted": "english" }}
      pickApplication={pickApplication}
      onChange={onChange}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "添加应用…" }));
  view.rerender(
    <AppInputModeRulesSection
      windows={false}
      value={{ "org.example.current": "english" }}
      pickApplication={pickApplication}
      onChange={onChange}
    />,
  );
  await act(async () => request.resolve({ id: "org.example.picked", name: "合成所选应用" }));
  expect(onChange).toHaveBeenCalledExactlyOnceWith({
    "org.example.current": "english",
    "org.example.picked": "chinese",
  });
});

test("同一批点击只打开一个应用选择对话框", async () => {
  const request = pickerRequest();
  const pickApplication = vi.fn(() => request.promise);
  render(
    <AppInputModeRulesSection
      windows={false}
      pickApplication={pickApplication}
      onChange={vi.fn()}
    />,
  );
  const add = screen.getByRole("button", { name: "添加应用…" });
  act(() => {
    fireEvent.click(add);
    fireEvent.click(add);
  });
  expect(pickApplication).toHaveBeenCalledOnce();
  await act(async () => request.resolve(null));
});

test.each(["成功", "失败"])(
  "更换应用选择器后旧%s不修改当前规则、警告或忙碌状态",
  async (outcome) => {
    const old = pickerRequest();
    const next = pickerRequest();
    const onChange = vi.fn();
    const oldPicker = vi.fn(() => old.promise);
    const nextPicker = vi.fn(() => next.promise);
    const view = render(
      <AppInputModeRulesSection windows={false} pickApplication={oldPicker} onChange={onChange} />,
    );
    const add = screen.getByRole<HTMLButtonElement>("button", { name: "添加应用…" });
    fireEvent.click(add);
    view.rerender(
      <AppInputModeRulesSection windows={false} pickApplication={nextPicker} onChange={onChange} />,
    );
    fireEvent.click(add);
    await act(async () => {
      if (outcome === "成功") old.resolve({ id: "org.example.old", name: "合成旧应用" });
      else old.reject(new Error("合成旧选择失败"));
    });
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(add.disabled).toBe(true);
    await act(async () => next.resolve({ id: "org.example.current", name: "合成当前应用" }));
    expect(onChange).toHaveBeenCalledExactlyOnceWith({ "org.example.current": "chinese" });
    expect(add.disabled).toBe(false);
  },
);

test("应用规则控件卸载后选择结果不再写回", async () => {
  const request = pickerRequest();
  const onChange = vi.fn();
  const view = render(
    <AppInputModeRulesSection
      windows={false}
      pickApplication={() => request.promise}
      onChange={onChange}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "添加应用…" }));
  view.unmount();
  await act(async () => request.resolve({ id: "org.example.closed", name: "合成已关闭应用" }));
  expect(onChange).not.toHaveBeenCalled();
});
