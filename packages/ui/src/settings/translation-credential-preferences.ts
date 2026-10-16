import type { Preferences } from "../core/preferences-model";

/** 自定义翻译服务的共享偏好模型。 */
export type CustomTranslationPreferences = NonNullable<Preferences["custom_translation"]>;

/** 腾讯云翻译服务的共享偏好模型。 */
export type TencentTranslationPreferences = NonNullable<Preferences["tencent_tmt"]>;

/** 小牛翻译服务的共享偏好模型。 */
export type NiuTransPreferences = NonNullable<Preferences["niutrans"]>;

/** 小牛凭据测试所需的最小偏好投影。 */
export type NiuTransCredentialPreferences = Pick<NiuTransPreferences, "app_id" | "apikey">;

/** 腾讯云翻译凭据测试所需的最小偏好投影。 */
export type TencentTranslationCredentialPreferences = Pick<
  TencentTranslationPreferences,
  "secret_id" | "secret_key" | "region"
>;

/** 自定义翻译凭据测试所需的最小偏好投影。 */
export type CustomTranslationCredentialPreferences = Pick<
  CustomTranslationPreferences,
  "endpoint" | "api_key"
>;
