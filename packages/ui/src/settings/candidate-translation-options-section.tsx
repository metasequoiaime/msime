import { Row } from "../core/platform-controls";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";

export type TranslationLanguage = "en" | "fr" | "ja" | "es" | "ru" | "de" | "ko";
export type TranslationSecondaryLanguage = TranslationLanguage | "";
export type TranslationLanguageOption = readonly [TranslationLanguage, string];
export type TranslationSecondaryLanguageOption = readonly [TranslationSecondaryLanguage, string];

export interface CandidateTranslationOptionsSectionProps {
  enabled: boolean;
  targetLanguage: TranslationLanguage;
  secondaryLanguage: TranslationSecondaryLanguage | null;
  candidateGlossLanguagesEnabled: boolean;
  visibleLanguages: readonly TranslationLanguageOption[];
  visibleSecondaryLanguages: readonly TranslationSecondaryLanguageOption[];
  showSecondaryLanguage: boolean;
  showAccountTranslation: boolean;
  accountTranslation: boolean;
  onEnabledChange: (enabled: boolean) => void;
  onTargetLanguageChange: (language: TranslationLanguage) => void;
  onSecondaryLanguageChange: (language: TranslationSecondaryLanguage) => void;
  onAccountTranslationChange: (enabled: boolean) => void;
}

/** Candidate translation enablement and language choices shared by desktop and mobile settings. */
export function CandidateTranslationOptionsSection({
  enabled,
  targetLanguage,
  secondaryLanguage,
  candidateGlossLanguagesEnabled,
  visibleLanguages,
  visibleSecondaryLanguages,
  showSecondaryLanguage,
  showAccountTranslation,
  accountTranslation,
  onEnabledChange,
  onTargetLanguageChange,
  onSecondaryLanguageChange,
  onAccountTranslationChange,
}: CandidateTranslationOptionsSectionProps) {
  return (
    <>
      <SwitchRow
        title="候选词翻译"
        description="为当前候选请求翻译结果并显示在候选行"
        checked={enabled}
        onChange={onEnabledChange}
      />
      <SelectRow
        title="目标语言"
        aria-label="候选词翻译目标语言"
        disabled={!candidateGlossLanguagesEnabled}
        value={targetLanguage}
        onChange={(event) => onTargetLanguageChange(event.target.value as TranslationLanguage)}
      >
        {visibleLanguages.map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
      </SelectRow>
      {showSecondaryLanguage && (
        <SelectRow
          title="第二种语言"
          description="候选词下方可同时显示第二种释义"
          aria-label="候选词翻译第二种语言"
          disabled={!candidateGlossLanguagesEnabled}
          value={secondaryLanguage ?? ""}
          onChange={(event) =>
            onSecondaryLanguageChange(event.target.value as TranslationSecondaryLanguage)
          }
        >
          {visibleSecondaryLanguages.map(([value, label]) => (
            <option key={value || "none"} value={value}>
              {label}
            </option>
          ))}
        </SelectRow>
      )}
      {showAccountTranslation && (
        <SwitchRow
          title="使用水杉账号翻译候选词"
          description="当前页的中文候选词会发送到 api.msime.app；匿名账号在 Linux 安装后的用户初始化中自动注册；不开启则不联网翻译"
          disabled={!enabled}
          checked={accountTranslation}
          onChange={onAccountTranslationChange}
        />
      )}
    </>
  );
}
