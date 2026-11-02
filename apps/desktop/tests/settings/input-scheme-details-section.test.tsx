// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputSchemeDetailsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("韩语布局仅在声明能力的实体键盘宿主显示并写回选择", () => {
  const onKoreanChange = vi.fn();
  const props = {
    scheme: "korean" as const,
    shuangpinProfile: "xiaohe" as const,
    hasTouchKeyboardSchemes: false,
    onShuangpinProfileChange: vi.fn(),
    onKoreanChange,
  };
  const { rerender } = render(<InputSchemeDetailsSection {...props} />);
  expect(screen.queryByRole("radio", { name: "罗马字" })).toBeNull();
  rerender(<InputSchemeDetailsSection {...props} koreanRomaja />);
  expect((screen.getByRole("radio", { name: "两套式" }) as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByRole("radio", { name: "罗马字" }));
  expect(onKoreanChange).toHaveBeenLastCalledWith({ layout: "romaja" });
  rerender(<InputSchemeDetailsSection {...props} koreanRomaja korean={{ layout: "romaja" }} />);
  expect((screen.getByRole("radio", { name: "罗马字" }) as HTMLInputElement).checked).toBe(true);
  rerender(<InputSchemeDetailsSection {...props} koreanRomaja hasTouchKeyboardSchemes />);
  expect(screen.queryByRole("radio", { name: "罗马字" })).toBeNull();
});

// 用不到的方案行留在页面里、只是藏起来（换方案时原样出现），所以按是否在 `hidden` 之下判断它显示与否。
function shown(text: string | RegExp): boolean {
  return screen.getByText(text).closest("[hidden]") === null;
}

test("the Shuangpin profile is hidden outside Shuangpin and changes under it", () => {
  const onShuangpinProfileChange = vi.fn();
  const { rerender } = render(
    <InputSchemeDetailsSection
      scheme="quanpin"
      shuangpinProfile="xiaohe"
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
    />,
  );

  // 全拼、五笔下不再灰着显示，而是藏起来；切到双拼时原样出现。
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  rerender(
    <InputSchemeDetailsSection
      scheme="wubi"
      shuangpinProfile="xiaohe"
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
    />,
  );
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();

  rerender(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={onShuangpinProfileChange}
    />,
  );
  const profile = screen.getByRole("combobox", { name: "双拼方案" }) as HTMLSelectElement;
  expect(profile.disabled).toBe(false);
  fireEvent.change(profile, { target: { value: "ziranma" } });
  expect(onShuangpinProfileChange).toHaveBeenCalledWith("ziranma");
});

test("双拼预编辑 sits in the Shuangpin details and appears only under Shuangpin", () => {
  const onShuangpinPreeditChange = vi.fn();
  const { rerender } = render(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      hasTouchKeyboardSchemes={false}
      shuangpinPreedit="raw"
      onShuangpinProfileChange={vi.fn()}
      onShuangpinPreeditChange={onShuangpinPreeditChange}
    />,
  );
  const preedit = screen.getByLabelText("双拼预编辑") as HTMLSelectElement;
  expect(preedit.value).toBe("raw");
  fireEvent.change(preedit, { target: { value: "pinyin" } });
  expect(onShuangpinPreeditChange).toHaveBeenCalledWith(false);

  for (const scheme of ["quanpin", "wubi", "japanese"] as const) {
    rerender(
      <InputSchemeDetailsSection
        scheme={scheme}
        shuangpinProfile="xiaohe"
        hasTouchKeyboardSchemes={false}
        shuangpinPreedit="raw"
        onShuangpinProfileChange={vi.fn()}
      />,
    );
    expect(screen.queryByLabelText("双拼预编辑")).toBeNull();
  }
  // 宿主不画双拼预编辑时不传，这一行不出现。
  rerender(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(screen.queryByLabelText("双拼预编辑")).toBeNull();
});

test("updates the Shuangpin keymap toggle when provided", () => {
  const onShuangpinKeymapHintChange = vi.fn();
  render(
    <InputSchemeDetailsSection
      scheme="shuangpin"
      shuangpinProfile="xiaohe"
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
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );

  // 只有一个方案，所以是一行说明，没有选择器。
  expect(shown("日语方案")).toBe(true);
  expect(shown(/^罗马音：直接输入罗马音/)).toBe(true);
  expect(screen.queryByRole("radiogroup")).toBeNull();
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "五笔方案" })).toBeNull();
});

