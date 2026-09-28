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
  expect(screen.getByText(/需要自备 whisper\.cpp/)).toBeTruthy();
});

test("offers the iOS voice entry when provided", () => {
  const onOpenVoice = vi.fn();
  render(<VoiceInputIntroSection {...common} ios onOpenVoice={onOpenVoice} />);

  fireEvent.click(screen.getByRole("button", { name: "开始 iOS 语音" }));
  expect(onOpenVoice).toHaveBeenCalledOnce();
});
