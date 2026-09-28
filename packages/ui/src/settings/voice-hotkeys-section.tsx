import {
  description,
  hotkeyOptions,
  type VoiceHotkeyKey,
  type VoiceHotkeyPlatform,
} from "./voice-hotkeys";

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
        <label className="section-header" key={key}>
          <span className="section-title">{label}</span>
          <input
            aria-label={label}
            className="toggle"
            type="checkbox"
            checked={values[key] !== false}
            onChange={(event) => onChange(key, event.target.checked)}
          />
        </label>
      ))}
    </div>
  );
}
