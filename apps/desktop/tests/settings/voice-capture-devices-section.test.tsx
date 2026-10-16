// @vitest-environment jsdom
import { afterEach, expect, test, vi, type Mock } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  VoiceCaptureDevicesSection,
  VoiceDevicePicker,
  VoiceMicrophonePicker,
  type VoiceCaptureBackend,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// 只有 Linux 的语音服务真有几种录音后端可选（`voice_capture_backends`），后端选择和手填设备名只在那里出现。
test("forwards backend and microphone edits", () => {
  const onBackendChange = vi.fn();
  const onDeviceChange = vi.fn();
  render(
    <VoiceCaptureDevicesSection
      windows={false}
      harmony={false}
      linux
      backend=""
      device=""
      backendOptions={[["pulse", "PulseAudio"]]}
      readDevices={vi.fn().mockResolvedValue([])}
      onBackendChange={onBackendChange}
      onDeviceChange={onDeviceChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("录音后端"), { target: { value: "pulse" } });
  fireEvent.change(screen.getByLabelText("麦克风设备"), { target: { value: "synthetic-device" } });

  expect(onBackendChange).toHaveBeenCalledWith("pulse", "");
  expect(onDeviceChange).toHaveBeenCalledWith("synthetic-device");
});

// Linux 的后端选择与 IBus、Fcitx5 和桌面语音面板的转发（`VoiceProviderOptions.h`、`voice.rs`）一样区分大小写：语音服务只认小写的后端名，存着的 `Pulse` 不会被转发，页面上也就写它不可用，不能显示成选中了 PulseAudio。
test("marks a stored backend with the wrong case unavailable on Linux, as the forwarders drop it", () => {
  render(
    <VoiceCaptureDevicesSection
      windows={false}
      harmony={false}
      linux
      backend={"Pulse" as VoiceCaptureBackend}
      device="synthetic-device"
      backendOptions={[["pulse", "PulseAudio"]]}
      readDevices={vi.fn().mockResolvedValue([])}
      onBackendChange={vi.fn()}
      onDeviceChange={vi.fn()}
    />,
  );

  const select = screen.getByLabelText<HTMLSelectElement>("录音后端");
  expect(select.value).toBe("Pulse");
  expect(within(select).getByRole("option", { name: "Pulse（此平台不可用）" })).toBeTruthy();
});

// HarmonyOS 选中的麦克风拔掉后宿主改用系统默认设备照常录音（HarmonyVoiceCaptureDevices），不能照搬 macOS、Windows 的「拔掉后无法录音」。
test("uses platform-specific microphone guidance", () => {
  const props = {
    backends: false,
    backend: "harmony" as const,
    device: "",
    readDevices: vi.fn().mockResolvedValue([]),
    onBackendChange: vi.fn(),
    onDeviceChange: vi.fn(),
  };
  const { rerender } = render(
    <VoiceCaptureDevicesSection
      {...props}
      windows={false}
      harmony
      backendOptions={[
        ["auto", "自动选择"],
        ["harmony", "HarmonyOS 音频"],
      ]}
    />,
  );
  expect(screen.getByText(/拔掉后改用系统默认设备/)).toBeTruthy();
  expect(screen.getByText(/系统语音识别由服务自行取音/)).toBeTruthy();
  expect(screen.queryByText(/拔掉后无法录音/)).toBeNull();
  rerender(
    <VoiceCaptureDevicesSection
      {...props}
      backend="windows"
      windows
      harmony={false}
      backendOptions={[
        ["auto", "自动选择"],
        ["windows", "Windows Audio"],
      ]}
    />,
  );
  expect(screen.getByText(/拔掉后无法录音/)).toBeTruthy();
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

// 「沿用服务设置」只在 Linux 说得通：那里录音由单独运行的语音服务负责。其他宿主的空后端就是系统默认。
test("names the empty backend after what it means on the platform", () => {
  const props = {
    windows: false,
    backend: "" as const,
    device: "",
    readDevices: vi.fn().mockResolvedValue([]),
    onBackendChange: vi.fn(),
    onDeviceChange: vi.fn(),
  };
  const { rerender } = render(
    <VoiceCaptureDevicesSection
      {...props}
      harmony={false}
      linux
      backendOptions={[["pulse", "PulseAudio"]]}
    />,
  );
  expect(screen.getByRole("option", { name: "沿用服务设置" })).toBeTruthy();
  expect(screen.getByText(/PulseAudio source/)).toBeTruthy();
  rerender(
    <VoiceCaptureDevicesSection
      {...props}
      harmony
      backendOptions={[["harmony", "HarmonyOS 音频"]]}
    />,
  );
  expect(screen.queryByRole("option", { name: "沿用服务设置" })).toBeNull();
  expect(screen.getByRole("option", { name: "系统默认" })).toBeTruthy();
});

const macDevices = [
  { backend: "macos" as const, id: "uid-usb", label: "Synthetic USB microphone" },
  { backend: "macos" as const, id: "uid-headset", label: "Synthetic headset" },
];

function renderMicrophone(
  overrides: {
    backend?: VoiceCaptureBackend;
    device?: string;
    active?: boolean;
    read?: Mock<() => Promise<typeof macDevices>>;
  } = {},
) {
  const read = overrides.read ?? vi.fn(async () => macDevices);
  const onBackendChange = vi.fn();
  const view = render(
    <VoiceCaptureDevicesSection
      windows={false}
      harmony={false}
      backends={false}
      active={overrides.active ?? true}
      backend={overrides.backend ?? ""}
      device={overrides.device ?? ""}
      backendOptions={[
        ["auto", "自动选择"],
        ["macos", "CoreAudio"],
      ]}
      readDevices={read}
      onBackendChange={onBackendChange}
      onDeviceChange={vi.fn()}
    />,
  );
  return { read, onBackendChange, view };
}

// macOS 只用 CoreAudio：录音后端、刷新按钮、可用设备和手填设备名合成一个「麦克风」下拉，第一项是系统默认设备，列表在可见时自动读取。
test("one host backend gets a single microphone select that loads by itself", async () => {
  const { read, onBackendChange } = renderMicrophone();
  expect(screen.queryByLabelText("录音后端")).toBeNull();
  expect(screen.queryByLabelText("麦克风设备")).toBeNull();
  expect(screen.queryByRole("button", { name: "刷新设备" })).toBeNull();
  expect(screen.queryByText(/PulseAudio/)).toBeNull();
  const select = screen.getByRole("combobox", { name: "麦克风" }) as HTMLSelectElement;
  expect(select.value).toBe("");
  expect(within(select).getByRole("option", { name: "系统默认设备" })).toBeTruthy();
  await waitFor(() =>
    expect(within(select).getByRole("option", { name: "Synthetic headset" })).toBeTruthy(),
  );
  expect(read).toHaveBeenCalledOnce();
  fireEvent.change(select, { target: { value: "1" } });
  expect(onBackendChange).toHaveBeenCalledWith("macos", "uid-headset");
});

// 系统默认设备这一项必须能选回去：选过的麦克风拔掉后录音会失败，只有改回它才能恢复。
test("an unplugged microphone stays visible and the system default clears it", async () => {
  const { onBackendChange } = renderMicrophone({ backend: "macos", device: "uid-gone" });
  const select = screen.getByRole("combobox", { name: "麦克风" }) as HTMLSelectElement;
  await waitFor(() =>
    expect(within(select).getByRole("option", { name: "已选的麦克风（未连接）" })).toBeTruthy(),
  );
  expect(select.value).toBe("stored");
  fireEvent.change(select, { target: { value: "" } });
  expect(onBackendChange).toHaveBeenCalledWith("", "");
});

// 别的平台写下的后端（比如 pulse）宿主不理会，连同它的设备一起，用系统默认设备录音；这里照实显示，打开时不改写存储。它用单独一项标出，选「系统默认设备」是一次真正的改动，能把外来的后端和设备清掉。
test("a foreign backend reads as the system default and choosing the default clears it", async () => {
  const { onBackendChange, read } = renderMicrophone({
    backend: "pulse",
    device: "alsa_input.synthetic",
  });
  const select = screen.getByRole("combobox", { name: "麦克风" }) as HTMLSelectElement;
  await waitFor(() => expect(read).toHaveBeenCalled());
  expect(select.value).toBe("foreign");
  expect(
    within(select).getByRole("option", { name: "系统默认设备（已忽略其他平台的录音设置）" }),
  ).toBeTruthy();
  expect(onBackendChange).not.toHaveBeenCalled();
  fireEvent.change(select, { target: { value: "" } });
  expect(onBackendChange).toHaveBeenCalledWith("", "");
});

// 宿主的 MSIMEVoiceCaptureBackendSupported 不分大小写，设置页也一样：存着 `MacOS` 时宿主照样读存着的设备。
test("the stored backend is matched without regard to case", async () => {
  renderMicrophone({ backend: "MacOS" as VoiceCaptureBackend, device: "uid-usb" });
  const select = screen.getByRole("combobox", { name: "麦克风" }) as HTMLSelectElement;
  await waitFor(() => expect(select.value).toBe("0"));
});

// 读取设备列表失败也算读完：说明里写着失败原因，存着的设备不能一直显示「读取中」。
test("a failed read stops showing the stored microphone as loading", async () => {
  renderMicrophone({
    backend: "macos",
    device: "uid-usb",
    read: vi.fn(async (): Promise<typeof macDevices> => {
      throw new Error("synthetic failure");
    }),
  });
  const select = screen.getByRole("combobox", { name: "麦克风" }) as HTMLSelectElement;
  await waitFor(() =>
    expect(screen.getByText("无法读取设备列表，可以先用系统默认设备")).toBeTruthy(),
  );
  expect(within(select).getByRole("option", { name: "已选的麦克风（未连接）" })).toBeTruthy();
  expect(within(select).queryByRole("option", { name: "已选的麦克风（读取中…）" })).toBeNull();
});

test("a stored device that is still connected is selected", async () => {
  renderMicrophone({ backend: "macos", device: "uid-usb" });
  const select = screen.getByRole("combobox", { name: "麦克风" }) as HTMLSelectElement;
  await waitFor(() => expect(select.value).toBe("0"));
});

test("the microphone list is not read while the group is off screen", async () => {
  const { read, view } = renderMicrophone({ active: false });
  expect(read).not.toHaveBeenCalled();
  view.rerender(
    <VoiceCaptureDevicesSection
      windows={false}
      harmony={false}
      backends={false}
      active
      backend=""
      device=""
      backendOptions={[["macos", "CoreAudio"]]}
      readDevices={read}
      onBackendChange={vi.fn()}
      onDeviceChange={vi.fn()}
    />,
  );
  await waitFor(() => expect(read).toHaveBeenCalledOnce());
});

test("切换麦克风读取器后不能选择旧读取器的设备", async () => {
  const initial = vi.fn(async () => macDevices);
  let finish!: (devices: typeof macDevices) => void;
  const pending = new Promise<typeof macDevices>((resolve) => {
    finish = resolve;
  });
  const replacement = vi.fn(() => pending);
  const props = { active: true, device: "uid-usb", backendSupported: true, choose: vi.fn() };
  const { rerender } = render(<VoiceMicrophonePicker {...props} read={initial} />);
  await screen.findByRole("option", { name: "Synthetic USB microphone" });
  rerender(<VoiceMicrophonePicker {...props} read={replacement} />);
  expect(screen.queryByRole("option", { name: "Synthetic USB microphone" })).toBeNull();
  expect(screen.getByRole("option", { name: "已选的麦克风（读取中…）" })).toBeTruthy();
  await act(async () => {
    finish([]);
  });
});

test("切换手动设备读取器后恢复待刷新提示", async () => {
  const initial = vi.fn(async () => macDevices);
  const replacement = vi.fn(async () => []);
  const props = { backend: "", device: "", choose: vi.fn() };
  const { rerender } = render(<VoiceDevicePicker {...props} read={initial} />);
  fireEvent.click(screen.getByRole("button", { name: "刷新设备" }));
  await screen.findByText("选择设备后从下一次录音生效");
  rerender(<VoiceDevicePicker {...props} read={replacement} />);
  expect(screen.getByRole("status").textContent).toBe("点击刷新读取可用录音设备");
  expect(screen.queryByRole("option", { name: /Synthetic USB microphone/ })).toBeNull();
  expect(replacement).not.toHaveBeenCalled();
});
