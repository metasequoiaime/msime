// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceCueSwitchesSection, VoiceRecordingBehaviorSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards recording behavior changes", () => {
  const onSoundEnabledChange = vi.fn();
  const onMuteSystemAudioChange = vi.fn();
  render(
    <VoiceRecordingBehaviorSection
      linux={false}
      soundEnabled
      muteSystemAudio={false}
      onSoundEnabledChange={onSoundEnabledChange}
      onMuteSystemAudioChange={onMuteSystemAudioChange}
    />,
  );

  fireEvent.click(screen.getByLabelText("语音提示音"));
  fireEvent.click(screen.getByLabelText("录音时静音其他声音"));

  expect(onSoundEnabledChange).toHaveBeenCalledWith(false);
  expect(onMuteSystemAudioChange).toHaveBeenCalledWith(true);
  // 开始、结束提示音不在这一组：它们只在分别读这两个值的宿主上出现，收在「服务配置」子页。
  expect(screen.queryByLabelText("开始录音提示音")).toBeNull();
  expect(screen.queryByLabelText("结束录音提示音")).toBeNull();
});

test("names the group 录音行为 on Linux too and says where the options go", () => {
  render(
    <VoiceRecordingBehaviorSection
      linux
      soundEnabled
      muteSystemAudio={false}
      onSoundEnabledChange={vi.fn()}
      onMuteSystemAudioChange={vi.fn()}
    />,
  );

  expect(screen.getByText("录音行为")).toBeTruthy();
  expect(screen.queryByText("Linux provider 行为")).toBeNull();
  expect(screen.getByText(/随请求传给用户管理的语音服务/)).toBeTruthy();
});

// 总开关关着时两个提示音都不响，所以两个分开的开关跟着变灰，与手机的做法一致。
test("the start and end cue switches follow the master switch", () => {
  const onStartSoundChange = vi.fn();
  const onEndSoundChange = vi.fn();
  const { rerender } = render(
    <VoiceCueSwitchesSection
      soundEnabled
      startSound
      endSound
      onStartSoundChange={onStartSoundChange}
      onEndSoundChange={onEndSoundChange}
    />,
  );
  fireEvent.click(screen.getByLabelText("开始录音提示音"));
  fireEvent.click(screen.getByLabelText("结束录音提示音"));
  expect(onStartSoundChange).toHaveBeenCalledWith(false);
  expect(onEndSoundChange).toHaveBeenCalledWith(false);

  rerender(
    <VoiceCueSwitchesSection
      soundEnabled={false}
      startSound
      endSound
      onStartSoundChange={onStartSoundChange}
      onEndSoundChange={onEndSoundChange}
    />,
  );
  expect((screen.getByLabelText("开始录音提示音") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("结束录音提示音") as HTMLInputElement).disabled).toBe(true);
});