test("shows only the Korean scheme details in Korean mode", () => {
  render(
    <InputSchemeDetailsSection
      scheme="korean"
      shuangpinProfile="xiaohe"
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );

  expect(shown(/^两套式：按两套式（두벌식）键位输入韩文字母/)).toBe(true);
  expect(screen.queryByRole("radiogroup")).toBeNull();
  expect(shown(/^罗马音：/)).toBe(false);
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "五笔方案" })).toBeNull();
});

test("粤拼方案在桌面宿主可切换为粤语双拼，触屏宿主不显示", () => {
  const onCantoneseChange = vi.fn();
  const props = {
    scheme: "cantonese" as const,
    shuangpinProfile: "xiaohe" as const,
    hasTouchKeyboardSchemes: false,
    onShuangpinProfileChange: vi.fn(),
    onCantoneseChange,
  };
  const { rerender } = render(<InputSchemeDetailsSection {...props} />);
  expect(shown("粤拼方案")).toBe(true);
  expect((screen.getByRole("radio", { name: "粤拼" }) as HTMLInputElement).checked).toBe(true);
  expect(screen.queryByText(/用 ' 分隔音节/)).not.toBeNull();
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "粤语双拼" }));
  expect(onCantoneseChange).toHaveBeenLastCalledWith({ input_method: "shuangpin" });

  rerender(<InputSchemeDetailsSection {...props} cantonese={{ input_method: "shuangpin" }} />);
  expect((screen.getByRole("radio", { name: "粤语双拼" }) as HTMLInputElement).checked).toBe(true);
  expect(screen.queryByText(/ing 和 ik 在 ; 键上/)).not.toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "粤拼" }));
  expect(onCantoneseChange).toHaveBeenLastCalledWith({ input_method: "jyutping" });

  rerender(<InputSchemeDetailsSection {...props} scheme="quanpin" />);
  expect(shown("粤拼方案")).toBe(false);
  rerender(<InputSchemeDetailsSection {...props} hasTouchKeyboardSchemes />);
  expect(shown("粤拼方案")).toBe(false);
});

test.each([
  ["zhuyin", "注音键盘", "大千"],
  ["stroke", "笔画方案", "横竖撇点折"],
  ["tibetan", "藏文方案", "威利转写"],
] as const)(
  "%s describes its only scheme as text and shows no pinyin or Wubi rows",
  (scheme, title, label) => {
    render(
      <InputSchemeDetailsSection
        scheme={scheme}
        shuangpinProfile="xiaohe"
        hasTouchKeyboardSchemes={false}
        onShuangpinProfileChange={vi.fn()}
      />,
    );
    expect(shown(title)).toBe(true);
    expect(shown(new RegExp(`^${label}：`))).toBe(true);
    expect(screen.queryByRole("radio")).toBeNull();
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
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(shown("笔画方案")).toBe(false);

  rerender(
    <InputSchemeDetailsSection
      scheme="stroke"
      shuangpinProfile="xiaohe"
      hasTouchKeyboardSchemes={false}
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(shown("笔画方案")).toBe(true);
  expect(shown(/按 h s p n z 依次输入横、竖、撇、点、折，x 代替不确定的一笔/)).toBe(true);

  rerender(
    <InputSchemeDetailsSection
      scheme="stroke"
      shuangpinProfile="xiaohe"
      hasTouchKeyboardSchemes
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(shown("笔画方案")).toBe(false);
});

test("Vietnamese input method and tone placement are live controls", () => {
  const onVietnameseChange = vi.fn();
  render(
    <InputSchemeDetailsSection
      scheme="vietnamese"
      shuangpinProfile="xiaohe"
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
      hasTouchKeyboardSchemes
      onShuangpinProfileChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("combobox", { name: "五笔方案" })).toBeNull();
});
