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
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          候选词翻译<small>为当前候选请求翻译结果并显示在候选行</small>
        </span>
        <input
          aria-label="候选词翻译"
          className="toggle"
          type="checkbox"
          checked={enabled}
          onChange={(event) => onEnabledChange(event.target.checked)}
        />
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">目标语言</span>
        <select
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
        </select>
      </label>
      {showSecondaryLanguage && (
        <>
          <div className="input-option-divider" />
          <label className="section-header">
            <span className="section-title">
              第二种语言<small>候选词下方可同时显示第二种释义</small>
            </span>
            <select
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
            </select>
          </label>
        </>
      )}
      {showAccountTranslation && (
        <>
          <div className="input-option-divider" />
          <label className="section-header">
            <span className="section-title">
              使用水杉账号翻译候选词
              <small>
                当前页的中文候选词会发送到 api.msime.app；匿名账号在 Linux
                安装后的用户初始化中自动注册；不开启则不联网翻译
              </small>
            </span>
            <input
              aria-label="使用水杉账号翻译候选词"
              className="toggle"
              type="checkbox"
              disabled={!enabled}
              checked={accountTranslation}
              onChange={(event) => onAccountTranslationChange(event.target.checked)}
            />
          </label>
        </>
      )}
    </div>
  );
}
