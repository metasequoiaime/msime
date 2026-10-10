import type { Preferences } from "../index";
import type { CandidateTranslationSettingsSectionProps } from "./candidate-translation-settings-section";
import type {
  TranslationLanguage,
  TranslationLanguageOption,
  TranslationSecondaryLanguage,
  TranslationSecondaryLanguageOption,
} from "./candidate-translation-options-section";
import type { TranslationProvider } from "./translation-service-selector-section";

export interface CandidateTranslationSettingsOptions {
  grouped?: boolean;
  candidateTranslations: boolean;
  translationTargetLanguage: TranslationLanguage;
  translationSecondaryLanguage: TranslationSecondaryLanguage | null | undefined;
  candidateGlossLanguagesEnabled: boolean;
  visibleTranslationLanguages: readonly TranslationLanguageOption[];
  visibleSecondaryLanguages: readonly TranslationSecondaryLanguageOption[];
  android: boolean;
  ios: boolean;
  macos: boolean;
  harmony: boolean;
  linux: boolean;
  /** Windows 的候选窗把每种目标语言的释义画成一行，所以也显示「第二种候选语言」；Server 的翻译线程会问水杉账号，所以翻译服务里也列出「水杉账号」。 */
  windows?: boolean;
  translationAccount: boolean;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  onDeviceMissingLanguages: readonly (readonly [string, string])[];
  openSettings?: () => Promise<void>;
  onError?: (message: string) => void;
  translationProvider: TranslationProvider;
  setTranslationProvider: (provider: TranslationProvider) => void;
}

/** Builds the shared candidate translation controls and preference bindings. */
export function createCandidateTranslationSettings({
  grouped = false,
  candidateTranslations,
  translationTargetLanguage,
  translationSecondaryLanguage,
  candidateGlossLanguagesEnabled,
  visibleTranslationLanguages,
  visibleSecondaryLanguages,
  android,
  ios,
  macos,
  harmony,
  linux,
  windows = false,
  translationAccount,
  onPreferencesChange,
  onDeviceMissingLanguages,
  openSettings,
  onError,
  translationProvider,
  setTranslationProvider,
}: CandidateTranslationSettingsOptions): CandidateTranslationSettingsSectionProps {
  return {
    grouped,
    enabled: candidateTranslations,
    targetLanguage: translationTargetLanguage,
    secondaryLanguage: translationSecondaryLanguage ?? "",
    candidateGlossLanguagesEnabled,
    visibleLanguages: visibleTranslationLanguages,
    visibleSecondaryLanguages,
    showSecondaryLanguage: android || ios || macos || harmony || windows,
    showAccountTranslation: android,
    accountTranslation: translationAccount,
    onEnabledChange: (enabled) => onPreferencesChange({ candidate_translations: enabled }),
    onTargetLanguageChange: (translation_target_language) =>
      onPreferencesChange({ translation_target_language }),
    onSecondaryLanguageChange: (value) =>
      onPreferencesChange({ translation_secondary_language: value === "" ? null : value }),
    onAccountTranslationChange: (enabled) =>
      enabled
        ? setTranslationProvider("account")
        : onPreferencesChange({ translation_account: undefined }),
    onDeviceMissingLanguages: onDeviceMissingLanguages.map(([, label]) => label),
    openSettings,
    onError,
    showTranslationService: !android,
    translationProvider,
    showAccountProvider: macos || linux || windows,
    onTranslationProviderChange: setTranslationProvider,
  };
}
