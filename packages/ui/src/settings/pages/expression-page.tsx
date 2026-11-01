import { useSettingsForm } from "../settings-form-context";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { GroupList, MoreOptions } from "../../core/platform-controls";
import { InputLanguageOptionsSection } from "../input-language-options-section";
import { CandidatePronunciationSection } from "../candidate-pronunciation-section";
import { ResourcePackRow, useResourcePacks } from "../resource-packs";
import { candidateTranslationScope } from "../candidate-translation-options-section";
import { EnglishSuggestionsSection } from "../english-suggestions-section";
import { PhonePunctuationSection, PunctuationSection } from "../punctuation-section";
import { SentenceAssociationLevelRow } from "../sentence-association-section";
import { TranslationSettingsContent } from "../translation-settings-content";
import { useSettingsTabs } from "../use-settings-tabs";
import { TranslationProviderSettingsSection } from "../translation-provider-settings-section";
import { TranslationServiceSelectorSection } from "../translation-service-selector-section";
import { createTranslationSettingsBindings } from "../translation-settings-bindings";
import { MobileInputAiNotice } from "../mobile-input-ai-notice";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { expressionTabs } from "../expression-tabs";
import type { Preferences } from "../../core/preferences-model";

/** 共享层除英文外还有读音表的释义语言，和 `msime_engine::host::PHONETIC_LANGUAGES` 一致。 */
const TARGET_READING_LANGUAGES: ReadonlySet<string> = new Set(["fr", "ja", "es", "ru", "de", "ko"]);

/** 主目标语言有没有外语读音表要用。Linux 宿主只给主目标语言那一行标读音（Fcitx5 的 `pronunciationTargets`、IBus 的 `ClientEngine` 都只传主目标语言），第二目标语言是外语时下载了也用不上。 */
function targetReadingLanguage(draft: Partial<Preferences>): boolean {
  const language = draft.translation_target_language;
  return typeof language === "string" && TARGET_READING_LANGUAGES.has(language);
}

/**
 * 标点与翻译页：打出来的内容以什么形式出现——标点、其他语言的候选与释义、翻译服务。混入的英文与表情颜文字是候选来源，模糊音改的是拼音怎么解析，两者都在输入页；AI 功能在「工具」组的「AI 辅助」页。
 *
 * 在鸿蒙手机上这是设计稿的「表达」页：「标点」分组和「智能」分组（整句联想档位和英文联想），对应 Android 的 `ExpressionPage`。候选翻译和「显示英文释义」在那里放在「输入」页的「翻译」分组，翻译服务及其凭据仍可在「更多选项」下找到，「高情商回复」卡片已移除。社区「发现短语」列表不提供，因为鸿蒙没有可供安装的短语库。
 */
