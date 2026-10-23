import { useSettingsForm } from "../settings-form-context";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { GroupList, MoreOptions } from "../../core/platform-controls";
import { InputLanguageOptionsSection } from "../input-language-options-section";
import { CandidatePronunciationSection } from "../candidate-pronunciation-section";
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
  } = useSettingsForm();
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
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
    translationTargetLanguage,
    translationSecondaryLanguage: draft.translation_secondary_language,
    candidateGlossLanguagesEnabled,
    visibleTranslationLanguages,
    visibleSecondaryLanguages,
    onDeviceMissingLanguages,
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
  // 「显示读音」是释义的子行，画在目标语言之后：没有任何释义来源打开时没有东西可读，这一行不出现，已存的值不动。
  const glossCandidate = {
    ...translation.candidate,
    beforeMore: client.candidatePronunciation && candidateGlossLanguagesEnabled && (
      <CandidatePronunciationSection
        value={draft.candidate_pronunciation ?? false}
        onChange={(candidate_pronunciation) => onPreferencesChange({ candidate_pronunciation })}
      />
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
      {mobilePlatform && <MobileInputAiNotice onOpenAi={() => selectPage("ai")} />}
    </SettingsPageFieldset>
  );
}
