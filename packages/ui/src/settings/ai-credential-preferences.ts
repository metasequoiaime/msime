import type { AiAssistantPreferences } from "./ai-assistant-preferences";

/** AI 凭据配置保存和测试共同使用的服务偏好投影。 */
export type AiCredentialPreferences = Pick<
  AiAssistantPreferences,
  "provider" | "endpoint" | "model"
>;

/** AI 凭据测试启用条件共同使用的最小偏好投影。 */
export type AiCredentialTestPreferences = Pick<AiAssistantPreferences, "enabled" | "model">;
