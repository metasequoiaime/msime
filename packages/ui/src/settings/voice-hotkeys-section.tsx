import {
  description,
  hotkeyOptions,
  type VoiceHotkeyKey,
  type VoiceHotkeyPlatform,
} from "./voice-hotkeys";
import * as settings from "./settings-style";
import { GroupList, Row, Switch } from "../core/platform-controls";

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
      <p className={settings.groupNote}>{description(platform)}</p>
      {/* Linux hosts share the desktop labels; only their native key event handling differs. */}
      {hotkeyOptions(platform).map(([key, label]) => (
        <Row key={key} title={label}>
          <Switch
            aria-label={label}
            checked={values[key] !== false}
            onChange={(enabled) => onChange(key, enabled)}
          />
        </Row>
      ))}
    </GroupList>
  );
}
