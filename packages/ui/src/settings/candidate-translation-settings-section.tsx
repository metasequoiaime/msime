import { GroupList } from "../core/platform-controls";
import {
  CandidateTranslationOptionsSection,
  type CandidateTranslationOptionsSectionProps,
} from "./candidate-translation-options-section";
import { OnDeviceTranslationNotice } from "./on-device-translation-notice";
import {
  TranslationServiceSelectorSection,
  type TranslationOfflineFallback,
  type TranslationProvider,
} from "./translation-service-selector-section";
import { SettingsGroupBlock } from "./settings-group-block";

export interface CandidateTranslationSettingsSectionProps extends CandidateTranslationOptionsSectionProps {
  grouped?: boolean;
  onDeviceMissingLanguages?: readonly string[];
  openSettings?: () => Promise<void>;
  onError?: (message: string) => void;
  showTranslationService?: boolean;
  translationProvider?: TranslationProvider;
  showAccountProvider?: boolean;
  /** 见 `TranslationServiceSelectorSectionProps.offlineFallback`。 */
  offlineFallback?: TranslationOfflineFallback;
  onTranslationProviderChange?: (provider: TranslationProvider) => void;
}

/** Shared candidate translation options, offline notice, and provider selector binding. */
export function CandidateTranslationSettingsSection({
  grouped = false,
  onDeviceMissingLanguages = [],
  openSettings,
  onError,
  showTranslationService = false,
  translationProvider = "none",
  showAccountProvider = false,
  offlineFallback,
  onTranslationProviderChange,
  ...options
}: CandidateTranslationSettingsSectionProps) {
  const content = (
    <>
      <CandidateTranslationOptionsSection {...options} />
      {onDeviceMissingLanguages.length > 0 &&
        (grouped ? (
          <SettingsGroupBlock>
            <OnDeviceTranslationNotice
              languages={onDeviceMissingLanguages}
              openSettings={openSettings}
              onError={onError}
            />
          </SettingsGroupBlock>
        ) : (
          <OnDeviceTranslationNotice
            languages={onDeviceMissingLanguages}
            openSettings={openSettings}
            onError={onError}
          />
        ))}
      {showTranslationService && onTranslationProviderChange && (
        <TranslationServiceSelectorSection
          grouped={grouped}
          available={options.enabled}
          provider={translationProvider}
          showAccountProvider={showAccountProvider}
          offlineFallback={offlineFallback}
          onChange={onTranslationProviderChange}
        />
      )}
    </>
  );

  return grouped ? <GroupList title="候选词翻译">{content}</GroupList> : content;
}
