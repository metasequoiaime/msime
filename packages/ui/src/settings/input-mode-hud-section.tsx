import { SwitchRow } from "./switch-row";

export interface InputModeHudSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
  /** Use the wording of the shortcut group, where macOS keeps this switch with the chords that trigger it. */
  shortcut?: boolean;
}

/** 中英文切换提示开关：设置窗口里是输入页「中英文」组的一行；`shortcut` 措辞留给把它放进快捷键组的宿主。 */
export function InputModeHudSection({
  value,
  onChange,
  shortcut = false,
}: InputModeHudSectionProps) {
  return shortcut ? (
    <SwitchRow
      title="切换中英文时显示提示"
      description="切换后在光标下方短暂显示「中」或「英」。"
      checked={value ?? true}
      onChange={onChange}
    />
  ) : (
    <SwitchRow
      title="中英文切换提示"
      description="切换输入模式后，在光标附近短暂显示“中”或“英”，不会抢占焦点。"
      checked={value ?? true}
      onChange={onChange}
    />
  );
}
