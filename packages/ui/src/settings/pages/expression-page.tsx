import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { SubPageEntries } from "./sub-page-entries";
import { GroupList } from "../../core/platform-controls";
import { InputLanguageOptionsSection } from "../input-language-options-section";
import { CustomTranslationsSection } from "../custom-translations-section";
import { PunctuationSection } from "../punctuation-section";
import { TranslationSettingsContent } from "../translation-settings-content";
import { createTranslationSettingsBindings } from "../translation-settings-bindings";
import { MobileInputAiNotice } from "../mobile-input-ai-notice";

/**
 * 标点与翻译页：打出来的内容以什么形式出现——标点、其他语言的候选、混入的英文与表情颜文字——以及改写它的 AI 功能，后者从这里进入各自的页面。模糊音改的是拼音怎么解析，在输入页。
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
    mixedInput,
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
          includeMixed
          mixedInput={mixedInput}
          onMixedInputChange={(mixed_input) => onPreferencesChange({ mixed_input })}
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
        <SubPageEntries
          title="AI"
          pages={[
            { id: "ai", description: "联想、回复与润色使用的模型服务" },
            { id: "chat", description: "与 AI 对话，结果可以直接用于输入" },
          ]}
        />
      </div>
    </fieldset>
  );
}
