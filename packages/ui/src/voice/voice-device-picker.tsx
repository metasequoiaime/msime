import { SYSTEM_DEFAULT, STORED, FOREIGN } from "./voice-device-model";
import {
  useVoiceMicrophonePicker,
  useVoiceDevicePicker,
  type UseVoiceMicrophonePickerOptions,
  type UseVoiceDevicePickerOptions,
} from "./use-voice-device-picker";
import { ActionRow } from "../settings/action-row";
import { SelectRow } from "../settings/select-row";

export type { VoiceCaptureDevice, VoiceDeviceReader } from "./voice-device-model";

export interface VoiceMicrophonePickerProps extends UseVoiceMicrophonePickerOptions {
  /** 选中的麦克风拔掉后改用系统默认设备（HarmonyOS），而不是拒绝录音（macOS、Windows）。 */
  fallsBackToDefault?: boolean;
}
export type VoiceDevicePickerProps = UseVoiceDevicePickerOptions;

/**
 * 只有一种录音后端的宿主（macOS 只用 CoreAudio，Windows、HarmonyOS 也各只有一种）上，录音设备只用一个「麦克风」下拉选：第一项是系统默认设备，下面是读到的设备。可见时自动读取设备列表，不需要先点刷新。
 *
 * 系统默认设备这一项必须有：在 macOS 和 Windows 上，选过的耳机或 USB 麦克风拔掉后录音会失败，只有改回它才能恢复；HarmonyOS 自己会退回系统默认设备，说明文字随之不同。存着别的平台的后端（比如 `pulse`）时宿主不理会它和它的设备、用系统默认设备录音，这里也照实显示为系统默认设备，但用单独的一项标出存着外来设置，选「系统默认设备」就把它清掉；打开页面时不改写存储。
 */
export function VoiceMicrophonePicker({
  read,
  active,
  device,
  backendSupported,
  fallsBackToDefault = false,
  choose,
}: VoiceMicrophonePickerProps) {
  const { devices, loaded, error, busy, value, select } = useVoiceMicrophonePicker({
    read,
    active,
    device,
    backendSupported,
    choose,
  });
  return (
    <SelectRow
      title="麦克风"
      description={
        fallsBackToDefault
          ? "从下一次录音生效。选中的麦克风拔掉后改用系统默认设备；系统语音识别由服务自行取音，不受此项影响。"
          : "从下一次录音生效。选中的麦克风拔掉后无法录音，改回系统默认设备即可。"
      }
      detail={<span role="status">{error}</span>}
      aria-label="麦克风"
      aria-busy={busy}
      value={value}
      onChange={(event) => select(event.target.value)}
    >
      <option value={SYSTEM_DEFAULT}>系统默认设备</option>
      {devices.map((item, position) => (
        <option key={`${item.backend}:${item.id}`} value={String(position)}>
          {item.label}
        </option>
      ))}
      {value === FOREIGN && (
        <option value={FOREIGN} disabled>
          系统默认设备（已忽略其他平台的录音设置）
        </option>
      )}
      {value === STORED && (
        <option value={STORED} disabled>
          {/* 读取失败也算读完了：失败原因已经写在说明里，不能一直显示「读取中」。 */}
          {loaded || error ? "已选的麦克风（未连接）" : "已选的麦克风（读取中…）"}
        </option>
      )}
    </SelectRow>
  );
}

export function VoiceDevicePicker({ read, backend, device, choose }: VoiceDevicePickerProps) {
  const { devices, notice, busy, refresh, selected, select } = useVoiceDevicePicker({
    read,
    backend,
    device,
    choose,
  });
  // 「录音设备」分组里的两行：先读取设备列表，再从中选择。HarmonyOS 手机上选择行与其他选择行一样点开选择面板。
  return (
    <>
      <ActionRow
        title="读取录音设备"
        detail={<span role="status">{notice}</span>}
        action={() => void refresh()}
        ariaBusy={busy}
        className="secondary m-0"
        disabled={busy}
        label={busy ? "读取中…" : "刷新设备"}
      />
      <SelectRow
        title="可用录音设备"
        aria-label="可用录音设备"
        value={selected < 0 ? "" : String(selected)}
        disabled={!devices.length || busy}
        onChange={(event) => select(event.target.value)}
      >
        <option value="" disabled hidden>
          选择设备
        </option>
        {devices.map((item, index) => (
          <option key={`${item.backend}:${item.id}`} value={index}>
            {item.label} ({item.backend})
          </option>
        ))}
      </SelectRow>
    </>
  );
}
