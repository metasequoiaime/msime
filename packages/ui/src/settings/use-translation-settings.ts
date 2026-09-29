import {
  defaultCustomTranslation,
  defaultNiuTrans,
  defaultTencentTranslation,
} from "./translation-defaults";
import {
  mobileTranslationLanguages,
  translationLanguages,
  translationSecondaryLanguages,
  type TranslationLanguage,
} from "./translation-language-helpers";
import type { Preferences } from "../index";

type TranslationSecondaryLanguage = "" | TranslationLanguage;

export interface UseTranslationSettingsOptions {
  preferences?: Preferences;
  mobile: boolean;
  macos: boolean;
  linux: boolean;
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
  const visibleTranslationLanguages = mobile
    ? [...mobileTranslationLanguages]
    : translationLanguages;
  if (
    mobile &&
    translationTargetLanguage === "ru" &&
    !visibleTranslationLanguages.some(([value]) => value === "ru")
  ) {
    visibleTranslationLanguages.push(["ru", "俄语（已保存）"]);
  }
  const visibleSecondaryLanguages = mobile
    ? [
        ["", "不显示第二种语言"] as ["", string],
        ...mobileTranslationLanguages,
        ...(translationSecondaryLanguage === "ru"
          ? [["ru", "俄语（已保存）"] as ["ru", string]]
          : []),
      ]
    : translationSecondaryLanguages;
  const customTranslation = preferences?.custom_translation ?? defaultCustomTranslation;
  const tencentTranslation = preferences?.tencent_tmt ?? defaultTencentTranslation;
  const niutrans = preferences?.niutrans ?? defaultNiuTrans;
  const translationProvider = niutrans.enabled
    ? "niutrans"
    : customTranslation.enabled
      ? "custom"
      : tencentTranslation.enabled
        ? "tencent"
        : (macos || linux) && preferences?.translation_account
          ? "account"
          : "none";
  const onDeviceTranslationInUse =
    macos &&
    candidateTranslations &&
    (translationProvider === "none" ||
      (translationProvider === "tencent" &&
        !(tencentTranslation.secret_id.trim() && tencentTranslation.secret_key.trim())));
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

export type { TranslationLanguage, TranslationSecondaryLanguage };
