// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoicePolishSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("the switch alone shows while polishing is off", () => {
  const onEnabledChange = vi.fn();
  render(
    <VoicePolishSection
      enabled={false}
      provider="siliconflow"
      model="Qwen/Qwen2.5-7B-Instruct"
      onEnabledChange={onEnabledChange}
      onProviderChange={vi.fn()}
      onModelChange={vi.fn()}
      providerPreset={<span>preset</span>}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "启用文本润色" }));
  expect(onEnabledChange).toHaveBeenCalledWith(true);
  expect(screen.queryByRole("combobox", { name: "文本润色服务提供商" })).toBeNull();
  expect(screen.queryByText("preset")).toBeNull();
});

test("updates the provider and model controls while polishing is on", () => {
  const onProviderChange = vi.fn();
  const onModelChange = vi.fn();
  render(
    <VoicePolishSection
      enabled
      provider="siliconflow"
      model="Qwen/Qwen2.5-7B-Instruct"
      onEnabledChange={vi.fn()}
      onProviderChange={onProviderChange}
      onModelChange={onModelChange}
      providerPreset={<span>preset</span>}
    />,
  );

  fireEvent.change(screen.getByRole("combobox", { name: "文本润色服务提供商" }), {
    target: { value: "openai" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "文本润色模型" }), {
    target: { value: "gpt-4o-mini" },
  });

  expect(onProviderChange).toHaveBeenCalledWith("openai");
  expect(onModelChange).toHaveBeenCalledWith("gpt-4o-mini");
  expect(screen.getByText("preset")).toBeTruthy();
});
