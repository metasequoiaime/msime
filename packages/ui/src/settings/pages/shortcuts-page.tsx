import { defaultNavigation } from "../navigation-section";
import { useSettingsForm } from "../settings-form-context";
import { ShortcutsSettingsSection } from "../shortcuts-settings-section";
import { createShortcutsSettingsActions } from "../shortcuts-settings-actions";

/** The 快捷键 page of the settings form. */
export function ShortcutSettingsPage() {
  const {
    harmonyPlatform,
    mobilePlatform,
    windowsPlatform,
    macosPlatform,
    linuxPlatform,
    iosPlatform,
    showModeSwitchShortcuts,
    showPanelShortcuts,
    showNumberRowSelection,
    showFullwidthChord,
    fullwidthChord,
    draft,
    setDraft,
    busy,
    page,
    keybindings,
    wordCharacter,
  } = useSettingsForm();
  const navigation = draft.navigation ?? defaultNavigation;
  const { onKeybindingsChange, onNumberRowSelectionChange } = createShortcutsSettingsActions({
    setDraft,
  });
  return (
    <ShortcutsSettingsSection
      disabled={busy}
      hidden={page !== "shortcuts"}
      mobile={mobilePlatform}
      keybindings={keybindings}
      onKeybindingsChange={onKeybindingsChange}
      showModeSwitchShortcuts={showModeSwitchShortcuts}
      macos={macosPlatform}
      linux={linuxPlatform}
      showFullwidthChord={showFullwidthChord}
      fullwidthChord={fullwidthChord}
      windows={windowsPlatform}
      navigation={navigation}
      // iOS 键盘扩展收不到实体键，以词定字靠长按候选，没有可列的键。
      wordCharacter={iosPlatform ? undefined : wordCharacter}
      numberRowSelection={draft.number_row_selection ?? true}
      showNumberRowSelection={showNumberRowSelection}
      onNumberRowSelectionChange={onNumberRowSelectionChange}
      showPanelShortcuts={showPanelShortcuts}
      harmony={harmonyPlatform}
    />
  );
}
