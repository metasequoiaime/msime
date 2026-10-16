import { SettingsGroupNote } from "./settings-group-note";
import {
  VOICE_HOLD_KEYS,
  description,
  holdChoice,
  holdKeyPatch,
  hotkeyOptions,
  selectedHoldKey,
  type VoiceHotkeyKey,
  type VoiceHotkeyPlatform,
} from "./voice-hotkeys";
import { GroupList, MoreOptions } from "../core/platform-controls";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";

export type { VoiceHotkeyKey, VoiceHotkeyPlatform } from "./voice-hotkeys";

/** 「不使用」在下拉里的值；三个组合键的值是各自的偏好名。 */
const NO_HOLD = "none";

export interface VoiceHotkeysSectionProps {
  platform: VoiceHotkeyPlatform;
  values: Partial<Record<VoiceHotkeyKey, boolean>>;
  /** 一次改动可能同时写几个键：换按住录音的组合键时三个一起写。 */
  onChange: (patch: Partial<Record<VoiceHotkeyKey, boolean>>) => void;
  /** 宿主读空格锁定（`voice_space_lock_switch`）。macOS、Windows、Linux、HarmonyOS 把它固定为开，不给开关。 */
  spaceLock?: boolean;
}

/** Platform-aware voice recording shortcut controls shared by desktop settings hosts. */
export function VoiceHotkeysSection({
  platform,
  values,
  onChange,
  spaceLock = true,
}: VoiceHotkeysSectionProps) {
  const labels = Object.fromEntries(hotkeyOptions(platform)) as Record<VoiceHotkeyKey, string>;
  const hold = holdChoice(platform);
  const selected = selectedHoldKey(values);
  return (
    <GroupList title="语音快捷键">
      <SettingsGroupNote>{description(platform)}</SettingsGroupNote>
      {/* 三个按住录音的组合键合成一个选择：宿主同一时刻只认一个按住的组合键。选项直接写成 `<option>`，HarmonyOS 的选择面板只认直接子元素。 */}
      <SelectRow
        title={hold.title}
        aria-label={hold.title}
        value={selected ?? NO_HOLD}
        onChange={(event) => {
          const value = event.target.value;
          onChange(holdKeyPatch(VOICE_HOLD_KEYS.find((key) => key === value) ?? null));
        }}
      >
        {VOICE_HOLD_KEYS.map((key) => (
          <option key={key} value={key}>
            {hold.labels[key]}
          </option>
        ))}
        <option value={NO_HOLD}>不使用</option>
      </SelectRow>
      {/* 空格锁定只在按住组合键期间起作用，没选组合键时这一行什么也不做，不显示。 */}
      {spaceLock && selected && (
        <SwitchRow
          title={labels.hotkey_hold_space_lock}
          aria-label={labels.hotkey_hold_space_lock}
          checked={values.hotkey_hold_space_lock !== false}
          onChange={(enabled) => onChange({ hotkey_hold_space_lock: enabled })}
        />
      )}
      <MoreOptions>
        <SwitchRow
          title={labels.hotkey_ctrl_f9}
          aria-label={labels.hotkey_ctrl_f9}
          checked={values.hotkey_ctrl_f9 !== false}
          onChange={(enabled) => onChange({ hotkey_ctrl_f9: enabled })}
        />
      </MoreOptions>
    </GroupList>
  );
}
