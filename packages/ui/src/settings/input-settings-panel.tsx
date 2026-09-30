import type { Dispatch, SetStateAction } from "react";
import type { Preferences, ProviderCredentialStatus, SettingsClient } from "../index";
import type { useProviderCredentials } from "./use-provider-credentials";
import type { useTranslationSettings } from "./use-translation-settings";
import type { useCustomTranslations } from "./use-custom-translations";
import { HandwritingPlatformNotice } from "./handwriting-platform-notice";
import { MobileInputAiNotice } from "./mobile-input-ai-notice";
import { InputModeSection } from "./input-mode-section";
import { TouchKeyboardSchemesSection } from "./touch-keyboard-schemes-section";
import { touchKeyboardSchemeOptions } from "./touch-keyboard-scheme-helpers";
import {
  InputSchemeSelectorSection,
  type InputSchemeSelectorValue,
} from "./input-scheme-selector-section";
import { InputSchemeDetailsSection, type ShuangpinProfile } from "./input-scheme-details-section";
import { WubiSection } from "./wubi-section";
import { NavigationSection, defaultNavigation } from "./navigation-section";
import {
  CandidateTranslationOptionsSection,
  type TranslationLanguage,
  type TranslationSecondaryLanguage,
} from "./candidate-translation-options-section";
import {
  TranslationServiceSelectorSection,
  type TranslationProvider,
} from "./translation-service-selector-section";
import { NiuTransSection } from "./niutrans-section";
import { LinuxTencentCredentialsSection } from "./linux-tencent-credentials-section";
import { TencentTranslationSection } from "./tencent-translation-section";
import { CustomTranslationsSection } from "./custom-translations-section";
import { CustomTranslationSection } from "./custom-translation-section";
import { OnDeviceTranslationNotice } from "./on-device-translation-notice";
import { WordCharacterSection, defaultWordCharacter } from "./word-character-section";
import { FuzzyPinyinSection, defaultFuzzyPinyin } from "./fuzzy-pinyin-section";
import { LearningSection } from "./learning-section";
import { PunctuationSection } from "./punctuation-section";
import { MixedInputSection, defaultMixedInput } from "./mixed-input-section";
import { InputModeHudSection } from "./input-mode-hud-section";
import { CandidateEnglishGlossSection } from "./candidate-english-gloss-section";
import { EnglishSuggestionsSection } from "./english-suggestions-section";
import { DefaultImeModeSection } from "./default-ime-mode-section";
import { ImeModeScopeSection } from "./ime-mode-scope-section";
import { TraditionalChineseOutputSection } from "./traditional-chinese-output-section";
import { CloudCandidatesSection } from "./cloud-candidates-section";
import { FrequencySection, defaultFrequency } from "./frequency-section";
import {
  MobileKeyboardFeedbackSection,
  type MobileKeyboardFeedback,
} from "./mobile-keyboard-feedback-section";
import { tencentCredentialIssue, translationEndpointIssue } from "./translation-validation";
import { tencentSecretConfigured } from "./credential-utils";
import {
  customTranslationCredentialTestConfig,
  customTranslationCredentialTestDisabled,
  niutransCredentialTestConfig,
  niutransCredentialTestDisabled,
  tencentTranslationCredentialTestConfig,
  tencentTranslationCredentialTestDisabled,
} from "./translation-credential-test-config";
import type { TouchKeyboardScheme as TouchKeyboardSchemePreference } from "./touch-keyboard-scheme-helpers";
import type { FuzzyPinyinPreferences } from "./fuzzy-pinyin-section";
import type { MixedInputPreferences } from "./mixed-input-section";
import type { FrequencyPreferences } from "./frequency-section";
import type { NavigationPreferences, WordCharacterPreferences } from "./word-character-section";
import { createSettingsDraftActions } from "./settings-draft-actions";

