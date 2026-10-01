import type { TranslationSettingsContentProps } from "./translation-settings-content";
import {
  createCandidateTranslationSettings,
  type CandidateTranslationSettingsOptions,
} from "./translation-candidate-settings";
import {
  createTranslationProviderSettings,
  type TranslationProviderSettingsOptions,
} from "./translation-provider-settings";

export type TranslationSettingsBindingsOptions = CandidateTranslationSettingsOptions &
  Omit<TranslationProviderSettingsOptions, "grouped">;

/** Composes the shared candidate and provider bindings used by both settings hosts. */
export function createTranslationSettingsBindings({
  android,
  ...options
}: TranslationSettingsBindingsOptions): TranslationSettingsContentProps {
  return {
    candidate: createCandidateTranslationSettings({ android, ...options }),
    providers: android ? undefined : createTranslationProviderSettings(options),
  };
}
