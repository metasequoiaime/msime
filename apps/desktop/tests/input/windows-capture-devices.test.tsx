// @vitest-environment jsdom
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(cleanup);
const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    voice_input: {
      enabled: true,
      language: "zh-CN",
      capture_backend: "windows",
      capture_device: "wasapi:0061",
    },
  },
};

test("Windows saves endpoint identity across duplicate labels and enumeration reorder", async () => {
  const a = { backend: "windows" as const, id: "wasapi:0061", label: "Synthetic microphone" };
  const b = { ...a, id: "wasapi:0062" };
  const read = vi.fn().mockResolvedValueOnce([a, b]).mockResolvedValueOnce([b, a]);
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...snapshot,
    revision: 3,
    preferences,
  }));
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => snapshot,
        save,
        host: { platform: "windows", voice_capture_devices: true } as never,
        listVoiceCaptureDevices: read,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  const backend = await screen.findByLabelText("录音后端");
  expect(within(backend).queryByRole("option", { name: "PulseAudio" })).toBeNull();
  expect(within(backend).queryByRole("option", { name: "CoreAudio" })).toBeNull();
  expect(screen.getByText(/旧的数字序号需重新选择/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "刷新设备" }));
  await waitFor(() =>
    expect((screen.getByLabelText("可用录音设备") as HTMLSelectElement).value).toBe("0"),
  );
  fireEvent.click(screen.getByRole("button", { name: "刷新设备" }));
  await waitFor(() =>
    expect((screen.getByLabelText("可用录音设备") as HTMLSelectElement).value).toBe("1"),
  );
  fireEvent.change(screen.getByLabelText("可用录音设备"), { target: { value: "0" } });
  expect((screen.getByLabelText("麦克风设备") as HTMLInputElement).value).toBe(b.id);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save.mock.calls[0][1].voice_input).toMatchObject({
    capture_backend: "windows",
    capture_device: b.id,
  });
});

test("missing or legacy Windows devices are retained until the user chooses a default", async () => {
  const previous = {
    ...snapshot,
    preferences: {
      ...snapshot.preferences,
      voice_input: { ...snapshot.preferences.voice_input!, capture_device: "0" },
    },
  };
  render(
    <SettingsPage
      client={{
        load: async () => previous,
        save: vi.fn(),
        host: { platform: "windows", voice_capture_devices: true } as never,
        listVoiceCaptureDevices: async () => [],
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  fireEvent.click(await screen.findByRole("button", { name: "刷新设备" }));
  await screen.findByText(/未发现设备/);
  expect((screen.getByLabelText("麦克风设备") as HTMLInputElement).value).toBe("0");
  fireEvent.change(screen.getByLabelText("录音后端"), { target: { value: "auto" } });
  expect((screen.getByLabelText("麦克风设备") as HTMLInputElement).value).toBe("");
});
