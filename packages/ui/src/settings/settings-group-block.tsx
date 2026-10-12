import * as settings from "./settings-style";
import { SettingsLayoutBlock, type SettingsLayoutBlockProps } from "./settings-layout-block";

export type SettingsGroupBlockProps = SettingsLayoutBlockProps;

/** Shared padded container for content that sits inside a settings group. */
export function SettingsGroupBlock(props: SettingsGroupBlockProps) {
  return <SettingsLayoutBlock baseClass={settings.groupBlock} {...props} />;
}
