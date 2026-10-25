import { useMemo, useState } from "react";
import { useSettingsTabs } from "../use-settings-tabs";
import { NavigationSection } from "../navigation-section";
import { navigationWithDefaults, pagingPreferencesPatch } from "../candidate-paging-preferences";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, Row } from "../../core/platform-controls";
import { InputSchemeSettingsContent } from "../input-scheme-settings-content";
import { supportedInputSchemes } from "../input-scheme-options";
import { InputSharedSettingsSection } from "../input-shared-settings-section";
import { CrossCommitSpacingSection } from "../cross-commit-spacing-section";
import { AppInputModeRulesSection } from "../app-input-mode-rules-section";
import { VimModeAppsSection } from "../vim-mode-apps-section";
import { LocalModesSection } from "../local-modes-section";
import { CharacterWidthRow } from "../punctuation-section";
import { FuzzyPinyinSection } from "../fuzzy-pinyin-section";
import {
  SentenceAssociationSection,
  SentenceWordLatticeRow,
} from "../sentence-association-section";
import { MobileEnglishSuggestionsRow } from "../mobile-keyboard-feedback-section";
import { HelpcodeSettingsGroup } from "./helpcode-page";
import { useHelpcodeImport } from "../use-helpcode-import";
import { createUtilitiesSettingsActions } from "../utilities-settings-actions";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { createHelpcodeSettingsActions } from "../helpcode-settings-actions";
import { ResourcePackRow, resourcePackStatus, useResourcePacks } from "../resource-packs";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SelectRow } from "../select-row";
import { SwitchRow } from "../switch-row";
import { CandidateTranslationOptionsSection } from "../candidate-translation-options-section";
import { CandidateEnglishGlossSection } from "../candidate-english-gloss-section";
import { ShuangpinCustomEditor, NineKeyShuangpinEditor } from "../shuangpin-custom-editor";
import type { ShuangpinCustomProfileClient } from "../shuangpin-custom-profile";
import { ActionButton } from "../action-button";
import {
  builtinNineKeyShuangpinProfiles,
  type BuiltinNineKeyShuangpinProfile,
} from "../nine-key-shuangpin-profile";
import { inputTabs } from "../input-tabs";
import { preeditControlState } from "../preedit-preferences";

