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
import { SelectRow } from "../select-row";
import { SwitchRow } from "../switch-row";
import { CandidateTranslationOptionsSection } from "../candidate-translation-options-section";
import { CandidateEnglishGlossSection } from "../candidate-english-gloss-section";

/** The 输入 page of the settings form. */
export function InputSettingsPage() {
  const {
    client,
    confirm,
    host,
    iosPlatform,
    harmonyPlatform,
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
    wordCharacter,
    frequency,
    fuzzyPinyin,
    mixedInput,
    touchKeyboardSchemes,
    selectedTouchKeyboardScheme,
    selectTouchKeyboardScheme,
    setTouchKeyboardSchemeEnabled,
    selectHomeScheme,
    localModes,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    saveMobileKeyboardFeedback,
    setError,
    retrySave,
    candidateTranslations,
    candidateEnglishGloss,
    candidateGlossLanguagesEnabled,
    translationTargetLanguage,
    visibleTranslationLanguages,
    visibleSecondaryLanguages,
  } = useSettingsForm();
  const { onLocalModesChange } = createUtilitiesSettingsActions({ setDraft });
  const { onPreferencesChange, onVoiceChange } = createSettingsDraftActions({ setDraft });
  const { onChange: onHelpcodeChange } = createHelpcodeSettingsActions({ setDraft });
  const navigation = draft.navigation ?? defaultNavigation;
  // 升级后补齐已保存偏好需要的资源包由宿主在启动时完成，这里挂载时不自动下载，只在用户选用方案、打开桌面神经联想或点「下载」时下载。提供哪些资源包由宿主的列表决定：日文和语言词库只有 macOS 列出。
  const resourcePacks = useResourcePacks(client.resourcePacks, {
    value: draft.voice_input?.asr_model_mirror ?? "",
    onChange: (asr_model_mirror) => onVoiceChange({ asr_model_mirror }),
    flush: retrySave,
  });
  const japanesePack = resourcePackStatus(resourcePacks, "japanese");
  // 不带临时日文的版本（host-api 也始终把它关掉）不列出这个开关和它的词库。
  const temporaryJapanese = host?.edition?.temporary_japanese ?? true;
  // 不带键盘神经模型的版本（host-api 也始终把它关掉）在触屏宿主上不列出神经联想开关。
  const neuralKeyboard = host?.edition?.neural_keyboard ?? true;
  // HarmonyOS 手机像 Android 的 `TypingPage` 一样画设计稿的 输入 页：语言与方案、中文、辅助码 和 翻译 直接可见，其余都放进末尾的 更多 折叠区。翻页键在它的 候选栏 页，整句联想 在它的 表达 页，所以这里不画。
  const harmonyPhone = harmonyPlatform && mobilePlatform;
  // 拼音纠错 是两项全拼纠错共用的一个开关，只有两项都开启时才显示为开；打开或关闭会同时设置两项，和 Android 的 `TypingPage` 一样。
  const autocorrect =
    (draft.quanpin?.autocorrect_transposition ?? true) &&
    (draft.quanpin?.autocorrect_neighbor ?? true);
  const fuzzyPinyinSection = (layout: "section" | "row") => (
    <FuzzyPinyinSection
      preferences={fuzzyPinyin}
      onChange={(fuzzy_pinyin) => onPreferencesChange({ fuzzy_pinyin })}
      confirm={confirm}
      layout={layout}
    />
  );
  const helpcodeGroup = showHelpcode && (
    <HelpcodeSettingsGroup
      value={draft}
      customSchemas={customHelpcodeSchemas}
      packs={helpcodePacks}
      mobile={mobilePlatform}
      showShiftEntry={showHelpcodeShiftEntry}
      // 当前方案所属那一族的辅助码，选法与 Android 的 `pickHelpcode` 一致：双拼 方案用 双拼，其他用 全拼。
      activeFamily={
        harmonyPhone
          ? draft.scheme === "shuangpin"
            ? "shuangpin_helpcode"
            : "quanpin_helpcode"
          : undefined
      }
      onChange={onHelpcodeChange}
    />
  );
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
        macosInputModes={client.macosInputModes}
        onError={setError}
        onPreferencesChange={onPreferencesChange}
        onSelectTouchKeyboardScheme={(scheme) => selectTouchKeyboardScheme(scheme)}
        onToggleTouchKeyboardScheme={(scheme, enabled) =>
          setTouchKeyboardSchemeEnabled(scheme, enabled)
        }
        onMacosShuangpinKeymapChange={setShuangpinKeymap}
        resourcePacks={resourcePacks}
        languageCard={harmonyPhone}
        onEnableAndSelectTouchKeyboardScheme={selectHomeScheme}
      />
      {harmonyPhone && (
        <>
          <GroupList title="中文">
            <SelectRow
              title="中文字符集"
              value={draft.traditional_chinese_output ? "traditional" : "simplified"}
              onChange={(event) =>
                onPreferencesChange({
                  traditional_chinese_output: event.target.value === "traditional",
                })
              }
            >
              <option value="simplified">简体</option>
              <option value="traditional">繁体</option>
            </SelectRow>
            <SwitchRow
              title="拼音纠错"
              checked={autocorrect}
              onChange={(checked) =>
                onPreferencesChange({
                  quanpin: {
                    ...draft.quanpin,
                    autocorrect_transposition: checked,
                    autocorrect_neighbor: checked,
                  },
                })
              }
            />
            {client.fuzzyPinyin && fuzzyPinyinSection("row")}
          </GroupList>
          {helpcodeGroup}
          <GroupList title="翻译">
            <CandidateTranslationOptionsSection
              layout="phone"
              enabled={candidateTranslations}
              targetLanguage={translationTargetLanguage}
              secondaryLanguage={draft.translation_secondary_language ?? ""}
              candidateGlossLanguagesEnabled={candidateGlossLanguagesEnabled}
              visibleLanguages={visibleTranslationLanguages}
              visibleSecondaryLanguages={visibleSecondaryLanguages}
              showSecondaryLanguage
              showAccountTranslation={false}
              accountTranslation={false}
              onEnabledChange={(candidate_translations) =>
                onPreferencesChange({ candidate_translations })
              }
              onTargetLanguageChange={(translation_target_language) =>
                onPreferencesChange({ translation_target_language })
              }
              onSecondaryLanguageChange={(value) =>
                onPreferencesChange({ translation_secondary_language: value === "" ? null : value })
              }
              onAccountTranslationChange={() => {}}
              beforeMore={
                client.candidateEnglishGloss && (
                  <CandidateEnglishGlossSection
                    value={candidateEnglishGloss}
                    onChange={(candidate_english_gloss) =>
                      onPreferencesChange({ candidate_english_gloss })
                    }
                  />
                )
              }
            />
          </GroupList>
        </>
      )}
      <InputSharedSettingsSection
        foldIntoMore={harmonyPhone}
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
          !harmonyPhone && (
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
          )
        }
        beforeLearning={
          !harmonyPhone && (
            <SentenceAssociationSection
              value={draft.sentence_association}
              mobile={mobilePlatform}
              neuralKeyboard={neuralKeyboard}
              resourcePacks={resourcePacks}
              onChange={(sentence_association) => onPreferencesChange({ sentence_association })}
            />
          )
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
            {temporaryJapanese &&
              localModes.temporary_japanese &&
              draft.scheme !== "japanese" &&
              japanesePack &&
              japanesePack.state !== "installed" && (
                <GroupList title="临时日语词库">
                  <ResourcePackRow packs={resourcePacks} id="japanese" note="临时日语需要它" />
                </GroupList>
              )}
            {!harmonyPhone && client.fuzzyPinyin && (
              <GroupList title="模糊音">{fuzzyPinyinSection("section")}</GroupList>
            )}
            {!harmonyPhone && helpcodeGroup}
          </>
        }
        onPreferencesChange={onPreferencesChange}
      />
    </SettingsPageFieldset>
  );
}
