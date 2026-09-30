import type { ReactNode } from "react";
import type { Preferences } from "../index";
import { GroupList } from "../core/platform-controls";
import { CustomTranslationsSection } from "./custom-translations-section";
import { CustomTranslationSection } from "./custom-translation-section";
import { translationEndpointIssue } from "./translation-validation";
import type { SettingsSaveState } from "./use-settings-persistence";

type CustomTranslationPreferences = NonNullable<Preferences["custom_translation"]>;

export interface CustomTranslationSettingsSectionProps {
  grouped?: boolean;
  mobile: boolean;
  customTranslationsAvailable: boolean;
  customTranslationsText: string;
  customTranslationsPlaceholder: string;
  customTranslationsNotice: string;
  customTranslationsSummary: string;
  customTranslationsSaveState: SettingsSaveState;
  customTranslationsSaveError: string;
  onCustomTranslationsChange: (value: string) => void;
  onFlushCustomTranslations: () => void;
  customTranslation: CustomTranslationPreferences;
  candidateTranslations: boolean;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  credentialTest?: ReactNode;
}

/** Shared custom translation text and endpoint settings; hosts choose whether to add a group wrapper. */
export function CustomTranslationSettingsSection({
  grouped = false,
  mobile,
  customTranslationsAvailable,
  customTranslationsText,
  customTranslationsPlaceholder,
  customTranslationsNotice,
  customTranslationsSummary,
  customTranslationsSaveState,
  customTranslationsSaveError,
  onCustomTranslationsChange,
  onFlushCustomTranslations,
  customTranslation,
  candidateTranslations,
  onPreferencesChange,
  credentialTest,
}: CustomTranslationSettingsSectionProps) {
  const content = (
    <>
      {customTranslationsAvailable && (
        <CustomTranslationsSection
          mobile={mobile}
          value={customTranslationsText}
          placeholder={customTranslationsPlaceholder}
          notice={customTranslationsNotice}
          summary={customTranslationsSummary}
          saveState={customTranslationsSaveState}
          saveError={customTranslationsSaveError}
          onChange={onCustomTranslationsChange}
          onFlush={onFlushCustomTranslations}
        />
      )}
      <CustomTranslationSection
        enabled={customTranslation.enabled}
        available={candidateTranslations}
        endpoint={customTranslation.endpoint}
        apiKey={customTranslation.api_key}
        endpointIssue={translationEndpointIssue(customTranslation.endpoint)}
        onToggle={(enabled) =>
          onPreferencesChange({
            custom_translation: { ...customTranslation, enabled },
            // Turning on a service of the user's own ends the account choice, so the account never keeps receiving candidates behind a visible selection.
            ...(enabled ? { translation_account: undefined } : {}),
          })
        }
        onEndpointChange={(endpoint) =>
          onPreferencesChange({
            custom_translation: { ...customTranslation, endpoint },
          })
        }
        onApiKeyChange={(api_key) =>
          onPreferencesChange({
            custom_translation: { ...customTranslation, api_key },
          })
        }
      >
        {credentialTest}
      </CustomTranslationSection>
    </>
  );

  return grouped ? <GroupList title="自定义服务">{content}</GroupList> : content;
}
