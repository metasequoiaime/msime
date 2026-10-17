import type { AiCredentialPreferences } from "./ai-credential-preferences";
import type { AiCredentialSaveInput } from "./ai-credential-model";

/** 构造 Linux provider 凭据保存内容；空白密钥省略，非空值保持原文。 */
export function aiCredentialSaveConfig(
  ai: AiCredentialPreferences,
  token: string,
): AiCredentialSaveInput {
  const { provider, endpoint, model } = ai;
  return {
    provider,
    endpoint,
    model,
    ...(token.trim() ? { token } : {}),
  };
}
