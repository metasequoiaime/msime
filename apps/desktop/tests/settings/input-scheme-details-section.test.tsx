// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputSchemeDetailsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("changes the Shuangpin profile and disables it outside Shuangpin on macOS", () => {
  const onShuangpinProfileChange = vi.fn();
  const { rerender } = render(
    <InputSchemeDetailsSection
      scheme="quanpin"
      shuangpinProfile="xiaohe"
      macos
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
    />,
  );

  const profile = screen.getByRole("combobox", { name: "双拼方案" }) as HTMLSelectElement;
  expect(profile.disabled).toBe(true);

  rerender(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      macos
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
    />,
  );
  fireEvent.change(profile, { target: { value: "ziranma" } });
  expect(onShuangpinProfileChange).toHaveBeenCalledWith("ziranma");
});

test("updates the macOS Shuangpin keymap toggle when provided", () => {
  const onMacosShuangpinKeymapChange = vi.fn();
  render(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      macos
      hasTouchKeyboardSchemes={false}
      macosShuangpinKeymap={false}
      onShuangpinProfileChange={vi.fn()}
      onMacosShuangpinKeymapChange={onMacosShuangpinKeymapChange}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "输入时显示双拼键位提示" }));
  expect(onMacosShuangpinKeymapChange).toHaveBeenCalledWith(true);
});

test("shows only the Japanese scheme details in Japanese mode", () => {
  render(
    <InputSchemeDetailsSection
      scheme="japanese"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );

  expect(screen.getByRole("group", { name: "日语方案" }).getAttribute("hidden")).toBeNull();
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "五笔方案" })).toBeNull();
});
