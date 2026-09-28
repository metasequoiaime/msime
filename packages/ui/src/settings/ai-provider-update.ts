import { AI_PROVIDER_OPTIONS } from "./ai-provider-options";
import type { AiAssistantPreferences } from "../index";

export type AiProviderPreset = { id: string; endpoint: string; model: string };
export type AiProviderPreferences = { provider: string; endpoint: string; model: string };

export function updateAiProvider<T extends AiProviderPreferences>(
  provider: string,
  current: T,
  options: readonly AiProviderPreset[],
): Partial<T> {
  const next = options.find((option) => option.id === provider) ?? options[options.length - 1];
  const previous = options.find((option) => option.id === current.provider);
  return {
    provider: next.id,
    endpoint:
      !current.endpoint.trim() || current.endpoint === previous?.endpoint
        ? next.endpoint
        : current.endpoint,
    model: !current.model.trim() || current.model === previous?.model ? next.model : current.model,
  } as Partial<T>;
}

/** Apply a catalog preset while preserving a deliberately custom endpoint or model. */
export function aiProviderUpdate(
  provider: string,
  current: AiAssistantPreferences,
): Partial<AiAssistantPreferences> {
  return updateAiProvider(provider, current, AI_PROVIDER_OPTIONS);
}
