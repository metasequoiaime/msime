import { FeedbackPageSection, type FeedbackPageSectionProps } from "./feedback-page-section";

export type SettingsFeedbackPageProps = FeedbackPageSectionProps;

/** Feedback settings page surface used by the shared settings form. */
export function SettingsFeedbackPage(props: SettingsFeedbackPageProps) {
  return <FeedbackPageSection {...props} />;
}
