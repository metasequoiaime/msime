import type { ReactNode } from "react";
import { SettingsFormFrame, type SettingsFormFrameProps } from "./settings-form-frame";

export interface SettingsFormPagesProps {
  frame: Omit<SettingsFormFrameProps, "children">;
  visual: ReactNode;
  dictionary: ReactNode;
  input: ReactNode;
  utility: ReactNode;
  about: ReactNode;
  interaction: ReactNode;
  voiceAi: ReactNode;
  feedback: ReactNode;
  footer: ReactNode;
}

/** Renders the shared form boundary and its settings page groups in stable order. */
export function SettingsFormPages({
  frame,
  visual,
  dictionary,
  input,
  utility,
  about,
  interaction,
  voiceAi,
  feedback,
  footer,
}: SettingsFormPagesProps) {
  return (
    <SettingsFormFrame {...frame}>
      {visual}
      {dictionary}
      {input}
      {utility}
      {about}
      {interaction}
      {voiceAi}
      {feedback}
      {footer}
    </SettingsFormFrame>
  );
}
