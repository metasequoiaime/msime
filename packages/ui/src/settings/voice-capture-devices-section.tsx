import { VoiceDevicePicker, type VoiceDeviceReader } from "../voice/voice-device-picker";

export type VoiceCaptureBackend =
  | ""
  | "auto"
  | "pulse"
  | "pipewire"
  | "alsa"
  | "windows"
  | "macos"
  | "harmony";
export type VoiceCaptureBackendOption = readonly [Exclude<VoiceCaptureBackend, "">, string];

export function VoiceCaptureDevicesSection({
  windows,
  harmony,
  backend,
  device,
  options,
  read,
  onChange,
}: {
  windows: boolean;
  harmony: boolean;
  backend?: VoiceCaptureBackend;
  device?: string;
  options: readonly VoiceCaptureBackendOption[];
  read: VoiceDeviceReader;
  onChange: (patch: { capture_backend?: VoiceCaptureBackend; capture_device?: string }) => void;
}) {
  return (
    <div className="section">
      <div className="section-title">
        录音设备<small>保存后从下一次录音生效，不打断当前录音</small>
      </div>
      <label className="section-header">
        <span className="section-title">录音后端</span>
        <select
          aria-label="录音后端"
          value={backend ?? ""}
          onChange={(event) =>
            onChange({
              capture_backend: event.target.value as VoiceCaptureBackend,
              capture_device: "",
            })
          }
        >
          <option value="">{windows ? "系统默认" : "沿用服务设置"}</option>
          {options.map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
          {backend && !options.some(([value]) => value === backend) && (
            <option value={backend} disabled>
              {backend}（此平台不可用）
            </option>
          )}
        </select>
      </label>
      <VoiceDevicePicker
        read={read}
        backend={backend ?? ""}
        device={device ?? ""}
        choose={(capture_backend, capture_device) => onChange({ capture_backend, capture_device })}
      />
      <label className="section-header">
        <span className="section-title">
          麦克风设备
          <small>
            {windows
              ? "刷新列表并选择麦克风，保存其端点标识而非设备序号。留空使用系统默认设备；已选设备不可用时录音失败，不切换到其他麦克风。旧的数字序号需重新选择。"
              : harmony
                ? "刷新列表并选择麦克风，保存的是设备类型与地址，重启后仍然有效。留空使用系统默认设备；已选设备拔掉后回到系统默认，不会中断录音。系统语音识别由服务自行取音，不受此项影响。"
                : "填写 PulseAudio source、PipeWire 节点名称或序号、ALSA PCM 名称。选择后端后留空使用系统默认设备；沿用服务设置时留空使用服务设备。"}
          </small>
        </span>
        <input
          aria-label="麦克风设备"
          maxLength={windows ? 1024 : 128}
          value={device ?? ""}
          onChange={(event) => onChange({ capture_device: event.target.value })}
        />
      </label>
    </div>
  );
}
