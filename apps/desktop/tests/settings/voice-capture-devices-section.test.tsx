// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { VoiceCaptureDevicesSection } from "@msime/ui";
import { VoiceDevicePicker } from "../../../../packages/ui/src/voice/voice-device-picker";

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

test("ignores a second device refresh while the first is pending", async () => {
  let resolveRead!: (devices: never[]) => void;
  const read = vi.fn(
    () =>
      new Promise<never[]>((resolve) => {
        resolveRead = resolve;
      }),
  );
  render(<VoiceDevicePicker read={read} backend="" device="" choose={vi.fn()} />);
  const refresh = screen.getByRole("button", { name: "刷新设备" });
  act(() => {
    fireEvent.click(refresh);
    fireEvent.click(refresh);
  });
  expect(read).toHaveBeenCalledOnce();
  resolveRead([]);
  await waitFor(() =>
    expect(screen.getByText("未发现设备，可手动填写设备名称或使用默认设备")).toBeTruthy(),
  );
});
