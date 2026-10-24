import { NavigationSection } from "../navigation-section";
import {
  effectiveNavigation,
  navigationWithDefaults,
  pagingPreferencesPatch,
  restorePagingChange,
  type CandidatePagingChange,
} from "../candidate-paging-preferences";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { useSettingsForm } from "../settings-form-context";
import { ShortcutsSettingsSection } from "../shortcuts-settings-section";
import { createShortcutsSettingsActions } from "../shortcuts-settings-actions";
import { offersJapaneseToggle, offersTrilingualCycle } from "../input-scheme-options";
import { ToggleHotkeysSection } from "../toggle-hotkeys-section";

/** The 快捷键 page of the settings form. */
export function ShortcutSettingsPage() {
  const {
    harmonyPlatform,
    mobilePlatform,
    windowsPlatform,
    macosPlatform,
    linuxPlatform,
    iosPlatform,
    host,
    showModeSwitchShortcuts,
    showPanelShortcuts,
    showNumberRowSelection,
    showFullwidthChord,
    showTabPagePagingSwitches,
    fullwidthChord,
    draft,
    setDraft,
    busy,
    page,
    keybindings,
    wordCharacter,
    client,
  } = useSettingsForm();
  const navigation = navigationWithDefaults(draft.navigation);
  const { onKeybindingsChange, onNumberRowSelectionChange } = createShortcutsSettingsActions({
    setDraft,
  });
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const harmonyPhone = harmonyPlatform && mobilePlatform;
  const onPagingChange = (next: CandidatePagingChange) =>
    onPreferencesChange(pagingPreferencesPatch(wordCharacter, next));
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
      capsLockLanguageSwitch={host?.caps_lock_language_switch ?? false}
      japaneseToggle={offersJapaneseToggle(host)}
      trilingualCycle={offersTrilingualCycle(host)}
      // 速查列出实际生效的翻页键：宿主把 Tab 和 PageUp/PageDown 固定为开时，不管文档里存的值。
      navigation={effectiveNavigation(navigation, showTabPagePagingSwitches)}
      // iOS 键盘扩展收不到实体键，以词定字靠长按候选，没有可列的键。
      wordCharacter={iosPlatform ? undefined : wordCharacter}
      numberRowSelection={draft.number_row_selection ?? true}
      showNumberRowSelection={showNumberRowSelection}
      onNumberRowSelectionChange={onNumberRowSelectionChange}
      showPanelShortcuts={showPanelShortcuts}
      harmony={harmonyPlatform}
      toggleHotkeys={
        (host?.toggle_hotkey_chords ?? false) &&
        !harmonyPhone && (
          <ToggleHotkeysSection
            keybindings={keybindings}
            check={client.checkToggleHotkey}
            voiceCtrlF9={draft.voice_input?.hotkey_ctrl_f9 ?? true}
            onChange={onKeybindingsChange}
            disabled={busy}
          />
        )
      }
      paging={
        harmonyPhone && (
          <NavigationSection
            navigation={navigation}
            wordCharacter={wordCharacter}
            linux={false}
            harmonyPhone
            tabPageSwitches={showTabPagePagingSwitches}
            onChange={onPagingChange}
          />
        )
      }
      // 默认翻页键里有 - / =，以词定字正占着这一对时照勾选时的规矩让出来。
      onRestorePaging={
        harmonyPhone ? () => onPagingChange(restorePagingChange(wordCharacter)) : undefined
      }
    />
  );
}
