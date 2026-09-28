// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceCaptureDevicesSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards backend and microphone edits", () => {
  const onBackendChange = vi.fn();
  const onDeviceChange = vi.fn();
  render(
    <VoiceCaptureDevicesSection
      windows
      harmony={false}
      backend=""
      device=""
      backendOptions={[["windows", "Windows Audio"]]}
      readDevices={vi.fn().mockResolvedValue([])}
      onBackendChange={onBackendChange}
      onDeviceChange={onDeviceChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("录音后端"), { target: { value: "windows" } });
  fireEvent.change(screen.getByLabelText("麦克风设备"), { target: { value: "synthetic-device" } });

  expect(onBackendChange).toHaveBeenCalledWith("windows", "");
  expect(onDeviceChange).toHaveBeenCalledWith("synthetic-device");
});

test("uses platform-specific microphone guidance", () => {
  render(
    <VoiceCaptureDevicesSection
      windows={false}
      harmony
      backend="harmony"
      device=""
      backendOptions={[["harmony", "HarmonyOS 音频"]]}
      readDevices={vi.fn().mockResolvedValue([])}
      onBackendChange={vi.fn()}
      onDeviceChange={vi.fn()}
    />,
  );

  expect(screen.getByText(/设备类型与地址/)).toBeTruthy();
});
