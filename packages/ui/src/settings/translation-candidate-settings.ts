import type { Preferences } from "../core/preferences-model";
import type { CandidateTranslationSettingsSectionProps } from "./candidate-translation-settings-section";
import type {
  TranslationLanguage,
  TranslationLanguageOption,
  TranslationSecondaryLanguage,
  TranslationSecondaryLanguageOption,
} from "./translation-language-helpers";
import {
  translationOfflineFallback,
  type TranslationProvider,
} from "./translation-service-selector-section";

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
  /** 触屏键盘（iOS、Android、HarmonyOS 手机）只显示一种候选翻译：第二种语言让每个候选下多一行，候选栏太高，所以不给「第二种候选语言」。 */
  mobile?: boolean;
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
  macos,
  harmony,
  linux,
  windows = false,
  mobile = false,
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
    showSecondaryLanguage: !mobile && (macos || harmony || windows),
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
    // 不选在线服务时宿主仍会出的释义：macOS 还有系统本机翻译，叫「离线翻译」；Windows、Linux、HarmonyOS 只有离线词库，叫「不使用在线服务」；Android、iOS 沿用「关闭」，等各自核对过再改。
    offlineFallback: translationOfflineFallback({ macos, windows, linux, harmony }),
    onTranslationProviderChange: setTranslationProvider,
  };
}
