import { SettingsAboutPage, type SettingsAboutPageProps } from "./settings-about-page";
import {
  SettingsDictionaryPage,
  type SettingsDictionaryPageProps,
} from "./settings-dictionary-page";
import { SettingsFeedbackPage, type SettingsFeedbackPageProps } from "./settings-feedback-page";
import { SettingsFormPages, type SettingsFormPagesProps } from "./settings-form-pages";
import { SettingsInputPage, type SettingsInputPageProps } from "./settings-input-page";
import {
  SettingsInteractionPages,
  type SettingsInteractionPagesProps,
} from "./settings-interaction-pages";
import { SettingsPageIntro, type SettingsPageIntroProps } from "./settings-page-intro";
import { SettingsUtilityPages, type SettingsUtilityPagesProps } from "./settings-utility-pages";
import { SettingsVisualPages, type SettingsVisualPagesProps } from "./settings-visual-pages";
import { SettingsVoiceAiPages, type SettingsVoiceAiPagesProps } from "./settings-voice-ai-pages";
import { SettingsFormFooter, type SettingsFormFooterProps } from "./settings-form-footer";

export interface SettingsPageContentForm {
  frame: SettingsFormPagesProps["frame"];
  visual: SettingsVisualPagesProps;
  dictionary: SettingsDictionaryPageProps;
  input: SettingsInputPageProps;
  utility: SettingsUtilityPagesProps;
  about: SettingsAboutPageProps;
  interaction: SettingsInteractionPagesProps;
  voiceAi: SettingsVoiceAiPagesProps;
  feedback: SettingsFeedbackPageProps;
  footer: SettingsFormFooterProps;
}

export interface SettingsPageContentProps {
  intro: Pick<
    SettingsPageIntroProps,
    "page" | "mobile" | "availablePages" | "status" | "standalone"
  >;
  form?: SettingsPageContentForm;
}

/** Composes the settings intro and all form page groups in their stable navigation order. */
export function SettingsPageContent({ intro, form }: SettingsPageContentProps) {
  return (
    <>
      <SettingsPageIntro {...intro} />
      {form && (
        <SettingsFormPages
          frame={form.frame}
          visual={<SettingsVisualPages {...form.visual} />}
          dictionary={<SettingsDictionaryPage {...form.dictionary} />}
          input={<SettingsInputPage {...form.input} />}
          utility={<SettingsUtilityPages {...form.utility} />}
          about={<SettingsAboutPage {...form.about} />}
          interaction={<SettingsInteractionPages {...form.interaction} />}
          voiceAi={<SettingsVoiceAiPages {...form.voiceAi} />}
          feedback={<SettingsFeedbackPage {...form.feedback} />}
          footer={<SettingsFormFooter {...form.footer} />}
        />
      )}
    </>
  );
}
