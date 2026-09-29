import { Row, Segmented, Switch } from "../core/platform-controls";

export type WordCharacterPreferences = {
  enabled: boolean;
  keys: "brackets" | "minus_equal";
};

export const defaultWordCharacter: WordCharacterPreferences = { enabled: true, keys: "brackets" };

export type NavigationPreferences = {
  minus_equal: boolean;
  comma_period: boolean;
  brackets: boolean;
  tab: boolean;
  page_up_down: boolean;
  mouse_wheel?: boolean;
  arrows: boolean;
};

export interface WordCharacterSectionProps {
  preferences: WordCharacterPreferences;
  navigation: NavigationPreferences;
  ios: boolean;
  onChange: (next: {
    wordCharacter: WordCharacterPreferences;
    navigation: NavigationPreferences;
  }) => void;
}

const wordCharacterKeyOptions = [
  { value: "brackets", label: "[ / ]" },
  { value: "minus_equal", label: "- / =" },
] as const satisfies readonly { value: WordCharacterPreferences["keys"]; label: string }[];

/** Shared 以词定字 controls and their mutual exclusion with paging shortcuts: rows of the 选词 group. */
export function WordCharacterSection({
  preferences,
  navigation,
  ios,
  onChange,
}: WordCharacterSectionProps) {
  return (
    <>
      <Row
        title="以词定字"
        description={
          ios
            ? "开启后，长按两个字以上的候选，可以只上屏它的首字或末字"
            : "开启后，按所选键组的左键上屏高亮候选的首个汉字，右键上屏末个汉字"
        }
      >
        <Switch
          checked={preferences.enabled}
          onChange={(enabled) =>
            onChange({
              wordCharacter: { ...preferences, enabled },
              navigation: enabled ? { ...navigation, [preferences.keys]: false } : navigation,
            })
          }
        />
      </Row>
      {/* An iOS keyboard extension never receives hardware keys; its candidates offer the first and last character on a long press instead. */}
      {!ios && (
        <Row title="以词定字快捷键">
          <Segmented
            options={wordCharacterKeyOptions.map((option) => ({
              ...option,
              disabled: navigation[option.value],
            }))}
            value={preferences.keys}
            onChange={(keys) => onChange({ wordCharacter: { ...preferences, keys }, navigation })}
          />
        </Row>
      )}
    </>
  );
}
import { SettingToggle } from "./setting-toggle";
