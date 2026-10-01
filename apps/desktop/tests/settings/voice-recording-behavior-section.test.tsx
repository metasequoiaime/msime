// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceRecordingBehaviorSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards recording behavior changes", () => {
  const onSoundEnabledChange = vi.fn();
  const onStartSoundChange = vi.fn();
  const onEndSoundChange = vi.fn();
  const onMuteSystemAudioChange = vi.fn();
  render(
    <VoiceRecordingBehaviorSection
      linux={false}
      soundEnabled
      startSound
      endSound
      muteSystemAudio={false}
      onSoundEnabledChange={onSoundEnabledChange}
      onStartSoundChange={onStartSoundChange}
      onEndSoundChange={onEndSoundChange}
      onMuteSystemAudioChange={onMuteSystemAudioChange}
    />,
  );

  fireEvent.click(screen.getByLabelText("语音提示音"));
  fireEvent.click(screen.getByLabelText("开始录音提示音"));
  fireEvent.click(screen.getByLabelText("结束录音提示音"));
  fireEvent.click(screen.getByLabelText("录音时静音其他声音"));

  expect(onSoundEnabledChange).toHaveBeenCalledWith(false);
  expect(onStartSoundChange).toHaveBeenCalledWith(false);
  expect(onEndSoundChange).toHaveBeenCalledWith(false);
  expect(onMuteSystemAudioChange).toHaveBeenCalledWith(true);
});

test("names the group 录音行为 on Linux too and says where the options go", () => {
  render(
    <VoiceRecordingBehaviorSection
      linux
      soundEnabled
      startSound
      endSound
      muteSystemAudio={false}
      onSoundEnabledChange={vi.fn()}
      onStartSoundChange={vi.fn()}
      onEndSoundChange={vi.fn()}
      onMuteSystemAudioChange={vi.fn()}
    />,
  );

  expect(screen.getByText("录音行为")).toBeTruthy();
  expect(screen.queryByText("Linux provider 行为")).toBeNull();
  expect(screen.getByText(/随请求传给用户管理的语音服务/)).toBeTruthy();
});
