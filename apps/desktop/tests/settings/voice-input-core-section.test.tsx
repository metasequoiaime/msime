// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceInputCoreSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const base = {
  enabled: true,
  provider: "doubao",
  language: "zh-cn",
  showProviderSettings: true,
  systemVoice: false,
  macos: false,
  harmony: false,
  android: false,
  localVoiceAvailable: false,
  nativeVoicePlatform: false,
  harmonyUnsupportedAsr: false,
};

test("updates voice enablement, provider, and language", () => {
  const onEnabledChange = vi.fn();
  const onProviderChange = vi.fn();
  const onLanguageChange = vi.fn();
  render(
    <VoiceInputCoreSection
      {...base}
      onEnabledChange={onEnabledChange}
      onProviderChange={onProviderChange}
      onLanguageChange={onLanguageChange}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "启用语音输入" }));
  expect(onEnabledChange).toHaveBeenCalledWith(false);
  fireEvent.change(screen.getByRole("combobox", { name: "识别服务" }), {
    target: { value: "openai" },
  });
  expect(onProviderChange).toHaveBeenCalledWith("openai");
  fireEvent.change(screen.getByRole("combobox", { name: "识别语言" }), {
    target: { value: "en" },
  });
  expect(onLanguageChange).toHaveBeenCalledWith("en");
});

test("shows native platform provider options and language guidance", () => {
  render(
    <VoiceInputCoreSection
      {...base}
      provider="system"
      language="zh-CN"
      systemVoice
      macos
      nativeVoicePlatform
      onEnabledChange={vi.fn()}
      onProviderChange={vi.fn()}
      onLanguageChange={vi.fn()}
    />,
  );

  expect(screen.getByRole("option", { name: "macOS 系统识别" })).toBeTruthy();
  expect(screen.getByText(/选择明确的语言代码/)).toBeTruthy();
  expect(screen.queryByRole("option", { name: "自动识别" })).toBeNull();
});