/** 输入设置页。 */
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
    showInputModeStatusItem,
    showPluginTriggers,
    showHelpcode,
    showHelpcodeShiftEntry,
    showHelpcodeTriggerKey,
    customHelpcodeSchemas,
    helpcodePacks,
    refreshHelpcodePacks,
    translationProvider,
    draft,
    setDraft,
    busy,
    page,
    macosShuangpinKeymap,
    showShuangpinKeymapHint,
    showShuangpinPreedit,
    showTabPagePagingSwitches,
    showMixedInputMinimumPrefix,
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
  const dictionary = client.dictionary;
  const wubiCodes = dictionary?.wubiCodes;
  const wubiCodeLookup = useMemo(() => wubiCodes?.bind(dictionary), [dictionary, wubiCodes]);
  // 自定义双拼的编辑页是输入页里的一个视图（像插件页的详情），打开时代替整页内容；宿主能校验表、能给出内置方案的表时才有它。去设置表单里的别的页时它不关，只随输入页一起藏起来，回来时还是那张没保存的表；和插件页详情离开就关不同，因为这里有还没保存的键位，而只有编辑页的返回会先问一声再丢掉它们。
  const validateShuangpinCustomProfile = client.validateShuangpinCustomProfile;
  const builtinShuangpinTable = client.builtinShuangpinTable;
  const shuangpinCustomClient = useMemo<ShuangpinCustomProfileClient | undefined>(
    () =>
      validateShuangpinCustomProfile && builtinShuangpinTable
        ? { validate: validateShuangpinCustomProfile, builtin: builtinShuangpinTable }
        : undefined,
    [validateShuangpinCustomProfile, builtinShuangpinTable],
  );
  const readNineKeyLayout = client.nineKeyShuangpinLayout;
  const nineKeyClient = useMemo<
    ShuangpinCustomProfileClient<BuiltinNineKeyShuangpinProfile> | undefined
  >(
    () =>
      readNineKeyLayout
        ? {
            validate: async (table) =>
              (await readNineKeyLayout({ profile: "custom", table })).problem,
            builtin: async (profile) => (await readNineKeyLayout({ profile })).table,
          }
        : undefined,
    [readNineKeyLayout],
  );
  const [nineKeyEditorOpen, setNineKeyEditorOpen] = useState(false);
  const { tabList, panel } = useSettingsTabs(inputTabs, "输入设置分类", !mobilePlatform);
  const [shuangpinEditorOpen, setShuangpinEditorOpen] = useState(false);
  const { onLocalModesChange } = createUtilitiesSettingsActions({ setDraft });
  const { onPreferencesChange, onVoiceChange } = createSettingsDraftActions({ setDraft });
  const { onChange: onHelpcodeChange } = createHelpcodeSettingsActions({ setDraft });
  const helpcodeImport = useHelpcodeImport(client.plugins, setDraft, refreshHelpcodePacks);
  const navigation = navigationWithDefaults(draft.navigation);
  // 升级后补齐已保存偏好需要的资源包由宿主在启动时完成，这里挂载时不自动下载，只在用户选用方案、打开桌面神经联想或点「下载」时下载。提供哪些资源包由宿主的列表决定：日文和语言词库只有 macOS 列出。
  const resourcePacks = useResourcePacks(client.resourcePacks, {
    value: draft.voice_input?.asr_model_mirror ?? "",
    onChange: (asr_model_mirror) => onVoiceChange({ asr_model_mirror }),
    flush: retrySave,
  });
  const japanesePack = resourcePackStatus(resourcePacks, "japanese");
  // 不带临时日文的版本（host-api 也始终把它和日文混输关掉）不列出这个开关、它的词库和日文混输开关。
  const temporaryJapanese = host?.edition?.temporary_japanese ?? true;
  // 不带键盘神经模型的版本（host-api 也始终把它关掉）在触屏宿主上不列出神经联想开关。
  const neuralKeyboard = host?.edition?.neural_keyboard ?? true;
  // 应用例外只在认得前台应用的宿主上提供（macOS 按 bundle id，Windows 按进程基名，Linux 按输入法框架报告的程序名）。
  const showAppInputModeRules = host?.app_input_mode_rules === true;
  // HarmonyOS 手机像 Android 的 `TypingPage` 一样画设计稿的 输入 页：语言与方案、中文、辅助码 和 翻译直接可见，输入通用设置也直接展开，不再额外套一个「更多」分组。翻页键在它的 外接键盘快捷键 页，整句联想 在它的 表达 页，所以这里不画。
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
      onImport={helpcodeImport.importTable}
      importBusy={helpcodeImport.busy}
      importStatus={helpcodeImport.status}
      mobile={mobilePlatform}
      showShiftEntry={showHelpcodeShiftEntry}
      showTriggerKey={showHelpcodeTriggerKey}
      showManualTab={macosPlatform}
      // 当前方案所属那一族的辅助码，选法与 Android 的 `pickHelpcode` 一致：双拼 方案用 双拼，其他用 全拼。鸿蒙手机和桌面只摆出这一族，iOS 和 Android 照旧两族直列。
      activeFamily={
        harmonyPhone || !mobilePlatform
          ? draft.scheme === "shuangpin"
            ? "shuangpin_helpcode"
            : "quanpin_helpcode"
          : undefined
      }
      onChange={onHelpcodeChange}
    />
  );
  if (nineKeyEditorOpen && nineKeyClient)
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "input"} ariaLabel="输入">
        <NineKeyShuangpinEditor
          client={nineKeyClient}
          saved={draft.nine_key_shuangpin_custom_profile}
          active={draft.nine_key_shuangpin_profile === "custom"}
          saveExport={client.saveExport}
          confirm={confirm}
          onSave={(nine_key_shuangpin_custom_profile) =>
            onPreferencesChange({
              nine_key_shuangpin_profile: "custom",
              nine_key_shuangpin_custom_profile,
            })
          }
          onBack={() => setNineKeyEditorOpen(false)}
        />
      </SettingsPageFieldset>
    );
  if (shuangpinEditorOpen && shuangpinCustomClient)
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "input"} ariaLabel="输入">
        <ShuangpinCustomEditor
          client={shuangpinCustomClient}
          saved={draft.shuangpin_custom_profile}
          active={draft.shuangpin_profile === "custom"}
          saveExport={client.saveExport}
          confirm={confirm}
          onSave={(shuangpin_custom_profile) =>
            onPreferencesChange({ shuangpin_profile: "custom", shuangpin_custom_profile })
          }
          onBack={() => setShuangpinEditorOpen(false)}
        />
      </SettingsPageFieldset>
    );
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "input"} ariaLabel="输入">
      {tabList}
      {/* 组的顺序按「基础 → 进阶」排：先选方案，再是每次打字都会碰到的中英文、选词与翻页，然后是候选从哪来（含中英混输）、以什么形式输出，最后是少数人才调的快捷模式、模糊音、辅助码和调频。这里不再沿用参考窗口的顺序，不要按参考窗口把它们挪回去。方案相关的行在当前方案用不到时隐藏而不删除，换方案时原样出现。 */}

      {panel(
        "scheme",
        <>
          <InputSchemeSettingsContent
            preferences={draft}
            hasTouchKeyboardSchemes={Boolean(client.touchKeyboardSchemes)}
            koreanRomaja={host?.korean_romaja === true}
            touchKeyboardSchemes={touchKeyboardSchemes}
            selectedTouchKeyboardScheme={selectedTouchKeyboardScheme}
            macos={macosPlatform}
            linux={linuxPlatform}
            inputSchemes={supportedInputSchemes(host)}
            edition={host?.edition}
            // 共享偏好里还没有这一项时，macOS 显示本机 defaults 里的旧选择，与输入法实际的行为一致；其他宿主按关显示。
            shuangpinKeymapHint={
              showShuangpinKeymapHint
                ? (draft.shuangpin_keymap_hint ??
                  (macosPlatform ? macosShuangpinKeymap : undefined) ??
                  false)
                : undefined
            }
            // 双拼预编辑放在方案组的双拼详情里，只在双拼方案下出现。有触屏方案的宿主（含鸿蒙二合一）方案组不画双拼各行，那里仍在候选窗口/候选栏页设置它；条件与候选窗口页一致。
            shuangpinPreedit={
              showShuangpinPreedit && !client.touchKeyboardSchemes
                ? preeditControlState(draft).shuangpinStyle
                : undefined
            }
            macosInputModes={client.macosInputModes}
            onError={setError}
            onPreferencesChange={onPreferencesChange}
            onSelectTouchKeyboardScheme={(scheme) => selectTouchKeyboardScheme(scheme)}
            onToggleTouchKeyboardScheme={(scheme, enabled) =>
              setTouchKeyboardSchemeEnabled(scheme, enabled)
            }
            resourcePacks={resourcePacks}
            languageCard={harmonyPhone}
            onEnableAndSelectTouchKeyboardScheme={selectHomeScheme}
            wubiCodeLookup={wubiCodeLookup}
            onEditCustomShuangpin={
              shuangpinCustomClient ? () => setShuangpinEditorOpen(true) : undefined
            }
          />
          {nineKeyClient && selectedTouchKeyboardScheme === "nine_key_shuangpin" && (
            <GroupList title="九键双拼">
              <SelectRow
                title="九键双拼方案"
                value={draft.nine_key_shuangpin_profile ?? "migala3"}
                onChange={(event) => {
                  const profile = event.target.value;
                  if (profile === "custom") setNineKeyEditorOpen(true);
                  else
                    onPreferencesChange({
                      nine_key_shuangpin_profile: profile as BuiltinNineKeyShuangpinProfile,
                    });
                }}
              >
                {builtinNineKeyShuangpinProfiles.map(([profile, title]) => (
                  <option key={profile} value={profile}>
                    {title}
                  </option>
                ))}
                <option value="custom">自定义九键双拼</option>
              </SelectRow>
              <Row title="自定义键位" description="从内置方案开始编辑，或导入、导出数字键位表。">
                <ActionButton
                  label="编辑自定义九键双拼"
                  action={() => setNineKeyEditorOpen(true)}
                />
              </Row>
            </GroupList>
          )}
        </>,
      )}
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
              showSecondaryLanguage={false}
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
        crossCommitSpacing={
          macosPlatform ? (
            <CrossCommitSpacingSection
              enabled={draft.cross_commit_spacing}
              excludedApps={draft.cross_commit_spacing_excluded_apps}
              onChange={(cross_commit_spacing) => onPreferencesChange({ cross_commit_spacing })}
              onExcludedAppsChange={(cross_commit_spacing_excluded_apps) =>
                onPreferencesChange({ cross_commit_spacing_excluded_apps })
              }
              pickApplication={client.pickApplication}
              applicationNames={client.applicationNames}
            />
          ) : undefined
        }
        renderGroup={panel}
        showTraditionalOutput={!harmonyPhone}
        traditionalChineseVariants={host?.traditional_chinese_variants === true}
        preferences={draft}
        wordCharacter={wordCharacter}
        navigation={navigation}
        frequency={frequency}
        ios={iosPlatform}
        // 中英文切换提示在所有平台都放在这里；macOS 以前把它放在快捷键页。
        showInputModeHUD={showInputModeHUD}
        showInputModeFocusHUD={linuxPlatform}
        showInputModeCornerHUD={linuxPlatform}
        showInputModeStatusItem={showInputModeStatusItem}
        showModeScope={showModeScope}
        modeExtra={
          <>
            {showAppInputModeRules && (
              <AppInputModeRulesSection
                value={draft.app_input_mode_rules}
                windows={host?.platform === "windows"}
                linux={linuxPlatform}
                // Linux 没有选应用的对话框，程序名手填。
                pickApplication={linuxPlatform ? undefined : client.pickApplication}
                // Windows 和 Linux 的规则就是程序名，名称没有更多信息，宿主也不提供查询。
                applicationNames={
                  host?.platform === "windows" || linuxPlatform
                    ? undefined
                    : client.applicationNames
                }
                onChange={(app_input_mode_rules) => onPreferencesChange({ app_input_mode_rules })}
              />
            )}
            {host?.vim_mode_apps === true && (
              <VimModeAppsSection
                value={draft.vim_mode_apps}
                windows={host?.platform === "windows"}
                pickApplication={client.pickApplication}
                applicationNames={
                  host?.platform === "windows" ? undefined : client.applicationNames
                }
                onChange={(vim_mode_apps) => onPreferencesChange({ vim_mode_apps })}
              />
            )}
          </>
        }
        mixedInput={mixedInput}
        onMixedInputChange={(mixed_input) => onPreferencesChange({ mixed_input })}
        showMixedInputMinimumPrefix={showMixedInputMinimumPrefix}
        temporaryJapanese={temporaryJapanese}
        showMultilingualInput={host?.edition?.input_schemes.includes("quanpin") ?? true}
        quickPhraseCandidates={host?.quick_phrase_candidates ?? false}
        paging={
          !harmonyPhone && (
            <NavigationSection
              navigation={navigation}
              wordCharacter={wordCharacter}
              linux={linuxPlatform}
              tabPageSwitches={showTabPagePagingSwitches}
              onChange={(next) => onPreferencesChange(pagingPreferencesPatch(wordCharacter, next))}
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
        candidatesMore={
          // 鸿蒙手机的整句联想在「表达」页。
          !harmonyPhone && (
            <SentenceWordLatticeRow
              value={draft.sentence_association}
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
              shiftLetterEntrySwitch={host?.shift_letter_entry_switch ?? false}
              quickPhraseCandidates={host?.quick_phrase_candidates ?? false}
              // 临时日语只是一个快捷模式，不为它自动下载 60 多 MB 的词库，由用户手动下载；下载行跟在 R 模式那一行后面，不再单占一组。当前方案是日文时方案组里已有同一行，这里不再重复。
              temporaryJapaneseExtra={
                temporaryJapanese &&
                localModes.temporary_japanese &&
                draft.scheme !== "japanese" &&
                japanesePack &&
                japanesePack.state !== "installed" && (
                  <ResourcePackRow packs={resourcePacks} id="japanese" note="临时日语需要它" />
                )
              }
              onChange={onLocalModesChange}
            />
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
