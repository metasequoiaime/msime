import type { InputModeShortcutPreferences } from "./input-mode-shortcuts-section";
import { InputModeShortcutsSection } from "./input-mode-shortcuts-section";
import { CandidateShortcutsSection } from "./candidate-shortcuts-section";
import { MaintenanceShortcutsSection } from "./maintenance-shortcuts-section";
import { PanelShortcutsSection } from "./panel-shortcuts-section";
import { InputMethodServiceSection } from "./input-method-service-section";
import { ShortcutsIntroSection } from "./shortcuts-intro-section";
import * as settings from "./settings-style";
import type { NavigationPreferences } from "./word-character-section";

export interface ShortcutsSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  mobile: boolean;
  keybindings: InputModeShortcutPreferences;
  onKeybindingsChange: (patch: Partial<InputModeShortcutPreferences>) => void;
  onInputModeHUDChange: (value: boolean) => void;
  showModeSwitchShortcuts: boolean;
  macos: boolean;
  showInputModeHUD: boolean;
  inputModeHUD: boolean;
  showFullwidthChord: boolean;
  fullwidthChord: string;
  windows: boolean;
  navigation: NavigationPreferences;
  numberRowSelection: boolean;
  showNumberRowSelection: boolean;
  onNumberRowSelectionChange: (value: boolean) => void;
  showPanelShortcuts: boolean;
  harmony: boolean;
  showDesktopMaintenanceShortcuts: boolean;
  linux: boolean;
  maintenanceChord: string;
  showRestartInputMethod: boolean;
  restartInputMethod?: () => Promise<void>;
  installInputSource?: () => Promise<void>;
}

/** Shortcut settings page composition shared by desktop and mobile hosts. */
export function ShortcutsSettingsSection({
  disabled,
  hidden,
  mobile,
  keybindings,
  onKeybindingsChange,
  onInputModeHUDChange,
  showModeSwitchShortcuts,
  macos,
  showInputModeHUD,
  inputModeHUD,
  showFullwidthChord,
  fullwidthChord,
  windows,
  navigation,
  numberRowSelection,
  showNumberRowSelection,
  onNumberRowSelectionChange,
  showPanelShortcuts,
  harmony,
  showDesktopMaintenanceShortcuts,
  linux,
  maintenanceChord,
  showRestartInputMethod,
  restartInputMethod,
  installInputSource,
}: ShortcutsSettingsSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="快捷键">
      <div className={settings.groups}>
        <ShortcutsIntroSection mobile={mobile} />
        <InputModeShortcutsSection
          keybindings={keybindings}
          onChange={onKeybindingsChange}
          onInputModeHUDChange={onInputModeHUDChange}
          showModeSwitchShortcuts={showModeSwitchShortcuts}
          macos={macos}
          showInputModeHUD={showInputModeHUD}
          inputModeHUD={inputModeHUD}
          showFullwidthChord={showFullwidthChord}
          fullwidthChord={fullwidthChord}
          windows={windows}
        />
        <PanelShortcutsSection visible={showPanelShortcuts} macos={macos} harmony={harmony} />
        <CandidateShortcutsSection
          navigation={navigation}
          numberRowSelection={numberRowSelection}
          showNumberRowSelection={showNumberRowSelection}
          mobile={mobile}
          onNumberRowSelectionChange={onNumberRowSelectionChange}
        />
        <MaintenanceShortcutsSection
          visible={showDesktopMaintenanceShortcuts}
          macos={macos}
          linux={linux}
          maintenanceChord={maintenanceChord}
        />
        <InputMethodServiceSection
          visible={showRestartInputMethod}
          macos={macos}
          linux={linux}
          restartInputMethod={restartInputMethod}
          installInputSource={installInputSource}
        />
      </div>
    </fieldset>
  );
}
