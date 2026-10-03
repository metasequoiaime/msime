import { SettingsGroupNote } from "./settings-group-note";
import {
  description,
  hotkeyOptions,
  type VoiceHotkeyKey,
  type VoiceHotkeyPlatform,
} from "./voice-hotkeys";
import { GroupList } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";

export type { VoiceHotkeyKey, VoiceHotkeyPlatform } from "./voice-hotkeys";

export interface VoiceHotkeysSectionProps {
  platform: VoiceHotkeyPlatform;
  values: Partial<Record<VoiceHotkeyKey, boolean>>;
  onChange: (key: VoiceHotkeyKey, enabled: boolean) => void;
}

/** Platform-aware voice recording shortcut controls shared by desktop settings hosts. */
export function VoiceHotkeysSection({ platform, values, onChange }: VoiceHotkeysSectionProps) {
  return (
    <GroupList title="语音快捷键">
      <SettingsGroupNote>{description(platform)}</SettingsGroupNote>
      {/* Linux hosts share the desktop labels; only their native key event handling differs. */}
      {hotkeyOptions(platform).map(([key, label]) => (
        <SwitchRow
          key={key}
          title={label}
          aria-label={label}
          checked={values[key] !== false}
          onChange={(enabled) => onChange(key, enabled)}
        />
      ))}
    </GroupList>
  );
}
