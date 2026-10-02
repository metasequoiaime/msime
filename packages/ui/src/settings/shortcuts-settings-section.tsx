import type { InputModeShortcutPreferences } from "./input-mode-shortcuts-section";
import { InputModeShortcutsSection } from "./input-mode-shortcuts-section";
import { CandidateShortcutsSection } from "./candidate-shortcuts-section";
import { PanelShortcutsSection } from "./panel-shortcuts-section";
import { ShortcutsIntroSection } from "./shortcuts-intro-section";
import * as settings from "./settings-style";
import type { NavigationPreferences, WordCharacterPreferences } from "./word-character-section";

export interface ShortcutsSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  mobile: boolean;
  keybindings: InputModeShortcutPreferences;
  onKeybindingsChange: (patch: Partial<InputModeShortcutPreferences>) => void;
  showModeSwitchShortcuts: boolean;
  macos: boolean;
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
}

/** 桌面和触屏宿主共用的快捷键页：输入模式切换、候选操作速查、面板快捷键。中英文切换提示在输入页，重启与重新注册输入法在「维护与诊断」页。 */
export function ShortcutsSettingsSection({
  disabled,
  hidden,
  mobile,
  keybindings,
  onKeybindingsChange,
  showModeSwitchShortcuts,
  macos,
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
}: ShortcutsSettingsSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="快捷键">
      <div className={settings.groups}>
        <ShortcutsIntroSection mobile={mobile} />
        <InputModeShortcutsSection
          keybindings={keybindings}
          onChange={onKeybindingsChange}
          showModeSwitchShortcuts={showModeSwitchShortcuts}
          macos={macos}
          showFullwidthChord={showFullwidthChord}
          fullwidthChord={fullwidthChord}
          windows={windows}
        />
        <CandidateShortcutsSection
          navigation={navigation}
          wordCharacter={wordCharacter}
          numberRowSelection={numberRowSelection}
          showNumberRowSelection={showNumberRowSelection}
          mobile={mobile}
          onNumberRowSelectionChange={onNumberRowSelectionChange}
        />
        <PanelShortcutsSection visible={showPanelShortcuts} macos={macos} harmony={harmony} />
      </div>
    </fieldset>
  );
}
