import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { SubPageEntries } from "./sub-page-entries";
import { GroupList, Row, Switch } from "../../core/platform-controls";
import { FuzzyPinyinSection } from "../fuzzy-pinyin-section";
import { InputLanguageOptionsSection } from "../input-language-options-section";
import { PunctuationSection } from "../punctuation-section";
import { TranslationSettingsContent } from "../translation-settings-content";
import { createTranslationSettingsBindings } from "../translation-settings-bindings";

/**
 * The 表达 page: how what is typed comes out -- punctuation, spelling tolerance, the candidates in other languages and the mixed-in English, emoji and kaomoji -- and the AI features that rewrite it, which open as pages of their own from here.
 */
export function ExpressionSettingsPage() {
  const {
    client,
    confirm,
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
    fuzzyPinyin,
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
  return (
    <fieldset disabled={busy} hidden={page !== "expression"} aria-label="表达">
      <div className={settings.groups}>
        <GroupList title="标点">
          <PunctuationSection
            preferences={draft}
            showCharacterWidth={false}
            onChange={onPreferencesChange}
          />
        </GroupList>
        {client.fuzzyPinyin && (
          <GroupList title="拼写纠错">
            <FuzzyPinyinSection
              preferences={fuzzyPinyin}
              onChange={(fuzzy_pinyin) => onPreferencesChange({ fuzzy_pinyin })}
              confirm={confirm}
            />
          </GroupList>
        )}
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
          showEnglishSuggestions={showEnglishSuggestions}
          englishSuggestions={englishSuggestions}
          onEnglishSuggestionsChange={(english_suggestions) =>
            onPreferencesChange({ english_suggestions })
          }
        />
        <TranslationSettingsContent
          {...createTranslationSettingsBindings({
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
          })}
        />
        {mobilePlatform && (
          <GroupList title="高情商回复">
            <Row
              title="高情商回复"
              description="复制对方的话，切换到高情商回复键盘，点“粘贴”后选择回复风格。支持帮你回、帮润色和换一句，点选回复插入聊天输入框。"
            >
              <button type="button" className="secondary" onClick={() => selectPage("ai")}>
                配置键盘 AI
              </button>
            </Row>
          </GroupList>
        )}
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
