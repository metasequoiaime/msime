import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { SubPageEntries } from "./sub-page-entries";
import { GroupList, Row, Switch } from "../../core/platform-controls";
import { FuzzyPinyinSection } from "../fuzzy-pinyin-section";
import { InputLanguageOptionsSection } from "../input-language-options-section";
import { PunctuationSection } from "../punctuation-section";
import { TranslationServiceSelectorSection } from "../translation-service-selector-section";
import { TranslationProviderSettingsSection } from "../translation-provider-settings-section";
import { CandidateTranslationOptionsSection } from "../candidate-translation-options-section";
import { tencentCredentialIssue, translationEndpointIssue } from "../translation-validation";
import { tencentSecretConfigured } from "../credential-utils";
import { OnDeviceTranslationNotice } from "../on-device-translation-notice";
import {
  customTranslationCredentialTestConfig,
  customTranslationCredentialTestDisabled,
  niutransCredentialTestConfig,
  niutransCredentialTestDisabled,
  tencentTranslationCredentialTestConfig,
  tencentTranslationCredentialTestDisabled,
} from "../translation-credential-test-config";

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
  const translationControlsDisabled = !candidateTranslations;
  const tencentIssue = tencentCredentialIssue(
    tencentTranslation.secret_id,
    tencentTranslation.secret_key,
    tencentTranslation.region,
  );
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
        <GroupList title="候选词翻译">
          <CandidateTranslationOptionsSection
            enabled={candidateTranslations}
            targetLanguage={translationTargetLanguage}
            secondaryLanguage={draft.translation_secondary_language ?? ""}
            candidateGlossLanguagesEnabled={candidateGlossLanguagesEnabled}
            visibleLanguages={visibleTranslationLanguages}
            visibleSecondaryLanguages={visibleSecondaryLanguages}
            showSecondaryLanguage={
              androidPlatform || iosPlatform || macosPlatform || harmonyPlatform
            }
            showAccountTranslation={androidPlatform}
            accountTranslation={draft.translation_account ?? false}
            onEnabledChange={(candidate_translations) =>
              onPreferencesChange({ candidate_translations })
            }
            onTargetLanguageChange={(translation_target_language) =>
              onPreferencesChange({ translation_target_language })
            }
            onSecondaryLanguageChange={(value) =>
              onPreferencesChange({ translation_secondary_language: value === "" ? null : value })
            }
            onAccountTranslationChange={(enabled) =>
              enabled
                ? setTranslationProvider("account")
                : onPreferencesChange({ translation_account: undefined })
            }
          />
          {onDeviceMissingLanguages.length > 0 && (
            <div className={settings.groupBlock}>
              <OnDeviceTranslationNotice
                languages={onDeviceMissingLanguages.map(([, label]) => label)}
                openSettings={client.onDeviceTranslation?.openSettings}
                onError={setError}
              />
            </div>
          )}
          {!androidPlatform && (
            <TranslationServiceSelectorSection
              grouped
              available={candidateTranslations}
              provider={translationProvider}
              showAccountProvider={macosPlatform || linuxPlatform}
              onChange={setTranslationProvider}
            />
          )}
        </GroupList>
        {!androidPlatform && (
          <TranslationProviderSettingsSection
            grouped
            niutrans={{
              enabled: niutrans.enabled,
              available: candidateTranslations,
              appId: niutrans.app_id,
              apiKey: niutrans.apikey,
              onToggle: (enabled) => setTranslationProvider(enabled ? "niutrans" : "none"),
              onAppIdChange: (app_id) => onPreferencesChange({ niutrans: { ...niutrans, app_id } }),
              onApiKeyChange: (apikey) =>
                onPreferencesChange({ niutrans: { ...niutrans, apikey } }),
              credentialTest: credentialTestControl(
                "translation.niutrans",
                "测试 NiuTrans 配置",
                niutransCredentialTestConfig(niutrans),
                niutransCredentialTestDisabled(candidateTranslations, niutrans),
              ),
            }}
            tencent={{
              linux: linuxPlatform,
              available: candidateTranslations,
              linuxCredentialsAvailable: Boolean(client.providerCredentials),
              enabled: tencentTranslation.enabled,
              secretId: tencentTranslation.secret_id,
              secretKey: tencentTranslation.secret_key,
              region: tencentTranslation.region,
              credentialIssue: tencentIssue,
              showMissingCredentialsWarning:
                !customTranslation.enabled &&
                !tencentSecretConfigured(tencentTranslation.secret_id) &&
                !tencentSecretConfigured(tencentTranslation.secret_key),
              status: providerCredentials
                ? {
                    tencent: providerCredentials.tencent,
                    tencentInvalid: providerCredentials.tencentInvalid,
                  }
                : undefined,
              input: tencentCredentialInput,
              busy: providerCredentialBusy === "tencent",
              message: providerCredentialMessages.tencent,
              onToggle: (enabled) =>
                onPreferencesChange({
                  tencent_tmt: { ...tencentTranslation, enabled },
                  // Turning on a service of the user's own ends the account choice, so the account never keeps receiving candidates behind a visible selection.
                  ...(enabled ? { translation_account: undefined } : {}),
                }),
              onSecretIdChange: (secret_id) =>
                onPreferencesChange({ tencent_tmt: { ...tencentTranslation, secret_id } }),
              onSecretKeyChange: (secret_key) =>
                onPreferencesChange({ tencent_tmt: { ...tencentTranslation, secret_key } }),
              onRegionChange: (region) =>
                onPreferencesChange({ tencent_tmt: { ...tencentTranslation, region } }),
              onInputChange: (patch) => updateTencentCredentialInput(patch),
              onSave: (credential) =>
                void runProviderCredential(
                  "tencent",
                  (credentials) => credentials.saveTencent(credential),
                  "凭据已保存，provider 服务下次请求时生效。",
                ),
              onClear: () =>
                void runProviderCredential(
                  "tencent",
                  (credentials) => credentials.clearTencent(),
                  "凭据已清除。",
                ),
              linuxCredentialTest:
                translationProvider === "tencent" &&
                credentialTestControl(
                  "translation.tencent",
                  "测试腾讯云翻译配置",
                  {},
                  translationControlsDisabled,
                ),
              serviceCredentialTest:
                (windowsPlatform || macosPlatform) &&
                credentialTestControl(
                  "translation.tencent",
                  "测试腾讯云翻译配置",
                  tencentTranslationCredentialTestConfig(tencentTranslation),
                  tencentTranslationCredentialTestDisabled(candidateTranslations, tencentIssue),
                ),
            }}
            custom={{
              mobile: mobilePlatform,
              customTranslationsAvailable: Boolean(client.customTranslations),
              customTranslationsText,
              customTranslationsPlaceholder,
              customTranslationsNotice,
              customTranslationsSummary,
              customTranslationsSaveState,
              customTranslationsSaveError,
              onCustomTranslationsChange: setCustomTranslationsText,
              onFlushCustomTranslations: () => void flushCustomTranslations(),
              customTranslation,
              candidateTranslations,
              onPreferencesChange,
              credentialTest: credentialTestControl(
                "translation.custom",
                "测试自定义翻译配置",
                customTranslationCredentialTestConfig(customTranslation),
                customTranslationCredentialTestDisabled(
                  candidateTranslations,
                  translationEndpointIssue(customTranslation.endpoint),
                ),
              ),
            }}
          />
        )}
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
