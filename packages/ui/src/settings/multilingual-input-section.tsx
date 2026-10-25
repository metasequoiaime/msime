import { SettingsRowStack } from "./settings-row-stack";
import { SwitchRow } from "./switch-row";

export type MultilingualLanguage = "chinese" | "english" | "japanese";

/** 取值与默认值沿用共享 `MultilingualInputPreferences`，有效语言由 Host API 筛选。 */
export type MultilingualInputPreferences = {
  enabled: boolean;
  languages: MultilingualLanguage[];
};

export const defaultMultilingualInput: MultilingualInputPreferences = {
  enabled: false,
  languages: ["chinese", "english", "japanese"],
};

const languages: { value: MultilingualLanguage; label: string }[] = [
  { value: "chinese", label: "中文" },
  { value: "english", label: "英文" },
  { value: "japanese", label: "日文" },
];

export function MultilingualInputSection({
  preferences = defaultMultilingualInput,
  temporaryJapanese,
  onChange,
}: {
  preferences?: MultilingualInputPreferences;
  temporaryJapanese: boolean;
  onChange: (preferences: MultilingualInputPreferences) => void;
}) {
  return (
    <SettingsRowStack role="group" aria-label="连续多语言输入">
      <SwitchRow
        title="连续多语言输入（实验）"
        description="开启候选混输后，在全拼 26 键中连续输入所选语言的编码；完整中文优先，混合结果作为备选。至少需要两种可用语言"
        checked={preferences.enabled}
        onChange={(enabled) => onChange({ ...preferences, enabled })}
      />
      {languages
        .filter(({ value }) => value !== "japanese" || temporaryJapanese)
        .map(({ value, label }) => (
          <SwitchRow
            key={value}
            title={`连续输入：${label}`}
            description={
              value === "japanese" ? "使用已有日文词典，词典未安装或准备中时暂不参与" : undefined
            }
            checked={preferences.languages.includes(value)}
            disabled={!preferences.enabled}
            onChange={(selected) =>
              onChange({
                ...preferences,
                languages: selected
                  ? [...preferences.languages.filter((language) => language !== value), value]
                  : preferences.languages.filter((language) => language !== value),
              })
            }
          />
        ))}
    </SettingsRowStack>
  );
}
