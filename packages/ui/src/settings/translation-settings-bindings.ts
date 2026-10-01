import type { SettingsClient } from "../index";
import type { CustomTranslationsSectionProps } from "./custom-translations-section";
import type { TranslationSettingsContentProps } from "./translation-settings-content";
import {
  createCandidateTranslationSettings,
  type CandidateTranslationSettingsOptions,
} from "./translation-candidate-settings";
import {
  createTranslationProviderSettings,
  type TranslationProviderSettingsOptions,
} from "./translation-provider-settings";
import type { SettingsSaveState } from "./use-settings-persistence";

/** What the custom candidate glosses editor is bound to; it belongs with 显示英文释义 rather than with any translation service. */
export interface CustomGlossesSettingsOptions {
  client: Pick<SettingsClient, "customTranslations">;
  mobile: boolean;
  customTranslationsText: string;
  customTranslationsPlaceholder: string;
  customTranslationsNotice: string;
  customTranslationsSummary: string;
  customTranslationsSaveState: SettingsSaveState;
  customTranslationsSaveError: string;
  onCustomTranslationsChange: (value: string) => void;
  onFlushCustomTranslations: () => void;
}

export type TranslationSettingsBindingsOptions = CandidateTranslationSettingsOptions &
  Omit<TranslationProviderSettingsOptions, "grouped" | "client"> &
  Omit<CustomGlossesSettingsOptions, "client"> & {
    client: TranslationProviderSettingsOptions["client"] & CustomGlossesSettingsOptions["client"];
  };

export interface TranslationSettingsBindings extends TranslationSettingsContentProps {
  /** The custom candidate glosses editor, absent where the host has no route for the overlay. Hosts place it themselves, beside 显示英文释义. */
  customGlosses?: CustomTranslationsSectionProps;
}

/** Composes the shared candidate, provider, and custom gloss bindings used by both settings hosts. */
export function createTranslationSettingsBindings({
  android,
  client,
  mobile,
  customTranslationsText,
  customTranslationsPlaceholder,
  customTranslationsNotice,
  customTranslationsSummary,
  customTranslationsSaveState,
  customTranslationsSaveError,
  onCustomTranslationsChange,
  onFlushCustomTranslations,
  ...options
}: TranslationSettingsBindingsOptions): TranslationSettingsBindings {
  return {
    candidate: createCandidateTranslationSettings({ android, ...options }),
    providers: android ? undefined : createTranslationProviderSettings({ client, ...options }),
    // Android is left out until its host is confirmed to read the overlay; it was hidden there with the provider groups it used to sit in.
    customGlosses:
      !android && client.customTranslations
        ? {
            mobile,
            value: customTranslationsText,
            placeholder: customTranslationsPlaceholder,
            notice: customTranslationsNotice,
            summary: customTranslationsSummary,
            saveState: customTranslationsSaveState,
            saveError: customTranslationsSaveError,
            onChange: onCustomTranslationsChange,
            onFlush: onFlushCustomTranslations,
          }
        : undefined,
  };
}
