import { useSettingsForm } from "../settings-form-context";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { GroupList, MoreOptions } from "../../core/platform-controls";
import { InputLanguageOptionsSection } from "../input-language-options-section";
import { EnglishSuggestionsSection } from "../english-suggestions-section";
import { PhonePunctuationSection, PunctuationSection } from "../punctuation-section";
import { SentenceAssociationLevelRow } from "../sentence-association-section";
import { TranslationSettingsContent } from "../translation-settings-content";
import { TranslationProviderSettingsSection } from "../translation-provider-settings-section";
import { TranslationServiceSelectorSection } from "../translation-service-selector-section";
import { createTranslationSettingsBindings } from "../translation-settings-bindings";
import { MobileInputAiNotice } from "../mobile-input-ai-notice";
import { SettingsPageFieldset } from "../settings-page-fieldset";

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
    translationAccount: draft.translation_account ?? false,
    translationTargetLanguage,
    translationSecondaryLanguage: draft.translation_secondary_language,
    candidateGlossLanguagesEnabled,
    visibleTranslationLanguages,
    visibleSecondaryLanguages,
    onDeviceMissingLanguages,
    openSettings: client.onDeviceTranslation?.openSettings,
    onError: setError,
  });
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
                onChange={setTranslationProvider}
              />
              <TranslationProviderSettingsSection {...providers} grouped={false} />
            </MoreOptions>
          </GroupList>
        )}
      </SettingsPageFieldset>
    );
  }
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "expression"} ariaLabel={ariaLabel}>
      <GroupList title="标点">
        <PunctuationSection
          preferences={draft}
          showCharacterWidth={false}
          showCapsLockPunctuation={host?.caps_lock_punctuation ?? false}
          onChange={onPreferencesChange}
        />
      </GroupList>
      <InputLanguageOptionsSection
        grouped
        includeCandidateControls
        showCandidateEnglishGloss={client.candidateEnglishGloss}
        candidateEnglishGloss={candidateEnglishGloss}
        onCandidateEnglishGlossChange={(candidate_english_gloss) =>
          onPreferencesChange({ candidate_english_gloss })
        }
        showCandidatePronunciation={client.candidatePronunciation}
        candidatePronunciation={draft.candidate_pronunciation ?? false}
        candidatePronunciationDisabled={!candidateGlossLanguagesEnabled}
        onCandidatePronunciationChange={(candidate_pronunciation) =>
          onPreferencesChange({ candidate_pronunciation })
        }
        showEnglishSuggestions={showEnglishSuggestions}
        englishSuggestions={englishSuggestions}
        onEnglishSuggestionsChange={(english_suggestions) =>
          onPreferencesChange({ english_suggestions })
        }
      />
      <TranslationSettingsContent
        candidate={translation.candidate}
        providers={translation.providers}
      />
      {mobilePlatform && <MobileInputAiNotice onOpenAi={() => selectPage("ai")} />}
    </SettingsPageFieldset>
  );
}
