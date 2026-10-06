// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { POLISH_PRESETS, PolishPromptSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("selects presets and shows their built-in prompt read-only", () => {
  const onSelectPrompt = vi.fn();
  render(
    <PolishPromptSection
      promptId="cleanup"
      onSelectPrompt={onSelectPrompt}
      onCustomPromptChange={vi.fn()}
    />,
  );

  fireEvent.change(screen.getByLabelText("润色方案"), { target: { value: "faithful" } });
  expect(onSelectPrompt).toHaveBeenCalledWith("faithful");

  const box = screen.getByLabelText("润色提示词") as HTMLTextAreaElement;
  expect(box.value).toBe(POLISH_PRESETS.cleanup);
  expect(box.readOnly).toBe(true);
  expect(screen.getByText("内置方案的完整提示词；要改写请选择自定义方案")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "恢复默认" })).toBeNull();
});

test("routes custom prompt edits to the selected slot", () => {
  const onCustomPromptChange = vi.fn();
  render(
    <PolishPromptSection
      promptId="custom_2"
      customPrompts={{ custom_2: "custom text" }}
      onSelectPrompt={vi.fn()}
      onCustomPromptChange={onCustomPromptChange}
    />,
  );

  const box = screen.getByLabelText("润色提示词") as HTMLTextAreaElement;
  expect(box.value).toBe("custom text");
  expect(box.readOnly).toBe(false);
  fireEvent.change(box, { target: { value: "updated custom" } });
  expect(onCustomPromptChange).toHaveBeenCalledWith("custom_2", "updated custom");
  expect(
    screen.getByText("这一段会保存到所选的自定义方案；留空时使用内置的「精炼整理」提示词"),
  ).toBeTruthy();
});
