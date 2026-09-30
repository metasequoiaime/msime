import {
  MobileKeyboardFeedbackSection,
  type MobileKeyboardFeedback,
  type MobileKeyboardFeedbackClient,
} from "./mobile-keyboard-feedback-section";

export interface MobileKeyboardFeedbackSettingsProps {
  mobile: boolean;
  client?: MobileKeyboardFeedbackClient;
  value: MobileKeyboardFeedback | undefined;
  busy: boolean;
  ios: boolean;
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
  onChange,
  onPreview,
}: MobileKeyboardFeedbackSettingsProps) {
  if (!mobile || !client || !value) return null;

  return (
    <MobileKeyboardFeedbackSection
      value={value}
      busy={busy}
      ios={ios}
      canPreview={Boolean(client.preview)}
      onChange={onChange}
      onPreview={onPreview}
    />
  );
}