export function ExpressionSettingsPage() {
  const {
    client,
    linuxPlatform,
    androidPlatform,
    iosPlatform,
    harmonyPlatform,
    mobilePlatform,
    windowsPlatform,
    macosPlatform,
    showEnglishSuggestions,
    draft,
    setDraft,
    busy,
    page,
    providerCredentials,
    tencentCredentialInput,
    updateTencentCredentialInput,
    providerCredentialBusy,
    candidateTranslations,
    candidateEnglishGloss,
    englishSuggestions,
    candidateGlossLanguagesEnabled,
    translationTargetLanguage,
    visibleTranslationLanguages,
    visibleSecondaryLanguages,
    customTranslation,
    tencentTranslation,
    niutrans,
    translationProvider,
    aiAssistant,
    aiTranslationReady,
    aiTranslationToken,
    onDeviceMissingLanguages,
    setError,
    setTranslationProvider,
    runProviderCredential,
    providerCredentialMessages,
    credentialTestControl,
    selectPage,
    host,
    showAiGeneration,
    retrySave,
  } = useSettingsForm();
  const { onPreferencesChange, onVoiceChange } = createSettingsDraftActions({ setDraft });
  // 只有安装包不带读音表的宿主（Linux）列出「离线英文音标词典」和「离线外语读音词典」；打开「显示读音」时在开关下面给出下载行，不自动下载，外语那一行只在主目标语言是法、日、西、俄、德、韩语时给出。其他平台这两行不会出现，不读资源包列表。
  const pronunciationDownloadable = linuxPlatform && Boolean(client.candidatePronunciation);
  const resourcePacks = useResourcePacks(
    pronunciationDownloadable ? client.resourcePacks : undefined,
    {
      value: draft.voice_input?.asr_model_mirror ?? "",
      onChange: (asr_model_mirror) => onVoiceChange({ asr_model_mirror }),
      flush: retrySave,
    },
  );
  const translation = createTranslationSettingsBindings({
    grouped: true,
    client,
    candidateTranslations,
    linux: linuxPlatform,
    windows: windowsPlatform,
    macos: macosPlatform,
    customTranslation,
    tencentTranslation,
    niutrans,
    translationProvider,
    aiAssistant,
    aiReady: aiTranslationReady,
    aiToken: aiTranslationToken,
    providerCredentials,
    tencentCredentialInput,
    updateTencentCredentialInput,
    providerCredentialBusy,
    providerCredentialMessages,
    runProviderCredential,
    credentialTestControl,
    onPreferencesChange,
    setTranslationProvider,
    android: androidPlatform,
    ios: iosPlatform,
    harmony: harmonyPlatform,
    mobile: mobilePlatform,
    translationAccount: draft.translation_account ?? false,
    aiGeneration: showAiGeneration,
    translationTargetLanguage,
    translationSecondaryLanguage: draft.translation_secondary_language,
    candidateGlossLanguagesEnabled,
    visibleTranslationLanguages,
    visibleSecondaryLanguages,
    onDeviceMissingLanguages,
    candidateTranslationScope: candidateTranslationScope(draft.candidate_translation_scope),
    candidateTranslationShowPos: draft.candidate_translation_show_pos ?? true,
    candidateTranslationSingleSense: draft.candidate_translation_single_sense ?? false,
    openSettings: client.onDeviceTranslation?.openSettings,
    onError: setError,
  });
  const { tabList, panel } = useSettingsTabs(
    expressionTabs.filter((tab) => tab.id !== "providers" || Boolean(translation.providers)),
    "标点与翻译设置分类",
    !mobilePlatform,
  );
  // 「显示读音」是释义的子行，画在目标语言之后：桌面只给释义标读音，没有任何释义来源打开时没有东西可读，这一行不出现，已存的值不动。鸿蒙键盘给英文候选标音标不看释义开关，这一行一直在，关掉释义后仍能关掉音标。
  const glossCandidate = {
    ...translation.candidate,
    beforeMore: client.candidatePronunciation &&
      (harmonyPlatform || candidateGlossLanguagesEnabled) && (
        <>
          <CandidatePronunciationSection
            value={draft.candidate_pronunciation ?? false}
            japaneseRomaji={!linuxPlatform && !harmonyPlatform}
            downloadable={pronunciationDownloadable}
            englishCandidates={harmonyPlatform}
            onChange={(candidate_pronunciation) => onPreferencesChange({ candidate_pronunciation })}
          />
          {pronunciationDownloadable && draft.candidate_pronunciation && (
            <ResourcePackRow packs={resourcePacks} id="pronunciations" note="英文读音需要它" />
          )}
          {pronunciationDownloadable &&
            draft.candidate_pronunciation &&
            targetReadingLanguage(draft) && (
              <ResourcePackRow
                packs={resourcePacks}
                id="target-pronunciations"
                note="法、日、西、俄、德、韩语释义的读音需要它"
              />
            )}
        </>
      ),
  };
  const harmonyPhone = harmonyPlatform && mobilePlatform;
  const ariaLabel = mobilePlatform ? "表达" : "标点与翻译";
  if (harmonyPhone) {
    const providers = translation.providers;
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "expression"} ariaLabel={ariaLabel}>
        <GroupList title="标点">
          <PhonePunctuationSection preferences={draft} onChange={onPreferencesChange} />
        </GroupList>
        <GroupList title="智能">
          <SentenceAssociationLevelRow
            value={draft.sentence_association}
            // 没有键盘模型的版本（`host-api` 在那里关闭了 `neural_keyboard`）不提供「增强」。
            neuralKeyboard={host?.edition?.neural_keyboard ?? true}
            onChange={(sentence_association) => onPreferencesChange({ sentence_association })}
          />
          {showEnglishSuggestions && (
            <EnglishSuggestionsSection
              title="英文联想"
              description={null}
              value={englishSuggestions}
              onChange={(english_suggestions) => onPreferencesChange({ english_suggestions })}
            />
          )}
        </GroupList>
        {providers && (
          <GroupList title="翻译服务">
            <MoreOptions>
              <TranslationServiceSelectorSection
                grouped
                available={candidateTranslations}
                provider={translationProvider}
                showAccountProvider={translation.candidate.showAccountProvider ?? false}
                showAiProvider={translation.candidate.showAiProvider}
                offlineFallback={translation.candidate.offlineFallback}
                onChange={setTranslationProvider}
              />
              {/* 这一组已经整体收在「更多选项」里，腾讯云地域不再套第二层。 */}
              <TranslationProviderSettingsSection
                {...providers}
                grouped={false}
                foldRegion={false}
              />
            </MoreOptions>
          </GroupList>
        )}
      </SettingsPageFieldset>
    );
  }
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "expression"} ariaLabel={ariaLabel}>
      {tabList}
      {panel(
        "punctuation",
        <GroupList title="标点">
          <PunctuationSection
            preferences={draft}
            showCharacterWidth={false}
            showCapsLockPunctuation={host?.caps_lock_punctuation ?? false}
            onChange={onPreferencesChange}
          />
        </GroupList>,
      )}
      <TranslationSettingsContent
        candidate={glossCandidate}
        providers={translation.providers}
        serviceInOwnGroup={!mobilePlatform}
        renderGroup={(tab, children) =>
          tab === "translation"
            ? panel(
                "gloss",
                <>
                  <InputLanguageOptionsSection
                    grouped
                    includeCandidateControls
                    showCandidateEnglishGloss={client.candidateEnglishGloss}
                    candidateEnglishGloss={candidateEnglishGloss}
                    onCandidateEnglishGlossChange={(candidate_english_gloss) =>
                      onPreferencesChange({ candidate_english_gloss })
                    }
                    showEnglishSuggestions={showEnglishSuggestions}
                    englishSuggestions={englishSuggestions}
                    onEnglishSuggestionsChange={(english_suggestions) =>
                      onPreferencesChange({ english_suggestions })
                    }
                  />
                  {children}
                </>,
              )
            : panel(tab, children)
        }
      />
      {mobilePlatform && showAiGeneration && (
        <MobileInputAiNotice onOpenAi={() => selectPage("ai")} />
      )}
    </SettingsPageFieldset>
  );
}
