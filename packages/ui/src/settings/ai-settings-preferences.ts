import type { AiAssistantPreferences, ProviderCredentialStatus } from "../index";
import { defaultAiAssistant } from "./ai-assistant-defaults";

export interface AiSettingsPreferences {
  ai: AiAssistantPreferences;
  storedAiCredential: ProviderCredentialStatus["ai"][number] | undefined;
}

/** Combines the AI draft defaults with the stored provider entry for the active provider. */
export function aiSettingsPreferences(
  ai?: AiAssistantPreferences,
  providerCredentials?: ProviderCredentialStatus,
): AiSettingsPreferences {
  const preferences = ai ?? defaultAiAssistant;
  return {
    ai: preferences,
    storedAiCredential: providerCredentials?.ai.find(
      (entry) => entry.provider === preferences.provider,
    ),
  };
}