export interface InputSettingsPanelProps {
  disabled: boolean;
  hidden: boolean;
  client: SettingsClient;
  draft: Preferences;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  confirm: (request: {
    message: string;
    title?: string;
    confirmLabel?: string;
    danger?: boolean;
  }) => Promise<boolean>;
  onOpenAi: () => void;
  openExternalUrl: (url: string) => Promise<void>;
  onError: (error: string) => void;
  iosPlatform: boolean;
  harmonyPlatform: boolean;
  androidPlatform: boolean;
  mobilePlatform: boolean;
  macosPlatform: boolean;
  linuxPlatform: boolean;
  windowsPlatform: boolean;
  touchKeyboardSchemes: NonNullable<Preferences["touch_keyboard_schemes"]>;
  selectedTouchKeyboardScheme: TouchKeyboardSchemePreference;
  selectTouchKeyboardScheme: (scheme: TouchKeyboardSchemePreference) => void;
  setTouchKeyboardSchemeEnabled: (scheme: TouchKeyboardSchemePreference, enabled: boolean) => void;
  macosShuangpinKeymap: boolean | undefined;
  setShuangpinKeymap: Dispatch<SetStateAction<boolean | undefined>>;
  macosWubiAutoCommitUnique: boolean | undefined;
  setWubiAutoCommitUnique: Dispatch<SetStateAction<boolean | undefined>>;
  wordCharacter: WordCharacterPreferences;
  candidateTranslations: ReturnType<typeof useTranslationSettings>["candidateTranslations"];
  candidateGlossLanguagesEnabled: ReturnType<
    typeof useTranslationSettings
  >["candidateGlossLanguagesEnabled"];
  translationTargetLanguage: ReturnType<typeof useTranslationSettings>["translationTargetLanguage"];
  translationSecondaryLanguage: ReturnType<
    typeof useTranslationSettings
  >["translationSecondaryLanguage"];
  visibleTranslationLanguages: ReturnType<
    typeof useTranslationSettings
  >["visibleTranslationLanguages"];
  visibleSecondaryLanguages: ReturnType<typeof useTranslationSettings>["visibleSecondaryLanguages"];
  customTranslation: ReturnType<typeof useTranslationSettings>["customTranslation"];
  tencentTranslation: ReturnType<typeof useTranslationSettings>["tencentTranslation"];
  niutrans: ReturnType<typeof useTranslationSettings>["niutrans"];
  translationProvider: TranslationProvider;
  onDeviceMissingLanguages: ReturnType<typeof useTranslationSettings>["onDeviceMissingLanguages"];
  setTranslationProvider: (provider: TranslationProvider) => void;
  providerCredentials: ProviderCredentialStatus | undefined;
  tencentCredentialInput: ReturnType<typeof useProviderCredentials>["tencentCredentialInput"];
  setTencentCredentialInput: ReturnType<typeof useProviderCredentials>["setTencentCredentialInput"];
  providerCredentialBusy: ReturnType<typeof useProviderCredentials>["providerCredentialBusy"];
  providerCredentialMessages: ReturnType<
    typeof useProviderCredentials
  >["providerCredentialMessages"];
  runProviderCredential: ReturnType<typeof useProviderCredentials>["runProviderCredential"];
  credentialTestControl: ReturnType<typeof useProviderCredentials>["credentialTestControl"];
  customTranslationsText: string;
  setCustomTranslationsText: (value: string) => void;
  customTranslationsNotice: string;
  customTranslationsSummary: string;
  customTranslationsBusy: boolean;
  customTranslationsPlaceholder: string;
  saveCustomTranslations: () => Promise<void>;
  fuzzyPinyin: FuzzyPinyinPreferences;
  mixedInput: MixedInputPreferences;
  frequency: FrequencyPreferences;
  showCharacterWidth: boolean;
  showInputModeHUD: boolean;
  showEnglishSuggestions: boolean;
  showModeScope: boolean;
  mobileKeyboardFeedback: MobileKeyboardFeedback | undefined;
  mobileKeyboardFeedbackBusy: boolean;
  saveMobileKeyboardFeedback: (value: MobileKeyboardFeedback) => Promise<void>;
  previewMobileKeyboardHaptics: () => Promise<void>;
}

