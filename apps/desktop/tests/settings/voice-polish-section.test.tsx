// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoicePolishSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("updates the polish toggle, provider, and model controls", () => {
  const onEnabledChange = vi.fn();
  const onProviderChange = vi.fn();
  const onModelChange = vi.fn();
  render(
    <VoicePolishSection
      enabled={false}
      provider="siliconflow"
      model="Qwen/Qwen2.5-7B-Instruct"
      onEnabledChange={onEnabledChange}
      onProviderChange={onProviderChange}
      onModelChange={onModelChange}
      providerPreset={<span>preset</span>}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "启用文本润色" }));
  fireEvent.change(screen.getByRole("combobox", { name: "文本润色服务提供商" }), {
    target: { value: "openai" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "文本润色模型" }), {
    target: { value: "gpt-4o-mini" },
  });

  expect(onEnabledChange).toHaveBeenCalledWith(true);
  expect(onProviderChange).toHaveBeenCalledWith("openai");
  expect(onModelChange).toHaveBeenCalledWith("gpt-4o-mini");
  expect(screen.getByText("preset")).toBeTruthy();
});
