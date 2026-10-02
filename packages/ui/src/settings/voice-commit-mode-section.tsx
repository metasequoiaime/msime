import { SelectRow } from "./select-row";

export type VoiceCommitMode = "tsf" | "sendinput" | "ctrl_v";

export interface VoiceCommitModeSectionProps {
  macos: boolean;
  value: VoiceCommitMode;
  onChange: (value: VoiceCommitMode) => void;
}

/** Result delivery strategy for desktop voice recognition. */
export function VoiceCommitModeSection({ macos, value, onChange }: VoiceCommitModeSectionProps) {
  return (
    <SelectRow
      title="结果提交策略"
      description={
        macos
          ? "系统按键和 Command-V 粘贴需要系统事件权限；不可用时回退到输入法会话，粘贴会替换剪贴板内容"
          : "由当前桌面宿主决定如何把识别结果交给前台窗口"
      }
      aria-label="结果提交策略"
      value={value}
      onChange={(event) => onChange(event.target.value as VoiceCommitMode)}
    >
      <option value="tsf">输入法会话</option>
      <option value="sendinput">系统按键</option>
      <option value="ctrl_v">剪贴板粘贴</option>
    </SelectRow>
  );
}
