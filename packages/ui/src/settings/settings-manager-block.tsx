import { GroupNotesInline } from "../core/platform-controls";
import * as settings from "./settings-style";
import { SettingsLayoutBlock, type SettingsLayoutBlockProps } from "./settings-layout-block";

export type SettingsManagerBlockProps = SettingsLayoutBlockProps;

/** Shared flex container for settings manager content and results. 里面的说明紧挨着它解释的按钮和结果，留在原处，不收进组名旁的「?」。 */
export function SettingsManagerBlock({ children, ...props }: SettingsManagerBlockProps) {
  return (
    <SettingsLayoutBlock baseClass={settings.managerBlock} {...props}>
      <GroupNotesInline>{children}</GroupNotesInline>
    </SettingsLayoutBlock>
  );
}
