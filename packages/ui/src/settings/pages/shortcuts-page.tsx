import { defaultNavigation } from "../navigation-section";
import { useSettingsForm } from "../settings-form-context";
import { ShortcutsSettingsSection } from "../shortcuts-settings-section";

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
  return (
    <ShortcutsSettingsSection
      disabled={busy}
      hidden={page !== "shortcuts"}
      mobile={mobilePlatform}
      keybindings={keybindings}
      onKeybindingsChange={(patch) =>
        setDraft({ ...draft, keybindings: { ...keybindings, ...patch } })
      }
      onInputModeHUDChange={(checked) => setDraft({ ...draft, input_mode_hud: checked })}
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
      onNumberRowSelectionChange={(checked) =>
        setDraft({ ...draft, number_row_selection: checked })
      }
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
