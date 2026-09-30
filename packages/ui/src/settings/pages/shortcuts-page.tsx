import { defaultNavigation } from "../navigation-section";
import { useSettingsForm } from "../settings-form-context";
import { ShortcutsSettingsSection } from "../shortcuts-settings-section";
import { createShortcutsSettingsActions } from "../shortcuts-settings-actions";

/** The 快捷键 page of the settings form. */
export function ShortcutSettingsPage() {
  const {
    client,
    linuxPlatform,
    harmonyPlatform,
    mobilePlatform,
    windowsPlatform,
    macosPlatform,
    showModeSwitchShortcuts,
    showPanelShortcuts,
    showNumberRowSelection,
    showRestartInputMethod,
    showInstallInputSource,
    showInputModeHUD,
    showDesktopMaintenanceShortcuts,
    showFullwidthChord,
    fullwidthChord,
    maintenanceChord,
    draft,
    setDraft,
    busy,
    page,
    keybindings,
    inputModeHUD,
  } = useSettingsForm();
  const navigation = draft.navigation ?? defaultNavigation;
  const { onKeybindingsChange, onInputModeHUDChange, onNumberRowSelectionChange } =
    createShortcutsSettingsActions({ setDraft });
  return (
    <ShortcutsSettingsSection
      disabled={busy}
      hidden={page !== "shortcuts"}
      mobile={mobilePlatform}
      keybindings={keybindings}
      onKeybindingsChange={onKeybindingsChange}
      onInputModeHUDChange={onInputModeHUDChange}
      showModeSwitchShortcuts={showModeSwitchShortcuts}
      macos={macosPlatform}
      showInputModeHUD={showInputModeHUD}
      inputModeHUD={inputModeHUD}
      showFullwidthChord={showFullwidthChord}
      fullwidthChord={fullwidthChord}
      windows={windowsPlatform}
      navigation={navigation}
      numberRowSelection={draft.number_row_selection ?? true}
      showNumberRowSelection={showNumberRowSelection}
      onNumberRowSelectionChange={onNumberRowSelectionChange}
      showPanelShortcuts={showPanelShortcuts}
      harmony={harmonyPlatform}
      showDesktopMaintenanceShortcuts={showDesktopMaintenanceShortcuts}
      linux={linuxPlatform}
      maintenanceChord={maintenanceChord}
      showRestartInputMethod={Boolean(showRestartInputMethod)}
      restartInputMethod={client.restartInputMethod}
      installInputSource={showInstallInputSource ? client.installInputSource : undefined}
    />
  );
}
