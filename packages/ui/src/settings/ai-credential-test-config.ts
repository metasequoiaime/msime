import type { AiAssistantPreferences } from "../index";

type AiCredentialPreferences = Pick<AiAssistantPreferences, "provider" | "endpoint" | "model">;

/** Configuration sent to the Linux provider when checking the AI settings. */
export function aiProviderCredentialTestConfig(
  ai: AiCredentialPreferences,
): Record<string, unknown> {
  return {
    provider: ai.provider,
    endpoint: ai.endpoint,
    model: ai.model,
  };
}

/** Configuration sent directly to a remote AI service for a credential check. */
export function aiServiceCredentialTestConfig(
  ai: AiCredentialPreferences,
  token: string,
): Record<string, unknown> {
  return {
    ...aiProviderCredentialTestConfig(ai),
    token,
  };
}

/** Whether the shared AI settings are incomplete for a provider test. */
export function aiCredentialTestDisabled(
  ai: Pick<AiAssistantPreferences, "enabled" | "model">,
  origin: string | null,
): boolean {
  return !ai.enabled || !origin || !ai.model.trim();
}

/** Whether the shared AI settings are incomplete for a remote service test. */
export function aiServiceCredentialTestDisabled(
  ai: Pick<AiAssistantPreferences, "enabled" | "model">,
  origin: string | null,
  token: string,
): boolean {
  return aiCredentialTestDisabled(ai, origin) || !token.trim();
}
