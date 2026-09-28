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

  fireEvent.click(screen.getByLabelText("编码打不出时用拼音候选"));
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

  expect((screen.getByLabelText("编码打不出时用拼音候选") as HTMLInputElement).checked).toBe(false);
  expect((screen.getByLabelText("候选显示剩余编码") as HTMLInputElement).checked).toBe(true);
  expect((screen.getByLabelText("五笔四码唯一候选自动上屏") as HTMLInputElement).checked).toBe(
    false,
  );

  fireEvent.click(screen.getByLabelText("五笔四码唯一候选自动上屏"));
  expect(onAutoCommitUniqueChange).toHaveBeenCalledWith(true);
});
