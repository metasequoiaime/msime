// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { CandidateFontSettingsRows, type CandidateFontSettingsRowsProps } from "@msime/ui";

afterEach(cleanup);

function props(
  overrides: Partial<CandidateFontSettingsRowsProps> = {},
): CandidateFontSettingsRowsProps {
  return {
    preferences: { candidate_font_size: 18, candidate_preedit_font_size: 16 },
    platform: "macos",
    showFontSize: true,
    showPreeditFont: false,
    englishFont: false,
    onChange: vi.fn(),
    ...overrides,
  };
}

test("自定义意图由父级控制，选择预设只报告意图与偏好补丁", () => {
  const onCustomChange = vi.fn();
  const values = props({ onCustomChange });
  const view = render(<CandidateFontSettingsRows {...values} />);
  expect(screen.queryByLabelText("主字体")).toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "自定义…" }));
  expect(onCustomChange).toHaveBeenLastCalledWith(true);
  expect(values.onChange).not.toHaveBeenCalled();
  expect(screen.queryByLabelText("主字体")).toBeNull();

  view.rerender(<CandidateFontSettingsRows {...values} custom />);
  expect(screen.getByLabelText("主字体")).toHaveProperty("value", "Noto Sans SC");
  fireEvent.click(screen.getByRole("radio", { name: "宋体" }));
  expect(onCustomChange).toHaveBeenLastCalledWith(false);
  expect(values.onChange).toHaveBeenLastCalledWith(
    expect.objectContaining({ candidate_font_family: "Songti SC" }),
  );
  view.rerender(
    <CandidateFontSettingsRows
      {...values}
      preferences={{ candidate_font_family: "Songti SC" }}
      custom={false}
    />,
  );
  expect(screen.queryByLabelText("主字体")).toBeNull();
});

test("非预设字体直接显示字体框，偏好替换后按新预设收起", () => {
  const values = props({ onCustomChange: vi.fn() });
  const view = render(
    <CandidateFontSettingsRows {...values} preferences={{ candidate_font_family: "示例字体" }} />,
  );
  expect(screen.getByLabelText("主字体")).toHaveProperty("value", "示例字体");
  expect(screen.getByRole("radio", { name: "自定义…" })).toHaveProperty("checked", true);
  view.rerender(<CandidateFontSettingsRows {...values} />);
  expect(screen.queryByLabelText("主字体")).toBeNull();
  expect(values.onChange).not.toHaveBeenCalled();
});

test("手机模式始终显示主字体，英文字体和预编辑字号独立受控", () => {
  const values = props({
    platform: "harmony",
    showFontSize: false,
    showPreeditFont: true,
    englishFont: true,
  });
  const view = render(<CandidateFontSettingsRows {...values} />);
  expect(screen.getByLabelText("主字体")).toBeTruthy();
  expect(screen.queryByRole("radio", { name: "自定义…" })).toBeNull();
  expect(screen.queryByLabelText("字号")).toBeNull();
  fireEvent.change(screen.getByLabelText("英文字体"), { target: { value: "示例西文字体" } });
  expect(values.onChange).toHaveBeenLastCalledWith({ candidate_english_font: "示例西文字体" });
  fireEvent.change(screen.getByLabelText("预编辑字号"), { target: { value: "20" } });
  expect(values.onChange).toHaveBeenLastCalledWith({ candidate_preedit_font_size: 20 });

  view.rerender(
    <CandidateFontSettingsRows {...values} englishFont={false} showPreeditFont={false} />,
  );
  expect(screen.queryByLabelText("英文字体")).toBeNull();
  expect(screen.queryByLabelText("预编辑字号")).toBeNull();
});

test("Windows 主字体接线保留回退链，候选字号报告数值", () => {
  const values = props({
    platform: "windows",
    preferences: {
      candidate_font_family: "旧示例字体",
      candidate_fallback_fonts: ["旧示例字体", "保留示例字体"],
    },
  });
  render(<CandidateFontSettingsRows {...values} />);
  expect(screen.getByRole("radio", { name: "圆体" })).toHaveProperty("disabled", true);
  fireEvent.change(screen.getByLabelText("主字体"), { target: { value: "新示例字体" } });
  expect(values.onChange).toHaveBeenLastCalledWith({
    candidate_font_family: "新示例字体",
    candidate_fallback_fonts: ["新示例字体", "保留示例字体"],
  });
  fireEvent.change(screen.getByLabelText("字号"), { target: { value: "24" } });
  expect(values.onChange).toHaveBeenLastCalledWith({ candidate_font_size: 24 });
});

test("macOS 译文字号独立保存并可恢复默认，其他宿主不显示", () => {
  const values = props();
  const view = render(<CandidateFontSettingsRows {...values} />);
  const translation = screen.getByLabelText("译文字号");
  expect(translation).toHaveProperty("value", "");
  expect(values.onChange).not.toHaveBeenCalled();
  fireEvent.change(translation, { target: { value: "21" } });
  expect(values.onChange).toHaveBeenLastCalledWith({ candidate_translation_font_size: 21 });
  view.rerender(
    <CandidateFontSettingsRows {...values} preferences={{ candidate_translation_font_size: 21 }} />,
  );
  expect(screen.getByLabelText("译文字号")).toHaveProperty("value", "21");
  fireEvent.change(screen.getByLabelText("译文字号"), { target: { value: "" } });
  expect(values.onChange).toHaveBeenLastCalledWith({ candidate_translation_font_size: null });
  for (const platform of ["windows", "linux", "harmony", "android", "ios"] as const) {
    view.rerender(<CandidateFontSettingsRows {...values} platform={platform} />);
    expect(screen.queryByLabelText("译文字号")).toBeNull();
  }
});

test("字体目录保持惰性，主字体和英文字体共用已有目录", async () => {
  const readFonts = vi.fn(async () => ["示例字体"]);
  render(<CandidateFontSettingsRows {...props({ readFonts, englishFont: true })} />);
  expect(readFonts).not.toHaveBeenCalled();
  expect(screen.getAllByRole("button", { name: "刷新字体列表" })).toHaveLength(1);
  fireEvent.focus(screen.getByLabelText("主字体"));
  await screen.findByRole("option", { name: "示例字体" });
  fireEvent.blur(screen.getByLabelText("主字体"));
  fireEvent.focus(screen.getByLabelText("英文字体"));
  await within(screen.getByRole("listbox", { name: "英文字体可用字体" })).findByRole("option", {
    name: "示例字体",
  });
  expect(readFonts).toHaveBeenCalledTimes(1);
});

test("组合保持各行同级且顺序不变", () => {
  const { container } = render(
    <CandidateFontSettingsRows {...props({ englishFont: true, showPreeditFont: true })} />,
  );
  const controls = [
    screen.getByRole("radiogroup", { name: "候选字体" }),
    screen.getByLabelText("主字体"),
    screen.getByLabelText("英文字体"),
    screen.getByLabelText("字号"),
    screen.getByLabelText("预编辑字号"),
    screen.getByLabelText("译文字号"),
  ];
  expect(container.children).toHaveLength(6);
  controls.forEach((control, index) =>
    expect(container.children[index].contains(control)).toBe(true),
  );
});
