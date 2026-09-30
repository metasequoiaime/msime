import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { SubPageEntries } from "./sub-page-entries";
import { GroupList, Row, Select, Switch } from "../../core/platform-controls";
import { FuzzyPinyinSection } from "../fuzzy-pinyin-section";
import { MixedInputSection } from "../mixed-input-section";
import { CandidateEnglishGlossSection } from "../candidate-english-gloss-section";
import { EnglishSuggestionsSection } from "../english-suggestions-section";
import { PunctuationSection } from "../punctuation-section";
import { NiuTransSection } from "../niutrans-section";
import { TencentTranslationSection } from "../tencent-translation-section";
import { CustomTranslationsSection } from "../custom-translations-section";
import { CustomTranslationSection } from "../custom-translation-section";
import { LinuxTencentCredentialsSection } from "../linux-tencent-credentials-section";
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
    customTranslationsBusy,
    customTranslationsSummary,
    providerCredentials,
    tencentCredentialInput,
    setTencentCredentialInput,
    providerCredentialBusy,
    saveCustomTranslations,
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
        <GroupList title="多语言候选">
          <MixedInputSection
            preferences={mixedInput}
            onChange={(mixed_input) => onPreferencesChange({ mixed_input })}
          />
          {client.candidateEnglishGloss && (
            <CandidateEnglishGlossSection
              value={candidateEnglishGloss}
              onChange={(candidate_english_gloss) =>
                onPreferencesChange({ candidate_english_gloss })
              }
            />
          )}
          {showEnglishSuggestions && (
            <EnglishSuggestionsSection
              value={englishSuggestions}
              onChange={(english_suggestions) => onPreferencesChange({ english_suggestions })}
            />
          )}
        </GroupList>
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
            <div role="group" aria-label="候选词翻译服务" className={settings.rowStack}>
              <Row title="翻译服务">
                <Select
                  aria-label="候选词翻译服务"
                  disabled={translationControlsDisabled}
                  value={translationProvider}
                  onChange={(event) =>
                    setTranslationProvider(
                      event.target.value as "none" | "custom" | "tencent" | "niutrans" | "account",
                    )
                  }
                >
                  <option value="none">关闭</option>
                  <option value="tencent">腾讯云机器翻译</option>
                  <option value="niutrans">小牛翻译（NiuTrans）</option>
                  <option value="custom">自定义 DeepLX 兼容服务</option>
                  {(macosPlatform || linuxPlatform) && (
                    <option value="account">水杉账号（候选词发送到 api.msime.app）</option>
                  )}
                </Select>
              </Row>
            </div>
          )}
        </GroupList>
        {!androidPlatform && (
          <>
            <GroupList title="小牛翻译">
              <NiuTransSection
                enabled={niutrans.enabled}
                available={candidateTranslations}
                appId={niutrans.app_id}
                apiKey={niutrans.apikey}
                onToggle={(enabled) => setTranslationProvider(enabled ? "niutrans" : "none")}
                onAppIdChange={(app_id) =>
                  onPreferencesChange({ niutrans: { ...niutrans, app_id } })
                }
                onApiKeyChange={(apikey) =>
                  onPreferencesChange({ niutrans: { ...niutrans, apikey } })
                }
              >
                {credentialTestControl(
                  "translation.niutrans",
                  "测试 NiuTrans 配置",
                  niutransCredentialTestConfig(niutrans),
                  niutransCredentialTestDisabled(candidateTranslations, niutrans),
                )}
              </NiuTransSection>
            </GroupList>
            <GroupList title="腾讯云机器翻译">
              {linuxPlatform ? (
                <LinuxTencentCredentialsSection
                  available={Boolean(client.providerCredentials)}
                  status={
                    providerCredentials
                      ? {
                          tencent: providerCredentials.tencent,
                          tencentInvalid: providerCredentials.tencentInvalid,
                        }
                      : undefined
                  }
                  input={tencentCredentialInput}
                  busy={providerCredentialBusy === "tencent"}
                  message={providerCredentialMessages.tencent}
                  onInputChange={(patch) =>
                    setTencentCredentialInput({ ...tencentCredentialInput, ...patch })
                  }
                  onSave={(credential) =>
                    void runProviderCredential(
                      "tencent",
                      (credentials) => credentials.saveTencent(credential),
                      "凭据已保存，provider 服务下次请求时生效。",
                    )
                  }
                  onClear={() =>
                    void runProviderCredential(
                      "tencent",
                      (credentials) => credentials.clearTencent(),
                      "凭据已清除。",
                    )
                  }
                >
                  {translationProvider === "tencent" &&
                    credentialTestControl(
                      "translation.tencent",
                      "测试腾讯云翻译配置",
                      {},
                      translationControlsDisabled,
                    )}
                </LinuxTencentCredentialsSection>
              ) : (
                <TencentTranslationSection
                  enabled={tencentTranslation.enabled}
                  available={candidateTranslations}
                  secretId={tencentTranslation.secret_id}
                  secretKey={tencentTranslation.secret_key}
                  region={tencentTranslation.region}
                  credentialIssue={tencentIssue}
                  showMissingCredentialsWarning={
                    !customTranslation.enabled &&
                    !tencentSecretConfigured(tencentTranslation.secret_id) &&
                    !tencentSecretConfigured(tencentTranslation.secret_key)
                  }
                  onToggle={(enabled) =>
                    onPreferencesChange({
                      tencent_tmt: { ...tencentTranslation, enabled },
                      // Turning on a service of the user's own ends the account choice, so the account never keeps receiving candidates behind a visible selection.
                      ...(enabled ? { translation_account: undefined } : {}),
                    })
                  }
                  onSecretIdChange={(secret_id) =>
                    onPreferencesChange({ tencent_tmt: { ...tencentTranslation, secret_id } })
                  }
                  onSecretKeyChange={(secret_key) =>
                    onPreferencesChange({ tencent_tmt: { ...tencentTranslation, secret_key } })
                  }
                  onRegionChange={(region) =>
                    onPreferencesChange({ tencent_tmt: { ...tencentTranslation, region } })
                  }
                >
                  {(windowsPlatform || macosPlatform) &&
                    credentialTestControl(
                      "translation.tencent",
                      "测试腾讯云翻译配置",
                      tencentTranslationCredentialTestConfig(tencentTranslation),
                      tencentTranslationCredentialTestDisabled(candidateTranslations, tencentIssue),
                    )}
                </TencentTranslationSection>
              )}
            </GroupList>
            <GroupList title="自定义服务">
              {client.customTranslations && (
                <CustomTranslationsSection
                  mobile={mobilePlatform}
                  value={customTranslationsText}
                  placeholder={customTranslationsPlaceholder}
                  notice={customTranslationsNotice}
                  summary={customTranslationsSummary}
                  busy={customTranslationsBusy}
                  onChange={setCustomTranslationsText}
                  onSave={() => void saveCustomTranslations()}
                />
              )}
              <CustomTranslationSection
                enabled={customTranslation.enabled}
                available={candidateTranslations}
                endpoint={customTranslation.endpoint}
                apiKey={customTranslation.api_key}
                endpointIssue={translationEndpointIssue(customTranslation.endpoint)}
                onToggle={(enabled) =>
                  onPreferencesChange({
                    custom_translation: { ...customTranslation, enabled },
                    // Same rule as the Tencent switch: a service of the user's own ends the account choice.
                    ...(enabled ? { translation_account: undefined } : {}),
                  })
                }
                onEndpointChange={(endpoint) =>
                  onPreferencesChange({
                    custom_translation: { ...customTranslation, endpoint },
                  })
                }
                onApiKeyChange={(api_key) =>
                  onPreferencesChange({
                    custom_translation: { ...customTranslation, api_key },
                  })
                }
              >
                {credentialTestControl(
                  "translation.custom",
                  "测试自定义翻译配置",
                  customTranslationCredentialTestConfig(customTranslation),
                  customTranslationCredentialTestDisabled(
                    candidateTranslations,
                    translationEndpointIssue(customTranslation.endpoint),
                  ),
                )}
              </CustomTranslationSection>
            </GroupList>
          </>
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
