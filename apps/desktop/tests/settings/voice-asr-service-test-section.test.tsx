// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { VoiceAsrServiceTestSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const voiceInput = {
  enabled: true,
  language: "zh-CN",
  asr_provider: "doubao",
  asr_token: "synthetic-token",
};

test("renders the supported ASR test with its provider-specific label", () => {
  const credentialTestControl = vi.fn((_service, label, _config, disabled) => (
    <button type="button" disabled={disabled}>
      {label}
    </button>
  ));

  render(
    <VoiceAsrServiceTestSection
      available
      voiceInput={voiceInput}
      doubaoAuthMode="api_key"
      credentialTestControl={credentialTestControl}
    />,
  );

  expect(screen.getByText("测试会向当前服务发送一秒合成静音，不使用麦克风；服务可能计入 API 用量。")).toBeTruthy();
  expect(screen.getByRole("button", { name: "测试豆包识别配置" })).toBeTruthy();
  expect(credentialTestControl).toHaveBeenCalledWith(
    "voice.asr",
    "测试豆包识别配置",
    expect.objectContaining({ provider: "doubao", token: "synthetic-token" }),
    false,
  );
});

test("does not render an unavailable or non-service ASR test", () => {
  const credentialTestControl = vi.fn(() => <button type="button">测试</button>);
  const { rerender } = render(
    <VoiceAsrServiceTestSection
      available={false}
      voiceInput={voiceInput}
      doubaoAuthMode="api_key"
      credentialTestControl={credentialTestControl}
    />,
  );
  expect(credentialTestControl).not.toHaveBeenCalled();
  expect(screen.queryByRole("button")).toBeNull();

  rerender(
    <VoiceAsrServiceTestSection
      available
      voiceInput={{ ...voiceInput, asr_provider: "system" }}
      doubaoAuthMode="api_key"
      credentialTestControl={credentialTestControl}
    />,
  );
  expect(credentialTestControl).not.toHaveBeenCalled();
  expect(screen.queryByRole("button")).toBeNull();
});
