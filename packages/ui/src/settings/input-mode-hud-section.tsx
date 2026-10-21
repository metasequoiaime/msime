import { SwitchRow } from "./switch-row";

export interface InputModeHudSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
  /** 只有 Linux 提供聚焦提示；与切换提示各自保存，不互相禁用。 */
  focus?: { value?: boolean; onChange: (value: boolean) => void };
  /** Use the wording of the shortcut group, where macOS keeps this switch with the chords that trigger it. */
  shortcut?: boolean;
}

/** 中英文切换提示开关：设置窗口里是输入页「中英文」组的一行；`shortcut` 措辞留给把它放进快捷键组的宿主。 */
export function InputModeHudSection({
  value,
  onChange,
  focus,
  shortcut = false,
}: InputModeHudSectionProps) {
  const switchRow = shortcut ? (
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
  return (
    <>
      {switchRow}
      {focus && (
        <SwitchRow
          title="聚焦输入框时提示"
          description="进入输入框时，短暂显示当前的中英文状态。"
          checked={focus.value ?? false}
          onChange={focus.onChange}
        />
      )}
    </>
  );
}
