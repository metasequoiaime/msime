// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PolishPromptSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("selects presets and restores their prompt", () => {
  const onSelectPrompt = vi.fn();
  const onRestore = vi.fn();
  render(
    <PolishPromptSection
      promptId="cleanup"
      prompt="edited prompt"
      onSelectPrompt={onSelectPrompt}
      onPromptChange={vi.fn()}
      onRestore={onRestore}
    />,
  );

  fireEvent.change(screen.getByLabelText("润色方案"), { target: { value: "faithful" } });
  expect(onSelectPrompt).toHaveBeenCalledWith(
    "faithful",
    expect.stringContaining("语音转写校对助手"),
  );

  fireEvent.click(screen.getByRole("button", { name: "恢复默认" }));
  expect(onRestore).toHaveBeenCalledWith(expect.stringContaining("语音转写整理助手"));
  expect(screen.getByText("内置方案的完整提示词，可以就地修改")).toBeTruthy();
});

test("routes custom prompt edits to the selected slot and disables restore when unchanged", () => {
  const onPromptChange = vi.fn();
  const onRestore = vi.fn();
  render(
    <PolishPromptSection
      promptId="custom_2"
      prompt="custom text"
      customPrompts={{ custom_2: "custom text" }}
      onSelectPrompt={vi.fn()}
      onPromptChange={onPromptChange}
      onRestore={onRestore}
    />,
  );

  const restore = screen.getByRole("button", { name: "恢复默认" });
  expect((restore as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("润色提示词"), { target: { value: "updated custom" } });
  expect(onPromptChange).toHaveBeenCalledWith("updated custom", "custom_2");
  expect(screen.getByText("这一段会保存到所选的自定义方案")).toBeTruthy();
});
