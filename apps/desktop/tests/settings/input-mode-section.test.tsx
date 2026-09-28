// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputModeSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("switching to Chinese restores the remembered Chinese scheme", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="japanese" lastChineseScheme="shuangpin" onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("中文"));
  expect(onChange).toHaveBeenCalledWith({ scheme: "shuangpin" });
});

test("switching to Japanese remembers the current Chinese scheme", () => {
  const onChange = vi.fn();
  render(<InputModeSection scheme="wubi" lastChineseScheme="wubi" onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("日文"));
  expect(onChange).toHaveBeenCalledWith({ last_chinese_scheme: "wubi", scheme: "japanese" });
});
