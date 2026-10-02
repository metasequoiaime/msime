import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { GroupList } from "../../core/platform-controls";
import { InputLanguageOptionsSection } from "../input-language-options-section";
import { CustomTranslationsSection } from "../custom-translations-section";
import { PunctuationSection } from "../punctuation-section";
import { TranslationSettingsContent } from "../translation-settings-content";
import { createTranslationSettingsBindings } from "../translation-settings-bindings";
import { MobileInputAiNotice } from "../mobile-input-ai-notice";

/**
 * 标点与翻译页：打出来的内容以什么形式出现——标点、其他语言的候选与释义、翻译服务。混入的英文与表情颜文字是候选来源，模糊音改的是拼音怎么解析，两者都在输入页；AI 功能在「工具」组的「AI 辅助」页。
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
    customTranslationsText,
    setCustomTranslationsText,
    customTranslationsNotice,
    customTranslationsPlaceholder,
    customTranslationsSaveState,
    customTranslationsSaveError,
    customTranslationsSummary,
    providerCredentials,
    tencentCredentialInput,
    updateTencentCredentialInput,
    providerCredentialBusy,
    flushCustomTranslations,
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
  } = useSettingsForm();
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const translation = createTranslationSettingsBindings({
    grouped: true,
    client,
    candidateTranslations,
    mobile: mobilePlatform,
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
    customTranslationsText,
    customTranslationsPlaceholder,
    customTranslationsNotice,
    customTranslationsSummary,
    customTranslationsSaveState,
    customTranslationsSaveError,
    onCustomTranslationsChange: setCustomTranslationsText,
    onFlushCustomTranslations: () => void flushCustomTranslations(),
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
  return (
    <fieldset disabled={busy} hidden={page !== "expression"} aria-label="标点与翻译">
      <div className={settings.groups}>
        <GroupList title="标点">
          <PunctuationSection
            preferences={draft}
            showCharacterWidth={false}
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
          afterCandidateEnglishGloss={
            translation.customGlosses && (
              <CustomTranslationsSection {...translation.customGlosses} />
            )
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
        {mobilePlatform && <MobileInputAiNotice grouped onOpenAi={() => selectPage("ai")} />}
      </div>
    </fieldset>
  );
}
