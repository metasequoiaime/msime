// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PunctuationSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("punctuation controls use existing defaults and send focused preference patches", () => {
  const onChange = vi.fn();
  render(
    <PunctuationSection
      preferences={{ chinese_punctuation: true }}
      showCharacterWidth={false}
      onChange={onChange}
    />,
  );

  expect((screen.getByRole("switch", { name: "中文标点" }) as HTMLInputElement).checked).toBe(true);
  expect(
    (screen.getByRole("switch", { name: /^成对标点自动补全/ }) as HTMLInputElement).checked,
  ).toBe(true);
  expect((screen.getByRole("switch", { name: /^智能标点/ }) as HTMLInputElement).checked).toBe(
    false,
  );
  expect(screen.queryByRole("switch", { name: "全角输入" })).toBeNull();

  fireEvent.click(screen.getByRole("switch", { name: /^智能标点/ }));
  expect(onChange).toHaveBeenLastCalledWith({ smart_punctuation: true });
  fireEvent.click(screen.getByRole("switch", { name: /^字母后直出/ }));
  expect(onChange).toHaveBeenLastCalledWith({ smart_punctuation_direct_letter: true });
  fireEvent.change(screen.getByRole("combobox", { name: "固定标点" }), {
    target: { value: "english" },
  });
  expect(onChange).toHaveBeenLastCalledWith({ punctuation_lock: "english" });
});

test("fullwidth control appears only when the host supports it", () => {
  const onChange = vi.fn();
  render(
    <PunctuationSection
      preferences={{ chinese_punctuation: true, character_width: "fullwidth" }}
      showCharacterWidth
      onChange={onChange}
    />,
  );

  const fullwidth = screen.getByRole("switch", { name: "全角输入" }) as HTMLInputElement;
  expect(fullwidth.checked).toBe(true);
  fireEvent.click(fullwidth);
  expect(onChange).toHaveBeenLastCalledWith({ character_width: "halfwidth" });
});
