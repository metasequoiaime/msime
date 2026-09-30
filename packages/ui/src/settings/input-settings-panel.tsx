import type { Dispatch, SetStateAction } from "react";
import type { Preferences, ProviderCredentialStatus, SettingsClient } from "../index";
import type { useProviderCredentials } from "./use-provider-credentials";
import type { useTranslationSettings } from "./use-translation-settings";
import type { useCustomTranslations } from "./use-custom-translations";
import type { SettingsSaveState } from "./use-settings-persistence";
import { HandwritingPlatformNotice } from "./handwriting-platform-notice";
import { MobileInputAiNotice } from "./mobile-input-ai-notice";
import { InputSchemeSettingsContent } from "./input-scheme-settings-content";
import { NavigationSection, defaultNavigation } from "./navigation-section";
import type { TouchKeyboardScheme as TouchKeyboardSchemePreference } from "./touch-keyboard-scheme-helpers";
import {
  type TranslationLanguage,
  type TranslationSecondaryLanguage,
} from "./candidate-translation-options-section";
import { TranslationSettingsContent } from "./translation-settings-content";
import { createTranslationProviderSettings } from "./translation-provider-settings";
import { type TranslationProvider } from "./translation-service-selector-section";
import { defaultWordCharacter } from "./word-character-section";
import { FuzzyPinyinSection, defaultFuzzyPinyin } from "./fuzzy-pinyin-section";
import { PunctuationSection } from "./punctuation-section";
import { defaultMixedInput, type MixedInputPreferences } from "./mixed-input-section";
import { defaultFrequency } from "./frequency-section";
import { InputSharedSettingsSection } from "./input-shared-settings-section";
import {
  MobileKeyboardFeedbackSection,
  type MobileKeyboardFeedback,
} from "./mobile-keyboard-feedback-section";
import type { FuzzyPinyinPreferences } from "./fuzzy-pinyin-section";
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
      <InputSchemeSettingsContent
        preferences={draft}
        hasTouchKeyboardSchemes={Boolean(client.touchKeyboardSchemes)}
        touchKeyboardSchemes={touchKeyboardSchemes}
        selectedTouchKeyboardScheme={selectedTouchKeyboardScheme}
        macos={macosPlatform}
        macosShuangpinKeymap={
          macosPlatform && client.loadMacosShuangpinKeymap && macosShuangpinKeymap !== undefined
            ? macosShuangpinKeymap
            : undefined
        }
        macosWubiAutoCommitUnique={macosWubiAutoCommitUnique}
        onPreferencesChange={onPreferencesChange}
        onSelectTouchKeyboardScheme={(scheme) => selectTouchKeyboardScheme(scheme)}
        onToggleTouchKeyboardScheme={(scheme, enabled) =>
          setTouchKeyboardSchemeEnabled(scheme, enabled)
        }
        onMacosShuangpinKeymapChange={setShuangpinKeymap}
        onMacosWubiAutoCommitUniqueChange={setWubiAutoCommitUnique}
      />
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
      <TranslationSettingsContent
        candidate={{
          enabled: candidateTranslations,
          targetLanguage: translationTargetLanguage,
          secondaryLanguage: translationSecondaryLanguage ?? "",
          candidateGlossLanguagesEnabled,
          visibleLanguages: visibleTranslationLanguages,
          visibleSecondaryLanguages,
          showSecondaryLanguage: androidPlatform || iosPlatform || macosPlatform || harmonyPlatform,
          showAccountTranslation: androidPlatform,
          accountTranslation: draft.translation_account ?? false,
          onEnabledChange: (candidate_translations) =>
            onPreferencesChange({ candidate_translations }),
          onTargetLanguageChange: (translation_target_language: TranslationLanguage) =>
            onPreferencesChange({ translation_target_language }),
          onSecondaryLanguageChange: (value: TranslationSecondaryLanguage) =>
            onPreferencesChange({
              translation_secondary_language: value === "" ? null : value,
            }),
          onAccountTranslationChange: (enabled) =>
            enabled
              ? setTranslationProvider("account")
              : onPreferencesChange({ translation_account: undefined }),
          onDeviceMissingLanguages: onDeviceMissingLanguages.map(([, label]) => label),
          openSettings: client.onDeviceTranslation?.openSettings,
          onError,
          showTranslationService: !androidPlatform,
          translationProvider,
          showAccountProvider: macosPlatform || linuxPlatform,
          onTranslationProviderChange: setTranslationProvider,
        }}
        providers={
          !androidPlatform
            ? createTranslationProviderSettings({
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
                serviceCredentialEnabled: tencentTranslation.enabled,
              })
            : undefined
        }
      />
      <InputSharedSettingsSection
        preferences={draft}
        wordCharacter={wordCharacter}
        navigation={draft.navigation ?? defaultNavigation}
        frequency={frequency}
        ios={iosPlatform}
        showInputModeHUD={showInputModeHUD && !macosPlatform}
        showModeScope={showModeScope}
        mixedInput={mixedInput}
        onMixedInputChange={(mixed_input) => onPreferencesChange({ mixed_input })}
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
