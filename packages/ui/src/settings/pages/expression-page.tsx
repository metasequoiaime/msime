import type { Preferences } from "../../index";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { CredentialStatusMessage } from "../credential-status-message";
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
import { tencentCredentialIssue, translationEndpointIssue } from "../translation-validation";
import { tencentSecretConfigured } from "../credential-utils";
import {
  customTranslationCredentialTestConfig,
  niutransCredentialTestConfig,
  tencentTranslationCredentialTestConfig,
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
    translationSecondaryLanguage,
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
            onChange={(patch) => setDraft({ ...draft, ...patch })}
          />
        </GroupList>
        {client.fuzzyPinyin && (
          <GroupList title="拼写纠错">
            <FuzzyPinyinSection
              preferences={fuzzyPinyin}
              onChange={(fuzzy_pinyin) => setDraft({ ...draft, fuzzy_pinyin })}
              confirm={confirm}
            />
          </GroupList>
        )}
        <GroupList title="多语言候选">
          <MixedInputSection
            preferences={mixedInput}
            onChange={(mixed_input) => setDraft({ ...draft, mixed_input })}
          />
          {client.candidateEnglishGloss && (
            <CandidateEnglishGlossSection
              value={candidateEnglishGloss}
              onChange={(checked) => setDraft({ ...draft, candidate_english_gloss: checked })}
            />
          )}
          {showEnglishSuggestions && (
            <EnglishSuggestionsSection
              value={englishSuggestions}
              onChange={(checked) => setDraft({ ...draft, english_suggestions: checked })}
            />
          )}
        </GroupList>
        <GroupList title="候选词翻译">
          <Row title="候选词翻译" description="为当前候选请求翻译结果并显示在候选行">
            <Switch
              checked={candidateTranslations}
              onChange={(checked) => setDraft({ ...draft, candidate_translations: checked })}
            />
          </Row>
          <Row title="目标语言">
            <Select
              aria-label="候选词翻译目标语言"
              disabled={!candidateGlossLanguagesEnabled}
              value={translationTargetLanguage}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  translation_target_language: event.target
                    .value as Preferences["translation_target_language"],
                })
              }
            >
              {visibleTranslationLanguages.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </Select>
          </Row>
          {(androidPlatform || iosPlatform || macosPlatform || harmonyPlatform) && (
            <Row title="第二种语言" description="候选词下方可同时显示第二种释义">
              <Select
                aria-label="候选词翻译第二种语言"
                disabled={!candidateGlossLanguagesEnabled}
                value={translationSecondaryLanguage}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    translation_secondary_language:
                      event.target.value === ""
                        ? null
                        : (event.target.value as Preferences["translation_target_language"]),
                  })
                }
              >
                {visibleSecondaryLanguages.map(([value, label]) => (
                  <option key={value || "none"} value={value}>
                    {label}
                  </option>
                ))}
              </Select>
            </Row>
          )}
          {androidPlatform && (
            <Row
              title="使用水杉账号翻译候选词"
              description="当前页的中文候选词会发送到 api.msime.app；匿名账号在 Linux 安装后的用户初始化中自动注册；不开启则不联网翻译"
            >
              <Switch
                disabled={translationControlsDisabled}
                checked={draft.translation_account ?? false}
                onChange={(checked) =>
                  checked
                    ? setTranslationProvider("account")
                    : setDraft({ ...draft, translation_account: undefined })
                }
              />
            </Row>
          )}
          {onDeviceMissingLanguages.length > 0 && (
            <div className={settings.groupBlock}>
              <div role="status" className={settings.groupNote} aria-label="系统翻译语言未下载">
                <p>
                  整句候选暂时没有翻译：macOS 还没有下载「中文（简体）→{" "}
                  {onDeviceMissingLanguages.map(([, label]) => label).join("、")}
                  」翻译语言。离线词库只收词语，「现在几点了」这样的整句要靠系统在本机翻译，不联网。
                </p>
                <p>
                  请在 系统设置 &gt; 通用 &gt; 语言与地区 &gt; 翻译语言
                  中下载，然后回到输入框继续输入即可生效。也可以在下方选择一个在线翻译服务。{" "}
                  <button
                    type="button"
                    className="secondary"
                    onClick={() =>
                      void client.onDeviceTranslation
                        ?.openSettings()
                        .catch(() =>
                          setError(
                            "无法打开系统设置，请手动前往 系统设置 > 通用 > 语言与地区 > 翻译语言。",
                          ),
                        )
                    }
                  >
                    打开语言与地区
                  </button>
                </p>
              </div>
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
                  setDraft({ ...draft, niutrans: { ...niutrans, app_id } })
                }
                onApiKeyChange={(apikey) =>
                  setDraft({ ...draft, niutrans: { ...niutrans, apikey } })
                }
              >
                {credentialTestControl(
                  "translation.niutrans",
                  "测试 NiuTrans 配置",
                  niutransCredentialTestConfig(niutrans),
                  translationControlsDisabled || !niutrans.app_id.trim() || !niutrans.apikey.trim(),
                )}
              </NiuTransSection>
            </GroupList>
            <GroupList title="腾讯云机器翻译">
              {linuxPlatform ? (
                <div role="group" aria-label="在线翻译服务" className={settings.rowStack}>
                  <Row
                    title="在线翻译服务"
                    description="由用户管理的 Linux provider 服务负责网络请求和凭据"
                  />
                  {client.providerCredentials ? (
                    <>
                      <p className={settings.groupNote}>
                        {providerCredentials?.tencentInvalid
                          ? "现有 tencent-provider.json 无效，provider 服务不会发出翻译请求；请修复或删除该文件。"
                          : providerCredentials?.tencent
                            ? "腾讯云凭据已保存；SecretId 和 SecretKey 留空则保留原值。"
                            : "凭据只写入用户配置目录的 tencent-provider.json，由 provider 服务读取，不进入共享设置。"}
                      </p>
                      <Row title="SecretId">
                        <input
                          aria-label="腾讯云 SecretId"
                          type="password"
                          autoComplete="off"
                          value={tencentCredentialInput.secretId}
                          onChange={(event) =>
                            setTencentCredentialInput({
                              ...tencentCredentialInput,
                              secretId: event.target.value,
                            })
                          }
                        />
                      </Row>
                      <Row title="SecretKey">
                        <input
                          aria-label="腾讯云 SecretKey"
                          type="password"
                          autoComplete="off"
                          value={tencentCredentialInput.secretKey}
                          onChange={(event) =>
                            setTencentCredentialInput({
                              ...tencentCredentialInput,
                              secretKey: event.target.value,
                            })
                          }
                        />
                      </Row>
                      <Row title="地域">
                        <input
                          aria-label="腾讯云地域"
                          value={
                            tencentCredentialInput.region ??
                            providerCredentials?.tencent?.region ??
                            "ap-guangzhou"
                          }
                          onChange={(event) =>
                            setTencentCredentialInput({
                              ...tencentCredentialInput,
                              region: event.target.value,
                            })
                          }
                        />
                      </Row>
                      <div className={settings.groupBlock}>
                        <div className={settings.serviceRow}>
                          <div>
                            <button
                              type="button"
                              className="secondary"
                              disabled={
                                providerCredentialBusy === "tencent" ||
                                (!providerCredentials?.tencent &&
                                  (!tencentCredentialInput.secretId.trim() ||
                                    !tencentCredentialInput.secretKey.trim()))
                              }
                              onClick={() =>
                                void runProviderCredential(
                                  "tencent",
                                  (credentials) =>
                                    credentials.saveTencent({
                                      ...(tencentCredentialInput.secretId.trim()
                                        ? { secretId: tencentCredentialInput.secretId }
                                        : {}),
                                      ...(tencentCredentialInput.secretKey.trim()
                                        ? { secretKey: tencentCredentialInput.secretKey }
                                        : {}),
                                      region:
                                        tencentCredentialInput.region ??
                                        providerCredentials?.tencent?.region ??
                                        "ap-guangzhou",
                                    }),
                                  "凭据已保存，provider 服务下次请求时生效。",
                                )
                              }
                            >
                              保存凭据
                            </button>
                            {providerCredentials?.tencent && (
                              <button
                                type="button"
                                className="secondary"
                                disabled={providerCredentialBusy === "tencent"}
                                onClick={() =>
                                  void runProviderCredential(
                                    "tencent",
                                    (credentials) => credentials.clearTencent(),
                                    "凭据已清除。",
                                  )
                                }
                              >
                                清除凭据
                              </button>
                            )}
                            <CredentialStatusMessage message={providerCredentialMessages.tencent} />
                          </div>
                        </div>
                      </div>
                    </>
                  ) : (
                    <p className={settings.groupNote}>
                      候选词翻译开启后，provider 从用户配置目录的 <code>tencent-provider.json</code>{" "}
                      读取腾讯云凭据；设置页不保存不会生效的 SecretId 或 SecretKey。
                    </p>
                  )}
                  {translationProvider === "tencent" && (
                    <div className={settings.groupBlock}>
                      {credentialTestControl(
                        "translation.tencent",
                        "测试腾讯云翻译配置",
                        {},
                        translationControlsDisabled,
                      )}
                    </div>
                  )}
                </div>
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
                    setDraft({
                      ...draft,
                      tencent_tmt: { ...tencentTranslation, enabled },
                      // Turning on a service of the user's own ends the account choice, so the account never keeps receiving candidates behind a visible selection.
                      ...(enabled ? { translation_account: undefined } : {}),
                    })
                  }
                  onSecretIdChange={(secret_id) =>
                    setDraft({ ...draft, tencent_tmt: { ...tencentTranslation, secret_id } })
                  }
                  onSecretKeyChange={(secret_key) =>
                    setDraft({ ...draft, tencent_tmt: { ...tencentTranslation, secret_key } })
                  }
                  onRegionChange={(region) =>
                    setDraft({ ...draft, tencent_tmt: { ...tencentTranslation, region } })
                  }
                >
                  {(windowsPlatform || macosPlatform) &&
                    credentialTestControl(
                      "translation.tencent",
                      "测试腾讯云翻译配置",
                      tencentTranslationCredentialTestConfig(tencentTranslation),
                      translationControlsDisabled || Boolean(tencentIssue),
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
                  setDraft({
                    ...draft,
                    custom_translation: { ...customTranslation, enabled },
                    // Same rule as the Tencent switch: a service of the user's own ends the account choice.
                    ...(enabled ? { translation_account: undefined } : {}),
                  })
                }
                onEndpointChange={(endpoint) =>
                  setDraft({ ...draft, custom_translation: { ...customTranslation, endpoint } })
                }
                onApiKeyChange={(api_key) =>
                  setDraft({ ...draft, custom_translation: { ...customTranslation, api_key } })
                }
              >
                {credentialTestControl(
                  "translation.custom",
                  "测试自定义翻译配置",
                  customTranslationCredentialTestConfig(customTranslation),
                  translationControlsDisabled ||
                    Boolean(translationEndpointIssue(customTranslation.endpoint)),
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
