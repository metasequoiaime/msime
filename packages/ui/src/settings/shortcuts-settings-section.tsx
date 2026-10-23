import type { ReactNode } from "react";
import type { InputModeShortcutPreferences } from "./input-mode-shortcuts-section";
import { InputModeShortcutsSection } from "./input-mode-shortcuts-section";
import {
  CandidateShortcutReference,
  CandidateShortcutsSection,
} from "./candidate-shortcuts-section";
import { GroupList } from "../core/platform-controls";
import { useSettingsTabs } from "./use-settings-tabs";
import { PanelShortcutRow, PanelShortcutsSection } from "./panel-shortcuts-section";
import { ShortcutsIntroSection } from "./shortcuts-intro-section";
import type { NavigationPreferences, WordCharacterPreferences } from "./word-character-section";
import { SettingsPageFieldset } from "./settings-page-fieldset";
import { defaultKeybindings, defaultNumberRowSelection } from "./keybinding-defaults";
import { useToast } from "../core/toast";

export interface ShortcutsSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  mobile: boolean;
  keybindings: InputModeShortcutPreferences;
  onKeybindingsChange: (patch: Partial<InputModeShortcutPreferences>) => void;
  showModeSwitchShortcuts: boolean;
  macos: boolean;
  linux?: boolean;
  showFullwidthChord: boolean;
  fullwidthChord: string;
  windows: boolean;
  /** 显示「单按 Caps Lock 切换中英文」（`HostCapabilities.caps_lock_language_switch`）。 */
  capsLockLanguageSwitch?: boolean;
  /** 显示「Ctrl+Shift+J 在中文和日语之间切换」（`offersJapaneseToggle`）。 */
  japaneseToggle?: boolean;
  /** 显示「中英日循环切换」（`offersTrilingualCycle`）。 */
  trilingualCycle?: boolean;
  navigation: NavigationPreferences;
  /** 以词定字的设置，用来在「候选操作」里列出它占用的键；没有实体键的宿主不传。 */
  wordCharacter?: WordCharacterPreferences;
  numberRowSelection: boolean;
  showNumberRowSelection: boolean;
  onNumberRowSelectionChange: (value: boolean) => void;
  showPanelShortcuts: boolean;
  harmony: boolean;
  /** HarmonyOS 手机的外接键盘翻页键；它们只对外接键盘有意义，所以放在这一页，不放在「候选栏」页。 */
  paging?: ReactNode;
  /** 「切换热键」录键组：宿主声明 `toggle_hotkey_chords` 且提供 `checkToggleHotkey` 时由页面上层传入。 */
  toggleHotkeys?: ReactNode;
  /** 把 `paging` 里的翻页键恢复为默认值；「恢复默认」恢复整页，所以和 `paging` 一起传。 */
  onRestorePaging?: () => void;
}

const shortcutTabs = [
  { id: "settings", title: "按键设置" },
  { id: "reference", title: "按键速查" },
] as const;

/** 桌面在顶部标签中分开按键设置与直接展开的速查；手机保留原有顺序和折叠，鸿蒙手机页末仍可恢复整页默认值。 */
export function ShortcutsSettingsSection({
  disabled,
  hidden,
  mobile,
  keybindings,
  onKeybindingsChange,
  showModeSwitchShortcuts,
  macos,
  linux,
  showFullwidthChord,
  fullwidthChord,
  windows,
  capsLockLanguageSwitch = false,
  japaneseToggle = false,
  trilingualCycle = false,
  navigation,
  wordCharacter,
  numberRowSelection,
  showNumberRowSelection,
  onNumberRowSelectionChange,
  showPanelShortcuts,
  harmony,
  paging,
  toggleHotkeys,
  onRestorePaging,
}: ShortcutsSettingsSectionProps) {
  const showToast = useToast();
  const harmonyPhone = harmony && mobile;
  const { tabList, panel } = useSettingsTabs(shortcutTabs, "按键设置分类", !mobile);
  const restoreDefaults = () => {
    onKeybindingsChange(defaultKeybindings);
    onNumberRowSelectionChange(defaultNumberRowSelection);
    onRestorePaging?.();
    showToast("已恢复默认快捷键");
  };
  return (
    <SettingsPageFieldset disabled={disabled} hidden={hidden} ariaLabel="快捷键">
      {tabList}
      {panel(
        "settings",
        <>
          {!harmonyPhone && <ShortcutsIntroSection mobile={mobile} />}
          <InputModeShortcutsSection
            keybindings={keybindings}
            onChange={onKeybindingsChange}
            showModeSwitchShortcuts={showModeSwitchShortcuts}
            macos={macos}
            linux={linux}
            showFullwidthChord={showFullwidthChord}
            fullwidthChord={fullwidthChord}
            windows={windows}
            capsLockLanguageSwitch={capsLockLanguageSwitch}
            japaneseToggle={japaneseToggle}
            trilingualCycle={trilingualCycle}
            harmonyPhone={harmonyPhone}
          />
          {toggleHotkeys}
          <CandidateShortcutsSection
            navigation={navigation}
            wordCharacter={wordCharacter}
            numberRowSelection={numberRowSelection}
            showNumberRowSelection={showNumberRowSelection}
            mobile={mobile}
            onNumberRowSelectionChange={onNumberRowSelectionChange}
            harmonyPhone={harmonyPhone}
            showReference={mobile}
            onRestoreDefaults={harmonyPhone ? restoreDefaults : undefined}
            paging={harmonyPhone ? paging : undefined}
            extraReference={
              mobile &&
              !harmonyPhone &&
              showPanelShortcuts && <PanelShortcutRow macos={macos} harmony={harmony} />
            }
          />
        </>,
      )}
      {!mobile &&
        panel(
          "reference",
          <GroupList title="按键速查">
            <CandidateShortcutReference
              navigation={navigation}
              wordCharacter={wordCharacter}
              numberRowSelection={numberRowSelection}
              mobile={mobile}
            />
            {showPanelShortcuts && <PanelShortcutRow macos={macos} harmony={harmony} />}
          </GroupList>,
        )}
      {/* 鸿蒙手机照旧单列一组；其他宿主把这一行并进「按键速查」。 */}
      <PanelShortcutsSection
        visible={harmonyPhone && showPanelShortcuts}
        macos={macos}
        harmony={harmony}
      />
    </SettingsPageFieldset>
  );
}
