import type { ProviderCredentialStatus } from "../core/provider-credential-contracts";
import type { SettingsClient } from "../core/settings-client";
import type { Preferences } from "../core/preferences-model";
import type { useProviderCredentials } from "./use-provider-credentials";
import {
  customTranslationCredentialTestConfig,
  customTranslationCredentialTestDisabled,
  niutransCredentialTestConfig,
  niutransCredentialTestDisabled,
  tencentTranslationCredentialTestConfig,
  tencentTranslationCredentialTestDisabled,
} from "./translation-credential-test-config";
import type { TranslationProviderSettingsSectionProps } from "./translation-provider-settings-section";
import {
  translationOfflineFallback,
  type TranslationProvider,
} from "./translation-service-selector-section";
import type {
  CustomTranslationPreferences,
  NiuTransPreferences,
  TencentTranslationPreferences,
} from "./translation-credential-preferences";
import type { AiAssistantPreferences } from "./ai-assistant-preferences";
import { aiServiceCredentialTestConfig } from "./ai-credential-test-config";
import { aiProviderOption } from "./ai-provider-options";
import {
  customTranslationKeyIssue,
  tencentCredentialIssue,
  translationEndpointIssue,
} from "./translation-validation";
import { tencentSecretConfigured } from "./credential-utils";

type ProviderCredentials = ReturnType<typeof useProviderCredentials>;

export interface TranslationProviderSettingsOptions {
  grouped?: boolean;
  client: Pick<SettingsClient, "providerCredentials">;
  candidateTranslations: boolean;
  linux: boolean;
  windows: boolean;
  macos: boolean;
  /** 只决定缺腾讯云凭据时警告的说法（`translationOfflineFallback`）。 */
  harmony?: boolean;
  customTranslation: CustomTranslationPreferences;
  tencentTranslation: TencentTranslationPreferences;
  niutrans: NiuTransPreferences;
  translationProvider: TranslationProvider;
  /** 「AI 与云联想」页的偏好投影，选「AI 辅助」时用于显示当前服务商与模型。 */
  aiAssistant?: AiAssistantPreferences;
  /** AI 翻译凭据是否就绪（接口地址、Token、模型齐全）；与 host-api 的填充条件一致。 */
  aiReady?: boolean;
  /** 当前接口地址来源键下保存的 Token，仅供凭据测试发送。 */
  aiToken?: string;
  providerCredentials: ProviderCredentialStatus | undefined;
  tencentCredentialInput: ProviderCredentials["tencentCredentialInput"];
  updateTencentCredentialInput: ProviderCredentials["updateTencentCredentialInput"];
  providerCredentialBusy: ProviderCredentials["providerCredentialBusy"];
  providerCredentialMessages: ProviderCredentials["providerCredentialMessages"];
  runProviderCredential: ProviderCredentials["runProviderCredential"];
  credentialTestControl: ProviderCredentials["credentialTestControl"];
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  /** The embedded panel only offers the service credential test when Tencent is enabled. */
  serviceCredentialEnabled?: boolean;
}

