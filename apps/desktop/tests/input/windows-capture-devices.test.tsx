// @vitest-environment jsdom
import { testHost } from "../support/host";
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

// Windows 只有一种录音方式（`voice_capture_backends` 为 false）：录音后端、刷新按钮、可用设备和手填设备名合成一个「麦克风」下拉，切到「录音行为」标签时自动读取列表。
test("Windows saves endpoint identity across duplicate labels and enumeration reorder", async () => {
  const a = { backend: "windows" as const, id: "wasapi:0061", label: "Synthetic microphone" };
  const b = { ...a, id: "wasapi:0062" };
  // 两个同名设备，而且列举顺序和上次不同：选中项按端点标识找，不按位置或名字。
  const read = vi.fn().mockResolvedValue([b, a]);
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
        host: testHost({ platform: "windows", voice_capture_devices: true }),
        listVoiceCaptureDevices: read,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  fireEvent.click(screen.getByRole("tab", { name: "录音行为" }));
  expect(screen.queryByLabelText("录音后端")).toBeNull();
  expect(screen.queryByRole("button", { name: "刷新设备" })).toBeNull();
  expect(screen.queryByLabelText("麦克风设备")).toBeNull();
  const microphone = (await screen.findByRole("combobox", { name: "麦克风" })) as HTMLSelectElement;
  await waitFor(() => expect(microphone.value).toBe("1"));
  expect(read).toHaveBeenCalledOnce();
  expect(screen.getByText(/拔掉后无法录音/)).toBeTruthy();
  fireEvent.change(microphone, { target: { value: "0" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save.mock.calls[0][1].voice_input).toMatchObject({
    capture_backend: "windows",
    capture_device: b.id,
  });
  expect(within(microphone).getAllByRole("option")).toHaveLength(3);
});

test("missing or legacy Windows devices are retained until the user chooses a default", async () => {
  const previous = {
    ...snapshot,
    preferences: {
      ...snapshot.preferences,
      voice_input: { ...snapshot.preferences.voice_input!, capture_device: "0" },
    },
  };
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...previous,
    revision: 3,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => previous,
        save,
        host: testHost({ platform: "windows", voice_capture_devices: true }),
        listVoiceCaptureDevices: async () => [],
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));
  fireEvent.click(screen.getByRole("tab", { name: "录音行为" }));
  const microphone = (await screen.findByRole("combobox", { name: "麦克风" })) as HTMLSelectElement;
  // 旧版存下的设备序号不在列表里：照实标成未连接，打开页面时不改写。
  await waitFor(() =>
    expect(within(microphone).getByRole("option", { name: "已选的麦克风（未连接）" })).toBeTruthy(),
  );
  expect(microphone.value).toBe("stored");
  expect(save).not.toHaveBeenCalled();
  fireEvent.change(microphone, { target: { value: "" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save.mock.calls[0][1].voice_input).toMatchObject({
    capture_backend: "",
    capture_device: "",
  });
});
