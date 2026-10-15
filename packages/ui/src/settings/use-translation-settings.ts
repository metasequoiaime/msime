import { tencentSecretConfigured } from "./credential-utils";
import {
  defaultCustomTranslation,
  defaultNiuTrans,
  defaultTencentTranslation,
} from "./translation-defaults";
import { translationLanguageChoices, translationLanguages } from "./translation-language-helpers";
import type { Preferences } from "../index";

export interface UseTranslationSettingsOptions {
  preferences?: Preferences;
  mobile: boolean;
  macos: boolean;
  linux: boolean;
  /** Windows Server 的翻译线程也会把候选发给水杉账号（TranslationWorker 的 account_glosses），所以同样认得「水杉账号」。 */
  windows?: boolean;
  candidateEnglishGlossAvailable: boolean;
  onDeviceDownloadable: string[];
  onChange: (preferences: Preferences) => void;
}

/** Derives translation provider and language controls from the settings draft. */
export function useTranslationSettings({
  preferences,
  mobile,
  macos,
  linux,
  windows = false,
  candidateEnglishGlossAvailable,
  onDeviceDownloadable,
  onChange,
}: UseTranslationSettingsOptions) {
  const candidateTranslations = preferences?.candidate_translations ?? true;
  const candidateEnglishGloss = preferences?.candidate_english_gloss ?? false;
  const candidateGlossLanguagesEnabled =
    candidateTranslations || (candidateEnglishGlossAvailable && candidateEnglishGloss);
  const translationTargetLanguage = preferences?.translation_target_language ?? "en";
  const translationSecondaryLanguage = preferences?.translation_secondary_language ?? "";
  const { visibleTranslationLanguages, visibleSecondaryLanguages } = translationLanguageChoices(
    mobile,
    translationTargetLanguage,
    translationSecondaryLanguage,
  );
  const customTranslation = preferences?.custom_translation ?? defaultCustomTranslation;
  const tencentTranslation = preferences?.tencent_tmt ?? defaultTencentTranslation;
  const niutrans = preferences?.niutrans ?? defaultNiuTrans;
  // Mirrors `selected_translation_services` in crates/host-api/src/ffi/providers.rs: Tencent's default `enabled: true` without usable secrets is not a user choice, so it does not shadow the account.
  const translationProvider =
    (macos || linux || windows) &&
    preferences?.translation_account &&
    !niutrans.enabled &&
    !customTranslation.enabled &&
    !(
      tencentTranslation.enabled &&
      tencentSecretConfigured(tencentTranslation.secret_id) &&
      tencentSecretConfigured(tencentTranslation.secret_key)
    )
      ? "account"
      : niutrans.enabled
        ? "niutrans"
        : customTranslation.enabled
          ? "custom"
          : tencentTranslation.enabled
            ? "tencent"
            : "none";
  const onDeviceTranslationInUse =
    macos &&
    candidateTranslations &&
    (translationProvider === "none" ||
      (translationProvider === "tencent" &&
        !(
          tencentSecretConfigured(tencentTranslation.secret_id) &&
          tencentSecretConfigured(tencentTranslation.secret_key)
        )));
  const onDeviceMissingLanguages = onDeviceTranslationInUse
    ? translationLanguages.filter(
        ([code]) =>
          (code === translationTargetLanguage || code === translationSecondaryLanguage) &&
          onDeviceDownloadable.includes(code),
      )
    : [];
  const setTranslationProvider = (
    provider: "none" | "custom" | "tencent" | "niutrans" | "account",
  ) => {
    if (!preferences) return;
    onChange({
      ...preferences,
      custom_translation: { ...customTranslation, enabled: provider === "custom" },
      tencent_tmt: { ...tencentTranslation, enabled: provider === "tencent" },
      niutrans: { ...niutrans, enabled: provider === "niutrans" },
      translation_account: provider === "account" ? true : undefined,
    });
  };

  return {
    candidateTranslations,
    candidateGlossLanguagesEnabled,
    translationTargetLanguage,
    translationSecondaryLanguage,
    visibleTranslationLanguages,
    visibleSecondaryLanguages,
    customTranslation,
    tencentTranslation,
    niutrans,
    translationProvider,
    onDeviceMissingLanguages,
    setTranslationProvider,
  } as const;
}

export type {
  TranslationLanguage,
  TranslationSecondaryLanguage,
} from "./translation-language-helpers";
