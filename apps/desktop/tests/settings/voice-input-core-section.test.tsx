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

  fireEvent.click(screen.getByRole("switch", { name: "启用语音输入" }));
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

test("offers the Windows system recognizer on Windows only", () => {
  const { rerender } = render(
    <VoiceInputCoreSection
      {...base}
      provider="system"
      language="zh-CN"
      systemVoice
      windows
      nativeVoicePlatform
      onEnabledChange={vi.fn()}
      onProviderChange={vi.fn()}
      onLanguageChange={vi.fn()}
    />,
  );

  const option = screen.getByRole("option", { name: "Windows 系统识别" }) as HTMLOptionElement;
  expect(option.disabled).toBe(false);
  expect(screen.queryByRole("option", { name: "macOS 系统识别" })).toBeNull();
  expect(screen.queryByRole("option", { name: "系统识别（当前平台不可用）" })).toBeNull();

  // Linux 等没有系统识别器的宿主仍把存下的 system 显示为不可用。
  rerender(
    <VoiceInputCoreSection
      {...base}
      provider="system"
      onEnabledChange={vi.fn()}
      onProviderChange={vi.fn()}
      onLanguageChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("option", { name: "Windows 系统识别" })).toBeNull();
  expect(screen.getByRole("option", { name: "系统识别（当前平台不可用）" })).toBeTruthy();
});

// 豆包的识别请求不读识别语言，宿主选豆包时可以不画这一行；额外的行接在同一组里。
test("can leave the language out and carry extra rows", () => {
  render(
    <VoiceInputCoreSection
      {...base}
      hideLanguage
      serviceRow={<button type="button">服务配置</button>}
      onEnabledChange={vi.fn()}
      onProviderChange={vi.fn()}
      onLanguageChange={vi.fn()}
    >
      <p>extra row</p>
    </VoiceInputCoreSection>,
  );
  expect(screen.queryByLabelText("识别语言")).toBeNull();
  const group = screen.getByRole("region", { name: "识别" });
  expect(group.textContent).toContain("服务配置");
  expect(group.textContent).toContain("extra row");
});
