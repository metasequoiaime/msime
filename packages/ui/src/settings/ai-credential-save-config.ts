import type { AiCredentialPreferences } from "./ai-credential-preferences";

/** Builds the Linux provider credential payload while preserving stored tokens on blank input. */
export function aiCredentialSaveConfig(
  ai: AiCredentialPreferences,
  token: string,
): AiCredentialPreferences & { token?: string } {
  const { provider, endpoint, model } = ai;
  return {
    provider,
    endpoint,
    model,
    ...(token.trim() ? { token } : {}),
  };
}
