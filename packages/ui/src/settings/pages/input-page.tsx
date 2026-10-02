import { NavigationSection, defaultNavigation } from "../navigation-section";
import { useSettingsForm } from "../settings-form-context";
import * as settings from "../settings-style";
import { GroupList } from "../../core/platform-controls";
import { InputSchemeSettingsContent } from "../input-scheme-settings-content";
import { supportedInputSchemes } from "../input-scheme-options";
import { InputSharedSettingsSection } from "../input-shared-settings-section";
import { LocalModesSection } from "../local-modes-section";
import { CharacterWidthRow } from "../punctuation-section";
import { FuzzyPinyinSection } from "../fuzzy-pinyin-section";
import { SentenceAssociationSection } from "../sentence-association-section";
import { MobileEnglishSuggestionsRow } from "../mobile-keyboard-feedback-section";
import { HelpcodeSettingsGroup } from "./helpcode-page";
import { createUtilitiesSettingsActions } from "../utilities-settings-actions";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { createHelpcodeSettingsActions } from "../helpcode-settings-actions";

/** The 输入 page of the settings form. */
export function InputSettingsPage() {
  const {
    client,
    confirm,
    host,
    iosPlatform,
    linuxPlatform,
    mobilePlatform,
    macosPlatform,
    showModeScope,
    showCharacterWidth,
    showInputModeHUD,
    showPluginTriggers,
    showHelpcode,
    showHelpcodeShiftEntry,
    customHelpcodeSchemas,
    helpcodePacks,
    translationProvider,
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
    fuzzyPinyin,
    mixedInput,
    touchKeyboardSchemes,
    selectedTouchKeyboardScheme,
    selectTouchKeyboardScheme,
    setTouchKeyboardSchemeEnabled,
    localModes,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    saveMobileKeyboardFeedback,
  } = useSettingsForm();
  const { onLocalModesChange } = createUtilitiesSettingsActions({ setDraft });
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const { onChange: onHelpcodeChange } = createHelpcodeSettingsActions({ setDraft });
  const navigation = draft.navigation ?? defaultNavigation;
  return (
    <fieldset disabled={busy} hidden={page !== "input"} aria-label="输入">
      {/* 组的顺序按「基础 → 进阶」排：先选方案，再是每次打字都会碰到的中英文、选词与翻页，然后是候选从哪来（含中英混输）、以什么形式输出，最后是少数人才调的快捷模式、模糊音、辅助码和调频。这里不再沿用参考窗口的顺序，不要按参考窗口把它们挪回去。方案相关的行在当前方案用不到时隐藏而不删除，换方案时原样出现。 */}
      <div className={settings.groups}>
        <InputSchemeSettingsContent
          grouped
          preferences={draft}
          hasTouchKeyboardSchemes={Boolean(client.touchKeyboardSchemes)}
          touchKeyboardSchemes={touchKeyboardSchemes}
          selectedTouchKeyboardScheme={selectedTouchKeyboardScheme}
          macos={macosPlatform}
          inputSchemes={supportedInputSchemes(host)}
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
          // 中英文切换提示在所有平台都放在这里；macOS 以前把它放在快捷键页。
          showInputModeHUD={showInputModeHUD}
          showModeScope={showModeScope}
          mixedInput={mixedInput}
          onMixedInputChange={(mixed_input) => onPreferencesChange({ mixed_input })}
          paging={
            <NavigationSection
              navigation={navigation}
              wordCharacter={wordCharacter}
              linux={linuxPlatform}
              onChange={(next) =>
                onPreferencesChange({
                  // 只有占用了「以词定字」按键的翻页键才会改动它；否则未设置的值保持未设置。
                  ...(next.wordCharacter !== wordCharacter
                    ? { word_character: next.wordCharacter }
                    : {}),
                  navigation: next.navigation,
                })
              }
            />
          }
          beforeLearning={
            <SentenceAssociationSection
              value={draft.sentence_association}
              mobile={mobilePlatform}
              onChange={(sentence_association) => onPreferencesChange({ sentence_association })}
            />
          }
          afterLearning={
            // iOS 的英文建议存在原生 App Group 里，读写走 mobileKeyboardFeedback，与屏幕键盘页的按键反馈同一个来源。
            iosPlatform &&
            client.mobileKeyboardFeedback &&
            mobileKeyboardFeedback && (
              <MobileEnglishSuggestionsRow
                value={mobileKeyboardFeedback}
                busy={mobileKeyboardFeedbackBusy}
                onChange={(value) => void saveMobileKeyboardFeedback(value)}
              />
            )
          }
          outputExtra={
            showCharacterWidth ? (
              <CharacterWidthRow preferences={draft} onChange={onPreferencesChange} />
            ) : null
          }
          beforeFrequency={
            <>
              <LocalModesSection
                preferences={localModes}
                ios={iosPlatform}
                triggers={showPluginTriggers}
                mentions={showPluginTriggers && Boolean(client.plugins)}
                translationService={translationProvider !== "none"}
                onChange={onLocalModesChange}
              />
              {client.fuzzyPinyin && (
                <GroupList title="模糊音">
                  <FuzzyPinyinSection
                    collapsible
                    preferences={fuzzyPinyin}
                    onChange={(fuzzy_pinyin) => onPreferencesChange({ fuzzy_pinyin })}
                    confirm={confirm}
                  />
                </GroupList>
              )}
              {showHelpcode && (
                <HelpcodeSettingsGroup
                  value={draft}
                  customSchemas={customHelpcodeSchemas}
                  packs={helpcodePacks}
                  mobile={mobilePlatform}
                  showShiftEntry={showHelpcodeShiftEntry}
                  onChange={onHelpcodeChange}
                />
              )}
            </>
          }
          onPreferencesChange={onPreferencesChange}
        />
      </div>
    </fieldset>
  );
}
