import * as settings from "./settings-style";
import { SettingsLayoutBlock, type SettingsLayoutBlockProps } from "./settings-layout-block";

export type SettingsPhraseFormProps = SettingsLayoutBlockProps;

/** Shared wrapping layout for editable phrase and mention forms. */
export function SettingsPhraseForm(props: SettingsPhraseFormProps) {
  return <SettingsLayoutBlock baseClass={settings.phraseForm} {...props} />;
}
