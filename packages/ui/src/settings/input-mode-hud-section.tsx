import { Row, Switch } from "../core/platform-controls";

export interface InputModeHudSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
  /** Use the wording of the shortcut group, where macOS keeps this switch with the chords that trigger it. */
  shortcut?: boolean;
}

/** Input-mode badge switch shared by the input and shortcut settings pages: one row of the group it is placed in. */
export function InputModeHudSection({
  value,
  onChange,
  shortcut = false,
}: InputModeHudSectionProps) {
  return shortcut ? (
    <Row title="切换中英文时显示提示" description="切换后在光标下方短暂显示「中」或「英」。">
      <Switch checked={value ?? true} onChange={onChange} />
    </Row>
  ) : (
    <Row
      title="中英文切换提示"
      description="切换输入模式后，在光标附近短暂显示“中”或“英”，不会抢占焦点。"
    >
      <Switch checked={value ?? true} onChange={onChange} />
    </Row>
  );
}
import { SettingToggle } from "./setting-toggle";
