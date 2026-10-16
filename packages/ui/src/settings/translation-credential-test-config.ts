import type {
  CustomTranslationCredentialPreferences,
  NiuTransCredentialPreferences,
  TencentTranslationCredentialPreferences,
} from "./translation-credential-preferences";

/** Configuration sent to the NiuTrans service credential check. */
export function niutransCredentialTestConfig(
  preferences: NiuTransCredentialPreferences,
): Record<string, string> {
  return {
    app_id: preferences.app_id,
    apikey: preferences.apikey,
  };
}

/** Configuration sent to the Tencent translation credential check. */
export function tencentTranslationCredentialTestConfig(
  preferences: TencentTranslationCredentialPreferences,
): Record<string, string> {
  return {
    secret_id: preferences.secret_id,
    secret_key: preferences.secret_key,
    region: preferences.region,
  };
}

/** Configuration sent to a custom translation credential check. */
export function customTranslationCredentialTestConfig(
  preferences: CustomTranslationCredentialPreferences,
): Record<string, string> {
  return {
    endpoint: preferences.endpoint,
    api_key: preferences.api_key,
  };
}

/** Whether NiuTrans can run its credential check from the current settings. */
export function niutransCredentialTestDisabled(
  available: boolean,
  preferences: NiuTransCredentialPreferences,
): boolean {
  return !available || !preferences.app_id.trim() || !preferences.apikey.trim();
}

/** Whether Tencent translation can run its credential check. */
export function tencentTranslationCredentialTestDisabled(
  available: boolean,
  credentialIssue: string,
): boolean {
  return !available || Boolean(credentialIssue);
}

/** Whether custom translation can run its credential check. */
export function customTranslationCredentialTestDisabled(
  available: boolean,
  endpointIssue: string,
): boolean {
  return !available || Boolean(endpointIssue);
}
