import {
  description,
  hotkeyOptions,
  type VoiceHotkeyKey,
  type VoiceHotkeyPlatform,
} from "./voice-hotkeys";
import { SettingToggle } from "./setting-toggle";

export type { VoiceHotkeyKey, VoiceHotkeyPlatform } from "./voice-hotkeys";

export interface VoiceHotkeysSectionProps {
  platform: VoiceHotkeyPlatform;
  values: Partial<Record<VoiceHotkeyKey, boolean>>;
  onChange: (key: VoiceHotkeyKey, enabled: boolean) => void;
}

/** Platform-aware voice recording shortcut controls shared by desktop settings hosts. */
export function VoiceHotkeysSection({ platform, values, onChange }: VoiceHotkeysSectionProps) {
  return (
    <div className="section">
      <div className="section-title">
        语音快捷键
        <small>{description(platform)}</small>
      </div>
      {/* Linux hosts share the desktop labels; only their native key event handling differs. */}
      {hotkeyOptions(platform).map(([key, label]) => (
        <SettingToggle
          key={key}
          label={label}
          ariaLabel={label}
          checked={values[key] !== false}
          compact
          onChange={(enabled) => onChange(key, enabled)}
        />
      ))}
    </div>
  );
}
