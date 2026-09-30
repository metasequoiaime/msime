import { defaultNavigation } from "../navigation-section";
import { useSettingsForm } from "../settings-form-context";
import * as settings from "../settings-style";
import { GroupList, Row, Switch } from "../../core/platform-controls";
import { InputSchemeSettingsContent } from "../input-scheme-settings-content";
import { InputSharedSettingsSection } from "../input-shared-settings-section";
import { LocalModesSection } from "../local-modes-section";
import { CharacterWidthRow } from "../punctuation-section";
import { createUtilitiesSettingsActions } from "../utilities-settings-actions";
import { createSettingsDraftActions } from "../settings-draft-actions";

/** The 输入 page of the settings form. */
export function InputSettingsPage() {
  const {
    client,
    iosPlatform,
    mobilePlatform,
    macosPlatform,
    showModeScope,
    showCharacterWidth,
    showInputModeHUD,
    draft,
    setDraft,
    busy,
    page,
    macosShuangpinKeymap,
    setShuangpinKeymap,
    macosWubiAutoCommitUnique,
    setWubiAutoCommitUnique,
    wordCharacter,
    frequency,
    touchKeyboardSchemes,
    selectedTouchKeyboardScheme,
    selectTouchKeyboardScheme,
    setTouchKeyboardSchemeEnabled,
    localModes,
  } = useSettingsForm();
  const { onLocalModesChange } = createUtilitiesSettingsActions({ setDraft });
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const navigation = draft.navigation ?? defaultNavigation;
  return (
    <fieldset disabled={busy} hidden={page !== "input"} aria-label="输入">
      {/* The groups keep the reference window's order of these settings; each row is present whenever the reference shows it and hidden, not removed, while the chosen scheme makes it moot, as the reference does. */}
      <div className={settings.groups}>
        <InputSchemeSettingsContent
          grouped
          preferences={draft}
          hasTouchKeyboardSchemes={Boolean(client.touchKeyboardSchemes)}
          touchKeyboardSchemes={touchKeyboardSchemes}
          selectedTouchKeyboardScheme={selectedTouchKeyboardScheme}
          macos={macosPlatform}
          macosShuangpinKeymap={
            macosPlatform && client.loadMacosShuangpinKeymap && macosShuangpinKeymap !== undefined
              ? macosShuangpinKeymap
              : undefined
          }
          macosWubiAutoCommitUnique={macosWubiAutoCommitUnique}
          onPreferencesChange={onPreferencesChange}
          onSelectTouchKeyboardScheme={(scheme) => selectTouchKeyboardScheme(scheme)}
          onToggleTouchKeyboardScheme={(scheme, enabled) =>
            setTouchKeyboardSchemeEnabled(scheme, enabled)
          }
          onMacosShuangpinKeymapChange={setShuangpinKeymap}
          onMacosWubiAutoCommitUniqueChange={setWubiAutoCommitUnique}
        />
        <InputSharedSettingsSection
          grouped
          preferences={draft}
          wordCharacter={wordCharacter}
          navigation={navigation}
          frequency={frequency}
          ios={iosPlatform}
          showInputModeHUD={showInputModeHUD && !macosPlatform}
          showModeScope={showModeScope}
          outputExtra={
            showCharacterWidth ? (
              <CharacterWidthRow preferences={draft} onChange={onPreferencesChange} />
            ) : null
          }
          beforeFrequency={
            <GroupList title="整句联想">
              <Row
                title="本地整句联想"
                description="把词库组合出的整句加入候选；关闭后仍保留单词候选。"
              >
                <Switch
                  checked={draft.sentence_association?.word_lattice ?? true}
                  onChange={(checked) =>
                    onPreferencesChange({
                      sentence_association: {
                        ...draft.sentence_association,
                        word_lattice: checked,
                      },
                    })
                  }
                />
              </Row>
              <Row
                title={mobilePlatform ? "键盘神经联想" : "桌面神经联想"}
                description="使用随包的神经模型重排整句候选；没有模型时保持现有候选。"
              >
                <Switch
                  checked={
                    mobilePlatform
                      ? (draft.sentence_association?.neural_keyboard ?? false)
                      : (draft.sentence_association?.neural_desktop ?? false)
                  }
                  onChange={(checked) =>
                    onPreferencesChange({
                      sentence_association: {
                        ...draft.sentence_association,
                        ...(mobilePlatform
                          ? { neural_keyboard: checked }
                          : { neural_desktop: checked }),
                      },
                    })
                  }
                />
              </Row>
            </GroupList>
          }
          afterFrequency={
            <LocalModesSection
              preferences={localModes}
              ios={iosPlatform}
              onChange={onLocalModesChange}
            />
          }
          onPreferencesChange={onPreferencesChange}
        />
      </div>
    </fieldset>
  );
}