/** Builds the shared NiuTrans, Tencent, and custom provider bindings for both settings hosts. */
export function createTranslationProviderSettings({
  grouped = false,
  client,
  candidateTranslations,
  linux,
  windows,
  macos,
  harmony = false,
  customTranslation,
  tencentTranslation,
  niutrans,
  translationProvider,
  aiAssistant,
  aiReady = false,
  aiToken = "",
  providerCredentials,
  tencentCredentialInput,
  updateTencentCredentialInput,
  providerCredentialBusy,
  providerCredentialMessages,
  runProviderCredential,
  credentialTestControl,
  onPreferencesChange,
  serviceCredentialEnabled = true,
}: TranslationProviderSettingsOptions): TranslationProviderSettingsSectionProps {
  const tencentIssue = tencentCredentialIssue(
    tencentTranslation.secret_id,
    tencentTranslation.secret_key,
    tencentTranslation.region,
  );

  return {
    grouped,
    provider: translationProvider,
    ai: {
      summary: aiAssistant
        ? `${aiProviderOption(aiAssistant.provider)?.title ?? aiAssistant.provider} · ${aiAssistant.model || "未配置"}`
        : "",
      ready: aiReady,
      credentialTest:
        translationProvider === "ai" &&
        credentialTestControl(
          "translation.ai",
          "测试 AI 辅助配置",
          aiAssistant ? aiServiceCredentialTestConfig(aiAssistant, aiToken) : {},
          !candidateTranslations || !aiReady,
        ),
    },
    niutrans: {
      available: candidateTranslations,
      appId: niutrans.app_id,
      apiKey: niutrans.apikey,
      onAppIdChange: (app_id) => onPreferencesChange({ niutrans: { ...niutrans, app_id } }),
      onApiKeyChange: (apikey) => onPreferencesChange({ niutrans: { ...niutrans, apikey } }),
      credentialTest: credentialTestControl(
        "translation.niutrans",
        "测试 NiuTrans 配置",
        niutransCredentialTestConfig(niutrans),
        niutransCredentialTestDisabled(candidateTranslations, niutrans),
      ),
    },
    tencent: {
      linux,
      available: candidateTranslations,
      linuxCredentialsAvailable: Boolean(client.providerCredentials),
      secretId: tencentTranslation.secret_id,
      secretKey: tencentTranslation.secret_key,
      region: tencentTranslation.region,
      credentialIssue: tencentIssue,
      showMissingCredentialsWarning:
        !customTranslation.enabled &&
        !tencentSecretConfigured(tencentTranslation.secret_id) &&
        !tencentSecretConfigured(tencentTranslation.secret_key),
      offlineFallback: translationOfflineFallback({ macos, windows, linux, harmony }),
      status: providerCredentials
        ? {
            tencent: providerCredentials.tencent,
            tencentInvalid: providerCredentials.tencentInvalid,
          }
        : undefined,
      input: tencentCredentialInput,
      busy: providerCredentialBusy === "tencent",
      message: providerCredentialMessages.tencent,
      onSecretIdChange: (secret_id) =>
        onPreferencesChange({ tencent_tmt: { ...tencentTranslation, secret_id } }),
      onSecretKeyChange: (secret_key) =>
        onPreferencesChange({ tencent_tmt: { ...tencentTranslation, secret_key } }),
      onRegionChange: (region) =>
        onPreferencesChange({ tencent_tmt: { ...tencentTranslation, region } }),
      onInputChange: (patch) => updateTencentCredentialInput(patch),
      onSave: (credential) =>
        void runProviderCredential(
          "tencent",
          (credentials) => credentials.saveTencent(credential),
          "凭据已保存，provider 服务下次请求时生效。",
        ),
      onClear: () =>
        void runProviderCredential(
          "tencent",
          (credentials) => credentials.clearTencent(),
          "凭据已清除。",
        ),
      linuxCredentialTest:
        translationProvider === "tencent" &&
        credentialTestControl(
          "translation.tencent",
          "测试腾讯云翻译配置",
          {},
          !candidateTranslations,
        ),
      serviceCredentialTest:
        (windows || macos) &&
        serviceCredentialEnabled &&
        credentialTestControl(
          "translation.tencent",
          "测试腾讯云翻译配置",
          tencentTranslationCredentialTestConfig(tencentTranslation),
          tencentTranslationCredentialTestDisabled(candidateTranslations, tencentIssue),
        ),
    },
    custom: {
      customTranslation,
      candidateTranslations,
      onPreferencesChange,
      credentialTest: credentialTestControl(
        "translation.custom",
        "测试自定义翻译配置",
        customTranslationCredentialTestConfig(customTranslation),
        customTranslationCredentialTestDisabled(
          candidateTranslations,
          translationEndpointIssue(customTranslation.endpoint) ||
            customTranslationKeyIssue(customTranslation.endpoint, customTranslation.api_key),
        ),
      ),
    },
  };
}
