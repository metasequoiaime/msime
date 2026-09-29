import * as settings from "../settings-style";
import { defaultNavigation } from "../settings-options";
import { useSettingsForm } from "../settings-form-context";
import { InputModeShortcutsSection } from "../input-mode-shortcuts-section";
import { PanelShortcutsSection } from "../panel-shortcuts-section";
import { CandidateShortcutsSection } from "../candidate-shortcuts-section";
import { MaintenanceShortcutsSection } from "../maintenance-shortcuts-section";
import { InputMethodServiceSection } from "../input-method-service-section";

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
    <fieldset disabled={busy} hidden={page !== "shortcuts"} aria-label="快捷键">
      <div className={`section ${settings.shortcutIntro}`}>
        {mobilePlatform
          ? "输入法快捷键仅在对应输入状态或候选栏显示时生效。翻页方式可在“候选栏”中启用或关闭。"
          : "输入法快捷键仅在对应输入状态或候选窗口显示时生效。翻页方式可在“候选窗口”中启用或关闭。"}
      </div>
      <div className={settings.groups}>
        <InputModeShortcutsSection
          keybindings={keybindings}
          onChange={(patch) => setDraft({ ...draft, keybindings: { ...keybindings, ...patch } })}
          onInputModeHUDChange={(checked) => setDraft({ ...draft, input_mode_hud: checked })}
          showModeSwitchShortcuts={showModeSwitchShortcuts}
          macos={macosPlatform}
          showInputModeHUD={showInputModeHUD}
          inputModeHUD={inputModeHUD}
          showFullwidthChord={showFullwidthChord}
          fullwidthChord={fullwidthChord}
          windows={windowsPlatform}
        />
        <PanelShortcutsSection
          visible={showPanelShortcuts}
          macos={macosPlatform}
          harmony={harmonyPlatform}
        />
        <CandidateShortcutsSection
          navigation={navigation}
          numberRowSelection={draft.number_row_selection ?? true}
          showNumberRowSelection={showNumberRowSelection}
          mobile={mobilePlatform}
          onNumberRowSelectionChange={(checked) =>
            setDraft({ ...draft, number_row_selection: checked })
          }
        />
        <MaintenanceShortcutsSection
          visible={showDesktopMaintenanceShortcuts}
          macos={macosPlatform}
          linux={linuxPlatform}
          maintenanceChord={maintenanceChord}
        />
        <InputMethodServiceSection
          visible={Boolean(showRestartInputMethod)}
          macos={macosPlatform}
          linux={linuxPlatform}
          restartInputMethod={client.restartInputMethod}
          installInputSource={showInstallInputSource ? client.installInputSource : undefined}
        />
      </div>
    </fieldset>
  );
}
