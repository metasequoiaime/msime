// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceInputIntroSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const common = {
  localVoice: false,
  localVoiceModelsAvailable: false,
  systemVoice: false,
  systemVoiceHostName: "",
  android: false,
  ios: false,
  macos: false,
  harmony: false,
  linux: false,
  showVoiceProviderSettings: true,
};

test("describes local recognition with and without model management", () => {
  const { rerender } = render(
    <VoiceInputIntroSection
      {...common}
      localVoice
      localVoiceModelsAvailable
      onOpenVoice={vi.fn()}
    />,
  );
  expect(screen.getByText(/下载一个模型并点击“使用”/)).toBeTruthy();

  rerender(
    <VoiceInputIntroSection
      {...common}
      localVoice
      localVoiceModelsAvailable={false}
      onOpenVoice={vi.fn()}
    />,
  );
  expect(screen.getByText(/已安装模型目录（包含 msime-model\.json）/)).toBeTruthy();
  // No host runs a Whisper model file any more, so nothing may still ask for one.
  expect(document.body.textContent).not.toMatch(/whisper\.cpp|ggml|Whisper 模型文件/);
});

test("offers the iOS voice entry when provided", () => {
  const onOpenVoice = vi.fn();
  render(<VoiceInputIntroSection {...common} ios onOpenVoice={onOpenVoice} />);

  fireEvent.click(screen.getByRole("button", { name: "开始 iOS 语音" }));
  expect(onOpenVoice).toHaveBeenCalledOnce();
});

test("describes the Windows system recognizer as on-device and names the language pack", () => {
  render(<VoiceInputIntroSection {...common} systemVoice systemVoiceHostName="Windows" />);

  expect(screen.getByText("Windows 系统语音")).toBeTruthy();
  expect(screen.getByText(/音频不会离开本机/)).toBeTruthy();
  expect(screen.getByText(/语言和区域/)).toBeTruthy();
  // macOS 的说明要人授予语音识别权限，Windows 的 SAPI 听写没有这一步。
  expect(document.body.textContent).not.toMatch(/语音识别权限/);
});
