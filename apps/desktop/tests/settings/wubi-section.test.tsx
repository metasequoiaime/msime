// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { WubiSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("Wubi section updates the mixed-pinyin preference", () => {
  const onChange = vi.fn();
  render(
    <WubiSection
      preferences={{ wubi_mixed_pinyin: false, wubi_code_hint: true }}
      onChange={onChange}
    />,
  );

  // 引擎打开混输后每次都查全拼，拼音候选接在五笔候选之后，不只是在五笔码表答不上时才出现，说明要照这个写。
  expect(
    screen.getByText("五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。"),
  ).toBeTruthy();
  fireEvent.click(screen.getByLabelText("五笔拼音混输"));
  expect(onChange).toHaveBeenCalledWith({ wubi_mixed_pinyin: true });
});

test("Wubi section renders the enabled defaults and optional macOS control", () => {
  const onChange = vi.fn();
  const onAutoCommitUniqueChange = vi.fn();
  render(
    <WubiSection
      preferences={{}}
      autoCommitUnique={false}
      onChange={onChange}
      onAutoCommitUniqueChange={onAutoCommitUniqueChange}
    />,
  );

  expect((screen.getByLabelText("五笔拼音混输") as HTMLInputElement).checked).toBe(false);
  expect((screen.getByLabelText("候选显示剩余编码") as HTMLInputElement).checked).toBe(true);
  expect((screen.getByLabelText("五笔四码唯一候选自动上屏") as HTMLInputElement).checked).toBe(
    false,
  );

  fireEvent.click(screen.getByLabelText("五笔四码唯一候选自动上屏"));
  expect(onAutoCommitUniqueChange).toHaveBeenCalledWith(true);
});
