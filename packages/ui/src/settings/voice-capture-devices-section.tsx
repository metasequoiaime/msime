import type { VoiceDeviceReader } from "../voice/voice-device-model";
import { SettingsGroupNote } from "./settings-group-note";
import { VoiceDevicePicker, VoiceMicrophonePicker } from "../voice/voice-device-picker";
import { GroupList } from "../core/platform-controls";
import { SelectRow } from "./select-row";
import { TextInputRow } from "./text-input-row";

export type VoiceCaptureBackendOption = readonly [string, string];
export type VoiceCaptureBackend =
  | ""
  | "auto"
  | "pulse"
  | "pipewire"
  | "alsa"
  | "macos"
  | "windows"
  | "harmony";

export interface VoiceCaptureDevicesSectionProps {
  windows: boolean;
  harmony: boolean;
  /** Linux 的空后端是「沿用服务设置」：录音由单独运行的语音服务负责。 */
  linux?: boolean;
  /** 宿主有不止一种录音后端（`voice_capture_backends`）。没有时只画一个「麦克风」下拉。 */
  backends?: boolean;
  /** 分组在屏幕上；只有一个下拉时，每次变为可见都自动读取设备列表。 */
  active?: boolean;
  backend: VoiceCaptureBackend;
  device: string;
  backendOptions: readonly VoiceCaptureBackendOption[];
  readDevices: VoiceDeviceReader;
  onBackendChange: (backend: VoiceCaptureBackend, device: string) => void;
  onDeviceChange: (device: string) => void;
}

/** Recording backend and microphone selection controls shared by desktop hosts. */
export function VoiceCaptureDevicesSection({
  windows,
  harmony,
  linux = false,
  backends = true,
  active = true,
  backend,
  device,
  backendOptions,
  readDevices,
  onBackendChange,
  onDeviceChange,
}: VoiceCaptureDevicesSectionProps) {
  if (!backends)
    return (
      <GroupList title="录音设备">
        <VoiceMicrophonePicker
          read={readDevices}
          active={active}
          device={device}
          // 与宿主的 MSIMEVoiceCaptureBackendSupported、Windows 的 VoiceCaptureSelection、HarmonyOS 的 VoiceCaptureDevicePolicy 一样不分大小写。
          backendSupported={
            backend === "" || backendOptions.some(([value]) => value === backend.toLowerCase())
          }
          fallsBackToDefault={harmony}
          choose={onBackendChange}
        />
      </GroupList>
    );
  return (
    <GroupList title="录音设备">
      <SettingsGroupNote>保存后从下一次录音生效，不打断当前录音</SettingsGroupNote>
      <SelectRow
        title="录音后端"
        aria-label="录音后端"
        value={backend}
        onChange={(event) => onBackendChange(event.target.value as VoiceCaptureBackend, "")}
      >
        <option value="">{linux ? "沿用服务设置" : "系统默认"}</option>
        {backendOptions.map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
        {backend && !backendOptions.some(([value]) => value === backend) && (
          <option value={backend} disabled>
            {backend}（此平台不可用）
          </option>
        )}
      </SelectRow>
      <VoiceDevicePicker
        read={readDevices}
        backend={backend}
        device={device}
        choose={onBackendChange}
      />
      <TextInputRow
        title="麦克风设备"
        description={
          windows
            ? "刷新列表并选择麦克风，保存其端点标识而非设备序号。留空使用系统默认设备；已选设备不可用时录音失败，不切换到其他麦克风。"
            : harmony
              ? "刷新列表并选择麦克风，保存的是设备类型与地址，重启后仍然有效。留空使用系统默认设备；已选设备拔掉后回到系统默认，不会中断录音。系统语音识别由服务自行取音，不受此项影响。"
              : linux
                ? "填写 PulseAudio source、PipeWire 节点名称或序号、ALSA PCM 名称。选择后端后留空使用系统默认设备；沿用服务设置时留空使用服务设备。"
                : "刷新列表并选择麦克风。留空使用系统默认设备。"
        }
        label="麦克风设备"
        maxLength={windows ? 1024 : 128}
        value={device}
        onChange={onDeviceChange}
      />
    </GroupList>
  );
}
