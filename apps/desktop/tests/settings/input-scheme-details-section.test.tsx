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

  fireEvent.click(screen.getByRole("switch", { name: "输入时显示双拼键位提示" }));
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

  expect(screen.getByRole("radiogroup", { name: "日语方案" })).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "五笔方案" })).toBeNull();
});

test("shows only the Korean scheme details in Korean mode", () => {
  render(
    <InputSchemeDetailsSection
      scheme="korean"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );

  expect(screen.getByRole("radiogroup", { name: "韩语方案" })).toBeTruthy();
  expect(screen.queryByRole("radiogroup", { name: "日语方案" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "五笔方案" })).toBeNull();
});

test.each([
  ["cantonese", "粤拼方案", "粤拼"],
  ["zhuyin", "注音键盘", "大千"],
] as const)(
  "%s shows its read-only scheme row and no pinyin or Wubi rows",
  (scheme, title, label) => {
    render(
      <InputSchemeDetailsSection
        scheme={scheme}
        shuangpinProfile="xiaohe"
        macos
        hasTouchKeyboardSchemes={false}
        onShuangpinProfileChange={vi.fn()}
      />,
    );
    expect(screen.getAllByText(title).length).toBeGreaterThanOrEqual(1);
    expect((screen.getByRole("radio", { name: label }) as HTMLInputElement).checked).toBe(true);
    expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
    expect(screen.queryByRole("combobox", { name: "五笔方案" })).toBeNull();
    expect(screen.queryByRole("radio", { name: "Telex" })).toBeNull();
  },
);

test("Vietnamese input method and tone placement are live controls", () => {
  const onVietnameseChange = vi.fn();
  render(
    <InputSchemeDetailsSection
      scheme="vietnamese"
      shuangpinProfile="xiaohe"
      macos
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
      onVietnameseChange={onVietnameseChange}
    />,
  );
  expect((screen.getByRole("radio", { name: "Telex" }) as HTMLInputElement).checked).toBe(true);
  expect((screen.getByRole("radio", { name: "新式 hoà" }) as HTMLInputElement).checked).toBe(true);
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "VNI" }));
  expect(onVietnameseChange).toHaveBeenLastCalledWith({
    input_method: "vni",
    tone_style: "modern",
  });
  fireEvent.click(screen.getByRole("radio", { name: "旧式 hòa" }));
  expect(onVietnameseChange).toHaveBeenLastCalledWith({
    input_method: "telex",
    tone_style: "classic",
  });
});

test("Vietnamese controls show the document's options", () => {
  render(
    <InputSchemeDetailsSection
      scheme="vietnamese"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      vietnamese={{ input_method: "vni", tone_style: "classic" }}
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect((screen.getByRole("radio", { name: "VNI" }) as HTMLInputElement).checked).toBe(true);
  expect((screen.getByRole("radio", { name: "旧式 hòa" }) as HTMLInputElement).checked).toBe(true);
});
