// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputModeShortcutsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const keybindings = {
  switch_language_shift: true,
  switch_language_ctrl: false,
  switch_language_ctrl_space: true,
  switch_language_ctrl_alt_space: true,
  toggle_character_set_ctrl_shift_f: true,
  toggle_fullwidth_option_shift_h: true,
};

test("input mode shortcut changes report a keybinding patch", () => {
  const onChange = vi.fn();
  render(
    <InputModeShortcutsSection
      keybindings={keybindings}
      onChange={onChange}
      showModeSwitchShortcuts
      macos={false}
      showInputModeHUD={false}
      inputModeHUD={false}
      showFullwidthChord={false}
      windows={false}
    />,
  );

  fireEvent.click(screen.getByLabelText("Ctrl+Shift+F 切换繁体输出"));
  expect(onChange).toHaveBeenCalledWith({ toggle_character_set_ctrl_shift_f: false });
});

const renderLanguageSwitch = (
  bindings: Partial<typeof keybindings>,
  onChange = vi.fn(),
  macos = false,
  linux = false,
) => {
  render(
    <InputModeShortcutsSection
      keybindings={{ ...keybindings, ...bindings }}
      onChange={onChange}
      showModeSwitchShortcuts
      macos={macos}
      linux={linux}
      showFullwidthChord={false}
      windows={false}
    />,
  );
  return screen.getByLabelText("切换中英文") as HTMLSelectElement;
};

test("the language switch is one dropdown in the agreed order", () => {
  const select = renderLanguageSwitch({});
  expect(select.tagName).toBe("SELECT");
  expect(Array.from(select.options).map((option) => option.text)).toEqual([
    "Shift",
    "单击 Ctrl",
    "Ctrl+Alt+Space",
    "不使用",
  ]);
  expect(screen.queryByRole("switch", { name: /切换中英文/ })).toBeNull();
});

test("Linux includes Ctrl+Space in the dropdown instead of an independent switch", () => {
  const onChange = vi.fn();
  const select = renderLanguageSwitch({}, onChange, false, true);
  expect(Array.from(select.options).map((option) => option.text)).toEqual([
    "Shift",
    "单击 Ctrl",
    "Ctrl+Space",
    "Ctrl+Alt+Space",
    "不使用",
  ]);
  expect(select.value).toBe("shift");
  expect(screen.queryByRole("switch", { name: "Ctrl+Space 切换中英文" })).toBeNull();
  expect(onChange).not.toHaveBeenCalled();
});

test.each(["shift", "ctrl", "ctrl_space", "ctrl_alt_space", "none"])(
  "Linux choosing %s writes all four booleans in one patch",
  (choice) => {
    const onChange = vi.fn();
    const select = renderLanguageSwitch({}, onChange, false, true);
    fireEvent.change(select, { target: { value: choice } });
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith({
      switch_language_shift: choice === "shift",
      switch_language_ctrl: choice === "ctrl",
      switch_language_ctrl_space: choice === "ctrl_space",
      switch_language_ctrl_alt_space: choice === "ctrl_alt_space",
    });
  },
);

test("Linux missing Ctrl+Space defaults on without rewriting preferences", () => {
  const onChange = vi.fn();
  const select = renderLanguageSwitch(
    {
      switch_language_shift: false,
      switch_language_ctrl: false,
      switch_language_ctrl_space: undefined,
      switch_language_ctrl_alt_space: false,
    },
    onChange,
    false,
    true,
  );
  expect(select.value).toBe("ctrl_space");
  expect(onChange).not.toHaveBeenCalled();
});

test("Linux shows none when all four shortcuts are disabled", () => {
  const select = renderLanguageSwitch(
    {
      switch_language_shift: false,
      switch_language_ctrl: false,
      switch_language_ctrl_space: false,
      switch_language_ctrl_alt_space: false,
    },
    vi.fn(),
    false,
    true,
  );
  expect(select.value).toBe("none");
});

test("macOS names the language switch keys Control and Option", () => {
  const select = renderLanguageSwitch({}, vi.fn(), true);
  expect(Array.from(select.options).map((option) => option.text)).toEqual([
    "Shift",
    "单击 Control",
    "Control+Option+Space",
    "不使用",
  ]);
});

test.each([
  [
    "ctrl",
    {
      switch_language_shift: false,
      switch_language_ctrl: true,
      switch_language_ctrl_alt_space: false,
    },
  ],
  [
    "ctrl_alt_space",
    {
      switch_language_shift: false,
      switch_language_ctrl: false,
      switch_language_ctrl_alt_space: true,
    },
  ],
  [
    "none",
    {
      switch_language_shift: false,
      switch_language_ctrl: false,
      switch_language_ctrl_alt_space: false,
    },
  ],
])("choosing %s writes all three booleans in one patch", (choice, patch) => {
  const onChange = vi.fn();
  const select = renderLanguageSwitch({}, onChange);
  fireEvent.change(select, { target: { value: choice } });
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(onChange).toHaveBeenCalledWith(patch);
});

test.each([
  [
    {
      switch_language_shift: true,
      switch_language_ctrl: true,
      switch_language_ctrl_alt_space: true,
    },
    "shift",
  ],
  [
    {
      switch_language_shift: false,
      switch_language_ctrl: true,
      switch_language_ctrl_alt_space: true,
    },
    "ctrl",
  ],
  [
    {
      switch_language_shift: false,
      switch_language_ctrl: false,
      switch_language_ctrl_alt_space: true,
    },
    "ctrl_alt_space",
  ],
  [
    {
      switch_language_shift: false,
      switch_language_ctrl: false,
      switch_language_ctrl_alt_space: false,
    },
    "none",
  ],
])("%o shows %s by priority", (bindings, shown) => {
  expect(renderLanguageSwitch(bindings).value).toBe(shown);
});

test("several enabled switches are not rewritten until the user chooses", () => {
  const onChange = vi.fn();
  const select = renderLanguageSwitch(
    {
      switch_language_shift: true,
      switch_language_ctrl: true,
      switch_language_ctrl_alt_space: true,
    },
    onChange,
  );
  expect(onChange).not.toHaveBeenCalled();
  // 用户第一次选择时一次写全三个布尔值，多余的随之关掉。
  fireEvent.change(select, { target: { value: "ctrl" } });
  expect(onChange).toHaveBeenCalledWith({
    switch_language_shift: false,
    switch_language_ctrl: true,
    switch_language_ctrl_alt_space: false,
  });
});

test("macOS input mode shortcut section exposes HUD and fullwidth controls", () => {
  const onChange = vi.fn();
  render(
    <InputModeShortcutsSection
      keybindings={keybindings}
      onChange={onChange}
      showModeSwitchShortcuts
      macos
      showInputModeHUD
      inputModeHUD={true}
      showFullwidthChord
      fullwidthChord="Option+Shift+H"
      windows={false}
    />,
  );

  expect(screen.getByLabelText("切换中英文时显示提示")).toBeTruthy();
  expect(screen.getByLabelText("Option+Shift+H 切换全半角")).toBeTruthy();
});

test("missing optional HUD preference does not affect shortcut toggle rendering", () => {
  render(
    <InputModeShortcutsSection
      keybindings={keybindings}
      onChange={vi.fn()}
      showModeSwitchShortcuts
      macos={false}
      showInputModeHUD={false}
      inputModeHUD={false}
      showFullwidthChord={false}
      windows={false}
    />,
  );

  expect((screen.getByLabelText("切换中英文") as HTMLSelectElement).value).toBe("shift");
});
