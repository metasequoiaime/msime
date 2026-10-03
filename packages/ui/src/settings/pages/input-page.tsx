import { NavigationSection, defaultNavigation } from "../navigation-section";
import { useSettingsForm } from "../settings-form-context";
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
import { ResourcePackRow, resourcePackStatus, useResourcePacks } from "../resource-packs";
import { SettingsPageFieldset } from "../settings-page-fieldset";

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
    setError,
  } = useSettingsForm();
  const { onLocalModesChange } = createUtilitiesSettingsActions({ setDraft });
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const { onChange: onHelpcodeChange } = createHelpcodeSettingsActions({ setDraft });
  const navigation = draft.navigation ?? defaultNavigation;
  // 升级后补齐已保存方案的词库由宿主在启动时完成，这里挂载时不自动下载，只在用户选用方案或点「下载」时下载。
  const resourcePacks = useResourcePacks(macosPlatform ? client.resourcePacks : undefined);
  const japanesePack = resourcePackStatus(resourcePacks, "japanese");
  // 不带临时日文的版本（host-api 也始终把它关掉）不列出这个开关和它的词库。
  const temporaryJapanese = host?.edition?.temporary_japanese ?? true;
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "input"} ariaLabel="输入">
      {/* 组的顺序按「基础 → 进阶」排：先选方案，再是每次打字都会碰到的中英文、选词与翻页，然后是候选从哪来（含中英混输）、以什么形式输出，最后是少数人才调的快捷模式、模糊音、辅助码和调频。这里不再沿用参考窗口的顺序，不要按参考窗口把它们挪回去。方案相关的行在当前方案用不到时隐藏而不删除，换方案时原样出现。 */}

      <InputSchemeSettingsContent
        preferences={draft}
        hasTouchKeyboardSchemes={Boolean(client.touchKeyboardSchemes)}
        touchKeyboardSchemes={touchKeyboardSchemes}
        selectedTouchKeyboardScheme={selectedTouchKeyboardScheme}
        macos={macosPlatform}
        inputSchemes={supportedInputSchemes(host)}
        edition={host?.edition}
        macosShuangpinKeymap={
          macosPlatform && client.loadMacosShuangpinKeymap && macosShuangpinKeymap !== undefined
            ? macosShuangpinKeymap
            : undefined
        }
        macosWubiAutoCommitUnique={macosWubiAutoCommitUnique}
        macosInputModes={client.macosInputModes}
        onError={setError}
        onPreferencesChange={onPreferencesChange}
        onSelectTouchKeyboardScheme={(scheme) => selectTouchKeyboardScheme(scheme)}
        onToggleTouchKeyboardScheme={(scheme, enabled) =>
          setTouchKeyboardSchemeEnabled(scheme, enabled)
        }
        onMacosShuangpinKeymapChange={setShuangpinKeymap}
        onMacosWubiAutoCommitUniqueChange={setWubiAutoCommitUnique}
        resourcePacks={resourcePacks}
      />
      <InputSharedSettingsSection
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
              temporaryJapanese={temporaryJapanese}
              onChange={onLocalModesChange}
            />
            {/* 临时日语只是一个快捷模式，不为它自动下载 60 多 MB 的词库，由用户手动下载。当前方案是日文时方案组里已有同一行，这里不再重复。 */}
            {macosPlatform &&
              temporaryJapanese &&
              localModes.temporary_japanese &&
              draft.scheme !== "japanese" &&
              japanesePack &&
              japanesePack.state !== "installed" && (
                <GroupList title="临时日语词库">
                  <ResourcePackRow packs={resourcePacks} id="japanese" note="临时日语需要它" />
                </GroupList>
              )}
            {client.fuzzyPinyin && (
              <GroupList title="模糊音">
                <FuzzyPinyinSection
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
    </SettingsPageFieldset>
  );
}
