import { SegmentedRow } from "./segmented-row";
import { SwitchRow } from "./switch-row";
import { wordCharacterKeyOptions } from "./candidate-paging-options";
import {
  wordCharacterChoice,
  wordCharacterChoiceChange,
  wordCharacterEnabledChange,
  type NavigationPreferences,
  type WordCharacterChoice,
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

/** 共用的以词定字设置及其与翻页键的互斥：输入页「选词与翻页」组里的一行。实体键盘宿主上开关和键组合成一个「关闭 / [ ] / - =」单选：三种状态本来就是一个决定，拆成开关加键组时还会出现键组被翻页占着、开关却能打开的情况。 */
export function WordCharacterSection({
  preferences,
  navigation,
  ios,
  onChange,
}: WordCharacterSectionProps) {
  // An iOS keyboard extension never receives hardware keys; its candidates offer the first and last character on a long press instead.
  if (ios)
    return (
      <SwitchRow
        title="以词定字"
        description="开启后，长按两个字以上的候选，可以只上屏它的首字或末字"
        checked={preferences.enabled}
        onChange={(enabled) =>
          onChange(wordCharacterEnabledChange(preferences, navigation, enabled))
        }
      />
    );
  // 已被翻页占用的键组不能选（选中的那组除外，否则当前值会显示成没有选中），与原生回退窗口的互斥一致；选「关闭」时保留记着的键组。
  const current = wordCharacterChoice(preferences);
  const options = [
    { value: "off" as const, label: "关闭" },
    ...wordCharacterKeyOptions(navigation).map((option) => ({
      ...option,
      disabled: option.disabled && option.value !== current,
    })),
  ];
  return (
    <SegmentedRow<WordCharacterChoice>
      title="以词定字"
      description="按所选键组的左键上屏高亮候选的首个汉字，右键上屏末个汉字；已用于翻页的键组不能选"
      options={options}
      value={current}
      onChange={(choice) => onChange(wordCharacterChoiceChange(preferences, navigation, choice))}
    />
  );
}
