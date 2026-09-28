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

/** Shared 以词定字 controls and their mutual exclusion with paging shortcuts. */
export function WordCharacterSection({
  preferences,
  navigation,
  ios,
  onChange,
}: WordCharacterSectionProps) {
  const updateWordCharacter = (wordCharacter: WordCharacterPreferences) =>
    onChange({ wordCharacter, navigation });

  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          以词定字
          <small>
            {ios
              ? "开启后，长按两个字以上的候选，可以只上屏它的首字或末字"
              : "开启后，按所选键组的左键上屏高亮候选的首个汉字，右键上屏末个汉字"}
          </small>
        </span>
        <input
          aria-label="以词定字"
          className="toggle"
          type="checkbox"
          checked={preferences.enabled}
          onChange={(event) => {
            const enabled = event.target.checked;
            onChange({
              wordCharacter: { ...preferences, enabled },
              navigation: enabled ? { ...navigation, [preferences.keys]: false } : navigation,
            });
          }}
        />
      </label>
      {!ios && (
        <div className="word-to-character-keys-row">
          <div className="section-title" id="word-character-title">
            以词定字快捷键
          </div>
          <div
            className="input-option-content"
            role="radiogroup"
            aria-labelledby="word-character-title"
          >
            {(
              [
                ["brackets", "[ / ]"],
                ["minus_equal", "- / ="],
              ] as const
            ).map(([keys, label], index) => (
              <div className="input-option-item" key={keys}>
                {index > 0 && <div className="input-option-divider" />}
                <label className="radio-option">
                  <input
                    type="radio"
                    name="word-character-keys"
                    checked={preferences.keys === keys}
                    disabled={navigation[keys]}
                    onChange={() => updateWordCharacter({ ...preferences, keys })}
                  />
                  <span>{label}</span>
                </label>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
