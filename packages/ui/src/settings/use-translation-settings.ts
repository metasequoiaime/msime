import { useState } from "react";
import { aiCredentialOrigin, tencentSecretConfigured } from "./credential-utils";
import { aiProviderUsesToken } from "./ai-provider-options";
import {
  defaultCustomTranslation,
  defaultNiuTrans,
  defaultTencentTranslation,
} from "./translation-defaults";
import { translationLanguageChoices, translationLanguages } from "./translation-language-helpers";
import type { Preferences } from "../core/preferences-model";

export interface UseTranslationSettingsOptions {
  preferences?: Preferences;
  mobile: boolean;
  macos: boolean;
  linux: boolean;
  /** Windows Server 的翻译线程也会把候选发给水杉账号（TranslationWorker 的 account_glosses），所以同样认得「水杉账号」。 */
  windows?: boolean;
  /** HarmonyOS 的腾讯云密钥存在偏好里（它画的是 `TencentTranslationSection`，不是 Linux 的凭据文件表单），没有可用密钥时 host-api 不把腾讯云放进查询，与 macOS、Windows 相同。 */
  harmony?: boolean;
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
  harmony = false,
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
            : preferences?.translation_ai
              ? "ai"
              : "none";
  // 没有可用密钥的腾讯云不会被请求：host-api 的 `selected_translation_services` 只在两个密钥都可用时才把它放进查询，macOS（`InputController` 的 `currentOnDeviceGlossRequest`）、Windows（Server 的 `msime_client_translation_query`）和 HarmonyOS（`TranslationPolicy.provider`）都按查询走。所以新装时默认的 `enabled: true` 在这三处显示成 `none`，与实际一致；用户本次选了腾讯云才显示腾讯云，让凭据输入框出现。Linux 的显示不变：它的密钥在 provider 自己的 tencent-provider.json 里，偏好里的空密钥不代表没配置，host-api 也正靠 `enabled: true` 把查询交给那边。
  const translationProvider =
    (macos || windows || harmony) &&
    storedProvider === "tencent" &&
    !tencentConfigured &&
    !tencentChosen
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
  const aiAssistant = preferences?.ai_assistant;
  // 与 host-api `selected_translation_services` 填充 `ai_assistant` 一栏的条件一致：
  // 服务商在目录内、模型非空、接口地址合规，且按来源键找得到 Token（Ollama 不鉴权除外）。
  const aiOrigin = aiAssistant ? aiCredentialOrigin(aiAssistant.endpoint) : null;
  const aiToken =
    aiAssistant && aiOrigin && aiProviderUsesToken(aiAssistant.provider)
      ? (aiAssistant.tokens?.[aiOrigin] ?? aiAssistant.token ?? "")
      : "";
  const aiTranslationReady = Boolean(
    aiAssistant &&
    aiOrigin &&
    aiAssistant.model.trim() &&
    (aiToken.trim() || !aiProviderUsesToken(aiAssistant.provider)),
  );
  const setTranslationProvider = (
    provider: "none" | "custom" | "tencent" | "niutrans" | "account" | "ai",
  ) => {
    if (!preferences) return;
    setTencentChosen(provider === "tencent");
    onChange({
      ...preferences,
      custom_translation: { ...customTranslation, enabled: provider === "custom" },
      tencent_tmt: { ...tencentTranslation, enabled: provider === "tencent" },
      niutrans: { ...niutrans, enabled: provider === "niutrans" },
      translation_account: provider === "account" ? true : undefined,
      translation_ai: provider === "ai" ? true : undefined,
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
    aiAssistant,
    aiTranslationReady,
    aiToken,
  } as const;
}

export type {
  TranslationLanguage,
  TranslationSecondaryLanguage,
} from "./translation-language-helpers";
