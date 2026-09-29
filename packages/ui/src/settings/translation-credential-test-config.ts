import type { Preferences } from "../index";

type NiuTransPreferences = Pick<NonNullable<Preferences["niutrans"]>, "app_id" | "apikey">;
type TencentTranslationPreferences = Pick<
  NonNullable<Preferences["tencent_tmt"]>,
  "secret_id" | "secret_key" | "region"
>;
type CustomTranslationPreferences = Pick<
  NonNullable<Preferences["custom_translation"]>,
  "endpoint" | "api_key"
>;

/** Configuration sent to the NiuTrans service credential check. */
export function niutransCredentialTestConfig(
  preferences: NiuTransPreferences,
): Record<string, string> {
  return {
    app_id: preferences.app_id,
    apikey: preferences.apikey,
  };
}

/** Configuration sent to the Tencent translation credential check. */
export function tencentTranslationCredentialTestConfig(
  preferences: TencentTranslationPreferences,
): Record<string, string> {
  return {
    secret_id: preferences.secret_id,
    secret_key: preferences.secret_key,
    region: preferences.region,
  };
}

/** Configuration sent to a custom translation credential check. */
export function customTranslationCredentialTestConfig(
  preferences: CustomTranslationPreferences,
): Record<string, string> {
  return {
    endpoint: preferences.endpoint,
    api_key: preferences.api_key,
  };
}
