import { FeedbackSettingsSection } from "./feedback-settings-section";

export type FeedbackPageSectionProps = {
  disabled: boolean;
  hidden: boolean;
} & Parameters<typeof FeedbackSettingsSection>[0];

/** Feedback page fieldset shared by desktop and mobile settings hosts. */
export function FeedbackPageSection({ disabled, hidden, ...props }: FeedbackPageSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="反馈">
      <FeedbackSettingsSection {...props} />
    </fieldset>
  );
}