/** Complete shared input and translation settings page; state remains owned by SettingsPage. */
export function InputSettingsPanel({
  disabled,
  hidden,
  client,
  draft,
  setDraft,
  confirm,
  onOpenAi,
  openExternalUrl,
  onError,
  iosPlatform,
  harmonyPlatform,
  androidPlatform,
  mobilePlatform,
  macosPlatform,
  linuxPlatform,
  windowsPlatform,
  touchKeyboardSchemes,
  selectedTouchKeyboardScheme,
  selectTouchKeyboardScheme,
  setTouchKeyboardSchemeEnabled,
  macosShuangpinKeymap,
  setShuangpinKeymap,
  macosWubiAutoCommitUnique,
  setWubiAutoCommitUnique,
  wordCharacter,
  candidateTranslations,
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
  setTranslationProvider,
  providerCredentials,
  tencentCredentialInput,
  setTencentCredentialInput,
  providerCredentialBusy,
  providerCredentialMessages,
  runProviderCredential,
  credentialTestControl,
  customTranslationsText,
  setCustomTranslationsText,
  customTranslationsNotice,
  customTranslationsSummary,
  customTranslationsBusy,
  customTranslationsPlaceholder,
  saveCustomTranslations,
  fuzzyPinyin,
  mixedInput,
  frequency,
  showCharacterWidth,
  showInputModeHUD,
  showEnglishSuggestions,
  showModeScope,
  mobileKeyboardFeedback,
  mobileKeyboardFeedbackBusy,
  saveMobileKeyboardFeedback,
  previewMobileKeyboardHaptics,
}: InputSettingsPanelProps) {
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="输入">
      {iosPlatform && (
        <HandwritingPlatformNotice
          platform="ios"
          onOpenExternalUrl={client.openExternalUrl ? openExternalUrl : undefined}
        />
      )}
      {harmonyPlatform && <HandwritingPlatformNotice platform="harmony" />}
      {androidPlatform && (
        <HandwritingPlatformNotice
          platform="android"
          onOpenExternalUrl={client.openExternalUrl ? openExternalUrl : undefined}
        />
      )}
      {mobilePlatform && <MobileInputAiNotice onOpenAi={onOpenAi} />}
      {!client.touchKeyboardSchemes && (
        <InputModeSection
          scheme={draft.scheme}
          lastChineseScheme={draft.last_chinese_scheme}
          onChange={onPreferencesChange}
        />
      )}
      {client.touchKeyboardSchemes && (
        <TouchKeyboardSchemesSection
          options={touchKeyboardSchemeOptions}
          enabled={touchKeyboardSchemes.enabled}
          selected={selectedTouchKeyboardScheme}
          onSelect={(scheme) => selectTouchKeyboardScheme(scheme as TouchKeyboardSchemePreference)}
          onToggle={(scheme, enabled) =>
            setTouchKeyboardSchemeEnabled(scheme as TouchKeyboardSchemePreference, enabled)
          }
        />
      )}
      <div hidden={client.touchKeyboardSchemes || draft.scheme === "japanese"}>
        <InputSchemeSelectorSection
          value={draft.scheme === "japanese" ? "quanpin" : draft.scheme}
          onChange={(scheme: InputSchemeSelectorValue) =>
            onPreferencesChange({ scheme, last_chinese_scheme: scheme })
          }
        />
      </div>
      <InputSchemeDetailsSection
        scheme={draft.scheme}
        shuangpinProfile={draft.shuangpin_profile}
        macos={macosPlatform}
        hasTouchKeyboardSchemes={client.touchKeyboardSchemes ?? false}
        macosShuangpinKeymap={
          macosPlatform && client.loadMacosShuangpinKeymap && macosShuangpinKeymap !== undefined
            ? macosShuangpinKeymap
            : undefined
        }
        onShuangpinProfileChange={(shuangpin_profile: ShuangpinProfile) =>
          onPreferencesChange({ shuangpin_profile })
        }
        onMacosShuangpinKeymapChange={setShuangpinKeymap}
      />
      {((client.touchKeyboardSchemes && touchKeyboardSchemes.enabled.includes("wubi")) ||
        draft.scheme === "wubi") && (
        <WubiSection
          preferences={draft}
          autoCommitUnique={macosPlatform ? macosWubiAutoCommitUnique : undefined}
          onChange={onPreferencesChange}
          onAutoCommitUniqueChange={setWubiAutoCommitUnique}
        />
      )}
      <NavigationSection
        navigation={draft.navigation ?? defaultNavigation}
        wordCharacter={wordCharacter}
        linux={linuxPlatform}
        onChange={({ navigation, wordCharacter: nextWordCharacter }) =>
          onPreferencesChange({
            navigation,
            word_character: nextWordCharacter,
          })
        }
      />
      <CandidateTranslationOptionsSection
        enabled={candidateTranslations}
        targetLanguage={translationTargetLanguage}
        secondaryLanguage={translationSecondaryLanguage ?? ""}
        candidateGlossLanguagesEnabled={candidateGlossLanguagesEnabled}
        visibleLanguages={visibleTranslationLanguages}
        visibleSecondaryLanguages={visibleSecondaryLanguages}
        showSecondaryLanguage={androidPlatform || iosPlatform || macosPlatform || harmonyPlatform}
        showAccountTranslation={androidPlatform}
        accountTranslation={draft.translation_account ?? false}
        onEnabledChange={(candidate_translations) =>
          onPreferencesChange({ candidate_translations })
        }
        onTargetLanguageChange={(translation_target_language: TranslationLanguage) =>
          onPreferencesChange({ translation_target_language })
        }
        onSecondaryLanguageChange={(value: TranslationSecondaryLanguage) =>
          onPreferencesChange({
            translation_secondary_language: value === "" ? null : value,
          })
        }
        onAccountTranslationChange={(enabled) =>
          enabled
            ? setTranslationProvider("account")
            : onPreferencesChange({ translation_account: undefined })
        }
      />
      {onDeviceMissingLanguages.length > 0 && (
        <OnDeviceTranslationNotice
          languages={onDeviceMissingLanguages.map(([, label]) => label)}
          openSettings={client.onDeviceTranslation?.openSettings}
          onError={onError}
        />
      )}
      {!androidPlatform && (
        <>
          <TranslationServiceSelectorSection
            available={candidateTranslations}
            provider={translationProvider}
            showAccountProvider={macosPlatform || linuxPlatform}
            onChange={setTranslationProvider}
          />
          <NiuTransSection
            enabled={niutrans.enabled}
            available={candidateTranslations}
            appId={niutrans.app_id}
            apiKey={niutrans.apikey}
            onToggle={(enabled) => setTranslationProvider(enabled ? "niutrans" : "none")}
            onAppIdChange={(app_id) => onPreferencesChange({ niutrans: { ...niutrans, app_id } })}
            onApiKeyChange={(apikey) => onPreferencesChange({ niutrans: { ...niutrans, apikey } })}
          >
            {credentialTestControl(
              "translation.niutrans",
              "测试 NiuTrans 配置",
              niutransCredentialTestConfig(niutrans),
              niutransCredentialTestDisabled(candidateTranslations, niutrans),
            )}
          </NiuTransSection>
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
                  !candidateTranslations,
                )}
            </LinuxTencentCredentialsSection>
          ) : (
            <TencentTranslationSection
              enabled={tencentTranslation.enabled}
              available={candidateTranslations}
              secretId={tencentTranslation.secret_id}
              secretKey={tencentTranslation.secret_key}
              region={tencentTranslation.region}
              credentialIssue={tencentCredentialIssue(
                tencentTranslation.secret_id,
                tencentTranslation.secret_key,
                tencentTranslation.region,
              )}
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
                onPreferencesChange({
                  tencent_tmt: { ...tencentTranslation, secret_id },
                })
              }
              onSecretKeyChange={(secret_key) =>
                onPreferencesChange({
                  tencent_tmt: { ...tencentTranslation, secret_key },
                })
              }
              onRegionChange={(region) =>
                onPreferencesChange({
                  tencent_tmt: { ...tencentTranslation, region },
                })
              }
            >
              {(windowsPlatform || macosPlatform) &&
                tencentTranslation.enabled &&
                credentialTestControl(
                  "translation.tencent",
                  "测试腾讯云翻译配置",
                  tencentTranslationCredentialTestConfig(tencentTranslation),
                  tencentTranslationCredentialTestDisabled(
                    candidateTranslations,
                    tencentCredentialIssue(
                      tencentTranslation.secret_id,
                      tencentTranslation.secret_key,
                      tencentTranslation.region,
                    ),
                  ),
                )}
            </TencentTranslationSection>
          )}
          {client.customTranslations && (
            <CustomTranslationsSection
              mobile={mobilePlatform}
              value={customTranslationsText}
              placeholder={customTranslationsPlaceholder}
              notice={customTranslationsNotice}
              summary={customTranslationsSummary}
              busy={customTranslationsBusy}
              onChange={(value) => {
                setCustomTranslationsText(value);
              }}
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
        </>
      )}
      <WordCharacterSection
        preferences={wordCharacter}
        navigation={draft.navigation ?? defaultNavigation}
        ios={iosPlatform}
        onChange={({ wordCharacter: nextWordCharacter, navigation }) =>
          onPreferencesChange({
            word_character: nextWordCharacter,
            navigation,
          })
        }
      />
      {client.fuzzyPinyin && (
        <FuzzyPinyinSection
          preferences={fuzzyPinyin}
          onChange={(fuzzy_pinyin) => onPreferencesChange({ fuzzy_pinyin })}
          confirm={confirm}
        />
      )}
      <LearningSection
        value={draft.learning}
        onChange={(learning) => onPreferencesChange({ learning })}
      />
      <PunctuationSection
        preferences={draft}
        showCharacterWidth={showCharacterWidth}
        onChange={onPreferencesChange}
      />
      <MixedInputSection
        preferences={mixedInput}
        onChange={(mixed_input) => onPreferencesChange({ mixed_input })}
      />
      {/* macOS keeps this with the chords that trigger it, on the shortcut page. */}
      {showInputModeHUD && !macosPlatform && (
        <InputModeHudSection
          value={draft.input_mode_hud}
          onChange={(input_mode_hud) => onPreferencesChange({ input_mode_hud })}
        />
      )}
      {client.candidateEnglishGloss && (
        <CandidateEnglishGlossSection
          value={draft.candidate_english_gloss}
          onChange={(candidate_english_gloss) => onPreferencesChange({ candidate_english_gloss })}
        />
      )}
      {showEnglishSuggestions && (
        <EnglishSuggestionsSection
          value={draft.english_suggestions}
          onChange={(english_suggestions) => onPreferencesChange({ english_suggestions })}
        />
      )}
      <DefaultImeModeSection
        value={draft.default_ime_mode}
        onChange={(default_ime_mode) => onPreferencesChange({ default_ime_mode })}
      />
      {showModeScope && (
        <ImeModeScopeSection
          value={draft.ime_mode_scope}
          onChange={(ime_mode_scope) => onPreferencesChange({ ime_mode_scope })}
        />
      )}
      <TraditionalChineseOutputSection
        value={draft.traditional_chinese_output}
        onChange={(traditional_chinese_output) =>
          onPreferencesChange({ traditional_chinese_output })
        }
      />
      <CloudCandidatesSection
        value={draft.cloud_candidates}
        onChange={(cloud_candidates) => onPreferencesChange({ cloud_candidates })}
      />
      <FrequencySection
        preferences={frequency}
        onChange={(frequency) => onPreferencesChange({ frequency })}
      />
      {mobilePlatform && client.mobileKeyboardFeedback && mobileKeyboardFeedback && (
        <MobileKeyboardFeedbackSection
          value={mobileKeyboardFeedback}
          busy={mobileKeyboardFeedbackBusy}
          ios={iosPlatform}
          canPreview={Boolean(client.mobileKeyboardFeedback.preview)}
          onChange={(next) => void saveMobileKeyboardFeedback(next)}
          onPreview={() => void previewMobileKeyboardHaptics()}
        />
      )}
    </fieldset>
  );
}
