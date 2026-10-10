import type { ReactNode } from "react";
import type { InputModeShortcutPreferences } from "./input-mode-shortcuts-section";
import { InputModeShortcutsSection } from "./input-mode-shortcuts-section";
import { CandidateShortcutsSection } from "./candidate-shortcuts-section";
import { PanelShortcutsSection } from "./panel-shortcuts-section";
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
}

/** 桌面和触屏宿主共用的快捷键页：输入模式切换、候选操作速查、面板快捷键。中英文切换提示在输入页，重启与重新注册输入法在「维护与诊断」页。HarmonyOS 手机上是「外接键盘快捷键」：没有页首说明，「通用」「候选」「翻页」三组加一个可展开的「按键速查」，候选组末尾可以一键恢复默认快捷键。 */
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
  navigation,
  wordCharacter,
  numberRowSelection,
  showNumberRowSelection,
  onNumberRowSelectionChange,
  showPanelShortcuts,
  harmony,
  paging,
}: ShortcutsSettingsSectionProps) {
  const showToast = useToast();
  const harmonyPhone = harmony && mobile;
  const restoreDefaults = () => {
    onKeybindingsChange(defaultKeybindings);
    onNumberRowSelectionChange(defaultNumberRowSelection);
    showToast("已恢复默认快捷键");
  };
  return (
    <SettingsPageFieldset disabled={disabled} hidden={hidden} ariaLabel="快捷键">
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
        harmonyPhone={harmonyPhone}
      />
      <CandidateShortcutsSection
        navigation={navigation}
        wordCharacter={wordCharacter}
        numberRowSelection={numberRowSelection}
        showNumberRowSelection={showNumberRowSelection}
        mobile={mobile}
        onNumberRowSelectionChange={onNumberRowSelectionChange}
        harmonyPhone={harmonyPhone}
        onRestoreDefaults={harmonyPhone ? restoreDefaults : undefined}
        paging={harmonyPhone ? paging : undefined}
      />
      <PanelShortcutsSection visible={showPanelShortcuts} macos={macos} harmony={harmony} />
    </SettingsPageFieldset>
  );
}
