import {
  CandidateTranslationSettingsSection,
  type CandidateTranslationSettingsSectionProps,
} from "./candidate-translation-settings-section";
import {
  TranslationProviderSettingsSection,
  type TranslationProviderSettingsSectionProps,
} from "./translation-provider-settings-section";

export interface TranslationSettingsContentProps {
  candidate: CandidateTranslationSettingsSectionProps;
  providers?: TranslationProviderSettingsSectionProps;
}

/** Shared composition for candidate translation choices and optional provider settings. */
export function TranslationSettingsContent({
  candidate,
  providers,
}: TranslationSettingsContentProps) {
  return (
    <>
      <CandidateTranslationSettingsSection {...candidate} />
      {providers && <TranslationProviderSettingsSection {...providers} />}
    </>
  );
}
