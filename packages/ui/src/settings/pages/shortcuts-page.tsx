import { NavigationSection, defaultNavigation } from "../navigation-section";
import type { NavigationPreferences, WordCharacterPreferences } from "../word-character-section";
import { createSettingsDraftActions } from "../settings-draft-actions";
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
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const harmonyPhone = harmonyPlatform && mobilePlatform;
  const onPagingChange = (next: {
    navigation: NavigationPreferences;
    wordCharacter: WordCharacterPreferences;
  }) =>
    onPreferencesChange({
      // 只有占用了「以词定字」按键的翻页键才会改动它；否则未设置的值保持未设置。
      ...(next.wordCharacter !== wordCharacter ? { word_character: next.wordCharacter } : {}),
      navigation: next.navigation,
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
      paging={
        harmonyPhone && (
          <NavigationSection
            navigation={navigation}
            wordCharacter={wordCharacter}
            linux={false}
            harmonyPhone
            onChange={onPagingChange}
          />
        )
      }
      // 默认翻页键里有 - / =，以词定字正占着这一对时照勾选时的规矩让出来。
      onRestorePaging={
        harmonyPhone
          ? () =>
              onPagingChange({
                navigation: defaultNavigation,
                wordCharacter:
                  wordCharacter.enabled && defaultNavigation[wordCharacter.keys]
                    ? { ...wordCharacter, enabled: false }
                    : wordCharacter,
              })
          : undefined
      }
    />
  );
}
