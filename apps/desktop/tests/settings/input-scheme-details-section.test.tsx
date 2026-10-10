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

test("updates the Shuangpin keymap toggle when provided", () => {
  const onShuangpinKeymapHintChange = vi.fn();
  render(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      shuangpinKeymapHint={false}
      onShuangpinProfileChange={vi.fn()}
      onShuangpinKeymapHintChange={onShuangpinKeymapHintChange}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "输入时显示双拼键位提示" }));
  expect(onShuangpinKeymapHintChange).toHaveBeenCalledWith(true);
});

test("leaves the Shuangpin keymap toggle out when the host draws no keymap", () => {
  render(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );

  expect(screen.queryByRole("switch", { name: "输入时显示双拼键位提示" })).toBeNull();
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
  ["stroke", "笔画方案", "横竖撇点折"],
  ["tibetan", "藏文方案", "威利转写"],
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

test("the 笔画方案 row appears only under Stroke and never on touch hosts", () => {
  const { rerender } = render(
    <InputSchemeDetailsSection
      scheme="quanpin"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("radiogroup", { name: "笔画方案" })).toBeNull();

  rerender(
    <InputSchemeDetailsSection
      scheme="stroke"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(screen.getByRole("radiogroup", { name: "笔画方案" })).toBeTruthy();
  expect(
    screen.getByText(/按 h s p n z 依次输入横、竖、撇、点、折，x 代替不确定的一笔/),
  ).toBeTruthy();

  rerender(
    <InputSchemeDetailsSection
      scheme="stroke"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("radiogroup", { name: "笔画方案" })).toBeNull();
});

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

test("Tibetan describes its syllable keys and shows no Vietnamese rows", () => {
  render(
    <InputSchemeDetailsSection
      scheme="tibetan"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(screen.getByText(/空格加音节点 ་ 上屏/)).toBeTruthy();
  expect(screen.queryByRole("radiogroup", { name: "声调位置" })).toBeNull();
});

test("changes the Wubi profile between 86 and 98 under Wubi", () => {
  const onWubiProfileChange = vi.fn();
  render(
    <InputSchemeDetailsSection
      scheme="wubi"
      shuangpinProfile="xiaohe"
      wubiProfile="wubi86"
      macos={false}
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
      onWubiProfileChange={onWubiProfileChange}
    />,
  );
  const profile = screen.getByRole("combobox", { name: "五笔方案" }) as HTMLSelectElement;
  expect(profile.value).toBe("wubi86");
  expect(Array.from(profile.options).map((option) => option.textContent)).toEqual([
    "86 五笔",
    "98 五笔",
  ]);
  fireEvent.change(profile, { target: { value: "wubi98" } });
  expect(onWubiProfileChange).toHaveBeenCalledWith("wubi98");
});

test("touch hosts show the Wubi profile only while the Wubi keyboard is enabled", () => {
  const { rerender } = render(
    <InputSchemeDetailsSection
      scheme="quanpin"
      shuangpinProfile="xiaohe"
      wubiProfile="wubi98"
      macos={false}
      hasTouchKeyboardSchemes
      touchKeyboardHasWubi
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect((screen.getByRole("combobox", { name: "五笔方案" }) as HTMLSelectElement).value).toBe(
    "wubi98",
  );
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  rerender(
    <InputSchemeDetailsSection
      scheme="wubi"
      shuangpinProfile="xiaohe"
      macos={false}
      hasTouchKeyboardSchemes
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("combobox", { name: "五笔方案" })).toBeNull();
});
