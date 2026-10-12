import * as settings from "./settings-style";
import { SettingsLayoutBlock, type SettingsLayoutBlockProps } from "./settings-layout-block";

export type SettingsServiceRowProps = SettingsLayoutBlockProps;

/** Shared service action row used by settings controls and status messages. */
export function SettingsServiceRow(props: SettingsServiceRowProps) {
  return <SettingsLayoutBlock baseClass={settings.serviceRow} {...props} />;
}
