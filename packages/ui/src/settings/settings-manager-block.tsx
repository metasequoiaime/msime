import * as settings from "./settings-style";
import { SettingsLayoutBlock, type SettingsLayoutBlockProps } from "./settings-layout-block";

export type SettingsManagerBlockProps = SettingsLayoutBlockProps;

/** Shared flex container for settings manager content and results. */
export function SettingsManagerBlock(props: SettingsManagerBlockProps) {
  return <SettingsLayoutBlock baseClass={settings.managerBlock} {...props} />;
}
