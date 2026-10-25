import {
  MobileKeyboardFeedbackSection,
  type MobileKeySoundPackChoice,
} from "./mobile-keyboard-feedback-section";
import type {
  MobileKeyboardFeedback,
  MobileKeyboardFeedbackClient,
} from "./mobile-keyboard-feedback";

export interface MobileKeyboardFeedbackSettingsProps {
  mobile: boolean;
  client?: MobileKeyboardFeedbackClient;
  value: MobileKeyboardFeedback | undefined;
  busy: boolean;
  ios: boolean;
  /** 见 `MobileKeyboardFeedbackSectionProps.showEnglishSuggestions`。 */
  showEnglishSuggestions?: boolean;
  /** 见 `MobileKeyboardFeedbackSectionProps.keySoundPack`。 */
  keySoundPack?: MobileKeySoundPackChoice;
  onChange: (value: MobileKeyboardFeedback) => void;
  onPreview: () => void;
}

/** Shared host binding for the optional mobile keyboard feedback settings group. */
export function MobileKeyboardFeedbackSettings({
  mobile,
  client,
  value,
  busy,
  ios,
  showEnglishSuggestions,
  keySoundPack,
  onChange,
  onPreview,
}: MobileKeyboardFeedbackSettingsProps) {
  if (!mobile || !client || !value) return null;

  return (
    <MobileKeyboardFeedbackSection
      value={value}
      busy={busy}
      ios={ios}
      showEnglishSuggestions={showEnglishSuggestions}
      canPreview={Boolean(client.preview)}
      keySoundPack={keySoundPack}
      onChange={onChange}
      onPreview={onPreview}
    />
  );
}
