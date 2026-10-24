// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ChineseEnglishSpacingSection } from "../../../../packages/ui/src/settings/chinese-english-spacing-section";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("Chinese-English spacing switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<ChineseEnglishSpacingSection value={false} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("中英文之间加空格"));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("Chinese-English spacing switch is off when the document has no key", () => {
  render(<ChineseEnglishSpacingSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("中英文之间加空格") as HTMLInputElement).checked).toBe(false);
});

test("Chinese-English spacing says what it leaves alone", () => {
  render(<ChineseEnglishSpacingSection value={false} onChange={vi.fn()} />);

  expect(
    screen.getByText(
      /两次上屏之间不加；网址、日期等快捷模式和全角输入不受影响，译文和释义上屏、常用语、剪贴板和语音结果也不加/,
    ),
  ).toBeTruthy();
});
