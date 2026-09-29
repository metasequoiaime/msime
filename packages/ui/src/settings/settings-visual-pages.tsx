import { AppearanceSettingsSection } from "./appearance-settings-section";
import { FloatingToolbarSettingsSection } from "./floating-toolbar-settings-section";
import { SkinSettingsSection } from "./skin-settings-section";

export interface SettingsVisualPagesProps {
  appearance: Parameters<typeof AppearanceSettingsSection>[0];
  skin: Parameters<typeof SkinSettingsSection>[0];
  floatingToolbar: Parameters<typeof FloatingToolbarSettingsSection>[0];
}

/** Groups the appearance, skin, and floating toolbar settings pages. */
export function SettingsVisualPages({
  appearance,
  skin,
  floatingToolbar,
}: SettingsVisualPagesProps) {
  return (
    <>
      <AppearanceSettingsSection {...appearance} />
      <SkinSettingsSection {...skin} />
      <FloatingToolbarSettingsSection {...floatingToolbar} />
    </>
  );
}
