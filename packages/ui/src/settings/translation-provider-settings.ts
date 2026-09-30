import type { Preferences, ProviderCredentialStatus, SettingsClient } from "../index";
import type { useProviderCredentials } from "./use-provider-credentials";
import type { SettingsSaveState } from "./use-settings-persistence";
import {
  customTranslationCredentialTestConfig,
  customTranslationCredentialTestDisabled,
  niutransCredentialTestConfig,
  niutransCredentialTestDisabled,
  tencentTranslationCredentialTestConfig,
  tencentTranslationCredentialTestDisabled,
} from "./translation-credential-test-config";
import type { TranslationProviderSettingsSectionProps } from "./translation-provider-settings-section";
import { type TranslationProvider } from "./translation-service-selector-section";
import { tencentCredentialIssue, translationEndpointIssue } from "./translation-validation";
import { tencentSecretConfigured } from "./credential-utils";

type CustomTranslationPreferences = NonNullable<Preferences["custom_translation"]>;
type NiuTransPreferences = NonNullable<Preferences["niutrans"]>;
type TencentTranslationPreferences = NonNullable<Preferences["tencent_tmt"]>;
type ProviderCredentials = ReturnType<typeof useProviderCredentials>;

export interface TranslationProviderSettingsOptions {
  grouped?: boolean;
  client: Pick<SettingsClient, "providerCredentials" | "customTranslations">;
  candidateTranslations: boolean;
  mobile: boolean;
  linux: boolean;
  windows: boolean;
  macos: boolean;
  customTranslation: CustomTranslationPreferences;
  tencentTranslation: TencentTranslationPreferences;
  niutrans: NiuTransPreferences;
  translationProvider: TranslationProvider;
  providerCredentials: ProviderCredentialStatus | undefined;
  tencentCredentialInput: ProviderCredentials["tencentCredentialInput"];
  updateTencentCredentialInput: ProviderCredentials["updateTencentCredentialInput"];
  providerCredentialBusy: ProviderCredentials["providerCredentialBusy"];
  providerCredentialMessages: ProviderCredentials["providerCredentialMessages"];
  runProviderCredential: ProviderCredentials["runProviderCredential"];
  credentialTestControl: ProviderCredentials["credentialTestControl"];
  customTranslationsText: string;
  customTranslationsPlaceholder: string;
  customTranslationsNotice: string;
  customTranslationsSummary: string;
  customTranslationsSaveState: SettingsSaveState;
  customTranslationsSaveError: string;
  onCustomTranslationsChange: (value: string) => void;
  onFlushCustomTranslations: () => void;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  setTranslationProvider: (provider: TranslationProvider) => void;
  /** The embedded panel only offers the service credential test when Tencent is enabled. */
  serviceCredentialEnabled?: boolean;
}

/** Builds the shared NiuTrans, Tencent, and custom provider bindings for both settings hosts. */
export function createTranslationProviderSettings({
  grouped = false,
  client,
  candidateTranslations,
  mobile,
  linux,
  windows,
  macos,
  customTranslation,
  tencentTranslation,
  niutrans,
  translationProvider,
  providerCredentials,
  tencentCredentialInput,
  updateTencentCredentialInput,
  providerCredentialBusy,
  providerCredentialMessages,
  runProviderCredential,
  credentialTestControl,
  customTranslationsText,
  customTranslationsPlaceholder,
  customTranslationsNotice,
  customTranslationsSummary,
  customTranslationsSaveState,
  customTranslationsSaveError,
  onCustomTranslationsChange,
  onFlushCustomTranslations,
  onPreferencesChange,
  setTranslationProvider,
  serviceCredentialEnabled = true,
}: TranslationProviderSettingsOptions): TranslationProviderSettingsSectionProps {
  const tencentIssue = tencentCredentialIssue(
    tencentTranslation.secret_id,
    tencentTranslation.secret_key,
    tencentTranslation.region,
  );

  return {
    grouped,
    niutrans: {
      enabled: niutrans.enabled,
      available: candidateTranslations,
      appId: niutrans.app_id,
      apiKey: niutrans.apikey,
      onToggle: (enabled) => setTranslationProvider(enabled ? "niutrans" : "none"),
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
      enabled: tencentTranslation.enabled,
      secretId: tencentTranslation.secret_id,
      secretKey: tencentTranslation.secret_key,
      region: tencentTranslation.region,
      credentialIssue: tencentIssue,
      showMissingCredentialsWarning:
        !customTranslation.enabled &&
        !tencentSecretConfigured(tencentTranslation.secret_id) &&
        !tencentSecretConfigured(tencentTranslation.secret_key),
      status: providerCredentials
        ? {
            tencent: providerCredentials.tencent,
            tencentInvalid: providerCredentials.tencentInvalid,
          }
        : undefined,
      input: tencentCredentialInput,
      busy: providerCredentialBusy === "tencent",
      message: providerCredentialMessages.tencent,
      onToggle: (enabled) =>
        onPreferencesChange({
          tencent_tmt: { ...tencentTranslation, enabled },
          // Turning on a service of the user's own ends the account choice, so the account never keeps receiving candidates behind a visible selection.
          ...(enabled ? { translation_account: undefined } : {}),
        }),
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
      mobile,
      customTranslationsAvailable: Boolean(client.customTranslations),
      customTranslationsText,
      customTranslationsPlaceholder,
      customTranslationsNotice,
      customTranslationsSummary,
      customTranslationsSaveState,
      customTranslationsSaveError,
      onCustomTranslationsChange,
      onFlushCustomTranslations,
      customTranslation,
      candidateTranslations,
      onPreferencesChange,
      credentialTest: credentialTestControl(
        "translation.custom",
        "测试自定义翻译配置",
        customTranslationCredentialTestConfig(customTranslation),
        customTranslationCredentialTestDisabled(
          candidateTranslations,
          translationEndpointIssue(customTranslation.endpoint),
        ),
      ),
    },
  };
}
