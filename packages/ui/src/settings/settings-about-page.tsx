import { AboutSettingsSection } from "./about-settings-section";

export type SettingsAboutPageProps = Parameters<typeof AboutSettingsSection>[0];

/** About settings page surface with update, data, diagnostics, and uninstall controls. */
export function SettingsAboutPage(props: SettingsAboutPageProps) {
  return <AboutSettingsSection {...props} />;
}
