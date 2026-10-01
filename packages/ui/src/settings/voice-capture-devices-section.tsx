import { VoiceDevicePicker, type VoiceDeviceReader } from "../voice/voice-device-picker";
import { GroupList, Row } from "../core/platform-controls";
import * as settings from "./settings-style";
import { SelectRow } from "./select-row";

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
  backend,
  device,
  backendOptions,
  readDevices,
  onBackendChange,
  onDeviceChange,
}: VoiceCaptureDevicesSectionProps) {
  return (
    <GroupList title="录音设备">
      <p className={settings.groupNote}>保存后从下一次录音生效，不打断当前录音</p>
      <SelectRow
        title="录音后端"
        aria-label="录音后端"
        value={backend}
        onChange={(event) => onBackendChange(event.target.value as VoiceCaptureBackend, "")}
      >
        <option value="">{windows ? "系统默认" : "沿用服务设置"}</option>
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
      <div className={settings.groupBlock}>
        <VoiceDevicePicker
          read={readDevices}
          backend={backend}
          device={device}
          choose={onBackendChange}
        />
      </div>
      <Row
        title="麦克风设备"
        description={
          windows
            ? "刷新列表并选择麦克风，保存其端点标识而非设备序号。留空使用系统默认设备；已选设备不可用时录音失败，不切换到其他麦克风。旧的数字序号需重新选择。"
            : harmony
              ? "刷新列表并选择麦克风，保存的是设备类型与地址，重启后仍然有效。留空使用系统默认设备；已选设备拔掉后回到系统默认，不会中断录音。系统语音识别由服务自行取音，不受此项影响。"
              : "填写 PulseAudio source、PipeWire 节点名称或序号、ALSA PCM 名称。选择后端后留空使用系统默认设备；沿用服务设置时留空使用服务设备。"
        }
      >
        <input
          aria-label="麦克风设备"
          maxLength={windows ? 1024 : 128}
          value={device}
          onChange={(event) => onDeviceChange(event.target.value)}
        />
      </Row>
    </GroupList>
  );
}
