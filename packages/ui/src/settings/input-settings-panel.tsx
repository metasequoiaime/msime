import type { Dispatch, SetStateAction } from "react";
import type { Preferences, ProviderCredentialStatus, SettingsClient } from "../index";
import type { useProviderCredentials } from "./use-provider-credentials";
import type { useTranslationSettings } from "./use-translation-settings";
import type { useCustomTranslations } from "./use-custom-translations";
import type { SettingsSaveState } from "./use-settings-persistence";
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
import { NiuTransSettingsSection } from "./niutrans-settings-section";
import { LinuxTencentCredentialsSection } from "./linux-tencent-credentials-section";
import { TencentTranslationSection } from "./tencent-translation-section";
import { CustomTranslationSettingsSection } from "./custom-translation-settings-section";
import { OnDeviceTranslationNotice } from "./on-device-translation-notice";
import { defaultWordCharacter } from "./word-character-section";
import { FuzzyPinyinSection, defaultFuzzyPinyin } from "./fuzzy-pinyin-section";
import { PunctuationSection } from "./punctuation-section";
import { MixedInputSection, defaultMixedInput } from "./mixed-input-section";
import { defaultFrequency } from "./frequency-section";
import { InputSharedSettingsSection } from "./input-shared-settings-section";
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
  updateTencentCredentialInput: ReturnType<
    typeof useProviderCredentials
  >["updateTencentCredentialInput"];
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
  customTranslationsSaveState: SettingsSaveState;
  customTranslationsSaveError: string;
  customTranslationsPlaceholder: string;
  flushCustomTranslations: () => Promise<void>;
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
  updateTencentCredentialInput,
  providerCredentialBusy,
  providerCredentialMessages,
  runProviderCredential,
  credentialTestControl,
  customTranslationsText,
  setCustomTranslationsText,
  customTranslationsNotice,
  customTranslationsSummary,
  customTranslationsSaveState,
  customTranslationsSaveError,
  customTranslationsPlaceholder,
  flushCustomTranslations,
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
          <NiuTransSettingsSection
            enabled={niutrans.enabled}
            available={candidateTranslations}
            appId={niutrans.app_id}
            apiKey={niutrans.apikey}
            onToggle={(enabled) => setTranslationProvider(enabled ? "niutrans" : "none")}
            onAppIdChange={(app_id) => onPreferencesChange({ niutrans: { ...niutrans, app_id } })}
            onApiKeyChange={(apikey) => onPreferencesChange({ niutrans: { ...niutrans, apikey } })}
            credentialTest={credentialTestControl(
              "translation.niutrans",
              "测试 NiuTrans 配置",
              niutransCredentialTestConfig(niutrans),
              niutransCredentialTestDisabled(candidateTranslations, niutrans),
            )}
          />
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
              onInputChange={(patch) => updateTencentCredentialInput(patch)}
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
          <CustomTranslationSettingsSection
            mobile={mobilePlatform}
            customTranslationsAvailable={Boolean(client.customTranslations)}
            customTranslationsText={customTranslationsText}
            customTranslationsPlaceholder={customTranslationsPlaceholder}
            customTranslationsNotice={customTranslationsNotice}
            customTranslationsSummary={customTranslationsSummary}
            customTranslationsSaveState={customTranslationsSaveState}
            customTranslationsSaveError={customTranslationsSaveError}
            onCustomTranslationsChange={setCustomTranslationsText}
            onFlushCustomTranslations={() => void flushCustomTranslations()}
            customTranslation={customTranslation}
            candidateTranslations={candidateTranslations}
            onPreferencesChange={onPreferencesChange}
            credentialTest={credentialTestControl(
              "translation.custom",
              "测试自定义翻译配置",
              customTranslationCredentialTestConfig(customTranslation),
              customTranslationCredentialTestDisabled(
                candidateTranslations,
                translationEndpointIssue(customTranslation.endpoint),
              ),
            )}
          />
        </>
      )}
      <InputSharedSettingsSection
        preferences={draft}
        wordCharacter={wordCharacter}
        navigation={draft.navigation ?? defaultNavigation}
        frequency={frequency}
        ios={iosPlatform}
        showInputModeHUD={showInputModeHUD && !macosPlatform}
        showModeScope={showModeScope}
        showCandidateEnglishGloss={Boolean(client.candidateEnglishGloss)}
        showEnglishSuggestions={showEnglishSuggestions}
        beforeLearning={
          client.fuzzyPinyin ? (
            <FuzzyPinyinSection
              preferences={fuzzyPinyin}
              onChange={(fuzzy_pinyin) => onPreferencesChange({ fuzzy_pinyin })}
              confirm={confirm}
            />
          ) : null
        }
        beforeLanguage={
          <>
            <PunctuationSection
              preferences={draft}
              showCharacterWidth={showCharacterWidth}
              onChange={onPreferencesChange}
            />
            <MixedInputSection
              preferences={mixedInput}
              onChange={(mixed_input) => onPreferencesChange({ mixed_input })}
            />
          </>
        }
        onPreferencesChange={onPreferencesChange}
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
