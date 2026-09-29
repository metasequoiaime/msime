import { HelpcodeSettingsPage } from "./pages/helpcode-page";
import { ShortcutsSettingsSection } from "./shortcuts-settings-section";
import { UtilitiesSettingsSection } from "./utilities-settings-section";
import { HelpSettingsPage } from "./help-settings-page";

export interface SettingsUtilityPagesProps {
  helpcode: Parameters<typeof HelpcodeSettingsPage>[0];
  shortcuts: Parameters<typeof ShortcutsSettingsSection>[0];
  utilities: Parameters<typeof UtilitiesSettingsSection>[0];
  help: Parameters<typeof HelpSettingsPage>[0];
}

/** Groups the utility-oriented settings pages while preserving each page's fieldset boundary. */
export function SettingsUtilityPages({
  helpcode,
  shortcuts,
  utilities,
  help,
}: SettingsUtilityPagesProps) {
  return (
    <>
      <HelpcodeSettingsPage {...helpcode} />
      <ShortcutsSettingsSection {...shortcuts} />
      <UtilitiesSettingsSection {...utilities} />
      <HelpSettingsPage {...help} />
    </>
  );
}
