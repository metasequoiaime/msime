import type { ReactNode } from "react";
import type { Preferences } from "../core/preferences-model";
import { GroupList } from "../core/platform-controls";
import { CustomTranslationSection } from "./custom-translation-section";
import type { CustomTranslationPreferences } from "./translation-credential-preferences";
import { customTranslationKeyIssue, translationEndpointIssue } from "./translation-validation";

export interface CustomTranslationSettingsSectionProps {
  grouped?: boolean;
  customTranslation: CustomTranslationPreferences;
  candidateTranslations: boolean;
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  credentialTest?: ReactNode;
}

/** 共享的自定义翻译接口设置；是否外包一层分组由宿主决定。 */
export function CustomTranslationSettingsSection({
  grouped = false,
  customTranslation,
  candidateTranslations,
  onPreferencesChange,
  credentialTest,
}: CustomTranslationSettingsSectionProps) {
  const content = (
    <CustomTranslationSection
      available={candidateTranslations}
      endpoint={customTranslation.endpoint}
      apiKey={customTranslation.api_key}
      endpointIssue={translationEndpointIssue(customTranslation.endpoint)}
      apiKeyIssue={customTranslationKeyIssue(customTranslation.endpoint, customTranslation.api_key)}
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
  );

  return grouped ? <GroupList title="自定义服务">{content}</GroupList> : content;
}
