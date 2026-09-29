import { InputSettingsPanel } from "./input-settings-panel";

export type SettingsInputPageProps = Parameters<typeof InputSettingsPanel>[0];

/** Input settings page surface with scheme, translation, and candidate controls. */
export function SettingsInputPage(props: SettingsInputPageProps) {
  return <InputSettingsPanel {...props} />;
}
