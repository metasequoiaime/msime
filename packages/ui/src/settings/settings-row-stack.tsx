import * as settings from "./settings-style";
import { SettingsLayoutBlock, type SettingsLayoutBlockProps } from "./settings-layout-block";

export type SettingsRowStackProps = SettingsLayoutBlockProps;

/** Shared stacked layout for related rows inside a settings group. */
export function SettingsRowStack(props: SettingsRowStackProps) {
  return <SettingsLayoutBlock baseClass={settings.rowStack} {...props} />;
}
