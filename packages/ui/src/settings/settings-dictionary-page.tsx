import { DictionarySettingsPanel } from "./dictionary-settings-panel";

export type SettingsDictionaryPageProps = Parameters<typeof DictionarySettingsPanel>[0];

/** Dictionary settings page surface with local dictionary and phrase management controls. */
export function SettingsDictionaryPage(props: SettingsDictionaryPageProps) {
  return <DictionarySettingsPanel {...props} />;
}
