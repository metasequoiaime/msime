import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import {
  SYSTEM_DEFAULT,
  STORED,
  FOREIGN,
  type VoiceCaptureDevice,
  type VoiceDeviceReader,
} from "./voice-device-model";

export interface UseVoiceMicrophonePickerOptions {
  read: VoiceDeviceReader;
  /** 下拉在屏幕上；每次变为可见时重新读取设备列表。 */
  active: boolean;
  device: string;
  /** 存着的后端在当前宿主上有效；无效时宿主忽略存着的设备。 */
  backendSupported: boolean;
  choose: (backend: VoiceCaptureDevice["backend"] | "", device: string) => void;
}

export interface UseVoiceDevicePickerOptions {
  read: VoiceDeviceReader;
  backend: string;
  device: string;
  choose: (backend: VoiceCaptureDevice["backend"], device: string) => void;
}

/** 单后端麦克风的自动读取、缺失设备占位与系统默认选择。 */
export function useVoiceMicrophonePicker({
  read,
  active,
  device,
  backendSupported,
  choose,
}: UseVoiceMicrophonePickerOptions) {
  const [devices, setDevices] = useState<VoiceCaptureDevice[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const { busy, run } = useAsyncActionRunner(setError, undefined, read);
  useEffect(() => {
    setDevices([]);
    setLoaded(false);
    setError("");
  }, [read]);
  useEffect(() => {
    if (!active) return;
    void run(
      async (isCurrent) => {
        const result = await read();
        if (!isCurrent()) return;
        setDevices(result);
        setLoaded(true);
      },
      { formatError: () => "无法读取设备列表，可以先用系统默认设备" },
    );
  }, [active, read, run]);
  const index = device ? devices.findIndex((item) => item.id === device) : -1;
  const value = !backendSupported
    ? FOREIGN
    : !device
      ? SYSTEM_DEFAULT
      : index >= 0
        ? String(index)
        : STORED;
  const select = (next: string) => {
    const item = next === SYSTEM_DEFAULT ? undefined : devices[Number(next)];
    if (item) choose(item.backend, item.id);
    else if (next === SYSTEM_DEFAULT) choose("", "");
  };
  return { devices, loaded, error, busy, value, select };
}

export type VoiceMicrophonePickerState = ReturnType<typeof useVoiceMicrophonePicker>;

/** 多后端设备的显式刷新与后端、设备标识匹配；挂载不读取设备。 */
export function useVoiceDevicePicker({
  read,
  backend,
  device,
  choose,
}: UseVoiceDevicePickerOptions) {
  const [devices, setDevices] = useState<VoiceCaptureDevice[]>([]);
  const [notice, setNotice] = useState("点击刷新读取可用录音设备");
  const { busy, run } = useAsyncActionRunner(
    (message) => {
      if (message) setNotice(message);
    },
    undefined,
    read,
  );
  useEffect(() => {
    setDevices([]);
    setNotice("点击刷新读取可用录音设备");
  }, [read]);
  function refresh() {
    return run(
      async (isCurrent) => {
        const result = await read();
        if (!isCurrent()) return;
        setDevices(result);
        setNotice(
          result.length
            ? "选择设备后从下一次录音生效"
            : "未发现设备，可手动填写设备名称或使用默认设备",
        );
      },
      { formatError: () => "无法读取设备列表，可重试或手动填写" },
    );
  }
  const selected = devices.findIndex((item) => item.backend === backend && item.id === device);
  const select = (value: string) => {
    // 空值是禁用占位，不能被转换成设备序号 0。
    if (value === "") return;
    const item = devices[Number(value)];
    if (item) choose(item.backend, item.id);
  };
  return { devices, notice, busy, refresh, selected, select };
}

export type VoiceDevicePickerState = ReturnType<typeof useVoiceDevicePicker>;
