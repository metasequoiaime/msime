import { useState } from "react";
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
  const tencentConfigured =
    tencentSecretConfigured(tencentTranslation.secret_id) &&
    tencentSecretConfigured(tencentTranslation.secret_key);
  // 用户这次在下拉框里选了腾讯云。腾讯云默认就是 `enabled: true`，选它不会改文档，只能靠这个窗口内的状态把它和「从没选过」分开。
  const [tencentChosen, setTencentChosen] = useState(false);
  // Mirrors `selected_translation_services` in crates/host-api/src/ffi/providers.rs: Tencent's default `enabled: true` without usable secrets is not a user choice, so it does not shadow the account.
  const storedProvider =
    (macos || linux || windows) &&
    preferences?.translation_account &&
    !niutrans.enabled &&
    !customTranslation.enabled &&
    !(tencentTranslation.enabled && tencentConfigured)
      ? "account"
      : niutrans.enabled
        ? "niutrans"
        : customTranslation.enabled
          ? "custom"
          : tencentTranslation.enabled
            ? "tencent"
            : "none";
  // macOS 上没有可用密钥的腾讯云不会被请求，宿主改用离线翻译（`InputController` 的 `currentOnDeviceGlossRequest` 只看共享查询里有没有服务配置，而 host-api 只在两个密钥都可用时才放进腾讯云），所以新装时默认的 `enabled: true` 显示成「离线翻译」，与实际一致；用户本次选了腾讯云才显示腾讯云，让凭据输入框出现。Windows、Linux 的显示不变：Linux 的密钥在 provider 自己的文件里，偏好里的空密钥不代表没配置。
  const translationProvider =
    macos && storedProvider === "tencent" && !tencentConfigured && !tencentChosen
      ? "none"
      : storedProvider;
  const onDeviceTranslationInUse =
    macos &&
    candidateTranslations &&
    (translationProvider === "none" || (translationProvider === "tencent" && !tencentConfigured));
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
    setTencentChosen(provider === "tencent");
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
