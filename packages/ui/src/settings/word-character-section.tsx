import { SegmentedRow } from "./segmented-row";
import { SwitchRow } from "./switch-row";
import { wordCharacterKeyOptions } from "./candidate-paging-options";
import {
  wordCharacterEnabledChange,
  type NavigationPreferences,
  type WordCharacterPreferences,
  type CandidatePagingChange,
} from "./candidate-paging-preferences";
export { defaultWordCharacter } from "./candidate-paging-preferences";
export type {
  NavigationPreferences,
  WordCharacterPreferences,
} from "./candidate-paging-preferences";

export interface WordCharacterSectionProps {
  preferences: WordCharacterPreferences;
  navigation: NavigationPreferences;
  ios: boolean;
  onChange: (next: CandidatePagingChange) => void;
}

/** 共用的以词定字设置及其与翻页键的互斥：输入页「选词与翻页」组里的几行。 */
export function WordCharacterSection({
  preferences,
  navigation,
  ios,
  onChange,
}: WordCharacterSectionProps) {
  return (
    <>
      <SwitchRow
        title="以词定字"
        description={
          ios
            ? "开启后，长按两个字以上的候选，可以只上屏它的首字或末字"
            : "开启后，按所选键组的左键上屏高亮候选的首个汉字，右键上屏末个汉字"
        }
        checked={preferences.enabled}
        onChange={(enabled) =>
          onChange(wordCharacterEnabledChange(preferences, navigation, enabled))
        }
      />
      {/* An iOS keyboard extension never receives hardware keys; its candidates offer the first and last character on a long press instead. */}
      {!ios && (
        <SegmentedRow
          title="以词定字快捷键"
          options={wordCharacterKeyOptions(navigation)}
          value={preferences.keys}
          onChange={(keys) => onChange({ wordCharacter: { ...preferences, keys }, navigation })}
        />
      )}
    </>
  );
}
