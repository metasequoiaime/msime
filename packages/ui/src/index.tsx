import { useConfirm } from "./core/confirm";
import { errorCode } from "./core/error-code";
import { errorMessage } from "./core/error-message";
import { clamp } from "./core/number";
import { randomRequestId } from "./core/random-id";
import {
  inferredTouchKeyboardScheme,
  selectTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  touchKeyboardSchemeOptions,
  allTouchKeyboardSchemes,
  type TouchKeyboardScheme,
  type TouchKeyboardSchemePreferences,
} from "./settings/touch-keyboard-scheme-helpers";
export {
  type TouchKeyboardScheme,
  type TouchKeyboardSchemePreferences,
  inferredTouchKeyboardScheme,
  selectTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  touchKeyboardSchemeOptions,
  allTouchKeyboardSchemes,
} from "./settings/touch-keyboard-scheme-helpers";
import { platformOsName, schemeTitle } from "./settings/label-helpers";
import { platformCopy, type PlatformCopyContext } from "./settings/platform-copy";
export {
  platformCopy,
  type PlatformCopyContext,
  type PlatformCopy,
} from "./settings/platform-copy";
import {
  mobilePrimaryPageIds,
  mobileTabForPage,
  pages,
  requestedPage,
  splitMobilePages,
  type MobilePrimaryPageId,
  type SettingsPageId,
} from "./settings/mobile-navigation";
import { isLinuxDesktop } from "./settings/platform-helpers";
import { resolveSettingsTheme } from "./settings/theme-helpers";
import {
  UPDATE_CHECK_TIMEOUT_MS,
  androidPrivacyUrl,
  clientReleasesUrl,
  desktopDownloadUrl,
  fallbackAppVersion,
  licenseUrl,
  linuxIssuesUrl,
  linuxLicenseUrl,
  linuxPrivacyUrl,
  linuxReleasesPageUrl,
  logo,
  privacyUrl,
  releasesPageUrl,
  updateManifestUrl,
} from "./settings/app-resources";
import { AI_PROVIDER_OPTIONS } from "./settings/ai-provider-options";
export { AI_PROVIDER_OPTIONS } from "./settings/ai-provider-options";
import { defaultVoiceInput } from "./settings/voice-input-defaults";
import { macosSidebarGroups } from "./settings/macos-sidebar-groups";
import { groupSidebarPages } from "./settings/sidebar-groups";
import { mobileHiddenPageIds as getMobileHiddenPageIds } from "./settings/mobile-hidden-pages";
import {
  defaultCustomTranslation,
  defaultNiuTrans,
  defaultTencentTranslation,
} from "./settings/translation-defaults";
import { aiPolishTestPrompt, defaultAiAssistant } from "./settings/ai-assistant-defaults";
import {
  mobileTranslationLanguages,
  translationLanguages,
  translationSecondaryLanguages,
} from "./settings/translation-language-helpers";
import { aiProviderUpdate } from "./settings/ai-provider-update";
export { aiProviderUpdate } from "./settings/ai-provider-update";
import { VoiceDevicePicker, type VoiceDeviceReader } from "./voice/voice-device-picker";
import {
  LocalModelManager,
  localModelInUse,
  validModelMirror,
  type LocalVoiceModelClient,
} from "./voice/local-models";
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  DICTIONARY_PAGE_SIZE,
  dictionaryPageStatus,
  readDictionaryFile,
  type DictionaryEntry,
  type LocalDictionaryFormat,
  type LocalDictionaryKind,
} from "./dictionary/dictionary-file";
import { localDictionaryKinds } from "./dictionary/dictionary-kinds";
export type {
  DictionaryEntry,
  LocalDictionaryFormat,
  LocalDictionaryKind,
} from "./dictionary/dictionary-file";
import { describeImportResult, dictionaryKindKeyHint } from "./dictionary/dictionary-messages";
import {
  dictionaryExportName,
  dictionaryExportPayload,
  personalDictionaryExportName,
  personalDictionaryExportPayload,
  loadAllPersonalDictionaryEntries,
  dictionaryKindLabel,
} from "./dictionary/dictionary-export";
import {
  dictionaryErrorMessage,
  dictionaryKeyMatches,
  importFailureMessage,
} from "./dictionary/dictionary-errors";
export { dictionaryErrorMessage } from "./dictionary/dictionary-errors";
export {
  dictionaryExportName,
  dictionaryExportPayload,
  personalDictionaryExportName,
  personalDictionaryExportPayload,
  loadAllPersonalDictionaryEntries,
  dictionaryKindLabel,
} from "./dictionary/dictionary-export";
export { describeImportResult, dictionaryKindKeyHint } from "./dictionary/dictionary-messages";
import { AppearanceCandidatePreview } from "./candidate/appearance-candidate-preview";
import { useCandidatePreviewTheme } from "./candidate/candidate-preview-theme";
import { CandidateFontControls } from "./candidate/candidate-font-controls";
import { CandidateSizingSection } from "./settings/candidate-sizing-section";
import { CandidatePageSizeSection } from "./settings/candidate-page-size-section";
import { CandidateLayoutSection } from "./settings/candidate-layout-section";
import { CandidateFollowCursorSection } from "./settings/candidate-follow-cursor-section";
import { CandidatePanelLimitSection } from "./settings/candidate-panel-limit-section";
import { CandidateFontUnsupportedNotice } from "./settings/candidate-font-unsupported-notice";
import { CandidateEnglishGlossSection } from "./settings/candidate-english-gloss-section";
import { BuiltInSkinsSection } from "./settings/built-in-skins-section";
import { EnglishSuggestionsSection } from "./settings/english-suggestions-section";
import { LearningSection } from "./settings/learning-section";
import { LearningDataSection } from "./settings/learning-data-section";
import { DictionaryManagerHeader } from "./settings/dictionary-manager-header";
import { SettingsActionsFooter } from "./settings/settings-actions-footer";
import { AboutHeroSection } from "./settings/about-hero-section";
import { SkinPlatformNotice } from "./settings/skin-platform-notice";
import { DefaultImeModeSection, type DefaultImeMode } from "./settings/default-ime-mode-section";
import { InputModeHudSection } from "./settings/input-mode-hud-section";
import { ImeModeScopeSection } from "./settings/ime-mode-scope-section";
import { TraditionalChineseOutputSection } from "./settings/traditional-chinese-output-section";
import { CloudCandidatesSection } from "./settings/cloud-candidates-section";
import { TelemetrySection } from "./settings/telemetry-section";
import { WubiSection } from "./settings/wubi-section";
import { InputModeSection } from "./settings/input-mode-section";
import {
  InputSchemeSelectorSection,
  type InputSchemeSelectorValue,
} from "./settings/input-scheme-selector-section";
import {
  InputSchemeDetailsSection,
  type ShuangpinProfile,
} from "./settings/input-scheme-details-section";
import { InputModeShortcutsSection } from "./settings/input-mode-shortcuts-section";
import { ShortcutsIntroSection } from "./settings/shortcuts-intro-section";
import { PanelShortcutsSection } from "./settings/panel-shortcuts-section";
import { CandidateShortcutsSection } from "./settings/candidate-shortcuts-section";
import { MaintenanceShortcutsSection } from "./settings/maintenance-shortcuts-section";
import { InputMethodServiceSection } from "./settings/input-method-service-section";
import { DataDirectorySection } from "./settings/data-directory-section";
import { LicenseUninstallSection } from "./settings/license-uninstall-section";
import { DiagnosticLogsSection } from "./settings/diagnostic-logs-section";
import { HelpFeedbackSection } from "./settings/help-feedback-section";
import { HelpSettingsPage } from "./settings/help-settings-page";
import { ScreenKeyboardThemeSection } from "./settings/screen-keyboard-theme-section";
import { ScreenKeyboardSkinsSection } from "./settings/screen-keyboard-skins-section";
import { ScreenKeyboardCommunitySection } from "./settings/screen-keyboard-community-section";
import { ScreenKeyboardLaunchSection } from "./settings/screen-keyboard-launch-section";
import { CandidatePaletteSection } from "./settings/candidate-palette-section";
import { CandidatePaletteFallbackNotice } from "./settings/candidate-palette-fallback-notice";
import { TouchKeyboardSchemesSection } from "./settings/touch-keyboard-schemes-section";
import {
  TouchKeyboardGeometrySection,
  type TouchToolbarPreferences,
} from "./settings/touch-keyboard-geometry-section";
import { VoiceInputIntroSection } from "./settings/voice-input-intro-section";
import { VoiceInputCoreSection } from "./settings/voice-input-core-section";
import { VoiceModelPathSection } from "./settings/voice-model-path-section";
import { VoiceModelPathDisclosure } from "./settings/voice-model-path-disclosure";
import { VoiceModelSection } from "./settings/voice-model-section";
import { VoiceEndpointSection } from "./settings/voice-endpoint-section";
import { VoiceCredentialFieldsSection } from "./settings/voice-credential-fields-section";
import { PolishCredentialFieldsSection } from "./settings/polish-credential-fields-section";
import { VoiceStreamPreeditSection } from "./settings/voice-stream-preedit-section";
import { VoiceCommitModeSection, type VoiceCommitMode } from "./settings/voice-commit-mode-section";
import { VoiceCaptureDevicesSection } from "./settings/voice-capture-devices-section";
import { VoiceSyntheticSilenceNotice } from "./settings/voice-synthetic-silence-notice";
import { VoiceHotkeysSection } from "./settings/voice-hotkeys-section";
import { VoicePolishSection } from "./settings/voice-polish-section";
import { FloatingToolbarAppearanceSection } from "./settings/floating-toolbar-appearance-section";
import { FloatingToolbarPlatformNotice } from "./settings/floating-toolbar-platform-notice";
import { FloatingToolbarComponentsSection } from "./settings/floating-toolbar-components-section";
import { FloatingToolbarToggleSection } from "./settings/floating-toolbar-toggle-section";
import { DoubaoAuthModeSection } from "./settings/doubao-auth-mode-section";
import { DoubaoStreamEndpointSection } from "./settings/doubao-stream-endpoint-section";
import { DoubaoOptionsSection } from "./settings/doubao-options-section";
import { DoubaoResourceIdSection } from "./settings/doubao-resource-id-section";
import { VoiceRecordingBehaviorSection } from "./settings/voice-recording-behavior-section";
import { FeedbackSettingsSection } from "./settings/feedback-settings-section";
import { HandwritingSettingsSection } from "./settings/handwriting-settings-section";
import { HandwritingPlatformNotice } from "./settings/handwriting-platform-notice";
import { MobileInputAiNotice } from "./settings/mobile-input-ai-notice";
import {
  InputSourceStartupNotice,
  type InputSourceStartupStatus,
} from "./settings/input-source-startup-notice";
import { VoiceModelMirrorSection } from "./settings/voice-model-mirror-section";
import {
  CredentialTestSection,
  type CredentialTestState,
} from "./settings/credential-test-section";
import { ProviderPresetSection, type ProviderPreset } from "./settings/provider-preset-section";
import { AiCredentialSection } from "./settings/ai-credential-section";
import { AiLinuxProviderSection } from "./settings/ai-linux-provider-section";
import { AiApiTokenSection } from "./settings/ai-api-token-section";
import {
  tencentCredentialIssue,
  translationEndpointIssue,
} from "./settings/translation-validation";
export {
  tencentCredentialIssue,
  translationEndpointIssue,
} from "./settings/translation-validation";
import {
  aiCredentialOrigin,
  providerCredentialErrorMessage,
  tencentSecretConfigured,
} from "./settings/credential-utils";
export {
  aiCredentialOrigin,
  providerCredentialErrorMessage,
  tencentSecretConfigured,
} from "./settings/credential-utils";
import { AiPromptSettingsSection } from "./settings/ai-prompt-settings-section";
import { AiTestToolsSection } from "./settings/ai-test-tools-section";
import { AiCandidateLimitSection } from "./settings/ai-candidate-limit-section";
import { AiModelCatalogSection } from "./settings/ai-model-catalog-section";
import {
  AiBasicSettingsSection,
  type AiProviderOption,
} from "./settings/ai-basic-settings-section";
import { NiuTransSection } from "./settings/niutrans-section";
import { CustomTranslationSection } from "./settings/custom-translation-section";
import { CustomTranslationsSection } from "./settings/custom-translations-section";
import { TencentTranslationSection } from "./settings/tencent-translation-section";
import { LinuxTencentCredentialsSection } from "./settings/linux-tencent-credentials-section";
import { TranslationServiceSelectorSection } from "./settings/translation-service-selector-section";
import { OnDeviceTranslationNotice } from "./settings/on-device-translation-notice";
import {
  VoiceCredentialSection,
  type VoiceCredentialSaveInput,
} from "./settings/voice-credential-section";
import {
  MobileKeyboardFeedbackSection,
  type MobileKeyboardFeedback,
  type MobileKeyboardFeedbackClient,
} from "./settings/mobile-keyboard-feedback-section";
import { PreeditSettingsSection } from "./settings/preedit-settings-section";
import { DictionaryManifestCard } from "./settings/dictionary-manifest-card";
import { PersonalDictionaryImportCard } from "./settings/personal-dictionary-import-card";
import { validCandidateFonts } from "./candidate/candidate-font-family";
import type { FontCatalogReader } from "./candidate/font-catalog";
import {
  asrProviderUpdate,
  polishProviderUpdate,
  ASR_PROVIDER_DEFAULTS,
  POLISH_PROVIDER_DEFAULTS,
} from "./voice/voice-providers";
import { PolishPromptSection } from "./settings/polish-prompt-section";
import type { TouchKeyboardSkin } from "./keyboard/screen-keyboard-preview";
import { TouchKeyboardSkinEditor } from "./keyboard/touch-keyboard-skin-editor";
import * as skin from "./keyboard/touch-skin-style";
import {
  defaultTouchKeyboardSkinDesign,
  type AiSkinClient,
  type CustomSkinLibraryClient,
  type TouchKeyboardSkinDesign,
} from "./keyboard/touch-keyboard-skin-design";
export type {
  AiSkinClient,
  AiSkinProposal,
  AiSkinProgress,
  CustomSkinLibraryAction,
  CustomSkinLibraryClient,
  SavedTouchKeyboardSkin,
  TouchKeyboardSkinDesign,
} from "./keyboard/touch-keyboard-skin-design";
import { ExternalSkins, type SkinCatalog } from "./skin/external-skins";
import { TypingStatisticsPage, type TypingStatisticsClient } from "./settings/typing-statistics";
import { VocabularyReviewPage, type VocabularyReviewClient } from "./settings/vocabulary-review";
import {
  McpConnectSection,
  type McpClientId,
  type McpInstallOutcome,
  type McpServerStatus,
} from "./settings/mcp-connect";
import {
  HelpcodeSettingsPage,
  type HelpcodePreferences,
  type HelpcodeSchema,
} from "./settings/pages/helpcode-page";
import {
  FuzzyPinyinSection,
  defaultFuzzyPinyin,
  type FuzzyPinyinPreferences,
} from "./settings/fuzzy-pinyin-section";
import {
  WordCharacterSection,
  defaultWordCharacter,
  type NavigationPreferences,
  type WordCharacterPreferences,
} from "./settings/word-character-section";
import { NavigationSection, defaultNavigation } from "./settings/navigation-section";
import {
  CandidateTranslationOptionsSection,
  type TranslationLanguage,
  type TranslationSecondaryLanguage,
} from "./settings/candidate-translation-options-section";
import { PunctuationSection } from "./settings/punctuation-section";
import {
  MixedInputSection,
  defaultMixedInput,
  type MixedInputPreferences,
} from "./settings/mixed-input-section";
import {
  FrequencySection,
  defaultFrequency,
  type FrequencyPreferences,
} from "./settings/frequency-section";
import {
  LocalModesSection,
  defaultLocalModes,
  type LocalModePreferences,
} from "./settings/local-modes-section";
import {
  ThemeSettingsSection,
  type SurfaceTheme,
  type ThemeMode,
} from "./settings/theme-settings-section";
import {
  CandidateColorsSection,
  type CandidateColorKey,
  type CandidateColorPreferences,
} from "./settings/candidate-colors-section";
import {
  ClipboardHistorySection,
  type ClipboardHistoryClient,
} from "./settings/clipboard-history-section";
import { CloudPanelSessionNotice } from "./settings/cloud-panel-session-notice";
import { WindowTitlebar } from "./settings/window-titlebar";
export { WindowTitlebar, type WindowTitlebarProps } from "./settings/window-titlebar";
import { windowResizeEdge } from "./settings/window-resize";
export {
  windowResizeEdge,
  type WindowResizeBounds,
  type WindowResizePoint,
} from "./settings/window-resize";
import { SettingsSidebar } from "./settings/settings-sidebar";
export {
  SettingsSidebar,
  type SettingsSidebarItem,
  type SettingsSidebarProps,
} from "./settings/settings-sidebar";
import { MobileSettingsTabs } from "./settings/mobile-settings-tabs";
export {
  MobileSettingsTabs,
  type MobileSettingsTab,
  type MobileSettingsTabsProps,
} from "./settings/mobile-settings-tabs";
import { SettingsPageHeader } from "./settings/settings-page-header";
export { SettingsPageHeader, type SettingsPageHeaderProps } from "./settings/settings-page-header";
import { SettingsStatusMessages } from "./settings/settings-status-messages";
export {
  SettingsStatusMessages,
  type SettingsStatusMessagesProps,
} from "./settings/settings-status-messages";
import * as surface from "./keyboard/panel-surface-style";
import * as settings from "./settings/settings-style";
import * as doc from "./settings/document-style";
import {
  AccountPage,
  type AccountClient,
  type AccountCommunityDestination,
  type AppIconClient,
} from "./account/account-page";
import { ChatPage, type ChatClient } from "./chat/chat-page";
import { HomePage, MoreSettingsPage, type HomePageActions } from "./keyboard/home-page";
import { CommunitySkinsPage, type CommunitySkinClient } from "./community/community-skins";
import {
  CommunityHomePage,
  CommunityResourcesPage,
  type CommunityResourceClient,
} from "./community/community-resources";
export { useConfirm, type ConfirmRequest } from "./core/confirm";
export {
  decodeDictionaryBytes,
  readDictionaryFile,
  UNBATCHED_DICTIONARY_FILE_BYTES,
} from "./dictionary/dictionary-file";
export {
  TypingStatisticsPage,
  retentionChoices,
  type StatisticsRetention,
  activityMetrics,
  addDays,
  currentStreak,
  dailyDetailRows,
  formatActiveTime,
  longestStreak,
  type ActivityMetrics,
  type DailyDetailRow,
  type TypingBreakdown,
  type TypingStatistics,
  type TypingStatisticsClient,
  type TypingStatisticsStatus,
} from "./settings/typing-statistics";
export {
  VocabularyReviewPage,
  VocabularyReviewPanel,
  type VocabularyCard,
  type VocabularyReviewClient,
  type VocabularyReviewSettings,
  type VocabularyReviewStatus,
  type VocabularyWordbook,
} from "./settings/vocabulary-review";
export {
  McpConnectSection,
  type McpClientId,
  type McpClientStatus,
  type McpInstallOutcome,
  type McpServerStatus,
} from "./settings/mcp-connect";
export {
  AccountPage,
  type AccountChallenge,
  type AccountClient,
  type AccountCommunityDestination,
  type AccountPreferenceSchema,
  type AccountPreferences,
  type AccountPreferenceValue,
  type AccountProfile,
  type AccountProviders,
  type AccountUser,
  type AppIconClient,
  type AppIconInfo,
  type SettingsSyncClient,
} from "./account/account-page";
export {
  ChatPage,
  type ChatClient,
  type ChatMessage,
  type ChatModel,
  type ChatModels,
} from "./chat/chat-page";
export { HomePage, MoreSettingsPage, type HomePageActions } from "./keyboard/home-page";
export {
  WelcomeFlowPage,
  type OnboardingActions,
  type OnboardingInputScheme,
} from "./account/onboarding-page";
export {
  LinuxSetupPage,
  type LinuxSetupClient,
  type LinuxSetupLine,
  type LinuxSetupStatus,
} from "./account/linux-setup-page";
export { SettingsStartupPage } from "./settings/settings-startup-page";
export {
  HelpcodeSettingsPage,
  type HelpcodePreferences,
  type HelpcodeSchema,
  type HelpcodeSettings,
} from "./settings/pages/helpcode-page";
export {
  ClipboardHistorySection,
  type ClipboardHistoryClient,
  type ClipboardHistoryEntry,
} from "./settings/clipboard-history-section";
export { CloudPanelSessionNotice } from "./settings/cloud-panel-session-notice";
export { FuzzyPinyinSection, type FuzzyPinyinPreferences } from "./settings/fuzzy-pinyin-section";
export {
  WordCharacterSection,
  type NavigationPreferences,
  type WordCharacterPreferences,
} from "./settings/word-character-section";
export { NavigationSection } from "./settings/navigation-section";
export {
  HandwritingPlatformNotice,
  type HandwritingPlatform,
  type HandwritingPlatformNoticeProps,
} from "./settings/handwriting-platform-notice";
export {
  MobileInputAiNotice,
  type MobileInputAiNoticeProps,
} from "./settings/mobile-input-ai-notice";
export {
  CandidateTranslationOptionsSection,
  type CandidateTranslationOptionsSectionProps,
  type TranslationLanguage,
  type TranslationLanguageOption,
  type TranslationSecondaryLanguage,
  type TranslationSecondaryLanguageOption,
} from "./settings/candidate-translation-options-section";
export { PunctuationSection, type PunctuationPreferences } from "./settings/punctuation-section";
export {
  MixedInputSection,
  defaultMixedInput,
  type MixedInputPreferences,
} from "./settings/mixed-input-section";
export {
  FrequencySection,
  defaultFrequency,
  type FrequencyPreferences,
} from "./settings/frequency-section";
export {
  LocalModesSection,
  defaultLocalModes,
  type LocalModeKey,
  type LocalModePreferences,
} from "./settings/local-modes-section";
export {
  ThemeSettingsSection,
  type SurfaceTheme,
  type ThemeMode,
  type ThemePreferenceKey,
  type ThemePreferences,
} from "./settings/theme-settings-section";
export {
  CandidateColorsSection,
  type CandidateColorKey,
  type CandidateColorPreferences,
} from "./settings/candidate-colors-section";
export { CandidatePaletteFallbackNotice } from "./settings/candidate-palette-fallback-notice";
export {
  AiLinuxProviderSection,
  type AiLinuxProviderSectionProps,
} from "./settings/ai-linux-provider-section";
export {
  BuiltInSkinsSection,
  type BuiltInSkinsSectionProps,
} from "./settings/built-in-skins-section";
export {
  CandidateSizingSection,
  type CandidateSizingPreferences,
} from "./settings/candidate-sizing-section";
export { CandidatePageSizeSection } from "./settings/candidate-page-size-section";
export { CandidateLayoutSection, type CandidateLayout } from "./settings/candidate-layout-section";
export {
  CandidateFollowCursorSection,
  type CandidateFollowCursorSectionProps,
} from "./settings/candidate-follow-cursor-section";
export {
  CandidatePanelLimitSection,
  type CandidatePanelLimit,
  type CandidatePanelLimitSectionProps,
} from "./settings/candidate-panel-limit-section";
export { CandidateFontUnsupportedNotice } from "./settings/candidate-font-unsupported-notice";
export {
  CandidateEnglishGlossSection,
  type CandidateEnglishGlossSectionProps,
} from "./settings/candidate-english-gloss-section";
export {
  EnglishSuggestionsSection,
  type EnglishSuggestionsSectionProps,
} from "./settings/english-suggestions-section";
export { LearningSection, type LearningSectionProps } from "./settings/learning-section";
export {
  LearningDataSection,
  type LearningDataSectionProps,
} from "./settings/learning-data-section";
export {
  DictionaryManagerHeader,
  type DictionaryManagerHeaderProps,
} from "./settings/dictionary-manager-header";
export {
  SettingsActionsFooter,
  type SettingsActionsFooterProps,
} from "./settings/settings-actions-footer";
export { AboutHeroSection, type AboutHeroSectionProps } from "./settings/about-hero-section";
export { SkinPlatformNotice, type SkinPlatformNoticeProps } from "./settings/skin-platform-notice";
export {
  DefaultImeModeSection,
  type DefaultImeMode,
  type DefaultImeModeSectionProps,
} from "./settings/default-ime-mode-section";
export {
  InputModeHudSection,
  type InputModeHudSectionProps,
} from "./settings/input-mode-hud-section";
export {
  ImeModeScopeSection,
  type ImeModeScope,
  type ImeModeScopeSectionProps,
} from "./settings/ime-mode-scope-section";
export {
  TraditionalChineseOutputSection,
  type TraditionalChineseOutputSectionProps,
} from "./settings/traditional-chinese-output-section";
export {
  CloudCandidatesSection,
  type CloudCandidatesSectionProps,
} from "./settings/cloud-candidates-section";
export { TelemetrySection, type TelemetrySectionProps } from "./settings/telemetry-section";
export { WubiSection, type WubiPreferences, type WubiSectionProps } from "./settings/wubi-section";
export {
  InputModeSection,
  type InputModeScheme,
  type InputModeSectionProps,
} from "./settings/input-mode-section";
export {
  InputSchemeSelectorSection,
  type InputSchemeSelectorSectionProps,
  type InputSchemeSelectorValue,
} from "./settings/input-scheme-selector-section";
export {
  InputSchemeDetailsSection,
  type InputSchemeDetailsSectionProps,
  type InputSchemeDetailsScheme,
  type ShuangpinProfile,
} from "./settings/input-scheme-details-section";
export {
  InputModeShortcutsSection,
  type InputModeShortcutPreferences,
  type InputModeShortcutsSectionProps,
} from "./settings/input-mode-shortcuts-section";
export {
  ShortcutsIntroSection,
  type ShortcutsIntroSectionProps,
} from "./settings/shortcuts-intro-section";
export {
  PanelShortcutsSection,
  type PanelShortcutsSectionProps,
} from "./settings/panel-shortcuts-section";
export {
  CandidateShortcutsSection,
  type CandidateShortcutsSectionProps,
} from "./settings/candidate-shortcuts-section";
export {
  MaintenanceShortcutsSection,
  type MaintenanceShortcutsSectionProps,
} from "./settings/maintenance-shortcuts-section";
export {
  InputMethodServiceSection,
  type InputMethodServiceSectionProps,
} from "./settings/input-method-service-section";
export {
  DataDirectorySection,
  type DataDirectoryInfo,
  type DataDirectorySectionProps,
} from "./settings/data-directory-section";
export {
  LicenseUninstallSection,
  type LicenseUninstallSectionProps,
} from "./settings/license-uninstall-section";
export {
  DiagnosticLogsSection,
  type DiagnosticLogPreferences,
  type DiagnosticLogsSectionProps,
} from "./settings/diagnostic-logs-section";
export {
  HelpFeedbackSection,
  type HelpFeedbackSectionProps,
} from "./settings/help-feedback-section";
export { HelpSettingsPage, type HelpSettingsPageProps } from "./settings/help-settings-page";
export {
  ScreenKeyboardThemeSection,
  type ScreenKeyboardThemeSectionProps,
} from "./settings/screen-keyboard-theme-section";
export {
  ScreenKeyboardSkinsSection,
  type ScreenKeyboardSkinsSectionProps,
} from "./settings/screen-keyboard-skins-section";
export {
  ScreenKeyboardCommunitySection,
  type ScreenKeyboardCommunitySectionProps,
} from "./settings/screen-keyboard-community-section";
export {
  TouchKeyboardSchemesSection,
  type TouchKeyboardSchemesSectionProps,
} from "./settings/touch-keyboard-schemes-section";
export {
  TouchKeyboardGeometrySection,
  type TouchKeyboardGeometrySectionProps,
  type TouchToolbarPreferences,
} from "./settings/touch-keyboard-geometry-section";
export {
  VoiceInputIntroSection,
  type VoiceInputIntroSectionProps,
} from "./settings/voice-input-intro-section";
export {
  VoiceInputCoreSection,
  type VoiceInputCoreSectionProps,
} from "./settings/voice-input-core-section";
export {
  VoiceModelPathSection,
  type VoiceModelPathSectionProps,
} from "./settings/voice-model-path-section";
export {
  VoiceModelPathDisclosure,
  type VoiceModelPathDisclosureProps,
} from "./settings/voice-model-path-disclosure";
export { VoiceModelSection, type VoiceModelSectionProps } from "./settings/voice-model-section";
export {
  VoiceEndpointSection,
  type VoiceEndpointSectionProps,
} from "./settings/voice-endpoint-section";
export {
  VoiceCredentialFieldsSection,
  type VoiceCredentialFieldsSectionProps,
} from "./settings/voice-credential-fields-section";
export {
  PolishCredentialFieldsSection,
  type PolishCredentialFieldsSectionProps,
} from "./settings/polish-credential-fields-section";
export {
  PolishPromptSection,
  type PolishCustomPromptValues,
  type PolishPromptSectionProps,
} from "./settings/polish-prompt-section";
export {
  VoiceStreamPreeditSection,
  type VoiceStreamPreeditSectionProps,
} from "./settings/voice-stream-preedit-section";
export {
  VoiceCommitModeSection,
  type VoiceCommitMode,
  type VoiceCommitModeSectionProps,
} from "./settings/voice-commit-mode-section";
export {
  VoiceCaptureDevicesSection,
  type VoiceCaptureDevicesSectionProps,
  type VoiceCaptureBackendOption,
  type VoiceCaptureBackend,
} from "./settings/voice-capture-devices-section";
export { VoiceSyntheticSilenceNotice } from "./settings/voice-synthetic-silence-notice";
export {
  VoiceHotkeysSection,
  type VoiceHotkeysSectionProps,
  type VoiceHotkeyKey,
  type VoiceHotkeyPlatform,
} from "./settings/voice-hotkeys-section";
export {
  FloatingToolbarAppearanceSection,
  type FloatingToolbarAppearanceSectionProps,
  type FloatingToolbarFontSize,
  type FloatingToolbarScale,
} from "./settings/floating-toolbar-appearance-section";
export { FloatingToolbarPlatformNotice } from "./settings/floating-toolbar-platform-notice";
export {
  FloatingToolbarComponentsSection,
  type FloatingToolbarCapability,
  type FloatingToolbarComponentKey,
  type FloatingToolbarComponentsSectionProps,
} from "./settings/floating-toolbar-components-section";
export {
  DoubaoAuthModeSection,
  type DoubaoAuthMode,
  type DoubaoAuthModeSectionProps,
} from "./settings/doubao-auth-mode-section";
export {
  DoubaoStreamEndpointSection,
  type DoubaoStreamEndpointSectionProps,
} from "./settings/doubao-stream-endpoint-section";
export {
  DoubaoOptionsSection,
  type DoubaoOptionsSectionProps,
} from "./settings/doubao-options-section";
export {
  DoubaoResourceIdSection,
  type DoubaoResourceIdSectionProps,
} from "./settings/doubao-resource-id-section";
export {
  VoiceRecordingBehaviorSection,
  type VoiceRecordingBehaviorSectionProps,
} from "./settings/voice-recording-behavior-section";
export {
  VoiceModelMirrorSection,
  type VoiceModelMirrorSectionProps,
} from "./settings/voice-model-mirror-section";
export {
  VoiceCredentialSection,
  type VoiceCredentialEntry,
  type VoiceCredentialInput,
  type VoiceCredentialMessage,
  type VoiceCredentialSaveInput,
  type VoiceCredentialSectionProps,
  type VoiceCredentialSectionKind,
  type VoiceCredentialStatus,
} from "./settings/voice-credential-section";
export {
  CredentialTestSection,
  type CredentialTestSectionProps,
  type CredentialTestState,
} from "./settings/credential-test-section";
export {
  ProviderPresetSection,
  type ProviderPreset,
  type ProviderPresetSectionProps,
} from "./settings/provider-preset-section";
export {
  CredentialStatusMessage,
  type CredentialStatusMessageProps,
  type CredentialStatusMessageValue,
} from "./settings/credential-status-message";
export { CredentialActions, type CredentialActionsProps } from "./settings/credential-actions";
export { SettingToggle, type SettingToggleProps } from "./settings/setting-toggle";
export {
  AiCredentialSection,
  type AiCredentialSectionProps,
  type AiCredentialStored,
} from "./settings/ai-credential-section";
export { AiApiTokenSection, type AiApiTokenSectionProps } from "./settings/ai-api-token-section";
export { NiuTransSection, type NiuTransSectionProps } from "./settings/niutrans-section";
export {
  CustomTranslationSection,
  type CustomTranslationSectionProps,
} from "./settings/custom-translation-section";
export {
  CustomTranslationsSection,
  type CustomTranslationsSectionProps,
} from "./settings/custom-translations-section";
export {
  TencentTranslationSection,
  type TencentTranslationSectionProps,
} from "./settings/tencent-translation-section";
export {
  LinuxTencentCredentialsSection,
  type LinuxTencentCredentialsSectionProps,
  type LinuxTencentCredentialInput,
  type LinuxTencentCredentialStatus,
} from "./settings/linux-tencent-credentials-section";
export {
  TranslationServiceSelectorSection,
  type TranslationProvider,
  type TranslationServiceSelectorSectionProps,
} from "./settings/translation-service-selector-section";
export {
  OnDeviceTranslationNotice,
  type OnDeviceTranslationNoticeProps,
} from "./settings/on-device-translation-notice";
export {
  DictionaryManifestCard,
  type DictionaryManifestCardProps,
} from "./settings/dictionary-manifest-card";
export {
  PersonalDictionaryImportCard,
  type PersonalDictionaryImportCardProps,
  type PersonalDictionaryImportClient,
} from "./settings/personal-dictionary-import-card";
export {
  MobileKeyboardFeedbackSection,
  type MobileKeyboardFeedback,
  type MobileKeyboardFeedbackClient,
  type MobileKeyboardFeedbackSectionProps,
} from "./settings/mobile-keyboard-feedback-section";
export {
  PreeditSettingsSection,
  type CandidatePreeditStyle,
  type PreeditSettingsPreferences,
  type TsfPreeditStyle,
} from "./settings/preedit-settings-section";
export {
  CommunitySkinsPage,
  type CommunitySkin,
  type CommunitySkinClient,
  type CommunitySkinDownload,
  type CommunitySkinPage,
  type CommunitySkinTrial,
} from "./community/community-skins";
export {
  CommunityHomePage,
  CommunityResourcesPage,
  type CommunityLocalDictionaryClient,
  type CommunityResource,
  type CommunityResourceApplication,
  type CommunityResourceClient,
  type CommunityResourceContent,
  type CommunityResourceKind,
  type CommunityResourcePage,
  type CommunityResourceScope,
  type CommunitySharedWord,
} from "./community/community-resources";
export type { SkinCatalog, ExternalSkin } from "./skin/external-skins";
import type { SkinImageReader } from "./skin/skin-image";
export type { SkinImage, SkinImageReader } from "./skin/skin-image";
import type { SkinFontReader } from "./skin/skin-font";
export type { SkinFont, SkinFontReader } from "./skin/skin-font";
export {
  POLISH_CUSTOM_IDS,
  POLISH_PRESETS,
  POLISH_PRESET_IDS,
  POLISH_PRESET_NAMES,
  isPolishCustomSlot,
  normalizePolishSlot,
  polishPresetPrompt,
  type PolishPresetId,
} from "./voice/polish-presets";
export {
  ASR_PROVIDER_DEFAULTS,
  POLISH_PROVIDER_DEFAULTS,
  asrProviderUpdate,
  polishProviderUpdate,
  type ProviderDefaults,
} from "./voice/voice-providers";
export {
  candidateTemplate,
  candidateThemeStylesheet,
  type CandidateAppearance,
  type CandidateOrientation,
  type CandidateTheme,
} from "./candidate/candidate-themes";
import {
  compareVersions,
  describeInstallerTrust,
  parseVersion,
  selectPlatformRelease,
  validateManifest,
  type GitHubRelease,
  type UpdateManifest,
  type ValidatedUpdate,
} from "./settings/update-manifest";
export {
  serializeWindowHostMessage,
  type WindowControl,
  type WindowHostMessage,
  type WindowResizeEdge,
} from "./keyboard/window-host";
export { emojiDisplayName } from "./keyboard/panels";
export type { TouchKeyboardSkin } from "./keyboard/screen-keyboard-preview";
export { VoicePolishSection, type VoicePolishSectionProps } from "./settings/voice-polish-section";
export {
  FloatingToolbarToggleSection,
  type FloatingToolbarToggleSectionProps,
} from "./settings/floating-toolbar-toggle-section";
export {
  ScreenKeyboardLaunchSection,
  type ScreenKeyboardLaunchSectionProps,
} from "./settings/screen-keyboard-launch-section";
export {
  CandidatePaletteSection,
  type CandidatePaletteSectionProps,
} from "./settings/candidate-palette-section";
export {
  CloudCandidatesPanel,
  CloudClipboardPanel,
  CloudDictionaryApplyPanel,
  CloudDictionaryCatalogPanel,
  CloudDictionaryFilesPanel,
  CloudDictionaryPanel,
  EmojiPanel,
  HandwritingPanel,
  KeyboardPanel,
  VoicePanel,
  type CloudCandidate,
  type CloudCandidateKind,
  type CloudClipboardAction,
  type CloudClipboardPanelClient,
  type CloudDictionaryAction,
  type CloudDictionaryCatalogEntry,
  type CloudDictionaryEntry,
  type CloudDictionaryFileFormat,
  type CloudDictionaryKind,
  type CloudDictionaryPanelClient,
  type CloudDictionarySnapshotMetadata,
  type CloudDictionarySnapshotRequest,
  type CloudFixedPosition,
  type CloudRankingMode,
  type EmojiPanelClient,
  type PanelClient,
  type VoicePanelClient,
} from "./keyboard/panels";
export type { EmojiCatalogGroup } from "./emoji/emoji-catalog";
import {
  customTranslationsExample,
  customTranslationsWithinBounds,
  parseCustomTranslations,
} from "./dictionary/custom-translations";
export {
  customTranslationsExample,
  customTranslationsWithinBounds,
  parseCustomTranslations,
  type CustomTranslationEntry,
  type CustomTranslationReport,
} from "./dictionary/custom-translations";
export type { VoiceCaptureDevice, VoiceDeviceReader } from "./voice/voice-device-picker";
export {
  LocalModelManager,
  formatModelBytes,
  localModelErrorMessage,
  localModelInUse,
  localModelProgressPercent,
  validModelMirror,
  visibleLocalModels,
  type LocalVoiceModel,
  type LocalVoiceModelClient,
  type LocalVoiceModelList,
  type LocalVoiceModelProgress,
} from "./voice/local-models";

export type KeybindingPreferences = {
  switch_language_shift: boolean;
  switch_language_ctrl: boolean;
  switch_language_ctrl_alt_space: boolean;
  toggle_character_set_ctrl_shift_f: boolean;
  toggle_fullwidth_option_shift_h: boolean;
};
import { defaultKeybindings } from "./settings/keybinding-defaults";
/**
 * macOS groups its sidebar the way the reference window does: what you type with, what it looks
 * like, what it stores, then where to get help. Pages the reference has no counterpart for keep
 * their place in a group of their own rather than disappearing -- they are features this client
 * has and that window does not.
 *
 * The native window this mirrors now names those four groups 打字 / 显示 / 数据与账号 / 支持, and it
 * carries eleven pages rather than thirteen: 辅助码 folds into the scheme card of 输入方案, 实用功能
 * becomes one card of 输入习惯, and 帮助 and 反馈 are one page. This page keeps all three as pages of
 * their own -- Windows, Linux and HarmonyOS have this page as their only settings UI, so nothing
 * here may be dropped -- and orders them where the native window puts their contents: helpcode
 * beside 输入, tools before 快捷键 rather than after it, feedback beside help.
 */
export type HostPlatform = "windows" | "macos" | "linux" | "android" | "ios" | "harmony";
/** Mirrors `client-core::host_surface::HostCapabilities`. */
export interface HostCapabilities {
  platform: HostPlatform;
  /** Whether settings use the phone navigation and touch-oriented surface. */
  mobile_settings?: boolean;
  restart_input_method: boolean;
  panel_windows: boolean;
  ime_mode_scope: boolean;
  typing_statistics: boolean;
  /** The host has wired the shared 背单词 entry point. Absent on a host older than the field. */
  vocabulary_review?: boolean;
  fuzzy_pinyin: boolean;
  system_fonts: boolean;
  window_chrome: boolean;
  floating_toolbar: boolean;
  floating_toolbar_appearance: boolean;
  floating_toolbar_components: boolean;
  /** The toolbar carries a handwriting panel button, which only this client's macOS toolbar does. */
  floating_toolbar_handwriting?: boolean;
  /** The toolbar carries a voice input button, for the same reason. */
  floating_toolbar_voice?: boolean;
  mode_switch_shortcuts: boolean;
  panel_shortcuts: boolean;
  number_row_selection?: boolean;
  voice_capture_devices: boolean;
  candidate_font_controls: boolean;
  candidate_preedit_font?: boolean;
  candidate_row_colors: boolean;
  candidate_selection_appearance: boolean;
  /** The host outlines the candidate panel in the border colour. Linux does (Fcitx5's classic UI theme) without any hover state; a host older than the field reads it from `candidate_selection_appearance`. */
  candidate_border_color?: boolean;
  candidate_follow_cursor: boolean;
  input_mode_hud?: boolean;
  candidate_english_font?: boolean;
  english_suggestions?: boolean;
  helpcode_shift_entry?: boolean;
  skin_directory_import?: boolean;
  /** The one candidate page size the host draws; set when the host offers no choice. */
  fixed_candidate_page_size?: number;
  /** The one candidate layout the host draws; set when the host offers no choice. */
  fixed_candidate_layout?: "horizontal" | "vertical";
  /** The touch keyboard picks its toolbar buttons from `touch_toolbar`. */
  touch_toolbar_components?: boolean;
  shuangpin_preedit?: boolean;
  /** The host routes the Ctrl+Shift+Alt maintenance chords. */
  maintenance_shortcuts?: boolean;
  /** The host reserves Option/Alt+Shift+H for the character width. */
  fullwidth_chord?: boolean;
  /** The host runs the configured transcription provider, so its controls have an effect. */
  voice_provider_settings?: boolean;
  /** The host draws interim recognition text, so the streaming-preedit switch has an effect. */
  voice_stream_preedit?: boolean;
  /** The host tells the runtime its character width, so 全角输入 has something to act on. */
  character_width?: boolean;
  ai_provider_credentials?: boolean;
  voice_commit_mode?: boolean;
  /** The OS release the host is running on, for the feedback page to attach. */
  os_version?: string;
  /** Why the Linux desktop panel drawing the candidate list ignores the candidate font, colours and skin, as the running host reported it. Absent when the panel honours them. */
  candidate_panel_limit?: "gnome_shell" | "fcitx_theme" | "kimpanel";
}

export { useCandidatePreviewTheme } from "./candidate/candidate-preview-theme";

export type Preferences = {
  theme?: ThemeMode;
  settings_theme?: SurfaceTheme;
  candidate_theme?: SurfaceTheme;
  toolbar_theme?: SurfaceTheme;
  screen_keyboard_theme?: SurfaceTheme;
  handwriting_theme?: SurfaceTheme;
  voice_theme?: SurfaceTheme;
  emoji_theme?: SurfaceTheme;
  menu_theme?: SurfaceTheme;
  ai_assistant?: AiAssistantPreferences;
  custom_translation?: { enabled: boolean; endpoint: string; api_key: string };
  tencent_tmt?: { enabled: boolean; secret_id: string; secret_key: string; region: string };
  niutrans?: { enabled: boolean; app_id: string; apikey: string };
  voice_input?: VoiceInputPreferences;
  local_modes?: LocalModePreferences;
  clipboard_history?: boolean;
  cloud_candidates?: boolean;
  candidate_translations?: boolean;
  candidate_english_gloss?: boolean;
  english_suggestions?: boolean;
  translation_target_language?: "en" | "fr" | "ja" | "es" | "ru" | "de" | "ko";
  /** Optional second candidate-translation language; null/absent keeps one gloss row. */
  translation_secondary_language?: "en" | "fr" | "ja" | "es" | "ru" | "de" | "ko" | null;
  /** The user explicitly chose the MSIME account (api.msime.app) for candidate translations; absent means not chosen. */
  translation_account?: boolean;
  /** Anonymous start and crash events; off by default and honoured only by the Windows Server. */
  telemetry_enabled?: boolean;
  floating_toolbar?: FloatingToolbarPreferences;
  mixed_input?: MixedInputPreferences;
  fuzzy_pinyin?: FuzzyPinyinPreferences;
  frequency?: FrequencyPreferences;
  word_character?: { enabled: boolean; keys: "brackets" | "minus_equal" };
  navigation?: NavigationPreferences;
  keybindings?: KeybindingPreferences;
  scheme: "quanpin" | "shuangpin" | "wubi" | "japanese";
  /** Width used when desktop hosts commit printable ASCII characters. */
  character_width?: "halfwidth" | "fullwidth";
  wubi_code_hint?: boolean;
  touch_keyboard_layout?: "twenty_six_key" | "nine_key" | "handwriting";
  touch_keyboard_skin?: TouchKeyboardSkin;
  custom_touch_keyboard_skin?: TouchKeyboardSkinDesign;
  touch_keyboard_schemes?: TouchKeyboardSchemePreferences;
  touch_key_spacing_tenths?: number;
  touch_row_spacing_tenths?: number;
  touch_keyboard_height_adjustment?: number;
  touch_voice_shortcut?: boolean;
  touch_toolbar?: Partial<TouchToolbarPreferences>;
  default_ime_mode?: "chinese" | "english";
  ime_mode_scope?: "app" | "global";
  last_chinese_scheme?: "quanpin" | "shuangpin" | "wubi" | null;
  shuangpin_profile: "xiaohe" | "ziranma" | "shoudao" | "microsoft";
  /** macOS exposes the native shuangpin preedit presentation in the appearance page. */
  shuangpin_preedit_uses_raw?: boolean;
  wubi_mixed_pinyin?: boolean;
  candidate_page_size: number;
  number_row_selection?: boolean;
  candidate_font_size?: number;
  candidate_preedit_font_size?: number;
  candidate_text_color?: string | null;
  candidate_follow_cursor?: boolean;
  /** macOS-only non-activating badge shown after switching Chinese/English input. */
  input_mode_hud?: boolean;
  candidate_number_color?: string | null;
  candidate_accent_color?: string | null;
  candidate_selected_color?: string | null;
  candidate_hover_color?: string | null;
  candidate_surface_color?: string | null;
  candidate_border_color?: string | null;
  candidate_font_family?: string;
  candidate_english_font?: string | null;
  candidate_fallback_fonts?: string[];
  candidate_layout?: "horizontal" | "vertical";
  /** Inline (host-drawn) preedit. Linux applies it via ClientEngine preedit_style(). */
  tsf_preedit_style?: "raw" | "pinyin" | "empty";
  candidate_preedit_style?: "pinyin" | "empty";
  candidate_skin?: string;
  learning: boolean;
  autocorrect?: boolean;
  diagnostic_log?: { server?: boolean; tsf?: boolean };
  quanpin?: {
    autocorrect_transposition?: boolean;
    autocorrect_neighbor?: boolean;
  };
  quanpin_helpcode?: HelpcodePreferences;
  shuangpin_helpcode?: HelpcodePreferences;
  chinese_punctuation: boolean;
  smart_punctuation?: boolean;
  smart_punctuation_repeat?: boolean;
  smart_punctuation_space_convert?: boolean;
  smart_punctuation_direct_digit?: boolean;
  smart_punctuation_direct_letter?: boolean;
  paired_punctuation?: boolean;
  punctuation_lock?: "follow" | "chinese" | "english";
  traditional_chinese_output?: boolean;
};
export type AiAssistantPreferences = {
  enabled: boolean;
  provider: string;
  model: string;
  endpoint: string;
  candidate_limit: number;
  token?: string;
  tokens?: Record<string, string>;
  prompt_id?: string;
  prompt?: string;
  prompt_custom_1: string;
  prompt_custom_2: string;
  prompt_custom_3: string;
};
/** What the Linux online provider holds, never the secrets themselves. */
export type ProviderCredentialStatus = {
  ai: { provider: string; endpoint: string; model: string }[];
  /** The AI file exists but the provider refuses it, so no AI provider works until it is repaired. */
  aiInvalid: boolean;
  tencent: { region: string } | null;
  tencentInvalid: boolean;
  voiceAsr: VoiceProviderCredential[];
  voicePolish: VoiceProviderCredential[];
  /** The voice file exists but the provider refuses it, so voice input does not start until it is repaired. */
  voiceInvalid: boolean;
};
/** A stored voice entry; empty `model` / `endpoint` mean the provider's defaults. */
export type VoiceProviderCredential = {
  provider: string;
  model: string;
  endpoint: string;
  resourceId: string | null;
  authMode: string | null;
};
export type VoiceCredentialKind = "asr" | "polish";
export type VoiceCredentialSaveResult = {
  status: ProviderCredentialStatus;
  /** Whether the user service manager accepted the voice socket change; the file is saved either way. */
  serviceUpdated: boolean;
};
/**
 * The Linux host's owner-only credential files for the online provider service. A secret passed as undefined keeps the stored one, so an endpoint, model or region can change without pasting the key again.
 */
export type ProviderCredentialClient = {
  status(): Promise<ProviderCredentialStatus>;
  saveAi(credential: {
    provider: string;
    endpoint: string;
    model: string;
    token?: string;
  }): Promise<ProviderCredentialStatus>;
  clearAi(provider: string): Promise<ProviderCredentialStatus>;
  saveTencent(credential: {
    secretId?: string;
    secretKey?: string;
    region: string;
  }): Promise<ProviderCredentialStatus>;
  clearTencent(): Promise<ProviderCredentialStatus>;
  saveVoice(credential: {
    kind: VoiceCredentialKind;
    provider: string;
    endpoint: string;
    model: string;
    token?: string;
    appKey?: string;
    resourceId: string;
    authMode: string;
  }): Promise<VoiceCredentialSaveResult>;
  clearVoice(kind: VoiceCredentialKind, provider: string): Promise<VoiceCredentialSaveResult>;
};
export type AiAssistantClient = {
  // `provider` is for the hosts whose provider service holds the credential: it
  // is what that service checks its private configuration against. The hosts that
  // hold the token themselves ignore it and authenticate with `token`.
  fetchModels(configuration: {
    endpoint: string;
    token: string;
    provider: string;
  }): Promise<string[]>;
  test(configuration: {
    endpoint: string;
    model: string;
    prompt: string;
    token: string;
    text: string;
    provider: string;
  }): Promise<string>;
};
export type ApiCredentialTestService =
  | "translation.tencent"
  | "translation.niutrans"
  | "translation.custom"
  | "voice.asr"
  | "voice.polish"
  | "ai.assistant";
export type ApiCredentialTestResult = { ok: boolean; message: string };
export type VoiceInputPreferences = {
  enabled: boolean;
  language: string;
  capture_backend?: "" | "auto" | "pulse" | "pipewire" | "alsa" | "windows" | "macos" | "harmony";
  capture_device?: string;
  asr_provider?: string;
  asr_endpoint?: string;
  asr_token?: string;
  asr_tokens?: Record<string, string>;
  asr_app_key?: string;
  doubao_auth_mode?: "api_key" | "legacy";
  hotkey_ralt?: boolean;
  hotkey_ctrl_f9?: boolean;
  hotkey_ctrl_win?: boolean;
  hotkey_rctrl_ralt?: boolean;
  hotkey_hold_space_lock?: boolean;
  sound_enabled?: boolean;
  start_sound?: boolean;
  end_sound?: boolean;
  mute_system_audio?: boolean;
  polish_enabled?: boolean;
  polish_text?: boolean;
  asr_model?: string;
  /** Absolute path the `local` provider loads: an installed model directory (one holding msime-model.json) or a Whisper model file. */
  asr_model_path?: string;
  /** Optional `https://` prefix put in front of every model download URL (a ghproxy-style mirror); empty downloads from the catalog URLs as-is. */
  asr_model_mirror?: string;
  asr_resource_id?: string;
  commit_mode?: "tsf" | "sendinput" | "ctrl_v";
  polish_provider?: string;
  polish_endpoint?: string;
  polish_token?: string;
  polish_tokens?: Record<string, string>;
  polish_model?: string;
  polish_prompt_id?: string;
  polish_prompt?: string;
  polish_prompt_custom_1?: string;
  polish_prompt_custom_2?: string;
  polish_prompt_custom_3?: string;
  stream_inline_preedit?: boolean;
  doubao_enable_itn?: boolean;
  doubao_enable_punc?: boolean;
  doubao_enable_ddc?: boolean;
  doubao_boosting_table_id?: string;
  [key: string]: unknown;
};
export type ExternalSkinCatalog = {
  scanned: boolean;
  directory?: string;
  revision?: number;
  packages: Array<{ id: string; title: string; description?: string; valid?: boolean }>;
  issues?: string[];
};
export type Snapshot = {
  format_version: number;
  revision: number;
  preferences: Preferences;
  candidate_skin_catalog?: ExternalSkinCatalog;
};
export type DictionaryFailure = { request_id: string; label: string; error: string };
/** Mirrors the import response from `client-core::dictionary_import`. */
export interface DictionaryImportResult {
  applied: number;
  /** Rows examined and skipped. Absent from hosts that predate the report. */
  failed?: number;
  /** Rows beyond the per-file cap were not examined. */
  truncated?: boolean;
  /** The file was read with its two columns the other way round from the format chosen. */
  swapped?: boolean;
  first_failures?: { line: number; issue: string }[];
}
/** What the packaged dictionary is: the specification it was built to, and where it came from. */
export type DictionaryManifest = { profile: string; sourceCommit: string };

export interface DictionaryClient {
  list(
    offset: number,
    limit: number,
    kind?: LocalDictionaryKind,
    query?: string,
  ): Promise<{
    entries: DictionaryEntry[];
    has_more: boolean;
    pending_count?: number;
    failed_requests?: DictionaryFailure[];
    snapshot_error?: string | null;
    page_offset?: number;
    requested_page_offset?: number;
  }>;
  edit(
    previous: DictionaryEntry | null,
    replacement: DictionaryEntry | null,
    request_id: string,
  ): Promise<void>;
  import?(
    kind: LocalDictionaryKind,
    format: LocalDictionaryFormat,
    text: string,
    request_id: string,
  ): Promise<DictionaryImportResult>;
  /** The largest file, in bytes, the page reads for `import`. */
  maxImportFileBytes?: number;
  importPersonal?(
    text: string,
    request_id: string,
  ): Promise<{ queued: boolean; pending_count: number }>;
  export?(
    kind: LocalDictionaryKind,
    format: Exclude<LocalDictionaryFormat, "rime" | "hans">,
    offset: number,
    limit: number,
  ): Promise<{ text: string; has_more: boolean }>;
  retry?(request_id: string): Promise<void>;
  dismissFailure?(request_id: string): Promise<void>;
}
export type FloatingToolbarPreferences = {
  enabled: boolean;
  english_mode: boolean;
  fullwidth: boolean;
  punctuation: boolean;
  character_set: boolean;
  emoji: boolean;
  handwriting: boolean;
  screen_keyboard: boolean;
  voice: boolean;
  settings: boolean;
  scale_percent: 75 | 100 | 125 | 150;
  font_size: 16 | 18 | 20 | 22 | 24 | 26 | 28;
};
import { defaultFloatingToolbar } from "./settings/floating-toolbar-defaults";
/** What the macOS settings app did with the input method it carries when it started. */
export {
  InputSourceStartupNotice,
  type InputSourceStartupStatus,
} from "./settings/input-source-startup-notice";
export interface SettingsClient {
  /** What the surrounding host can do. Absent hosts fall back to user-agent detection. */
  host?: HostCapabilities;
  /** Android's platform-adapted Apple-style keyboard home surface. */
  home?: HomePageActions;
  /** Mobile account commands expose user/profile DTOs but never session tokens. */
  account?: AccountClient;
  /** Mobile hosts expose platform-native launcher or alternate icon selection. */
  appIcon?: AppIconClient;
  /** Mobile account commands expose the authenticated EveryAPI chat surface. */
  chat?: ChatClient;
  /** Android performs user-configured AI service requests in its native host. */
  aiAssistant?: AiAssistantClient;
  /** Linux writes AI and translation credentials to the online provider's owner-only files. */
  providerCredentials?: ProviderCredentialClient;
  /** Native hosts test credentials without exposing private provider secrets to the webview. */
  testApiCredential?: (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ) => Promise<ApiCredentialTestResult>;
  /** Android community commands expose bounded public skin metadata and designs. */
  communitySkins?: CommunitySkinClient;
  /** Android community commands expose dictionaries and reply templates. */
  communityResources?: CommunityResourceClient;
  listVoiceCaptureDevices?: VoiceDeviceReader;
  /**
   * The user's own candidate glosses. Windows delivers these as a file dropped in the profile
   * directory; a host whose user data lives in an app sandbox has to offer a way in instead.
   */
  customTranslations?: { load(): Promise<string>; save(text: string): Promise<void> };
  listFontFamilies?: FontCatalogReader;
  resolveFontFamilies?: (names: string[]) => Promise<string[]>;
  scanSkinCatalog?: () => Promise<SkinCatalog>;
  readSkinImage?: SkinImageReader;
  readSkinFont?: SkinFontReader;
  readSkinToolbarCss?: (id: string, relative?: string) => Promise<string | null>;
  openSkinDirectory?: () => Promise<void>;
  /**
   * Shows the input method's diagnostic log in the file manager so it can be sent after a reproduction. The host resolves the location itself; absent on hosts without a reachable file manager, which then show no button.
   */
  openDiagnosticLogDirectory?: () => Promise<void>;
  /**
   * Write an exported document into the user's Downloads folder and resolve to the absolute path written, which may carry a " (2)" suffix when the name was taken. A host whose webview drops download links (the macOS WKWebView cancels them) offers this; without it the page falls back to a download link.
   */
  saveExport?: (name: string, contents: string) => Promise<string | null>;
  load(): Promise<Snapshot>;
  save(revision: number, preferences: Preferences): Promise<Snapshot>;
  onPreferencesChanged?(listener: (snapshot: Snapshot) => void): Promise<() => void>;
  dictionary?: DictionaryClient;
  /** macOS can atomically restore packaged dictionaries and clear all learning state. */
  resetLearnedData?: () => Promise<void>;
  /**
   * What a restore-to-defaults would write, without writing it. The host decides what survives --
   * the credentials, and the endpoint and model that address the same service -- because that rule
   * belongs with the document, not with this page.
   */
  loadDefaultPreferences?: () => Promise<Preferences>;
  /**
   * Repair a preferences document the host cannot read, as the Windows source repairs a config.toml that does not parse. The host backs the damaged file up beside it first, then keeps every setting and service key it still recognises. `backupPath` is absent when the document already loaded and nothing was written.
   */
  recoverPreferences?: () => Promise<PreferencesRecovery>;
  /** Open the folder holding the preferences document, where a repair leaves its backup. */
  openPreferencesDirectory?: () => Promise<void>;
  readAppVersion?: () => Promise<string>;
  openExternalUrl?: (url: string) => Promise<void>;
  /** macOS opens the versioned third-party notices shipped with the app bundle. */
  openThirdPartyLicenses?: () => Promise<void>;
  /** macOS keeps the native shuangpin keymap panel preference outside shared Engine preferences. */
  loadMacosShuangpinKeymap?: () => Promise<boolean>;
  saveMacosShuangpinKeymap?: (enabled: boolean) => Promise<void>;
  /** macOS keeps Wubi unique-candidate auto-commit in the native input-method defaults domain. */
  loadMacosWubiAutoCommitUnique?: () => Promise<boolean>;
  saveMacosWubiAutoCommitUnique?: (enabled: boolean) => Promise<void>;
  copyText?: (text: string) => Promise<void>;
  /** The desktop hosts ship `msime-mcp` beside the settings app and report where it is and the entry an AI assistant runs it with. */
  mcpServerStatus?: () => Promise<McpServerStatus>;
  /** Write that entry into an assistant's configuration file. A different `msime` entry there rejects with code `mcp_entry_exists` unless `replace` is set. */
  installMcpClient?: (client: McpClientId, replace: boolean) => Promise<McpInstallOutcome>;
  /** Mobile hosts can open the platform keyboard/input-method settings. */
  openSystemKeyboardSettings?: () => Promise<void>;
  /** Mobile hosts persist keyboard sound and haptic feedback in native preferences. */
  mobileKeyboardFeedback?: MobileKeyboardFeedbackClient;
  openScreenKeyboard?: () => Promise<void>;
  openHandwriting?: () => Promise<void>;
  openVoice?: () => Promise<void>;
  /** Opens the 背单词 panel. Absent on hosts with no panel windows; the settings page then keeps
   * the review inline rather than offering a button that opens nothing. */
  openVocabulary?: () => Promise<void>;
  openCloudClipboard?: () => Promise<void>;
  openCloudDictionary?: () => Promise<void>;
  restartInputMethod?: () => Promise<void>;
  /** macOS installs/updates the separate InputMethodKit bundle before registering it. */
  installInputSource?: () => Promise<void>;
  /** macOS installs or refreshes the input method on every start; this reports what that did. */
  inputSourceStartup?: {
    /** Resolves once the start-time check has finished; `null` when it did not run for this launch. */
    status(): Promise<InputSourceStartupStatus | null>;
    /** Opens the System Settings page where input sources are added and enabled. */
    openSettings(): Promise<void>;
  };
  /**
   * macOS translates the Chinese candidates no offline dictionary answers, whole sentences included, with Apple's on-device models, but only for a language pair already downloaded in System Settings.
   */
  onDeviceTranslation?: {
    /** Target language codes the input method last found downloadable but not yet downloaded. */
    downloadableLanguages(): Promise<string[]>;
    /** Opens 语言与地区, where 翻译语言 are downloaded. */
    openSettings(): Promise<void>;
  };
  /** macOS moves the installed input source to Trash; data removal is explicit. */
  uninstallInputSource?: (removeUserData: boolean) => Promise<void>;
  /** macOS keeps small fixed locators while the state root itself may move to another volume. */
  dataDirectory?: {
    status(): Promise<{ path: string; isDefault: boolean }>;
    pick(): Promise<string | null>;
    /** `inputMethodRestarted` is false when the data moved but the host could not restart the input method; absent means the host restarts nothing itself or it succeeded. */
    move(): Promise<{
      path: string;
      isDefault: boolean;
      retainedOldData: boolean;
      inputMethodRestarted?: boolean;
    }>;
  };
  /**
   * Ask the host for a file path, resolving to null when the user cancels. A local speech model is loaded
   * by path and a file input hands back contents instead, so only the host can answer this.
   */
  pickVoiceModelPath?: () => Promise<string | null>;
  /** The host's on-device speech model store; hosts that provide it offer the `local` provider with a model manager. */
  localVoiceModels?: LocalVoiceModelClient;
  windowControl?: (action: "minimize" | "maximize" | "restore" | "close") => Promise<void>;
  beginWindowDrag?: () => Promise<void>;
  resizeWindow?: (edge: "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw") => Promise<void>;
  onWindowStateChanged?: (
    listener: (maximized: boolean) => void,
    onError?: () => void,
  ) => Promise<() => void>;
  clipboard?: ClipboardHistoryClient;
  typingStatistics?: TypingStatisticsClient;
  /** Absent on a host that has not wired the shared vocabulary entry point. */
  vocabularyReview?: VocabularyReviewClient;
  /** The host exposes the shared fuzzy-pinyin settings. */
  fuzzyPinyin?: boolean;
  /** Android exposes Apple-compatible touch-keyboard scheme visibility and selection. */
  touchKeyboardSchemes?: boolean;
  /** Android exposes Apple's current custom touch-keyboard design editor and renderer. */
  customTouchKeyboardSkins?: boolean;
  /** Named custom designs use a separate bounded file, outside hot-path preferences. */
  customSkinLibrary?: CustomSkinLibraryClient;
  /** Android account-backed AI skin draw and artwork jobs. */
  aiSkins?: AiSkinClient;
  /** Mobile and desktop hosts can show packaged offline English glosses without changing candidate identity. */
  candidateEnglishGloss?: boolean;
  /**
   * Which packaged dictionary is installed, for the dictionary page to state.
   *
   * A host that stages its resources into a sandbox is the only thing that knows where the
   * manifest ended up, and the path is not something this page should be told. Absent means the
   * host cannot answer, and the section is not drawn: a dictionary version stated wrongly is worse
   * than one not stated at all.
   */
  dictionaryManifest?: () => Promise<DictionaryManifest>;
}

export interface PreferencesRecovery {
  snapshot: Snapshot;
  backupPath?: string | null;
  salvaged: boolean;
}

/** What the platform calls itself, for text a person reads rather than a switch the code takes. */
/**
 * Mirrors the SecretId/Region rules in `Preferences::validate`. Saving a value
 * outside them is rejected wholesale, so the user is told here instead of
 * losing the save with no explanation.
 */
export function SettingsPage({
  client,
  initialPage,
  route,
  onReplayOnboarding,
}: {
  client: SettingsClient;
  /** The section the page opens on. Read once, at mount. */
  initialPage?: string;
  /** A section requested after mount; a new `nonce` navigates there and keeps the draft. */
  route?: { page: string; nonce: number };
  onReplayOnboarding?: () => void;
}) {
  const { confirm, confirmation } = useConfirm();
  // Hosts that report capabilities are authoritative; the user-agent probe stays
  // only so a host that predates the contract keeps its current behaviour.
  const linuxPlatform = client.host ? client.host.platform === "linux" : isLinuxDesktop();
  const androidPlatform = client.host?.platform === "android";
  const iosPlatform = client.host?.platform === "ios";
  // This repository ships the HarmonyOS host too, so its release, license and issue links follow the client-hosted set rather than the Windows ones.
  const harmonyPlatform = client.host?.platform === "harmony";
  const mobilePlatform =
    client.host?.mobile_settings ?? (iosPlatform || androidPlatform || harmonyPlatform);
  // Ctrl+Space belongs to Windows, not to us, so only that host gets the note
  // explaining where to change it.
  const windowsPlatform = client.host?.platform === "windows";
  const macosPlatform = client.host?.platform === "macos";
  // Hosts with a system recogniser of their own. Android's is what it falls back to when nothing
  // is configured, so `system` is a real choice there rather than a value to report unavailable.
  const nativeVoicePlatform = macosPlatform || harmonyPlatform || androidPlatform;
  // Functional controls follow what the host declares it can do. Only the prose
  // below still varies by platform name. A host that predates the contract keeps
  // the previous Linux-only behaviour.
  const host = client.host;
  const showModeScope = host ? host.ime_mode_scope : linuxPlatform;
  const showModeSwitchShortcuts = host ? host.mode_switch_shortcuts : linuxPlatform;
  const showPanelShortcuts = host ? host.panel_shortcuts : linuxPlatform;
  const showNumberRowSelection = host ? host.number_row_selection === true : linuxPlatform;
  const showRestartInputMethod =
    (host ? host.restart_input_method : linuxPlatform) && client.restartInputMethod;
  const showInstallInputSource = macosPlatform && client.installInputSource;
  const showFloatingToolbar = host ? host.floating_toolbar : true;
  // An IBus property menu has no scale or icon size to apply, but it can
  // expose component visibility as individual menu entries.
  const showToolbarAppearance = host ? host.floating_toolbar_appearance : true;
  const showToolbarComponents = host ? host.floating_toolbar_components : true;
  const showCandidateFontControls = host ? host.candidate_font_controls : true;
  // Linux sets the panel's font from the family and candidate size, but the composition is drawn by
  // the focused application there, so a preedit size would be a control with nothing to change.
  const showCandidatePreeditFont =
    showCandidateFontControls && (host?.candidate_preedit_font ?? true);
  // Was a list of platform names, which is how HarmonyOS came to consume the preference without
  // anyone being able to set it. A host that predates the capability keeps the old reading.
  const showCandidateEnglishFont =
    host?.candidate_english_font ?? (windowsPlatform || macosPlatform || androidPlatform);
  // iOS shows its own switch for the same surface, from the native store, so it is not here.
  const showEnglishSuggestions = host?.english_suggestions ?? androidPlatform;
  // Which hosts mark a helper code with Shift rather than appending it to a finished spelling.
  // Was a platform name, which is how HarmonyOS came to run the same ported policy and show the
  // page without the one sentence that says how to type one.
  const showHelpcodeShiftEntry = host?.helpcode_shift_entry ?? androidPlatform;
  // Every host's Engine honours the preference; this is about which of them draw the composition
  // themselves, and so show the user a difference between the raw keys and the expanded pinyin.
  const showShuangpinPreedit = host?.shuangpin_preedit ?? macosPlatform;
  // Was hidden for every touch platform, on the reading that a phone keyboard has no width to switch. It has: every keyboard, iOS included, routes it to the runtime the same way the desktop hosts do, and reaches it from its own surfaces.
  const showCharacterWidth = host?.character_width ?? !mobilePlatform;
  // The host's provider holds the AI credential, so the page does not ask for a token and does not
  // withhold the service controls for want of one. Reaching the service still works - through that
  // provider - which is why these controls are offered rather than hidden.
  const aiProviderCredentials = host?.ai_provider_credentials ?? linuxPlatform;
  // A host with one way to commit a recognized result has nothing to choose between, and a select with one outcome reads as a setting being ignored. The Linux hosts commit only through IBus or Fcitx5, so the fallback for a page without host capabilities withholds it there too.
  const showVoiceCommitMode = host?.voice_commit_mode ?? (!androidPlatform && !linuxPlatform);
  // These read `!androidPlatform` because that host once had only the platform recogniser. It runs
  // the configured provider now, uploads and streaming socket both, so keying on the name would
  // leave a user unable to configure something the host honours. What it still cannot do is draw
  // interim text, and that switch stays hidden for exactly that reason.
  const showVoiceProviderSettings = host?.voice_provider_settings ?? !androidPlatform;
  const showVoiceStreamPreedit = host?.voice_stream_preedit ?? !androidPlatform;
  const showCandidateRowColors = host ? host.candidate_row_colors : true;
  const showCandidateSelectionAppearance = host ? host.candidate_selection_appearance : true;
  const showCandidateBorderColor = host
    ? (host.candidate_border_color ?? host.candidate_selection_appearance)
    : true;
  const showCandidateFollowCursor = host ? host.candidate_follow_cursor : false;
  // Was written as "macOS only" when macOS was the only host that drew the badge. A host that
  // predates the capability keeps that reading rather than losing a control it does honour; one
  // that declares it decides for itself, which is how HarmonyOS's 2in1 badge reaches the page.
  const showInputModeHUD = host?.input_mode_hud ?? macosPlatform;
  const showVoiceCaptureDevices =
    !androidPlatform &&
    (host ? host.voice_capture_devices : linuxPlatform) &&
    client.listVoiceCaptureDevices;
  // panel_windows is the injected projection of host_surface::is_desktop, so this follows the capability instead of listing the mobile hosts by name and missing the next one.
  // Either the host draws desktop panels, or it says outright that it routes the chords. The
  // second half is why this is no longer read off `panel_windows` alone: a keyboard attached to a
  // phone sends them just as well, and the host that gained them has no desktop panels at all.
  const showDesktopMaintenanceShortcuts =
    (host?.maintenance_shortcuts ?? false) || !host || host.panel_windows;
  const showFullwidthChord = host?.fullwidth_chord ?? macosPlatform;
  const fullwidthChord = macosPlatform ? "Option+Shift+H" : "Alt+Shift+H";
  const maintenanceChord = macosPlatform ? "Ctrl+Shift+Option" : "Ctrl+Shift+Alt";
  // Windows is built from this repository now too, so it reads this repository's releases; msime.app/update.json describes the reference Windows product and names its repository, which the validation below rightly refuses.
  const clientHostedPlatform =
    windowsPlatform ||
    linuxPlatform ||
    androidPlatform ||
    macosPlatform ||
    harmonyPlatform ||
    host?.platform === "ios";
  const platformReleasesPageUrl = clientHostedPlatform ? linuxReleasesPageUrl : releasesPageUrl;
  const platformLicenseUrl = clientHostedPlatform ? linuxLicenseUrl : licenseUrl;
  const platformIssuesUrl = linuxIssuesUrl;
  const captureBackendOptions: readonly (readonly [
    NonNullable<VoiceInputPreferences["capture_backend"]>,
    string,
  ])[] = [
    ["auto", "自动选择"],
    ...(linuxPlatform
      ? ([
          ["pulse", "PulseAudio"],
          ["pipewire", "PipeWire"],
          ["alsa", "ALSA"],
        ] as const)
      : []),
    ...(macosPlatform ? ([["macos", "CoreAudio"]] as const) : []),
    ...(windowsPlatform ? ([["windows", "Windows Audio"]] as const) : []),
    ...(harmonyPlatform ? ([["harmony", "HarmonyOS 音频"]] as const) : []),
  ];
  const {
    helpIntro: platformHelpIntro,
    quickStart: platformQuickStart,
    networkDescription: platformNetworkDescription,
    aboutDescription: platformAboutDescription,
  } = platformCopy({
    android: androidPlatform,
    linux: linuxPlatform,
    macos: macosPlatform,
    harmony: harmonyPlatform,
    ios: iosPlatform,
    mobile: mobilePlatform,
  } satisfies PlatformCopyContext);
  // Whether this host draws the shared panels as windows of its own — `panel_windows` is the
  // injected projection of `host_surface::is_desktop`.
  //
  // The modifier-chord voice shortcuts and the voice popup bar's theme were both gated on
  // `!androidPlatform`, so they reached every host that was not Android — including HarmonyOS and
  // iOS, neither of which has a Ctrl, an Alt or a Win key to press or a panel window to theme. A
  // phone was being shown `Ctrl+F9 切换语音`.
  //
  // Falls back to the form factor when the host answers without the flag: a partial capability
  // record would otherwise read as "not a desktop" and hide these from Windows too.
  const desktopPanels = host ? (host.panel_windows ?? !mobilePlatform) : true;
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [draft, setDraft] = useState<Preferences>();
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  /** The backup the last repair wrote, while its notice is showing. */
  const [recoveredBackup, setRecoveredBackup] = useState("");
  const [removeUserDataOnUninstall, setRemoveUserDataOnUninstall] = useState(false);
  const [uninstallConfirmation, setUninstallConfirmation] = useState(false);
  const [uninstallBusy, setUninstallBusy] = useState(false);
  const [uninstallResult, setUninstallResult] = useState<"success" | "error" | null>(null);
  const [dataDirectory, setDataDirectory] = useState<{ path: string; isDefault: boolean }>();
  const [dataDirectoryBusy, setDataDirectoryBusy] = useState(false);
  const [dataDirectoryResult, setDataDirectoryResult] = useState("");
  const [inputSourceStartup, setInputSourceStartup] = useState<InputSourceStartupStatus | null>(
    null,
  );
  const [onDeviceDownloadable, setOnDeviceDownloadable] = useState<string[]>([]);
  const restoredMobilePage =
    mobilePlatform &&
    typeof window !== "undefined" &&
    window.history.state?.msimeSettings === true &&
    typeof window.history.state.page === "string"
      ? window.history.state.page
      : undefined;
  const [page, setPage] = useState<SettingsPageId>(() =>
    requestedPage(initialPage ?? restoredMobilePage ?? (client.home ? "home" : undefined)),
  );
  const [accountLoginReturnPage, setAccountLoginReturnPage] = useState<SettingsPageId | null>(null);
  // Each bottom tab owns a navigation stack in the source app. This shared page has a flat route,
  // so remember the visible leaf for each tab: leaving 输入 for 社区 and returning to 键盘 must
  // restore 输入 rather than reset the first tab to 首页.
  const mobileInitialTab = mobileTabForPage(page);
  const mobileLastPageByTab = useRef<Record<MobilePrimaryPageId, SettingsPageId>>({
    home: mobileInitialTab === "home" ? page : "home",
    community: mobileInitialTab === "community" ? page : "community",
    "typing-statistics": mobileInitialTab === "typing-statistics" ? page : "typing-statistics",
    account: mobileInitialTab === "account" ? page : "account",
  });
  const settingsContentRef = useRef<HTMLElement>(null);
  // Every settings category shares this one scrolling surface. Reset it after
  // the new category is committed so sidebar clicks, in-page links and mobile
  // back navigation all open the destination at its beginning.
  useLayoutEffect(() => {
    if (settingsContentRef.current) settingsContentRef.current.scrollTop = 0;
  }, [page]);
  // Mobile hosts use the WebView history stack for the system back gesture. The
  // native activity can therefore dismiss a nested page without the shared UI
  // having to know which Android/iOS navigation API is in use.
  useEffect(() => {
    if (!mobilePlatform || typeof window === "undefined") return;
    const current = window.history.state;
    if (!current || current.msimeSettings !== true) {
      window.history.replaceState(
        { ...(current && typeof current === "object" ? current : {}), msimeSettings: true, page },
        "",
      );
    }
    const onPopState = (event: PopStateEvent) => {
      const state = event.state;
      if (state?.msimeSettings === true && typeof state.page === "string") {
        const restored = requestedPage(state.page);
        mobileLastPageByTab.current[mobileTabForPage(restored)] = restored;
        setPage(restored);
      }
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, [mobilePlatform]);
  const [communityDestination, setCommunityDestination] = useState<
    AccountCommunityDestination | "all"
  >("all");
  const [updateStatus, setUpdateStatus] = useState("");
  const [updateBusy, setUpdateBusy] = useState(false);
  const [availableUpdate, setAvailableUpdate] = useState<ValidatedUpdate | null>(null);
  const [currentAppVersion, setCurrentAppVersion] = useState(fallbackAppVersion);
  const [feedbackCopied, setFeedbackCopied] = useState(false);
  const [feedbackKind, setFeedbackKind] = useState("功能异常");
  const [feedbackDetail, setFeedbackDetail] = useState("");
  const [feedbackReportCopied, setFeedbackReportCopied] = useState(false);
  const [mobileKeyboardFeedback, setMobileKeyboardFeedback] = useState<MobileKeyboardFeedback>();
  const [mobileKeyboardFeedbackBusy, setMobileKeyboardFeedbackBusy] = useState(false);
  const [customTranslationsText, setCustomTranslationsText] = useState("");
  const [customTranslationsNotice, setCustomTranslationsNotice] = useState("");
  const [customTranslationsBusy, setCustomTranslationsBusy] = useState(false);
  const customTranslationsReport = useMemo(
    () => parseCustomTranslations(customTranslationsText),
    [customTranslationsText],
  );
  const customTranslationsSummary = customTranslationsText.trim()
    ? `${customTranslationsReport.entries.length} 条释义` +
      (customTranslationsReport.skipped ? `，${customTranslationsReport.skipped} 行无法识别` : "")
    : "还没有自定义释义。";
  const [macosShuangpinKeymap, setMacosShuangpinKeymap] = useState<boolean>();
  const [macosWubiAutoCommitUnique, setMacosWubiAutoCommitUnique] = useState<boolean>();
  const [savedMacosWubiAutoCommitUnique, setSavedMacosWubiAutoCommitUnique] = useState<boolean>();
  const [phrases, setPhrases] = useState<DictionaryEntry[]>([]);
  const [phrasePage, setPhrasePage] = useState({ offset: 0, hasMore: false, status: "" });
  const [phraseBusy, setPhraseBusy] = useState(false);
  const [phraseError, setPhraseError] = useState("");
  const [dictionaryPendingCount, setDictionaryPendingCount] = useState(0);
  const [dictionaryFailures, setDictionaryFailures] = useState<DictionaryFailure[]>([]);
  const [dictionarySnapshotError, setDictionarySnapshotError] = useState("");
  const [phraseNotice, setPhraseNotice] = useState("");
  const [phraseSearch, setPhraseSearch] = useState("");
  const [phraseForm, setPhraseForm] = useState<{
    key: string;
    value: string;
    weight: number;
    previous: DictionaryEntry | null;
  } | null>(null);
  const [dictionaryKind, setDictionaryKind] = useState<LocalDictionaryKind>("quick_phrase");
  const [dictionaryFormat, setDictionaryFormat] = useState<LocalDictionaryFormat>("standard");
  const phraseRequestGeneration = useRef(0);
  const phraseListRef = useRef<HTMLUListElement>(null);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      phraseRequestGeneration.current += 1;
    };
  }, []);
  const [windowMaximized, setWindowMaximized] = useState(false);
  const [skinPreviewThemes, setSkinPreviewThemes] = useState<
    Partial<Record<NonNullable<Preferences["candidate_skin"]>, "light" | "dark">>
  >({});
  const [showTouchSkinEditor, setShowTouchSkinEditor] = useState(false);
  const touchGeometryDrag = useRef<{
    pointerId: number;
    x: number;
    y: number;
    key: number;
    row: number;
    axis: "key" | "row" | null;
  } | null>(null);
  const [aiModels, setAiModels] = useState<string[] | null>(null);
  const [aiModelsStatus, setAiModelsStatus] = useState("");
  const [aiModelsBusy, setAiModelsBusy] = useState(false);
  const [aiTestInput, setAiTestInput] = useState("");
  const [aiTestOutput, setAiTestOutput] = useState("");
  const [aiTestStatus, setAiTestStatus] = useState("");
  const [aiTestBusy, setAiTestBusy] = useState(false);
  const aiRequestGeneration = useRef(0);
  const [credentialTests, setCredentialTests] = useState<
    Partial<
      Record<
        ApiCredentialTestService,
        {
          signature: string;
          busy: boolean;
          ok?: boolean;
          message: string;
        }
      >
    >
  >({});
  const credentialTestGeneration = useRef<Partial<Record<ApiCredentialTestService, number>>>({});
  const [providerCredentials, setProviderCredentials] = useState<ProviderCredentialStatus>();
  const [aiCredentialInput, setAiCredentialInput] = useState("");
  const [tencentCredentialInput, setTencentCredentialInput] = useState<{
    secretId: string;
    secretKey: string;
    region?: string;
  }>({ secretId: "", secretKey: "" });
  const [voiceCredentialInput, setVoiceCredentialInput] = useState<
    Record<VoiceCredentialKind, { token: string; appKey: string; endpoint?: string }>
  >({ asr: { token: "", appKey: "" }, polish: { token: "", appKey: "" } });
  const [providerCredentialBusy, setProviderCredentialBusy] = useState<
    "ai" | "tencent" | VoiceCredentialKind
  >();
  const [providerCredentialMessages, setProviderCredentialMessages] = useState<
    Partial<Record<"ai" | "tencent" | VoiceCredentialKind, { ok: boolean; text: string }>>
  >({});
  // What a report needs first is the release and the scheme, because that is what a repro is
  // written against. The user agent only says which web view drew this window, so it is the
  // fallback for a host that cannot name its own OS rather than a line of its own.
  const supportDiagnostics = [
    `水杉 IME ${currentAppVersion}`,
    host?.os_version
      ? `${platformOsName(host.platform)} ${host.os_version}`
      : `平台：${host?.platform ?? (androidPlatform ? "android" : iosPlatform ? "ios" : "desktop")}`,
    draft ? `输入方案：${schemeTitle(draft.scheme)}` : "",
    host?.os_version || typeof navigator === "undefined"
      ? ""
      : `User-Agent：${navigator.userAgent.slice(0, 256)}`,
  ]
    .filter(Boolean)
    .join("\n");
  const feedbackReport = `### 类型\n${feedbackKind}\n\n### 描述\n${feedbackDetail}\n\n### 环境\n${supportDiagnostics}\n`;
  const submitFeedback = () => {
    if (!client.openExternalUrl) return;
    const body = feedbackReport.slice(0, 4000);
    const query = new URLSearchParams({ title: feedbackKind, body });
    void client.openExternalUrl(`${platformIssuesUrl}/new?${query.toString()}`);
  };
  useEffect(() => {
    if (!(macosPlatform || linuxPlatform) || !client.dataDirectory) return;
    let active = true;
    client.dataDirectory
      .status()
      .then((value) => {
        if (active) setDataDirectory(value);
      })
      .catch(() => {
        if (active) setDataDirectoryResult("无法读取当前数据目录。");
      });
    return () => {
      active = false;
    };
  }, [client, macosPlatform, linuxPlatform]);
  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    setWindowMaximized(false);
    const subscribe = client.onWindowStateChanged;
    if (subscribe) {
      void Promise.resolve()
        .then(() => {
          if (!active) return;
          return subscribe(
            (maximized) => {
              if (active) setWindowMaximized(maximized);
            },
            () => {
              if (active) setError("无法读取窗口状态，请重试。");
            },
          );
        })
        .then((value) => {
          if (active) unsubscribe = value;
          else value?.();
        })
        .catch(() => {
          if (active) setError("无法读取窗口状态，请重试。");
        });
    }
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [client]);
  useEffect(() => {
    const credentials = client.providerCredentials;
    if (!credentials) return;
    let active = true;
    void credentials
      .status()
      .then((status) => {
        if (active) setProviderCredentials(status);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [client]);
  useEffect(() => {
    let active = true;
    setCurrentAppVersion(fallbackAppVersion);
    if (client.readAppVersion) {
      void client
        .readAppVersion()
        .then((value) => {
          const version = parseVersion(value);
          if (active && version) setCurrentAppVersion(version.display);
        })
        .catch(() => undefined);
    }
    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    const custom = client.customTranslations;
    if (!custom) return;
    let active = true;
    void custom
      .load()
      .then((text) => {
        if (active) setCustomTranslationsText(text);
      })
      .catch(() => {
        // An unreadable overlay is not an error worth a dialog: the field stays empty and saving
        // it would simply write a new one.
      });
    return () => {
      active = false;
    };
  }, [client.customTranslations]);

  async function saveCustomTranslations() {
    const custom = client.customTranslations;
    if (!custom || customTranslationsBusy) return;
    if (!customTranslationsWithinBounds(customTranslationsText)) {
      setCustomTranslationsNotice("自定义释义过大，请精简后再保存。");
      return;
    }
    setCustomTranslationsBusy(true);
    try {
      await custom.save(customTranslationsText);
      setCustomTranslationsNotice(
        `已保存 ${customTranslationsReport.entries.length} 条释义，重新启动输入法后生效。`,
      );
    } catch (error) {
      setCustomTranslationsNotice(errorMessage(error));
    } finally {
      setCustomTranslationsBusy(false);
    }
  }

  useEffect(() => {
    if (!mobilePlatform || !client.mobileKeyboardFeedback) {
      setMobileKeyboardFeedback(undefined);
      return;
    }
    let active = true;
    void client.mobileKeyboardFeedback
      .load()
      .then((value) => {
        if (active) setMobileKeyboardFeedback(value);
      })
      .catch(() => {
        if (active) setError("无法读取按键反馈设置，请重试。");
      });
    return () => {
      active = false;
    };
  }, [client, mobilePlatform]);

  // The Windows installer registers the input method on every install and upgrade; on macOS the settings app does it when it starts, and this tells the user what happened and whether the source still has to be enabled in System Settings.
  useEffect(() => {
    if (!macosPlatform || !client.inputSourceStartup) {
      setInputSourceStartup(null);
      return;
    }
    let active = true;
    void client.inputSourceStartup
      .status()
      .then((value) => {
        if (active) setInputSourceStartup(value);
      })
      .catch(() => {
        if (active) setInputSourceStartup(null);
      });
    return () => {
      active = false;
    };
  }, [client, macosPlatform]);

  // The input method records the missing pairs when it next translates, so read again whenever the window comes back - typically from System Settings after a download.
  useEffect(() => {
    const onDeviceTranslation = client.onDeviceTranslation;
    if (!macosPlatform || !onDeviceTranslation || typeof window === "undefined") {
      setOnDeviceDownloadable([]);
      return;
    }
    let active = true;
    const refresh = () =>
      void onDeviceTranslation
        .downloadableLanguages()
        .then((codes) => {
          if (active) setOnDeviceDownloadable(codes);
        })
        .catch(() => {
          if (active) setOnDeviceDownloadable([]);
        });
    refresh();
    window.addEventListener("focus", refresh);
    return () => {
      active = false;
      window.removeEventListener("focus", refresh);
    };
  }, [client, macosPlatform]);

  useEffect(() => {
    if (!macosPlatform || !client.loadMacosShuangpinKeymap) {
      setMacosShuangpinKeymap(undefined);
      return;
    }
    let active = true;
    void client
      .loadMacosShuangpinKeymap()
      .then((value) => {
        if (active) setMacosShuangpinKeymap(value);
      })
      .catch(() => {
        if (active) setError("无法读取双拼键位提示设置，请重试。");
      });
    return () => {
      active = false;
    };
  }, [client, macosPlatform]);
  useEffect(() => {
    if (!macosPlatform || !client.loadMacosWubiAutoCommitUnique) {
      setMacosWubiAutoCommitUnique(undefined);
      setSavedMacosWubiAutoCommitUnique(undefined);
      return;
    }
    let active = true;
    void client
      .loadMacosWubiAutoCommitUnique()
      .then((value) => {
        if (!active) return;
        setMacosWubiAutoCommitUnique(value);
        setSavedMacosWubiAutoCommitUnique(value);
      })
      .catch(() => {
        if (active) setError("无法读取五笔自动上屏设置，请重试。");
      });
    return () => {
      active = false;
    };
  }, [client, macosPlatform]);
  const snapshotRef = useRef(snapshot);
  const draftRef = useRef(draft);

  useEffect(() => {
    snapshotRef.current = snapshot;
    draftRef.current = draft;
  }, [snapshot, draft]);

  // The refs are also written the moment a load or save resolves: the host's monitor echoes this
  // window's own save back as a change, and it can arrive before React commits the new snapshot.
  const adoptSnapshot = (value: Snapshot) => {
    if (!mounted.current) return;
    snapshotRef.current = value;
    draftRef.current = value.preferences;
    setSnapshot(value);
    setDraft(value.preferences);
  };
  // A change that arrives while this window's save is in flight waits for it rather than being
  // judged against the revision the save is about to replace; the revision check then drops the
  // echo and still applies another window's later write.
  const savingRef = useRef(false);
  const heldChange = useRef<Snapshot>(undefined);
  const applyPreferencesChange = (value: Snapshot) => {
    const currentSnapshot = snapshotRef.current;
    // Our own save echoed back, or an event older than what a reload already read.
    if (currentSnapshot && value.revision <= currentSnapshot.revision) return;
    const currentDraft = draftRef.current;
    const dirty =
      !!currentSnapshot &&
      !!currentDraft &&
      JSON.stringify(currentDraft) !== JSON.stringify(currentSnapshot.preferences);
    if (dirty) {
      setNotice("设置已被其他窗口修改。请重新读取后再保存。");
      return;
    }
    adoptSnapshot(value);
    setError("");
    setNotice("设置已从其他窗口更新。");
  };
  // The subscription outlives renders; it reaches the handler through this so it never runs a stale
  // one (the handler itself only touches refs and state setters).
  const applyPreferencesChangeRef = useRef(applyPreferencesChange);
  applyPreferencesChangeRef.current = applyPreferencesChange;

  useEffect(() => {
    if (!client.onPreferencesChanged) return;
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void client
      .onPreferencesChanged((value) => {
        if (!active) return;
        if (savingRef.current) {
          if (!heldChange.current || value.revision > heldChange.current.revision)
            heldChange.current = value;
          return;
        }
        applyPreferencesChangeRef.current(value);
      })
      .then((value) => {
        if (active) unsubscribe = value;
        else value();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [client]);

  useEffect(() => {
    let active = true;
    client
      .load()
      .then((value) => {
        if (active) adoptSnapshot(value);
      })
      .catch((reason) => {
        if (active) setError(errorMessage(reason));
      })
      .finally(() => {
        if (active) setBusy(false);
      });
    return () => {
      active = false;
    };
  }, [client]);

  async function reload() {
    if (!mounted.current) return;
    setBusy(true);
    setError("");
    setNotice("");
    setRecoveredBackup("");
    try {
      const value = await client.load();
      if (!mounted.current) return;
      adoptSnapshot(value);
    } catch (reason) {
      if (mounted.current) setError(errorMessage(reason));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  useEffect(() => {
    if (!mobilePlatform || typeof document === "undefined") return;
    let hidden = document.hidden;
    const onVisibilityChange = () => {
      const nextHidden = document.hidden;
      const resumed = hidden && !nextHidden;
      hidden = nextHidden;
      if (!resumed) return;
      const currentSnapshot = snapshotRef.current;
      const currentDraft = draftRef.current;
      const dirty =
        !!currentSnapshot &&
        !!currentDraft &&
        JSON.stringify(currentDraft) !== JSON.stringify(currentSnapshot.preferences);
      if (dirty) {
        setNotice("设置已被其他窗口修改。请重新读取后再保存。");
        return;
      }
      void reload();
    };
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => document.removeEventListener("visibilitychange", onVisibilityChange);
  }, [client, mobilePlatform]);

  async function save() {
    if (!draft || !snapshot || !validCandidateFonts(draft)) return;
    setBusy(true);
    setError("");
    setNotice("");
    setRecoveredBackup("");
    savingRef.current = true;
    try {
      const value = await client.save(snapshot.revision, draft);
      if (macosPlatform && client.saveMacosShuangpinKeymap && macosShuangpinKeymap !== undefined) {
        await client.saveMacosShuangpinKeymap(macosShuangpinKeymap);
      }
      if (
        macosPlatform &&
        client.saveMacosWubiAutoCommitUnique &&
        macosWubiAutoCommitUnique !== undefined
      ) {
        await client.saveMacosWubiAutoCommitUnique(macosWubiAutoCommitUnique);
        if (!mounted.current) return;
        setSavedMacosWubiAutoCommitUnique(macosWubiAutoCommitUnique);
      }
      if (!mounted.current) return;
      adoptSnapshot(value);
      setNotice("设置已保存。");
    } catch (reason) {
      if (mounted.current) setError(errorMessage(reason));
    } finally {
      if (mounted.current) setBusy(false);
      savingRef.current = false;
      const held = heldChange.current;
      heldChange.current = undefined;
      if (held) applyPreferencesChange(held);
    }
  }

  async function saveMobileKeyboardFeedback(next: MobileKeyboardFeedback) {
    const feedback = client.mobileKeyboardFeedback;
    if (!feedback) return;
    const previous = mobileKeyboardFeedback;
    setMobileKeyboardFeedback(next);
    setMobileKeyboardFeedbackBusy(true);
    setError("");
    try {
      setMobileKeyboardFeedback(await feedback.save(next));
    } catch {
      if (previous) setMobileKeyboardFeedback(previous);
      setError("无法保存按键反馈设置，请重试。");
    } finally {
      setMobileKeyboardFeedbackBusy(false);
    }
  }

  async function uninstallInputSource() {
    if (!client.uninstallInputSource || uninstallBusy) return;
    setUninstallBusy(true);
    setUninstallResult(null);
    try {
      await client.uninstallInputSource(removeUserDataOnUninstall);
      setUninstallConfirmation(false);
      setUninstallResult("success");
    } catch {
      setUninstallResult("error");
    } finally {
      setUninstallBusy(false);
    }
  }

  async function chooseDataDirectory() {
    if (!client.dataDirectory || dataDirectoryBusy) return;
    setDataDirectoryBusy(true);
    setDataDirectoryResult("");
    try {
      const target = await client.dataDirectory.pick();
      if (!target) return;
      const confirmed = await confirm({
        title: "移动输入法数据？",
        message: `词库、学习记录、皮肤、剪贴板历史和设置将移动到“${target}”。移动期间输入法会短暂退出；完成后设置窗口会关闭。`,
        confirmLabel: "移动",
      });
      if (!confirmed) return;
      const result = await client.dataDirectory.move();
      setDataDirectory({ path: result.path, isDefault: result.isDefault });
      const restartNote =
        result.inputMethodRestarted === false
          ? "输入法未能自动重启，请手动重启输入法后再继续输入。"
          : "";
      setDataDirectoryResult(
        result.retainedOldData
          ? `数据已切换到新目录；旧目录不属于水杉输入法，已为安全起见保留。${restartNote}设置窗口即将关闭。`
          : `数据已移动。${restartNote}设置窗口即将关闭，请重新打开后继续使用。`,
      );
    } catch (reason) {
      const code = errorCode(reason);
      setDataDirectoryResult(
        code === "data_directory_picker_unavailable"
          ? "未找到目录选择工具，请安装 zenity 或 kdialog 后重试。"
          : code === "data_directory_not_empty"
            ? "请选择空文件夹；现有文件不会被覆盖。"
            : code === "data_directory_invalid"
              ? "该位置不能作为数据目录，请选择其他空文件夹。"
              : code === "data_directory_busy"
                ? "输入法仍在使用数据目录，请稍后重试。"
                : "移动失败，仍在使用原目录，原有数据未被删除。",
      );
    } finally {
      setDataDirectoryBusy(false);
    }
  }

  async function previewMobileKeyboardHaptics() {
    const feedback = client.mobileKeyboardFeedback;
    if (!feedback?.preview || !mobileKeyboardFeedback?.hapticsEnabled) return;
    setError("");
    try {
      await feedback.preview(mobileKeyboardFeedback.hapticStrength);
    } catch {
      setError("无法预览按键振动，请重试。");
    }
  }

  async function resetTouchKeyboardSettings() {
    if (!draft) return;
    const confirmed = await confirm({
      title: "恢复屏幕键盘默认值",
      message: "高度、间距、顶部语音入口和工具栏按钮都会回到默认。",
      confirmLabel: "恢复",
    });
    if (!confirmed || !draft) return;
    const next = { ...draft };
    // Delete the optional fields instead of storing the current defaults. This keeps reset
    // forward-compatible when a host changes its fallback values.
    delete next.touch_key_spacing_tenths;
    delete next.touch_row_spacing_tenths;
    delete next.touch_keyboard_height_adjustment;
    delete next.touch_voice_shortcut;
    delete next.touch_toolbar;
    setDraft(next);
    setError("");
    setNotice("屏幕键盘设置已恢复默认，请点击保存设置。");
  }

  function beginTouchGeometryDrag(event: ReactPointerEvent<HTMLDivElement>) {
    if (!draft || (event.pointerType === "mouse" && event.button !== 0)) return;
    touchGeometryDrag.current = {
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      key: draft.touch_key_spacing_tenths ?? 60,
      row: draft.touch_row_spacing_tenths ?? 70,
      axis: null,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function updateTouchGeometryDrag(event: ReactPointerEvent<HTMLDivElement>) {
    const drag = touchGeometryDrag.current;
    if (!drag || drag.pointerId !== event.pointerId || !draft) return;
    const dx = event.clientX - drag.x;
    const dy = event.clientY - drag.y;
    if (!drag.axis && Math.abs(dx) + Math.abs(dy) < 4) return;
    drag.axis ??= Math.abs(dy) >= Math.abs(dx) ? "row" : "key";
    const delta = drag.axis === "row" ? dy : dx;
    const value = Math.round((drag.axis === "row" ? drag.row : drag.key) + (delta * 10) / 18);
    setDraft((current) =>
      current
        ? {
            ...current,
            ...(drag.axis === "row"
              ? { touch_row_spacing_tenths: clamp(value, 40, 100) }
              : { touch_key_spacing_tenths: clamp(value, 30, 60) }),
          }
        : current,
    );
  }

  function endTouchGeometryDrag(event: ReactPointerEvent<HTMLDivElement>) {
    const drag = touchGeometryDrag.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    touchGeometryDrag.current = null;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId);
  }

  async function openExternalUrl(url: string) {
    try {
      if (client.openExternalUrl) {
        await client.openExternalUrl(url);
      } else {
        const opened = window.open(url, "_blank", "noopener,noreferrer");
        if (!opened) throw new Error("popup blocked");
      }
    } catch {
      setError("无法打开外部链接，请稍后重试。");
    }
  }

  async function checkForUpdate() {
    setUpdateBusy(true);
    setUpdateStatus("");
    setAvailableUpdate(null);
    // A stalled connection would otherwise leave the button busy indefinitely; the shipped settings page gives up after ten seconds.
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), UPDATE_CHECK_TIMEOUT_MS);
    try {
      const releasePlatform = client.host?.platform ?? (linuxPlatform ? "linux" : null);
      const endpoint =
        clientHostedPlatform && releasePlatform
          ? `${clientReleasesUrl}?per_page=100&t=${Date.now()}`
          : `${updateManifestUrl}?t=${Date.now()}`;
      const response = await fetch(endpoint, { cache: "no-store", signal: controller.signal });
      if (!response.ok) throw new Error(`update manifest returned ${response.status}`);
      const manifest = (await response.json()) as UpdateManifest | GitHubRelease[];
      let update: ValidatedUpdate | null;
      if (clientHostedPlatform && releasePlatform) {
        if (!Array.isArray(manifest)) throw new Error("invalid release list");
        update = selectPlatformRelease(manifest, releasePlatform, platformReleasesPageUrl);
        if (!update) {
          setUpdateStatus("暂无可用发行版");
          return;
        }
      } else {
        update = validateManifest(manifest as UpdateManifest, platformReleasesPageUrl);
      }
      const current = parseVersion(currentAppVersion);
      if (!update || !current) throw new Error("invalid update manifest");
      if (compareVersions(update.version, current) > 0) {
        setAvailableUpdate(update);
        setUpdateStatus(`发现新版本 v${update.version.display}`);
      } else {
        setUpdateStatus("已是最新版本");
      }
    } catch {
      setUpdateStatus("检查失败，请稍后重试");
    } finally {
      window.clearTimeout(timeout);
      setUpdateBusy(false);
    }
  }

  async function openPanel(action: (() => Promise<void>) | undefined) {
    if (!action) {
      setError("当前宿主未接入该原生面板。");
      return;
    }
    try {
      await action();
    } catch {
      setError("无法打开原生面板，请稍后重试。");
    }
  }

  // One page per request: a real dictionary is far too large to pull into the
  // page before showing anything.
  async function loadPhrases(kind: LocalDictionaryKind = dictionaryKind, offset = 0) {
    if (!client.dictionary || !mounted.current) return;
    const generation = ++phraseRequestGeneration.current;
    setPhraseBusy(true);
    setPhraseError("");
    setPhraseNotice("");
    setPhrasePage((current) => ({ ...current, status: "查询中…" }));
    try {
      // Ask the host for this kind and code prefix. Filtering a page the host
      // had already chosen meant a user with more than a page of pinyin words
      // saw an empty list when they picked another dictionary, and the status
      // line counted the filtered rows against the unfiltered page.
      const query = phraseSearch.trim();
      const page = await client.dictionary.list(offset, DICTIONARY_PAGE_SIZE, kind, query);
      if (!mounted.current || generation !== phraseRequestGeneration.current) return;
      // Older hosts and the mobile personal dictionary ignore the extra arguments, so keep filtering defensively - with the host's own rule, so nothing it matched is dropped here.
      const entries = page.entries.filter(
        (entry) => entry.kind === kind && dictionaryKeyMatches(kind, entry.key, query),
      );
      setPhrases(entries);
      setDictionaryPendingCount(page.pending_count ?? 0);
      setDictionaryFailures(page.failed_requests ?? []);
      setDictionarySnapshotError(page.snapshot_error ?? "");
      setPhrasePage({
        offset,
        hasMore: page.has_more && page.entries.length > 0,
        status: dictionaryPageStatus(offset, entries.length, page.has_more),
      });
    } catch {
      if (!mounted.current || generation !== phraseRequestGeneration.current) return;
      setPhraseError("无法读取词库。");
      setPhrasePage((current) => ({ ...current, status: "查询失败，请重试" }));
    } finally {
      if (mounted.current && generation === phraseRequestGeneration.current) setPhraseBusy(false);
    }
  }
  function turnPhrasePage(offset: number) {
    // The list is its own scrolling surface. A page turn must reveal the new
    // page's first entry instead of preserving the previous page's bottom.
    if (phraseListRef.current) phraseListRef.current.scrollTop = 0;
    void loadPhrases(dictionaryKind, offset);
  }
  async function removePhrase(entry: DictionaryEntry) {
    if (!client.dictionary || !mounted.current) return;
    // Deletion is not undoable and the row is one click away from 编辑.
    const confirmed = await confirm({
      title: "删除词条",
      message: `“${entry.value}”（${entry.key}）将被删除，此操作无法撤销。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed || !client.dictionary || !mounted.current) return;
    setPhraseBusy(true);
    setPhraseError("");
    setPhraseNotice("");
    try {
      await client.dictionary.edit(entry, null, randomRequestId("ui-remove"));
      // Stay on the page the user was reading; deleting the last row on a page
      // would otherwise leave them looking at an empty one.
      const remaining = phrases.length - 1;
      const offset =
        remaining === 0 && phrasePage.offset > 0
          ? Math.max(0, phrasePage.offset - DICTIONARY_PAGE_SIZE)
          : phrasePage.offset;
      await loadPhrases(dictionaryKind, offset);
    } catch (error) {
      if (mounted.current)
        setPhraseError(
          dictionaryErrorMessage(
            error,
            `${dictionaryKindLabel(dictionaryKind)}删除失败，请稍后重试。`,
          ),
        );
    } finally {
      if (mounted.current) setPhraseBusy(false);
    }
  }
  async function savePhrase() {
    if (!client.dictionary || !phraseForm || !mounted.current) return;
    const bundled = phraseForm.previous?.source === "bundled" ? phraseForm.previous : null;
    // A bundled row keeps its code and word; only the weight is the user's to change.
    const replacement: DictionaryEntry = bundled
      ? { ...bundled, weight: phraseForm.weight }
      : {
          kind: dictionaryKind,
          key: phraseForm.key.trim(),
          value: phraseForm.value,
          weight: phraseForm.weight,
        };
    if (!replacement.key || !replacement.value) {
      setPhraseError("编码和短语不能为空。");
      return;
    }
    setPhraseBusy(true);
    setPhraseError("");
    setPhraseNotice("");
    try {
      await client.dictionary.edit(
        phraseForm.previous,
        replacement,
        randomRequestId(phraseForm.previous ? "ui-edit" : "ui-add"),
      );
      if (!mounted.current) return;
      setPhraseForm(null);
      // An edit keeps the reader where they were; only a new entry returns to
      // the first page, where the shared runtime lists it.
      await loadPhrases(dictionaryKind, phraseForm.previous ? phrasePage.offset : 0);
    } catch (error) {
      if (mounted.current)
        setPhraseError(
          dictionaryErrorMessage(
            error,
            `${dictionaryKindLabel(dictionaryKind)}保存失败，请稍后重试。`,
            dictionaryKind,
          ),
        );
    } finally {
      if (mounted.current) setPhraseBusy(false);
    }
  }
  async function importPhrases(file: File) {
    if (!client.dictionary || !mounted.current) return;
    setPhraseBusy(true);
    setPhraseError("");
    setPhraseNotice("");
    try {
      const text = await readDictionaryFile(file, client.dictionary.maxImportFileBytes);
      let imported: DictionaryImportResult | null = null;
      if (client.dictionary.import) {
        imported = await client.dictionary.import(
          dictionaryKind,
          dictionaryFormat,
          text,
          randomRequestId("ui-import"),
        );
      } else {
        if (dictionaryFormat === "hans") throw new Error("hans format requires batch import");
        const lines = text.split(/\r?\n/).filter(Boolean);
        for (const line of lines) {
          const [first, second, weight = "10000"] = line.split("\t");
          if (!first || !second) continue;
          const [value, key] = dictionaryFormat === "windows" ? [second, first] : [first, second];
          const parsedWeight = Number(weight);
          const normalizedWeight =
            weight.trim() !== "" && Number.isSafeInteger(parsedWeight) && parsedWeight >= 0
              ? parsedWeight
              : 10000;
          await client.dictionary.edit(
            null,
            { kind: dictionaryKind, key: key.trim(), value, weight: normalizedWeight },
            randomRequestId("ui-import"),
          );
        }
      }
      await loadPhrases(dictionaryKind);
      if (!mounted.current) return;
      // The host reports what it skipped; saying nothing reads as a clean import.
      if (imported)
        setPhraseNotice(describeImportResult(dictionaryKindLabel(dictionaryKind), imported));
    } catch (error) {
      if (mounted.current)
        setPhraseError(importFailureMessage(dictionaryKindLabel(dictionaryKind), error));
    } finally {
      if (mounted.current) setPhraseBusy(false);
    }
  }
  async function retryDictionaryFailure(requestId: string) {
    if (!client.dictionary?.retry || !mounted.current) return;
    setPhraseBusy(true);
    setPhraseError("");
    try {
      await client.dictionary.retry(requestId);
      await loadPhrases(dictionaryKind, phrasePage.offset);
    } catch {
      if (mounted.current) setPhraseError("词条重试失败，请稍后重试。");
    } finally {
      if (mounted.current) setPhraseBusy(false);
    }
  }
  async function dismissDictionaryFailure(requestId: string) {
    if (!client.dictionary?.dismissFailure || !mounted.current) return;
    setPhraseBusy(true);
    setPhraseError("");
    try {
      await client.dictionary.dismissFailure(requestId);
      await loadPhrases(dictionaryKind, phrasePage.offset);
    } catch {
      if (mounted.current) setPhraseError("移除失败记录失败，请稍后重试。");
    } finally {
      if (mounted.current) setPhraseBusy(false);
    }
  }
  /**
   * Hand an exported dictionary to the user as a file.
   *
   * Resolves to the path when the host wrote the file, to null when a download link was used instead (the host reports nothing back, so there is no path to show), and to undefined when the host refused the write, in which case the error has already been shown and no success may be reported.
   */
  async function deliverDictionaryExport(
    name: string,
    body: string,
  ): Promise<string | null | undefined> {
    if (client.saveExport) {
      try {
        return await client.saveExport(name, body);
      } catch {
        setPhraseNotice("");
        setPhraseError("无法写入“下载”文件夹，词库未导出。");
        return undefined;
      }
    }
    const url = URL.createObjectURL(new Blob([body], { type: "text/plain;charset=utf-8" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    URL.revokeObjectURL(url);
    return null;
  }
  async function exportPhrases() {
    if (!client.dictionary) return;
    if (dictionaryFormat === "hans") {
      setPhraseError("汉字自动注音格式仅支持导入。");
      return;
    }
    setPhraseBusy(true);
    setPhraseError("");
    setPhraseNotice("");
    try {
      let text = "";
      if (client.dictionary.export) {
        let offset = 0;
        let hasMore = true;
        while (hasMore && offset <= 1000000) {
          const page = await client.dictionary.export(
            dictionaryKind,
            dictionaryFormat === "rime" ? "standard" : dictionaryFormat,
            offset,
            1000,
          );
          text += page.text;
          const count = page.text ? page.text.trimEnd().split("\n").length : 0;
          offset += count;
          hasMore = page.has_more && count > 0;
        }
      } else {
        text = phrases
          .map((entry) =>
            dictionaryFormat === "windows"
              ? `${entry.key}\t${entry.value}\t${entry.weight}`
              : `${entry.value}\t${entry.key}\t${entry.weight}`,
          )
          .join("\n");
      }
      const payload = dictionaryExportPayload(dictionaryKind, dictionaryFormat, text);
      if (!payload.rows) {
        setPhraseError("当前没有可导出的用户新增词条。");
        return;
      }
      const path = await deliverDictionaryExport(
        dictionaryExportName(dictionaryKind),
        payload.body,
      );
      if (path === undefined) return;
      setPhraseNotice(
        path === null
          ? `已导出 ${payload.rows} 条用户词条。`
          : `已导出 ${payload.rows} 条用户词条到 ${path}。`,
      );
    } catch (error) {
      setPhraseError(dictionaryErrorMessage(error, "词库导出失败，请稍后重试。"));
    } finally {
      setPhraseBusy(false);
    }
  }
  async function exportAllPhrases() {
    if (!client.dictionary) return;
    setPhraseBusy(true);
    setPhraseError("");
    setPhraseNotice("正在读取全部用户词库…");
    try {
      const payload = personalDictionaryExportPayload(
        await loadAllPersonalDictionaryEntries(client.dictionary),
      );
      if (!payload.rows) {
        setPhraseNotice("当前没有可导出的用户词条。");
        return;
      }
      const path = await deliverDictionaryExport(personalDictionaryExportName(), payload.body);
      if (path === undefined) return;
      setPhraseNotice(
        path === null
          ? `已导出全部 ${payload.rows} 条用户词条。`
          : `已导出全部 ${payload.rows} 条用户词条到 ${path}。`,
      );
    } catch (error) {
      setPhraseError(dictionaryErrorMessage(error, "全部词库导出失败，请稍后重试。"));
    } finally {
      setPhraseBusy(false);
    }
  }

  /**
   * Put every setting back to its default, as a draft.
   *
   * The source window writes the defaults the moment the button is clicked. Here the result goes
   * into the draft instead and the user saves it like any other edit, because this page has an
   * explicit save and a dirty marker -- writing behind them would be the one action on the page
   * that does not work the way the rest of it does. It also means a misclick costs nothing.
   */
  async function restoreDefaults() {
    if (!client.loadDefaultPreferences || busy) return;
    const confirmed = await confirm({
      title: "恢复默认设置",
      message: "语音和翻译服务的密钥、词库和学习数据都不会改变。恢复后需要点击保存设置才会生效。",
      confirmLabel: "恢复",
    });
    if (!confirmed || !client.loadDefaultPreferences || busy) return;
    setError("");
    setNotice("");
    try {
      setDraft(await client.loadDefaultPreferences());
      setNotice("所有设置已恢复默认，请点击保存设置。");
    } catch {
      setError("无法读取默认设置，请重试。原有设置不会被自动重置。");
    }
  }

  /**
   * The Windows source repairs an unparseable config.toml by itself when the IME starts. The input method here does the same for a document that is not JSON at all, and this is the explicit path for everything else it refuses -- a newer build's fields or format, which an automatic rewrite could have destroyed. An unsaved edit survives the repair: it stays in the draft on top of the repaired revision, so saving it still works.
   */
  async function recoverPreferences() {
    if (!client.recoverPreferences || busy) return;
    const confirmed = await confirm({
      title: "修复配置文件",
      message:
        "损坏的配置文件会先备份到同一目录，然后尽量保留能识别的设置和服务密钥，其余恢复默认。",
      confirmLabel: "修复",
    });
    if (!confirmed || !client.recoverPreferences) return;
    setBusy(true);
    setError("");
    setNotice("");
    setRecoveredBackup("");
    try {
      const result = await client.recoverPreferences();
      const currentSnapshot = snapshotRef.current;
      const currentDraft = draftRef.current;
      const dirty =
        !!currentSnapshot &&
        !!currentDraft &&
        JSON.stringify(currentDraft) !== JSON.stringify(currentSnapshot.preferences);
      setSnapshot(result.snapshot);
      if (!dirty) setDraft(result.snapshot.preferences);
      if (!result.backupPath) {
        setNotice("配置文件已可以正常读取，无需修复。");
        return;
      }
      const backupName = result.backupPath.split(/[\\/]/).pop() ?? result.backupPath;
      setRecoveredBackup(result.backupPath);
      setNotice(
        `配置文件已修复，原文件已备份为 ${backupName}。${
          result.salvaged ? "" : "原有设置无法识别，已恢复默认。"
        }${dirty ? "未保存的修改仍保留，请点击保存设置。" : ""}`,
      );
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function resetLearnedData() {
    if (!client.resetLearnedData || phraseBusy) return;
    const confirmed = await confirm({
      title: "清除学习数据",
      message:
        "候选词频、用户词典和拼音学习记录将永久删除，此操作无法撤销。输入方案等设置不会改变。",
      confirmLabel: "清除",
      danger: true,
    });
    if (!confirmed || !client.resetLearnedData || phraseBusy) return;
    setPhraseBusy(true);
    setPhraseError("");
    setPhraseNotice("");
    try {
      await client.resetLearnedData();
      setPhrases([]);
      setPhrasePage({ offset: 0, hasMore: false, status: "已清除学习数据" });
      setDictionaryPendingCount(0);
      setDictionaryFailures([]);
      setDictionarySnapshotError("");
      setPhraseNotice("已清除所有学习数据；输入方案和设置保持不变。");
    } catch (error) {
      setPhraseError(
        dictionaryErrorMessage(error, "清除学习数据失败，请关闭正在使用输入法的程序后重试。"),
      );
    } finally {
      setPhraseBusy(false);
    }
  }

  const dirty =
    (!!draft && !!snapshot && JSON.stringify(draft) !== JSON.stringify(snapshot.preferences)) ||
    (macosWubiAutoCommitUnique !== undefined &&
      macosWubiAutoCommitUnique !== savedMacosWubiAutoCommitUnique);
  const ai = draft?.ai_assistant ?? defaultAiAssistant;
  const aiOrigin = aiCredentialOrigin(ai.endpoint);
  const aiToken = aiOrigin ? (ai.tokens?.[aiOrigin] ?? "") : "";
  const storedAiCredential = providerCredentials?.ai.find(
    (entry) => entry.provider === ai.provider,
  );
  const updateAi = (patch: Partial<AiAssistantPreferences>) => {
    aiRequestGeneration.current += 1;
    setAiModelsBusy(false);
    setAiTestBusy(false);
    setAiTestOutput("");
    setAiTestStatus("");
    if (patch.provider !== undefined || patch.endpoint !== undefined) {
      setAiModels(null);
      setAiModelsStatus("");
    }
    // Merged into the current draft, not this render's: fetchAiModels applies its result after an
    // await, and a render-time copy would revert whatever was typed meanwhile.
    setDraft((current) =>
      current
        ? {
            ...current,
            ai_assistant: { ...(current.ai_assistant ?? defaultAiAssistant), ...patch },
          }
        : current,
    );
  };
  const updateAiToken = (value: string) => {
    aiRequestGeneration.current += 1;
    setAiModelsBusy(false);
    setAiTestBusy(false);
    setAiTestOutput("");
    setAiTestStatus("");
    if (aiOrigin) updateAi({ token: "", tokens: { ...ai.tokens, [aiOrigin]: value } });
  };
  const fetchAiModels = async () => {
    if (!client.aiAssistant) return;
    if (!aiOrigin) {
      setAiModelsStatus("请先填写完整的 HTTPS 接口地址。");
      return;
    }
    if (!aiProviderCredentials && !aiToken.trim()) {
      setAiModelsStatus("请先填写 API Token，或使用已保存的密钥。");
      return;
    }
    const generation = aiRequestGeneration.current;
    setAiModelsBusy(true);
    setAiModelsStatus("");
    try {
      const models = await client.aiAssistant.fetchModels({
        endpoint: ai.endpoint,
        token: aiToken,
        provider: ai.provider,
      });
      if (generation !== aiRequestGeneration.current) return;
      setAiModels(models);
      setAiModelsStatus(`已获取 ${models.length} 个可用模型。`);
      if (models.length && !models.includes(ai.model)) updateAi({ model: models[0] });
    } catch (cause) {
      setAiModelsStatus(
        cause instanceof Error ? cause.message : "获取模型失败，请检查地址、密钥和网络。",
      );
    } finally {
      if (generation === aiRequestGeneration.current) setAiModelsBusy(false);
    }
  };
  const testAi = async () => {
    if (!client.aiAssistant) return;
    const text = aiTestInput;
    if (!text.trim()) {
      setAiTestStatus("请先输入待润色文字。");
      return;
    }
    if (!aiOrigin || (!aiProviderCredentials && !aiToken.trim())) {
      setAiTestStatus(
        aiProviderCredentials
          ? "请先填写有效的 HTTPS 接口地址。"
          : "请先填写有效的 HTTPS 接口地址和 API Token。",
      );
      return;
    }
    const generation = ++aiRequestGeneration.current;
    setAiTestBusy(true);
    setAiTestStatus("");
    setAiTestOutput("");
    try {
      const result = await client.aiAssistant.test({
        endpoint: ai.endpoint,
        model: ai.model,
        provider: ai.provider,
        prompt: aiPolishTestPrompt,
        token: aiToken,
        text,
      });
      if (generation !== aiRequestGeneration.current) return;
      setAiTestOutput(result);
      setAiTestStatus("已完成");
    } catch (cause) {
      setAiTestStatus(
        cause instanceof Error ? cause.message : "AI 请求失败，请检查地址、模型、密钥和网络。",
      );
    } finally {
      if (generation === aiRequestGeneration.current) setAiTestBusy(false);
    }
  };
  const wordCharacter = draft?.word_character ?? defaultWordCharacter;
  const keybindings = draft?.keybindings ?? defaultKeybindings;
  const frequency = draft?.frequency ?? defaultFrequency;
  const mixedInput = draft?.mixed_input ?? defaultMixedInput;
  const fuzzyPinyin = draft?.fuzzy_pinyin ?? defaultFuzzyPinyin;
  const touchKeyboardSchemes = draft?.touch_keyboard_schemes ?? {
    enabled: allTouchKeyboardSchemes,
  };
  const selectedTouchKeyboardScheme = draft ? inferredTouchKeyboardScheme(draft) : "quanpin";
  const setTouchKeyboardSchemeEnabled = (scheme: TouchKeyboardScheme, enabled: boolean) => {
    if (!draft) return;
    const next = updateTouchKeyboardSchemeEnabled(
      draft,
      scheme,
      enabled,
      selectedTouchKeyboardScheme,
    );
    if (next) setDraft(next);
  };
  const selectHomeScheme = (scheme: TouchKeyboardScheme) => {
    if (!draft) return;
    setDraft(selectHomeTouchKeyboardScheme(draft, scheme));
  };
  const localModes = draft?.local_modes ?? defaultLocalModes;
  // Every platform sees every mode. macOS used to hide emoji, kaomoji and temporary Japanese on the
  // grounds that its bundle shipped only msime.db and english.db, but others.db and dict_japanese.dat have
  // been in resources/desktop-dictionary.lock.json since 780a9381b and tauri.macos.conf.json bundles the
  // whole verified set - so the switches were hidden for modes that worked. Temporary English, gated the
  // same way on english.db, was visible throughout, which is how inconsistent this had become.
  //
  // A host missing a catalog is still handled, and handled better than by hiding a switch: the runtime
  // turns that mode off when its resource is absent, so the trigger key inserts its capital instead of
  // being swallowed.
  const clipboardHistory = iosPlatform || (draft?.clipboard_history ?? false);
  function toggleClipboardHistory(enabled: boolean) {
    if (!draft) return;
    setDraft({ ...draft, clipboard_history: enabled });
    // Clearing on opt-out keeps the local history from lingering while the
    // draft is unsaved. Hosts may omit this capability, so the preference
    // still changes independently when no clear hook is available.
    if (!enabled && clipboardHistory && client.clipboard?.clear) {
      void client.clipboard.clear().catch(() => setError("无法清空剪贴板历史，请稍后重试。"));
    }
  }
  const diagnosticLog = {
    server: draft?.diagnostic_log?.server ?? false,
    tsf: draft?.diagnostic_log?.tsf ?? false,
  };
  const candidateTranslations = draft?.candidate_translations ?? true;
  // Offline glosses are opt-in in client-core and in every native host. Keep
  // the settings view aligned when older snapshots omit the optional field.
  const candidateEnglishGloss = draft?.candidate_english_gloss ?? false;
  const candidateGlossLanguagesEnabled =
    candidateTranslations || Boolean(client.candidateEnglishGloss && candidateEnglishGloss);
  const translationTargetLanguage = draft?.translation_target_language ?? "en";
  const translationSecondaryLanguage = draft?.translation_secondary_language ?? "";
  const visibleTranslationLanguages = mobilePlatform
    ? [...mobileTranslationLanguages]
    : translationLanguages;
  const visibleSecondaryLanguages = mobilePlatform
    ? [
        ["", "不显示第二种语言"] as ["", string],
        ...mobileTranslationLanguages,
        ...(translationSecondaryLanguage === "ru"
          ? [["ru", "俄语（已保存）"] as ["ru", string]]
          : []),
      ]
    : translationSecondaryLanguages;
  if (
    mobilePlatform &&
    translationTargetLanguage === "ru" &&
    !visibleTranslationLanguages.some(([value]) => value === "ru")
  ) {
    visibleTranslationLanguages.push(["ru", "俄语（已保存）"]);
  }
  const voiceInput = { ...defaultVoiceInput, ...draft?.voice_input };
  const systemVoice = nativeVoicePlatform && voiceInput.asr_provider === "system";
  // Naming the host rather than assuming macOS. This read `harmonyPlatform ? "HarmonyOS" : "macOS"`
  // and was correct while those were the only two; Android gained a system recogniser of its own
  // and the card then announced itself as macOS on an Android phone.
  const systemVoiceHostName = harmonyPlatform ? "HarmonyOS" : androidPlatform ? "Android" : "macOS";
  // On-device Whisper. Like the system recognizer it has no service behind it, so it hides the same endpoint, token and model rows - but unlike it, the user has to say which model file to load.
  // macOS has always run a hand-picked Whisper file; a host with a model store can run the downloadable models too.
  const localVoiceAvailable = macosPlatform || client.localVoiceModels !== undefined;
  const localVoice = localVoiceAvailable && voiceInput.asr_provider === "local";
  // A Whisper file picked by hand: the whole setting on a host without a model store, and an advanced option under the model manager otherwise.
  const serviceVoice = !systemVoice && !localVoice;
  const harmonyUnsupportedAsr =
    harmonyPlatform &&
    !["doubao", "system", "openai", "siliconflow", "groq", "everyapi", "mistral"].includes(
      String(voiceInput.asr_provider),
    );
  const doubaoAuthMode =
    voiceInput.doubao_auth_mode ||
    (voiceInput.asr_app_key && !voiceInput.asr_app_key.startsWith("<") ? "legacy" : "api_key");
  // Functional for the same reason as updateAi: a local model download finishes into the draft
  // minutes after the click, and the render-time copy would revert every edit made in between.
  const updateVoice = (patch: Partial<VoiceInputPreferences>) => {
    setDraft((current) =>
      current
        ? {
            ...current,
            voice_input: { ...defaultVoiceInput, ...current.voice_input, ...patch },
          }
        : current,
    );
  };
  const customTranslation = draft?.custom_translation ?? defaultCustomTranslation;
  const tencentTranslation = draft?.tencent_tmt ?? defaultTencentTranslation;
  const niutrans = draft?.niutrans ?? defaultNiuTrans;
  const translationProvider = niutrans.enabled
    ? "niutrans"
    : customTranslation.enabled
      ? "custom"
      : tencentTranslation.enabled
        ? "tencent"
        : (macosPlatform || linuxPlatform) && draft?.translation_account
          ? "account"
          : "none";
  // The macOS input method falls back to Apple's on-device translation when no service will answer: none chosen, or Tencent (on by default) still without its secrets. The pairs among the chosen targets that it found downloadable but not downloaded are why a sentence candidate shows no translation.
  const onDeviceTranslationInUse =
    macosPlatform &&
    candidateTranslations &&
    (translationProvider === "none" ||
      (translationProvider === "tencent" &&
        !(tencentTranslation.secret_id.trim() && tencentTranslation.secret_key.trim())));
  const onDeviceMissingLanguages = onDeviceTranslationInUse
    ? translationLanguages.filter(
        ([code]) =>
          (code === translationTargetLanguage || code === translationSecondaryLanguage) &&
          onDeviceDownloadable.includes(code),
      )
    : [];
  // One service at a time: the MSIME account is only ever used when chosen here, and any other choice clears it. A cleared choice is left undefined rather than false, because the saved document omits the key while it is false and an undone edit must compare equal to it again.
  const setTranslationProvider = (
    provider: "none" | "custom" | "tencent" | "niutrans" | "account",
  ) => {
    if (!draft) return;
    setDraft({
      ...draft,
      custom_translation: { ...customTranslation, enabled: provider === "custom" },
      tencent_tmt: { ...tencentTranslation, enabled: provider === "tencent" },
      niutrans: { ...niutrans, enabled: provider === "niutrans" },
      translation_account: provider === "account" ? true : undefined,
    });
  };
  const runCredentialTest = async (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ) => {
    if (!client.testApiCredential) return;
    const signature = JSON.stringify(config);
    const generation = (credentialTestGeneration.current[service] ?? 0) + 1;
    credentialTestGeneration.current[service] = generation;
    setCredentialTests((current) => ({
      ...current,
      [service]: { signature, busy: true, message: "" },
    }));
    try {
      const result = await client.testApiCredential(service, config);
      if (credentialTestGeneration.current[service] !== generation) return;
      setCredentialTests((current) => ({
        ...current,
        [service]: { signature, busy: false, ...result },
      }));
    } catch {
      if (credentialTestGeneration.current[service] !== generation) return;
      setCredentialTests((current) => ({
        ...current,
        [service]: {
          signature,
          busy: false,
          ok: false,
          message: "无法连接 provider，请确认服务已启动。",
        },
      }));
    }
  };
  const runProviderCredential = async (
    kind: "ai" | "tencent",
    operation: (credentials: ProviderCredentialClient) => Promise<ProviderCredentialStatus>,
    success: string,
  ) => {
    const credentials = client.providerCredentials;
    if (!credentials) return;
    setProviderCredentialBusy(kind);
    setProviderCredentialMessages((current) => ({ ...current, [kind]: undefined }));
    try {
      setProviderCredentials(await operation(credentials));
      if (kind === "ai") setAiCredentialInput("");
      else setTencentCredentialInput({ secretId: "", secretKey: "" });
      setProviderCredentialMessages((current) => ({
        ...current,
        [kind]: { ok: true, text: success },
      }));
    } catch (error) {
      setProviderCredentialMessages((current) => ({
        ...current,
        [kind]: { ok: false, text: providerCredentialErrorMessage(error) },
      }));
    } finally {
      setProviderCredentialBusy(undefined);
    }
  };
  const runVoiceCredential = async (
    kind: VoiceCredentialKind,
    operation: (credentials: ProviderCredentialClient) => Promise<VoiceCredentialSaveResult>,
    success: string,
  ) => {
    const credentials = client.providerCredentials;
    if (!credentials) return;
    setProviderCredentialBusy(kind);
    setProviderCredentialMessages((current) => ({ ...current, [kind]: undefined }));
    try {
      const result = await operation(credentials);
      setProviderCredentials(result.status);
      setVoiceCredentialInput((current) => ({ ...current, [kind]: { token: "", appKey: "" } }));
      setProviderCredentialMessages((current) => ({
        ...current,
        [kind]: result.serviceUpdated
          ? { ok: true, text: success }
          : {
              ok: false,
              text: `${success}但未能更新语音服务，请运行 systemctl --user enable --now msime-linux-voice.socket。`,
            },
      }));
    } catch (error) {
      setProviderCredentialMessages((current) => ({
        ...current,
        [kind]: { ok: false, text: providerCredentialErrorMessage(error) },
      }));
    } finally {
      setProviderCredentialBusy(undefined);
    }
  };
  const credentialTestControl = (
    service: ApiCredentialTestService,
    label: string,
    config: Record<string, unknown>,
    disabled = false,
  ) => {
    return (
      <CredentialTestSection
        label={label}
        config={config}
        state={credentialTests[service] as CredentialTestState | undefined}
        disabled={disabled}
        available={Boolean(client.testApiCredential)}
        onTest={() => void runCredentialTest(service, config)}
      />
    );
  };
  /**
   * The provider's known models and its own integration page.
   *
   * Apple's settings put both next to the provider row, and they are what makes
   * a freshly picked provider usable: the endpoint is filled in automatically,
   * but the model box is a free text field, and a user with no credential yet
   * has nowhere to learn which models that service accepts or where its API key
   * comes from. Selecting a preset only writes the model; 自定义模型 leaves
   * whatever the user typed alone.
   */
  const providerPresetControls = (
    label: string,
    preset: ProviderPreset | undefined,
    model: string,
    onSelectModel: (model: string) => void,
    className = "section provider-preset-section",
  ) => {
    return (
      <ProviderPresetSection
        label={label}
        preset={preset}
        model={model}
        onSelectModel={onSelectModel}
        openExternalUrl={client.openExternalUrl}
        className={className}
      />
    );
  };
  const inputModeHUD = draft?.input_mode_hud ?? true;
  const floatingToolbar = { ...defaultFloatingToolbar, ...draft?.floating_toolbar };
  const themeMode = draft?.theme ?? "system";
  const settingsTheme = draft?.settings_theme ?? "follow";
  const candidatePreviewTheme = useCandidatePreviewTheme(themeMode, draft?.candidate_theme);
  const toolbarPreviewTheme = useCandidatePreviewTheme(themeMode, draft?.toolbar_theme);
  const keyboardPreviewTheme = useCandidatePreviewTheme(themeMode, draft?.screen_keyboard_theme);
  const touchKeyboardSkin = draft?.touch_keyboard_skin ?? "forest";
  const customTouchKeyboardSkin =
    draft?.custom_touch_keyboard_skin ?? defaultTouchKeyboardSkinDesign;
  useEffect(() => setSkinPreviewThemes({}), [candidatePreviewTheme]);
  const touchKeySpacingTenths = draft?.touch_key_spacing_tenths ?? 60;
  const touchRowSpacingTenths = draft?.touch_row_spacing_tenths ?? 70;
  const touchKeyboardHeightAdjustment = draft?.touch_keyboard_height_adjustment ?? 0;
  const installerTrust = availableUpdate
    ? describeInstallerTrust(
        availableUpdate,
        client.host?.platform ?? (linuxPlatform ? "linux" : null),
      )
    : null;
  const availablePages = pages.filter(
    (item) =>
      (item.id !== "home" || Boolean(client.home)) &&
      (item.id !== "typing-statistics" || Boolean(client.typingStatistics)) &&
      (item.id !== "vocabulary" || Boolean(client.vocabularyReview)) &&
      (item.id !== "account" || Boolean(client.account || client.appIcon)) &&
      (item.id !== "chat" || Boolean(client.chat)) &&
      (item.id !== "community" || Boolean(client.communitySkins || client.communityResources)) &&
      (item.id !== "floating-toolbar" || showFloatingToolbar) &&
      (item.id !== "more" || mobilePlatform),
  );
  // Physical-keyboard shortcuts and a desktop floating toolbar have no phone
  // surface. HarmonyOS keeps those controls in the input-method panel on a 2-in-1,
  // but its phone panel is still a touch keyboard, so the settings entry must not
  // leak the PC key descriptions into the phone's "全部设置" list.
  //
  // Helper codes are per-host rather than per-form-factor. The Android keyboard
  // sends them: Shift during a quanpin or shuangpin composition passes the next
  // letter to the Engine as a helper code, and the Engine reads the schema and
  // the candidate-row hint from these very preferences. Hiding the page left
  // that shipping feature with no way to pick a schema or turn it off. The
  // iOS keyboard extension marks a helper code the same way, so the page also
  // follows the host's `helpcode_shift_entry`; the platform names stay for
  // hosts that predate the capability.
  //
  // HarmonyOS was in the hidden list while shipping the same input: its
  // ChineseHelpcodePolicy is the Android one, ported, and the session calls it
  // on every shifted key. So it keeps the helper-code page, while the physical
  // keyboard shortcut page is only available on the 2-in-1 branch where the
  // corresponding capability projection is true.
  // The shortcuts page carries the hardware-keyboard chords, so it is hidden where the host does
  // not route any of them rather than where the platform happens to be a phone. Any of these
  // devices can have a keyboard attached, and its owner has to be able to reach the switches the
  // host already reads; hiding the page by platform name left them unreachable on Android.
  const mobileHiddenPageIds: readonly SettingsPageId[] = getMobileHiddenPageIds({
    modeSwitchShortcuts: showModeSwitchShortcuts,
    panelShortcuts: showPanelShortcuts,
    desktopMaintenanceShortcuts: showDesktopMaintenanceShortcuts,
    helpcodeShiftEntry: showHelpcodeShiftEntry,
    android: androidPlatform,
    harmony: harmonyPlatform,
  });
  // The sidebar is the list this page duplicates, so it does not list it. A mobile host above phone width still shows the sidebar, and `selectPage` refuses the pages hidden above, so listing them there left buttons that did nothing when tapped.
  const sidebarPages = availablePages.filter(
    (item) => item.id !== "more" && !(mobilePlatform && mobileHiddenPageIds.includes(item.id)),
  );
  const sidebarGroups = groupSidebarPages(sidebarPages, macosPlatform, macosSidebarGroups);
  // Walked in tab order rather than filtered out of `availablePages`, which is in the order the
  // pages happen to be declared in — that put 我的 second, and the bar read 键盘 / 我的 / 社区 / 统计
  // against the source's 键盘 / 社区 / 统计 / 我的.
  // A page without a tab of its own was reached from inside the 键盘 tab, so that is the tab still
  // standing on. Keyed off the page alone, the bar went blank the moment anyone opened one — nothing
  // lit, and no way to read where in the app you were.
  const mobileActiveTab: MobilePrimaryPageId = mobileTabForPage(page);
  const untitledOnPhone: readonly SettingsPageId[] = ["home", "typing-statistics", "account"];
  const { primary: mobilePrimaryPages, secondary: mobileSecondaryPages } = splitMobilePages(
    availablePages,
    mobileHiddenPageIds,
  );
  const selectPage = (next: SettingsPageId) => {
    if (mobilePlatform && mobileHiddenPageIds.includes(next)) return;
    if (next === page) return;
    if (mobilePlatform) mobileLastPageByTab.current[mobileTabForPage(next)] = next;
    setPage(next);
    if (mobilePlatform && typeof window !== "undefined") {
      const current = window.history.state;
      const state = {
        ...(current && typeof current === "object" ? current : {}),
        msimeSettings: true,
        page: next,
      } as Record<string, unknown>;
      delete state.panel;
      window.history.pushState(state, "");
    }
    if (next === "community") setCommunityDestination("all");
  };
  // The request the page mounted with is already in `page`'s initializer; only later ones navigate.
  const handledRoute = useRef(route?.nonce);
  useEffect(() => {
    if (!route || route.nonce === handledRoute.current) return;
    handledRoute.current = route.nonce;
    selectPage(requestedPage(route.page));
  }, [route?.nonce]);
  const selectMobileTab = (tab: SettingsPageId) => {
    if (!mobilePrimaryPageIds.includes(tab as MobilePrimaryPageId)) return;
    const primary = tab as MobilePrimaryPageId;
    if (primary === "account") setAccountLoginReturnPage(null);
    const remembered = mobileLastPageByTab.current[primary];
    const available = availablePages.some((item) => item.id === remembered);
    selectPage(available && !mobileHiddenPageIds.includes(remembered) ? remembered : primary);
  };
  const openAccountLogin = () => {
    if (mobilePlatform) setAccountLoginReturnPage(page);
    selectPage("account");
  };
  const finishAccountLogin = () => {
    const previous = accountLoginReturnPage;
    setAccountLoginReturnPage(null);
    if (!previous) return;
    mobileLastPageByTab.current[mobileTabForPage(previous)] = previous;
    setPage(previous);
    if (mobilePlatform && typeof window !== "undefined") window.history.back();
  };
  useEffect(() => {
    const pageAvailable =
      availablePages.some((item) => item.id === page) &&
      (!mobilePlatform || !mobileHiddenPageIds.includes(page));
    if (!pageAvailable) setPage(mobilePlatform && client.home ? "home" : "appearance");
  }, [availablePages, client.home, mobilePlatform, page]);
  useEffect(() => {
    if (typeof document === "undefined") return;
    const apply = () => {
      document.documentElement.dataset.theme = resolveSettingsTheme(themeMode, settingsTheme);
    };
    apply();
    if (
      themeMode !== "system" ||
      settingsTheme !== "follow" ||
      typeof window === "undefined" ||
      typeof window.matchMedia !== "function"
    )
      return;
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const listener = () => apply();
    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", listener);
      return () => media.removeEventListener("change", listener);
    }
    media.addListener(listener);
    return () => media.removeListener(listener);
  }, [settingsTheme, themeMode]);
  const openLocalDesigns = () => {
    selectPage("appearance");
    setShowTouchSkinEditor(true);
  };
  const openCommunity = (destination: AccountCommunityDestination | "all") => {
    selectPage("community");
    setCommunityDestination(destination);
  };
  const initialCommunityCategory =
    communityDestination === "published-reply" || communityDestination === "saved-reply"
      ? "reply"
      : communityDestination === "published-dictionary" ||
          communityDestination === "saved-dictionary"
        ? "dictionary"
        : "skin";
  const initialCommunityScope =
    communityDestination === "published-dictionary" || communityDestination === "published-reply"
      ? "mine"
      : communityDestination === "saved-dictionary" || communityDestination === "saved-reply"
        ? "saved"
        : "";
  return (
    <div
      className={settings.shell}
      data-settings-shell=""
      // The phone hosts read as one product with the Apple app, which is where the palette below
      // comes from. The inherited one is the Windows settings accent.
      data-mobile={mobilePlatform ? "" : undefined}
      onPointerDownCapture={(event) => {
        if (!client.resizeWindow || event.button !== 0 || windowMaximized) return;
        const rect = event.currentTarget.getBoundingClientRect();
        const value = windowResizeEdge(event, rect);
        if (value) {
          event.preventDefault();
          event.stopPropagation();
          void client.resizeWindow(value).catch(() => setError("无法调整窗口大小，请重试。"));
        }
      }}
    >
      {confirmation}
      {/* A phone has no window to minimise, maximise, close or drag: the OS owns the frame. The host
          still exposes the window commands on mobile because the same Tauri app binary backs both, so
          the presence of a command is not the question -- the platform is. */}
      {!mobilePlatform && (
        <WindowTitlebar
          maximized={windowMaximized}
          windowControl={client.windowControl}
          beginWindowDrag={client.beginWindowDrag}
          resizeWindow={client.resizeWindow}
          onError={setError}
        />
      )}
      <div
        className="flex min-h-0 min-w-0 flex-1 overflow-hidden max-phone:flex-col"
        data-settings-body=""
      >
        {/* A bottom tab bar. `order-2` seats it below the content while the DOM keeps it ahead, so
            assistive technology and keyboard focus still reach the navigation first, and the bottom
            padding clears the gesture inset. Hidden above phone width, where the sidebar serves. */}
        {mobilePlatform && (
          <MobileSettingsTabs
            tabs={mobilePrimaryPages}
            activeTab={mobileActiveTab}
            onSelect={selectMobileTab}
          />
        )}
        <SettingsSidebar groups={sidebarGroups} selectedPage={page} onSelectPage={selectPage} />
        <main
          ref={settingsContentRef}
          id="settings-content"
          className="min-h-0 min-w-0 flex-1 overflow-y-auto pt-0 pr-6 pb-0 pl-4 [scrollbar-gutter:stable] max-phone:px-2"
          aria-labelledby="page-title"
        >
          <div className="mx-auto mt-0.5 mb-0 w-full max-w-[900px] p-3 max-phone:px-1 max-phone:py-3">
            {/* Three of the four tabs open on something that already names them — a headline, a
                profile card, a row of figures — and the source prints no page title over any of
                them. 社区 is the one that does. Hidden rather than dropped: it labels `main`. */}
            <SettingsPageHeader
              title={availablePages.find((item) => item.id === page)?.title ?? "外观"}
              hiddenOnPhone={mobilePlatform && untitledOnPhone.includes(page)}
            />
            <SettingsStatusMessages
              error={error}
              notice={notice}
              busy={busy}
              recoveredBackup={recoveredBackup}
              canRecover={Boolean(client.recoverPreferences)}
              onRecover={() => void recoverPreferences()}
              openPreferencesDirectory={client.openPreferencesDirectory}
              macos={macosPlatform}
              onError={setError}
            />
            {inputSourceStartup &&
              (inputSourceStartup.action !== "up_to_date" ||
                inputSourceStartup.enabled === false) && (
                <InputSourceStartupNotice
                  status={inputSourceStartup}
                  onOpenSettings={() => client.inputSourceStartup?.openSettings()}
                  onDismiss={() => setInputSourceStartup(null)}
                  onError={setError}
                />
              )}
            {busy && !draft && <p role="status">正在读取设置…</p>}
            {client.home && draft && page === "home" && (
              <HomePage
                preferences={draft}
                actions={client.home}
                onOpenPage={(value) => selectPage(value as SettingsPageId)}
                onSelectScheme={selectHomeScheme}
                onOpenChat={client.chat ? () => selectPage("chat") : undefined}
                touchLayout={mobilePlatform}
              />
            )}
            {page === "more" && (
              <MoreSettingsPage
                pages={mobileSecondaryPages.map((item) => ({
                  id: item.id,
                  title: item.title,
                  icon: item.icon,
                }))}
                onOpenPage={(value) => selectPage(value as SettingsPageId)}
              />
            )}
            {(client.account || client.appIcon) && page === "account" && (
              <AccountPage
                client={client.account}
                appIcon={client.appIcon}
                platform={
                  androidPlatform
                    ? "android"
                    : iosPlatform
                      ? "ios"
                      : harmonyPlatform
                        ? "harmony"
                        : undefined
                }
                mobile={mobilePlatform}
                onCancelLogin={accountLoginReturnPage ? finishAccountLogin : undefined}
                onLoginComplete={accountLoginReturnPage ? finishAccountLogin : undefined}
                onOpenLocalDesigns={client.customTouchKeyboardSkins ? openLocalDesigns : undefined}
                onOpenCommunity={
                  client.communitySkins && client.communityResources ? openCommunity : undefined
                }
                onOpenCloudDictionary={
                  client.openCloudDictionary
                    ? () => {
                        void client.openCloudDictionary!().catch(() =>
                          setError("无法打开云词库，请重试。"),
                        );
                      }
                    : undefined
                }
                onOpenCloudClipboard={
                  client.openCloudClipboard
                    ? () => {
                        void client.openCloudClipboard!().catch(() =>
                          setError("无法打开云剪贴板，请重试。"),
                        );
                      }
                    : undefined
                }
                onOpenAbout={mobilePlatform ? () => selectPage("about") : undefined}
                onOpenDesktopDownload={
                  mobilePlatform && client.openExternalUrl
                    ? () => {
                        void openExternalUrl(desktopDownloadUrl);
                      }
                    : undefined
                }
                onReplayOnboarding={mobilePlatform ? onReplayOnboarding : undefined}
              />
            )}
            {client.chat && page === "chat" && (
              <ChatPage
                client={client.chat}
                autoFocus={mobilePlatform}
                touch={mobilePlatform}
                onLogin={openAccountLogin}
              />
            )}
            {client.communitySkins && client.communityResources && page === "community" && (
              <CommunityHomePage
                key={communityDestination}
                skins={client.communitySkins}
                resources={client.communityResources}
                theme={keyboardPreviewTheme}
                initialMine={communityDestination === "published-skins"}
                initialCategory={initialCommunityCategory}
                initialScope={initialCommunityScope}
                localDictionary={client.dictionary}
                localSkinLibrary={client.customSkinLibrary}
                mobile={mobilePlatform}
                onLogin={openAccountLogin}
              />
            )}
            {client.communitySkins && !client.communityResources && page === "community" && (
              <CommunitySkinsPage
                key={communityDestination}
                client={client.communitySkins}
                theme={keyboardPreviewTheme}
                localSkinLibrary={client.customSkinLibrary}
                initialMine={communityDestination === "published-skins"}
                mobile={mobilePlatform}
                onLogin={openAccountLogin}
              />
            )}
            {!client.communitySkins && client.communityResources && page === "community" && (
              <CommunityResourcesPage
                client={client.communityResources}
                kind={initialCommunityCategory === "reply" ? "reply" : "dictionary"}
                initialScope={initialCommunityScope}
                mobile={mobilePlatform}
              />
            )}
            {client.typingStatistics && page === "typing-statistics" && (
              <TypingStatisticsPage
                client={client.typingStatistics}
                mobile={mobilePlatform}
                platform={client.host?.platform}
                openSystemSettings={client.openSystemKeyboardSettings}
              />
            )}
            {client.vocabularyReview && page === "vocabulary" && (
              <VocabularyReviewPage
                client={client.vocabularyReview}
                mobile={mobilePlatform}
                openPanel={client.openVocabulary}
              />
            )}
            {draft &&
              page !== "typing-statistics" &&
              page !== "vocabulary" &&
              page !== "account" &&
              page !== "chat" &&
              page !== "more" &&
              page !== "community" && (
                <form
                  onSubmit={(event) => {
                    event.preventDefault();
                    void save();
                  }}
                >
                  <fieldset disabled={busy} hidden={page !== "appearance"} aria-label="外观">
                    <AppearanceCandidatePreview
                      preferences={{
                        ...draft,
                        candidate_english_font:
                          host?.platform === "windows"
                            ? (draft.candidate_english_font ?? "Segoe UI")
                            : draft.candidate_english_font,
                      }}
                      scan={client.scanSkinCatalog}
                      readImage={client.readSkinImage}
                      resolveFonts={client.resolveFontFamilies}
                      active={page === "appearance"}
                      revision={snapshot?.revision ?? 0}
                      mobile={mobilePlatform}
                    />
                    {host?.candidate_panel_limit && (
                      <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
                    )}
                    {showCandidateFollowCursor && (
                      <CandidateFollowCursorSection
                        value={draft.candidate_follow_cursor}
                        onChange={(candidate_follow_cursor) =>
                          setDraft({ ...draft, candidate_follow_cursor })
                        }
                      />
                    )}
                    {showCandidateFontControls ? (
                      <CandidateFontControls
                        value={draft}
                        onChange={(patch) => setDraft({ ...draft, ...patch })}
                        readFonts={client.listFontFamilies}
                        windows={host?.platform === "windows"}
                        englishFont={showCandidateEnglishFont}
                        mobile={mobilePlatform}
                      />
                    ) : (
                      <CandidateFontUnsupportedNotice />
                    )}
                    <CandidateSizingSection
                      preferences={draft}
                      mobile={mobilePlatform}
                      showFontControls={showCandidateFontControls}
                      showPreeditFont={showCandidatePreeditFont}
                      onChange={(patch) => setDraft({ ...draft, ...patch })}
                    />
                    {mobileKeyboardFeedback?.candidatePaletteFollowsDesktop === false && (
                      <CandidatePaletteFallbackNotice />
                    )}
                    <CandidateColorsSection
                      preferences={draft}
                      previewTheme={candidatePreviewTheme}
                      showRowColors={showCandidateRowColors}
                      showSelectionAppearance={showCandidateSelectionAppearance}
                      showBorderColor={showCandidateBorderColor}
                      linux={linuxPlatform}
                      onChange={(key, value) => setDraft({ ...draft, [key]: value })}
                    />
                    <CandidatePageSizeSection
                      value={draft.candidate_page_size}
                      fixed={host?.fixed_candidate_page_size !== undefined}
                      onChange={(candidate_page_size) =>
                        setDraft({ ...draft, candidate_page_size })
                      }
                    />
                    <ThemeSettingsSection
                      preferences={draft}
                      mobile={mobilePlatform}
                      linux={linuxPlatform}
                      floatingToolbar={showFloatingToolbar}
                      desktopPanels={desktopPanels}
                      onChange={(key, value) => setDraft({ ...draft, [key]: value })}
                    />
                    <CandidateLayoutSection
                      value={draft.candidate_layout}
                      fixed={host?.fixed_candidate_layout !== undefined}
                      onChange={(candidate_layout) => setDraft({ ...draft, candidate_layout })}
                    />
                    <PreeditSettingsSection
                      preferences={draft}
                      mobile={mobilePlatform}
                      showShuangpinPreedit={showShuangpinPreedit}
                      inlinePreedit={mobileKeyboardFeedback?.inlinePreedit}
                      inlinePreeditBusy={mobileKeyboardFeedbackBusy}
                      onChange={(patch) => setDraft({ ...draft, ...patch })}
                      onInlinePreeditChange={(inlinePreedit) =>
                        mobileKeyboardFeedback &&
                        void saveMobileKeyboardFeedback({
                          ...mobileKeyboardFeedback,
                          inlinePreedit,
                        })
                      }
                    />
                  </fieldset>
                  <fieldset disabled={busy} hidden={page !== "dictionary"} aria-label="词库">
                    {client.dictionaryManifest && (
                      <DictionaryManifestCard read={client.dictionaryManifest} />
                    )}
                    {client.dictionary?.importPersonal && (
                      <PersonalDictionaryImportCard
                        dictionary={client.dictionary}
                        platform={client.host?.platform}
                      />
                    )}
                    {client.dictionary && (
                      <>
                        <DictionaryManagerHeader
                          disabled={phraseBusy}
                          dictionaryFormat={dictionaryFormat}
                          onQuery={() => void loadPhrases(dictionaryKind, 0)}
                          onAdd={() =>
                            setPhraseForm({
                              key: "",
                              value: "",
                              weight: 10,
                              previous: null,
                            })
                          }
                          onExportCurrent={() => void exportPhrases()}
                          onExportAll={() => void exportAllPhrases()}
                          onImport={(file) => void importPhrases(file)}
                          onFormatChange={setDictionaryFormat}
                        />
                        {dictionaryPendingCount > 0 && (
                          <p className="input-setting-description" role="status">
                            {dictionaryPendingCount}{" "}
                            项等待键盘同步。打开水杉键盘后会在空闲时逐条生效。
                          </p>
                        )}
                        {dictionarySnapshotError && (
                          <p role="alert" className="error">
                            {dictionarySnapshotError}
                          </p>
                        )}
                        {dictionaryFailures.length > 0 && (
                          <div className={settings.failures} role="alert">
                            <p>
                              有 {dictionaryFailures.length}{" "}
                              项词库请求同步失败，可以重试或移除失败记录。
                            </p>
                            <ul>
                              {dictionaryFailures.map((failure) => (
                                <li key={failure.request_id}>
                                  <span>
                                    <strong>{failure.label}</strong>
                                    <small>{failure.error}</small>
                                  </span>
                                  <span>
                                    <button
                                      type="button"
                                      className="secondary"
                                      disabled={phraseBusy || !client.dictionary?.retry}
                                      onClick={() =>
                                        void retryDictionaryFailure(failure.request_id)
                                      }
                                    >
                                      重试
                                    </button>{" "}
                                    <button
                                      type="button"
                                      className="secondary"
                                      disabled={phraseBusy || !client.dictionary?.dismissFailure}
                                      onClick={() =>
                                        void dismissDictionaryFailure(failure.request_id)
                                      }
                                    >
                                      移除记录
                                    </button>
                                  </span>
                                </li>
                              ))}
                            </ul>
                          </div>
                        )}
                        <div className={settings.managerControls}>
                          <label>
                            词库{" "}
                            <select
                              aria-label="本地词库类型"
                              value={dictionaryKind}
                              disabled={phraseBusy}
                              onChange={(event) => {
                                const kind = event.target.value as LocalDictionaryKind;
                                setDictionaryKind(kind);
                                if (kind !== "pinyin" && dictionaryFormat === "hans")
                                  setDictionaryFormat("standard");
                                setPhrases([]);
                                void loadPhrases(kind);
                              }}
                            >
                              {localDictionaryKinds.map(([kind, label]) => (
                                <option key={kind} value={kind}>
                                  {label}
                                </option>
                              ))}
                            </select>
                          </label>
                          <label>
                            文件格式{" "}
                            <select
                              aria-label="本地词库文件格式"
                              value={dictionaryFormat}
                              disabled={phraseBusy}
                              onChange={(event) =>
                                setDictionaryFormat(event.target.value as LocalDictionaryFormat)
                              }
                            >
                              <option value="standard">词在前（标准 TSV）</option>
                              <option value="windows">编码在前（Windows TSV）</option>
                              <option value="rime">Rime userdb / dict.yaml</option>
                              {dictionaryKind === "pinyin" && (
                                <option value="hans">汉字自动注音（仅导入）</option>
                              )}
                            </select>
                          </label>
                          <label>
                            编码前缀{" "}
                            <input
                              value={phraseSearch}
                              placeholder="留空查看全部"
                              onChange={(event) => setPhraseSearch(event.target.value)}
                            />
                          </label>
                        </div>
                        {phraseError && (
                          <p role="alert" className="error">
                            {phraseError}
                          </p>
                        )}
                        {phraseNotice && (
                          <p role="status" className={settings.empty}>
                            {phraseNotice}
                          </p>
                        )}
                        {phraseForm && (
                          <div className={settings.phraseForm}>
                            <label>
                              编码{" "}
                              <input
                                value={phraseForm.key}
                                readOnly={phraseForm.previous?.source === "bundled"}
                                onChange={(event) =>
                                  setPhraseForm({ ...phraseForm, key: event.target.value })
                                }
                              />
                              <small className={settings.keyHint}>
                                {dictionaryKindKeyHint(dictionaryKind)}
                              </small>
                            </label>
                            <label>
                              {dictionaryKind === "quick_phrase" ? "短语" : "词条"}{" "}
                              <input
                                value={phraseForm.value}
                                readOnly={phraseForm.previous?.source === "bundled"}
                                onChange={(event) =>
                                  setPhraseForm({ ...phraseForm, value: event.target.value })
                                }
                              />
                            </label>
                            <label>
                              权重{" "}
                              <input
                                type="number"
                                value={phraseForm.weight}
                                onChange={(event) =>
                                  setPhraseForm({
                                    ...phraseForm,
                                    weight: Number(event.target.value),
                                  })
                                }
                              />
                            </label>
                            <button
                              type="button"
                              disabled={phraseBusy}
                              onClick={() => void savePhrase()}
                            >
                              保存
                            </button>
                            <button
                              type="button"
                              className="secondary"
                              disabled={phraseBusy}
                              onClick={() => setPhraseForm(null)}
                            >
                              取消
                            </button>
                          </div>
                        )}
                        {phrases.length === 0 ? (
                          <p className={settings.empty}>
                            点击查询后查看
                            {localDictionaryKinds.find(([kind]) => kind === dictionaryKind)?.[1] ??
                              "词库"}
                            词条
                          </p>
                        ) : (
                          <ul
                            ref={phraseListRef}
                            className={settings.phraseList}
                            aria-label="词库查询结果"
                          >
                            {phrases.map((entry, index) => (
                              <li key={`${entry.key}-${entry.value}-${index}`}>
                                <span>
                                  <code>{entry.key}</code>　{entry.value}　
                                  <small>{entry.weight}</small>
                                  {entry.source === "bundled" && (
                                    <>
                                      {" "}
                                      <small className={settings.bundledBadge}>内置</small>
                                    </>
                                  )}
                                </span>
                                <span>
                                  <button
                                    type="button"
                                    className="secondary"
                                    disabled={phraseBusy}
                                    onClick={() =>
                                      setPhraseForm({
                                        key: entry.key,
                                        value: entry.value,
                                        weight: entry.weight,
                                        previous: entry,
                                      })
                                    }
                                  >
                                    {entry.source === "bundled" ? "调权重" : "编辑"}
                                  </button>{" "}
                                  <button
                                    type="button"
                                    className="secondary"
                                    disabled={phraseBusy}
                                    onClick={() => void removePhrase(entry)}
                                  >
                                    删除
                                  </button>
                                </span>
                              </li>
                            ))}
                          </ul>
                        )}
                        <div className="flex items-center justify-center gap-4 text-xs text-secondary">
                          <button
                            type="button"
                            className="secondary"
                            disabled={phraseBusy || phrasePage.offset === 0}
                            onClick={() =>
                              turnPhrasePage(Math.max(0, phrasePage.offset - DICTIONARY_PAGE_SIZE))
                            }
                          >
                            上一页
                          </button>
                          <span aria-live="polite">{phrasePage.status}</span>
                          <button
                            type="button"
                            className="secondary"
                            disabled={phraseBusy || !phrasePage.hasMore}
                            onClick={() => turnPhrasePage(phrasePage.offset + DICTIONARY_PAGE_SIZE)}
                          >
                            下一页
                          </button>
                        </div>
                      </>
                    )}
                    {macosPlatform && client.resetLearnedData && (
                      <LearningDataSection
                        disabled={phraseBusy}
                        onReset={() => void resetLearnedData()}
                      />
                    )}
                  </fieldset>
                  <fieldset disabled={busy} hidden={page !== "skin"} aria-label="皮肤">
                    <SkinPlatformNotice mobile={mobilePlatform} linux={linuxPlatform} />
                    {host?.candidate_panel_limit && (
                      <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
                    )}
                    {mobileKeyboardFeedback?.candidatePaletteFollowsDesktop !== undefined && (
                      <CandidatePaletteSection
                        value={mobileKeyboardFeedback.candidatePaletteFollowsDesktop}
                        busy={mobileKeyboardFeedbackBusy}
                        onChange={(candidatePaletteFollowsDesktop) =>
                          void saveMobileKeyboardFeedback({
                            ...mobileKeyboardFeedback,
                            candidatePaletteFollowsDesktop,
                          })
                        }
                      />
                    )}
                    {snapshot?.candidate_skin_catalog && (
                      <div className={settings.externalMeta} role="status">
                        外部皮肤目录：
                        {snapshot.candidate_skin_catalog.scanned
                          ? `已扫描（${snapshot.candidate_skin_catalog.packages.length} 个）`
                          : "尚未扫描"}
                        {snapshot.candidate_skin_catalog.issues?.length
                          ? `，${snapshot.candidate_skin_catalog.issues.length} 个问题`
                          : ""}
                      </div>
                    )}
                    <BuiltInSkinsSection
                      selected={draft.candidate_skin ?? "willow_green"}
                      previewThemes={skinPreviewThemes}
                      defaultTheme={candidatePreviewTheme}
                      linux={linuxPlatform}
                      onSelect={(id) => setDraft({ ...draft, candidate_skin: id })}
                      onTogglePreview={(id) =>
                        setSkinPreviewThemes((current) => ({
                          ...current,
                          [id]:
                            (current[id] ?? candidatePreviewTheme) === "dark" ? "light" : "dark",
                        }))
                      }
                    />
                    <ExternalSkins
                      activeTheme={candidatePreviewTheme}
                      scan={client.scanSkinCatalog}
                      openDirectory={client.openSkinDirectory}
                      importsSkin={host?.skin_directory_import === true}
                      readImage={client.readSkinImage}
                      readFont={client.readSkinFont}
                      readToolbarCss={client.readSkinToolbarCss}
                      selected={draft.candidate_skin ?? "willow_green"}
                      // A host that draws one layout judges a skin by that layout, not by a setting it ignores.
                      layout={host?.fixed_candidate_layout ?? draft.candidate_layout ?? "vertical"}
                      onSelect={(id) => setDraft({ ...draft, candidate_skin: id })}
                      toolbarPreview={!linuxPlatform}
                    />
                  </fieldset>
                  <fieldset
                    disabled={busy}
                    hidden={page !== "floating-toolbar"}
                    aria-label="悬浮工具栏"
                  >
                    <FloatingToolbarToggleSection
                      preferences={floatingToolbar}
                      skin={draft.candidate_skin ?? "willow_green"}
                      theme={toolbarPreviewTheme}
                      onEnabledChange={(enabled) =>
                        setDraft({
                          ...draft,
                          floating_toolbar: { ...floatingToolbar, enabled },
                        })
                      }
                    />
                    {!showToolbarAppearance && <FloatingToolbarPlatformNotice />}
                    {showToolbarAppearance && (
                      <FloatingToolbarAppearanceSection
                        scale={floatingToolbar.scale_percent}
                        fontSize={floatingToolbar.font_size}
                        onScaleChange={(scale_percent) =>
                          setDraft({
                            ...draft,
                            floating_toolbar: { ...floatingToolbar, scale_percent },
                          })
                        }
                        onFontSizeChange={(font_size) =>
                          setDraft({
                            ...draft,
                            floating_toolbar: { ...floatingToolbar, font_size },
                          })
                        }
                      />
                    )}
                    {showToolbarComponents && (
                      <FloatingToolbarComponentsSection
                        values={floatingToolbar}
                        capabilities={
                          host
                            ? {
                                floating_toolbar_handwriting: host.floating_toolbar_handwriting,
                                floating_toolbar_voice: host.floating_toolbar_voice,
                              }
                            : undefined
                        }
                        onChange={(key, enabled) =>
                          setDraft({
                            ...draft,
                            floating_toolbar: { ...floatingToolbar, [key]: enabled },
                          })
                        }
                      />
                    )}
                  </fieldset>
                  <fieldset disabled={busy} hidden={page !== "input"} aria-label="输入">
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
                    {mobilePlatform && <MobileInputAiNotice onOpenAi={() => selectPage("ai")} />}
                    {!client.touchKeyboardSchemes && (
                      <InputModeSection
                        scheme={draft.scheme}
                        lastChineseScheme={draft.last_chinese_scheme}
                        onChange={(patch) => setDraft({ ...draft, ...patch })}
                      />
                    )}
                    {client.touchKeyboardSchemes && (
                      <TouchKeyboardSchemesSection
                        options={touchKeyboardSchemeOptions}
                        enabled={touchKeyboardSchemes.enabled}
                        selected={selectedTouchKeyboardScheme}
                        onSelect={(scheme) =>
                          setDraft(selectTouchKeyboardScheme(draft, scheme as TouchKeyboardScheme))
                        }
                        onToggle={(scheme, enabled) =>
                          setTouchKeyboardSchemeEnabled(scheme as TouchKeyboardScheme, enabled)
                        }
                      />
                    )}
                    <div hidden={client.touchKeyboardSchemes || draft.scheme === "japanese"}>
                      <InputSchemeSelectorSection
                        value={draft.scheme === "japanese" ? "quanpin" : draft.scheme}
                        onChange={(scheme: InputSchemeSelectorValue) =>
                          setDraft({ ...draft, scheme, last_chinese_scheme: scheme })
                        }
                      />
                    </div>
                    <InputSchemeDetailsSection
                      scheme={draft.scheme}
                      shuangpinProfile={draft.shuangpin_profile}
                      macos={macosPlatform}
                      hasTouchKeyboardSchemes={client.touchKeyboardSchemes ?? false}
                      macosShuangpinKeymap={
                        macosPlatform &&
                        client.loadMacosShuangpinKeymap &&
                        macosShuangpinKeymap !== undefined
                          ? macosShuangpinKeymap
                          : undefined
                      }
                      onShuangpinProfileChange={(shuangpin_profile: ShuangpinProfile) =>
                        setDraft({ ...draft, shuangpin_profile })
                      }
                      onMacosShuangpinKeymapChange={setMacosShuangpinKeymap}
                    />
                    {((client.touchKeyboardSchemes &&
                      touchKeyboardSchemes.enabled.includes("wubi")) ||
                      draft.scheme === "wubi") && (
                      <WubiSection
                        preferences={draft}
                        autoCommitUnique={macosPlatform ? macosWubiAutoCommitUnique : undefined}
                        onChange={(patch) => setDraft({ ...draft, ...patch })}
                        onAutoCommitUniqueChange={setMacosWubiAutoCommitUnique}
                      />
                    )}
                    <NavigationSection
                      navigation={draft.navigation ?? defaultNavigation}
                      wordCharacter={wordCharacter}
                      linux={linuxPlatform}
                      onChange={({ navigation, wordCharacter: nextWordCharacter }) =>
                        setDraft({
                          ...draft,
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
                      showSecondaryLanguage={
                        androidPlatform || iosPlatform || macosPlatform || harmonyPlatform
                      }
                      showAccountTranslation={androidPlatform}
                      accountTranslation={draft.translation_account ?? false}
                      onEnabledChange={(candidate_translations) =>
                        setDraft({ ...draft, candidate_translations })
                      }
                      onTargetLanguageChange={(translation_target_language: TranslationLanguage) =>
                        setDraft({ ...draft, translation_target_language })
                      }
                      onSecondaryLanguageChange={(value: TranslationSecondaryLanguage) =>
                        setDraft({
                          ...draft,
                          translation_secondary_language: value === "" ? null : value,
                        })
                      }
                      onAccountTranslationChange={(enabled) =>
                        enabled
                          ? setTranslationProvider("account")
                          : setDraft({ ...draft, translation_account: undefined })
                      }
                    />
                    {onDeviceMissingLanguages.length > 0 && (
                      <OnDeviceTranslationNotice
                        languages={onDeviceMissingLanguages.map(([, label]) => label)}
                        openSettings={client.onDeviceTranslation?.openSettings}
                        onError={setError}
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
                          onToggle={(enabled) =>
                            setTranslationProvider(enabled ? "niutrans" : "none")
                          }
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
                            { app_id: niutrans.app_id, apikey: niutrans.apikey },
                            !candidateTranslations ||
                              !niutrans.app_id.trim() ||
                              !niutrans.apikey.trim(),
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
                              setDraft({
                                ...draft,
                                tencent_tmt: { ...tencentTranslation, enabled },
                                // Turning on a service of the user's own ends the account choice, so the account never keeps receiving candidates behind a visible selection.
                                ...(enabled ? { translation_account: undefined } : {}),
                              })
                            }
                            onSecretIdChange={(secret_id) =>
                              setDraft({
                                ...draft,
                                tencent_tmt: { ...tencentTranslation, secret_id },
                              })
                            }
                            onSecretKeyChange={(secret_key) =>
                              setDraft({
                                ...draft,
                                tencent_tmt: { ...tencentTranslation, secret_key },
                              })
                            }
                            onRegionChange={(region) =>
                              setDraft({
                                ...draft,
                                tencent_tmt: { ...tencentTranslation, region },
                              })
                            }
                          >
                            {(windowsPlatform || macosPlatform) &&
                              tencentTranslation.enabled &&
                              credentialTestControl(
                                "translation.tencent",
                                "测试腾讯云翻译配置",
                                {
                                  secret_id: tencentTranslation.secret_id,
                                  secret_key: tencentTranslation.secret_key,
                                  region: tencentTranslation.region,
                                },
                                !candidateTranslations ||
                                  Boolean(
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
                            placeholder={customTranslationsExample}
                            notice={customTranslationsNotice}
                            summary={customTranslationsSummary}
                            busy={customTranslationsBusy}
                            onChange={(value) => {
                              setCustomTranslationsText(value);
                              setCustomTranslationsNotice("");
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
                            setDraft({
                              ...draft,
                              custom_translation: { ...customTranslation, enabled },
                              // Same rule as the Tencent switch: a service of the user's own ends the account choice.
                              ...(enabled ? { translation_account: undefined } : {}),
                            })
                          }
                          onEndpointChange={(endpoint) =>
                            setDraft({
                              ...draft,
                              custom_translation: { ...customTranslation, endpoint },
                            })
                          }
                          onApiKeyChange={(api_key) =>
                            setDraft({
                              ...draft,
                              custom_translation: { ...customTranslation, api_key },
                            })
                          }
                        >
                          {credentialTestControl(
                            "translation.custom",
                            "测试自定义翻译配置",
                            {
                              endpoint: customTranslation.endpoint,
                              api_key: customTranslation.api_key,
                            },
                            !candidateTranslations ||
                              Boolean(translationEndpointIssue(customTranslation.endpoint)),
                          )}
                        </CustomTranslationSection>
                      </>
                    )}
                    <WordCharacterSection
                      preferences={wordCharacter}
                      navigation={draft.navigation ?? defaultNavigation}
                      ios={iosPlatform}
                      onChange={({ wordCharacter: nextWordCharacter, navigation }) =>
                        setDraft({
                          ...draft,
                          word_character: nextWordCharacter,
                          navigation,
                        })
                      }
                    />
                    {client.fuzzyPinyin && (
                      <FuzzyPinyinSection
                        preferences={fuzzyPinyin}
                        onChange={(fuzzy_pinyin) => setDraft({ ...draft, fuzzy_pinyin })}
                        confirm={confirm}
                      />
                    )}
                    <LearningSection
                      value={draft.learning}
                      onChange={(learning) => setDraft({ ...draft, learning })}
                    />
                    <PunctuationSection
                      preferences={draft}
                      showCharacterWidth={showCharacterWidth}
                      onChange={(patch) => setDraft({ ...draft, ...patch })}
                    />
                    <MixedInputSection
                      preferences={mixedInput}
                      onChange={(mixed_input) => setDraft({ ...draft, mixed_input })}
                    />
                    {/* macOS keeps this with the chords that trigger it, on the shortcut page. */}
                    {showInputModeHUD && !macosPlatform && (
                      <InputModeHudSection
                        value={draft.input_mode_hud}
                        onChange={(input_mode_hud) => setDraft({ ...draft, input_mode_hud })}
                      />
                    )}
                    {client.candidateEnglishGloss && (
                      <CandidateEnglishGlossSection
                        value={draft.candidate_english_gloss}
                        onChange={(candidate_english_gloss) =>
                          setDraft({ ...draft, candidate_english_gloss })
                        }
                      />
                    )}
                    {showEnglishSuggestions && (
                      <EnglishSuggestionsSection
                        value={draft.english_suggestions}
                        onChange={(english_suggestions) =>
                          setDraft({ ...draft, english_suggestions })
                        }
                      />
                    )}
                    <DefaultImeModeSection
                      value={draft.default_ime_mode}
                      onChange={(default_ime_mode) => setDraft({ ...draft, default_ime_mode })}
                    />
                    {showModeScope && (
                      <ImeModeScopeSection
                        value={draft.ime_mode_scope}
                        onChange={(ime_mode_scope) => setDraft({ ...draft, ime_mode_scope })}
                      />
                    )}
                    <TraditionalChineseOutputSection
                      value={draft.traditional_chinese_output}
                      onChange={(traditional_chinese_output) =>
                        setDraft({ ...draft, traditional_chinese_output })
                      }
                    />
                    <CloudCandidatesSection
                      value={draft.cloud_candidates}
                      onChange={(cloud_candidates) => setDraft({ ...draft, cloud_candidates })}
                    />
                    <FrequencySection
                      preferences={frequency}
                      onChange={(frequency) => setDraft({ ...draft, frequency })}
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
                  <HelpcodeSettingsPage
                    value={draft}
                    mobile={mobilePlatform}
                    showShiftEntry={showHelpcodeShiftEntry}
                    disabled={busy}
                    hidden={page !== "helpcode"}
                    onChange={(patch) => setDraft({ ...draft, ...patch })}
                  />
                  <fieldset disabled={busy} hidden={page !== "shortcuts"} aria-label="快捷键">
                    <ShortcutsIntroSection mobile={mobilePlatform} />
                    <InputModeShortcutsSection
                      keybindings={keybindings}
                      onChange={(patch) =>
                        setDraft({ ...draft, keybindings: { ...keybindings, ...patch } })
                      }
                      onInputModeHUDChange={(input_mode_hud) =>
                        setDraft({ ...draft, input_mode_hud })
                      }
                      showModeSwitchShortcuts={showModeSwitchShortcuts}
                      macos={macosPlatform}
                      showInputModeHUD={showInputModeHUD}
                      inputModeHUD={inputModeHUD}
                      showFullwidthChord={showFullwidthChord}
                      fullwidthChord={fullwidthChord}
                      windows={windowsPlatform}
                    />
                    <PanelShortcutsSection
                      visible={showPanelShortcuts}
                      macos={macosPlatform}
                      harmony={harmonyPlatform}
                    />
                    <CandidateShortcutsSection
                      navigation={draft.navigation ?? defaultNavigation}
                      numberRowSelection={draft.number_row_selection ?? true}
                      showNumberRowSelection={showNumberRowSelection}
                      mobile={mobilePlatform}
                      onNumberRowSelectionChange={(number_row_selection) =>
                        setDraft({ ...draft, number_row_selection })
                      }
                    />
                    <MaintenanceShortcutsSection
                      visible={showDesktopMaintenanceShortcuts}
                      macos={macosPlatform}
                      linux={linuxPlatform}
                      maintenanceChord={maintenanceChord}
                    />
                    <InputMethodServiceSection
                      visible={Boolean(showRestartInputMethod)}
                      macos={macosPlatform}
                      linux={linuxPlatform}
                      restartInputMethod={client.restartInputMethod}
                      installInputSource={
                        showInstallInputSource ? client.installInputSource : undefined
                      }
                    />
                  </fieldset>
                  <fieldset disabled={busy} hidden={page !== "tools"} aria-label="实用功能">
                    <ClipboardHistorySection
                      client={client.clipboard}
                      historyEnabled={clipboardHistory}
                      persistedHistoryEnabled={snapshot?.preferences.clipboard_history ?? false}
                      revision={snapshot?.revision}
                      page={page}
                      ios={iosPlatform}
                      onToggle={toggleClipboardHistory}
                      onError={setError}
                    >
                      {macosPlatform ? (
                        <CloudPanelSessionNotice />
                      ) : (
                        <>
                          {client.openCloudClipboard && (
                            <button
                              type="button"
                              className="secondary"
                              onClick={() => void openPanel(client.openCloudClipboard)}
                            >
                              打开云剪贴板
                            </button>
                          )}
                          {client.openCloudDictionary && (
                            <button
                              type="button"
                              className="secondary"
                              onClick={() => void openPanel(client.openCloudDictionary)}
                            >
                              打开云词典
                            </button>
                          )}
                        </>
                      )}
                    </ClipboardHistorySection>
                    <LocalModesSection
                      preferences={localModes}
                      ios={iosPlatform}
                      onChange={(local_modes) => setDraft({ ...draft, local_modes })}
                    />
                  </fieldset>
                  <HelpSettingsPage
                    busy={busy}
                    hidden={page !== "help"}
                    macos={macosPlatform}
                    mobile={mobilePlatform}
                    ios={iosPlatform}
                    android={androidPlatform}
                    platformHelpIntro={platformHelpIntro}
                    platformQuickStart={platformQuickStart}
                    platformNetworkDescription={platformNetworkDescription}
                    onOpenDocumentation={
                      client.openExternalUrl
                        ? () => void openExternalUrl("https://msime.app/docs/")
                        : undefined
                    }
                    onOpenSystemKeyboardSettings={
                      mobilePlatform && client.openSystemKeyboardSettings
                        ? () => void client.openSystemKeyboardSettings!()
                        : undefined
                    }
                  />
                  <fieldset disabled={busy} hidden={page !== "about"} aria-label="关于">
                    <AboutHeroSection logo={logo} description={platformAboutDescription} />
                    <div className={`section ${doc.linkList}`}>
                      <div className={`${doc.linkRow} ${doc.versionRow}`}>
                        <div>
                          <div className={doc.linkTitle}>当前版本</div>
                          <div className={doc.version}>v{currentAppVersion}</div>
                          {updateStatus && (
                            <p className={doc.updateStatus} role="status">
                              {updateStatus}
                            </p>
                          )}
                        </div>
                        <button
                          type="button"
                          className={`secondary ${doc.updateButton}`}
                          disabled={updateBusy}
                          onClick={() => void checkForUpdate()}
                        >
                          {updateBusy ? "正在检查…" : "检查更新"}
                        </button>
                      </div>
                      {availableUpdate && (
                        <div className={doc.updateResult}>
                          <p>水杉 IME v{availableUpdate.version.display} 已发布。</p>
                          {installerTrust?.warning && (
                            <p className={doc.updateWarning}>{installerTrust.warning}</p>
                          )}
                          {installerTrust?.verify && (
                            <>
                              <p>
                                下载后请核对 SHA256：<code>{installerTrust.verify.sha256}</code>
                              </p>
                              <p>
                                核对命令：<code>{installerTrust.verify.command}</code>
                              </p>
                            </>
                          )}
                          <button
                            type="button"
                            className="secondary"
                            onClick={() => void openExternalUrl(availableUpdate.releaseUrl)}
                          >
                            前往下载
                          </button>
                        </div>
                      )}
                      <button
                        type="button"
                        className={doc.linkRow}
                        onClick={() => void openExternalUrl(platformLicenseUrl)}
                      >
                        <span className={doc.linkTitle}>开源许可协议</span>
                        <span aria-hidden="true">↗</span>
                      </button>
                      <button
                        type="button"
                        className={doc.linkRow}
                        onClick={() =>
                          void openExternalUrl(
                            linuxPlatform
                              ? linuxPrivacyUrl
                              : clientHostedPlatform
                                ? androidPrivacyUrl
                                : privacyUrl,
                          )
                        }
                      >
                        <span className={doc.linkTitle}>隐私政策</span>
                        <span aria-hidden="true">↗</span>
                      </button>
                    </div>
                    <DataDirectorySection
                      visible={Boolean((macosPlatform || linuxPlatform) && client.dataDirectory)}
                      linux={linuxPlatform}
                      dataDirectory={dataDirectory}
                      busy={dataDirectoryBusy}
                      result={dataDirectoryResult}
                      onChoose={() => void chooseDataDirectory()}
                    />
                    {macosPlatform && (
                      <LicenseUninstallSection
                        openThirdPartyLicenses={client.openThirdPartyLicenses}
                        uninstallInputSource={client.uninstallInputSource}
                        removeUserData={removeUserDataOnUninstall}
                        uninstallBusy={uninstallBusy}
                        uninstallConfirmation={uninstallConfirmation}
                        uninstallResult={uninstallResult}
                        onRemoveUserDataChange={setRemoveUserDataOnUninstall}
                        onRequestUninstall={() => {
                          setUninstallResult(null);
                          setUninstallConfirmation(true);
                        }}
                        onConfirmUninstall={() => void uninstallInputSource()}
                        onCancelUninstall={() => setUninstallConfirmation(false)}
                      />
                    )}
                    <HelpFeedbackSection
                      visible={mobilePlatform}
                      onHelp={() => selectPage("help")}
                      onFeedback={() => selectPage("feedback")}
                    />
                    <DiagnosticLogsSection
                      visible={!client.host || linuxPlatform || windowsPlatform || macosPlatform}
                      linux={linuxPlatform}
                      macos={macosPlatform}
                      windows={windowsPlatform || !client.host}
                      values={diagnosticLog}
                      openDirectory={client.openDiagnosticLogDirectory}
                      onChange={(patch) =>
                        setDraft({ ...draft, diagnostic_log: { ...diagnosticLog, ...patch } })
                      }
                      onError={setError}
                    />
                    {/* Only the Windows Server reads this switch; the other hosts report on their own terms, described in PRIVACY.md, so offering it there would be a switch that changes nothing. */}
                    {windowsPlatform && (
                      <TelemetrySection
                        value={draft.telemetry_enabled}
                        onChange={(telemetry_enabled) => setDraft({ ...draft, telemetry_enabled })}
                      />
                    )}
                  </fieldset>
                  <fieldset
                    disabled={busy}
                    hidden={page !== "screen-keyboard"}
                    aria-label="屏幕键盘"
                  >
                    <ScreenKeyboardThemeSection
                      mobile={mobilePlatform}
                      value={draft.screen_keyboard_theme ?? "follow"}
                      onChange={(screen_keyboard_theme) =>
                        setDraft({ ...draft, screen_keyboard_theme })
                      }
                    />
                    <ScreenKeyboardSkinsSection
                      mobile={mobilePlatform}
                      theme={keyboardPreviewTheme}
                      selected={touchKeyboardSkin}
                      customDesign={customTouchKeyboardSkin}
                      customAvailable={Boolean(client.customTouchKeyboardSkins)}
                      editorOpen={showTouchSkinEditor}
                      onSelect={(touch_keyboard_skin) =>
                        setDraft({ ...draft, touch_keyboard_skin })
                      }
                      onToggleEditor={() => setShowTouchSkinEditor((value) => !value)}
                    />
                    {mobilePlatform && client.communitySkins && (
                      <ScreenKeyboardCommunitySection onOpen={() => openCommunity("all")} />
                    )}
                    {client.customTouchKeyboardSkins && showTouchSkinEditor && (
                      <div className="section">
                        <TouchKeyboardSkinEditor
                          design={customTouchKeyboardSkin}
                          selected={touchKeyboardSkin === "custom"}
                          theme={keyboardPreviewTheme}
                          disabled={busy}
                          library={client.customSkinLibrary}
                          aiSkins={client.aiSkins}
                          communitySkins={client.communitySkins}
                          onChange={(design) =>
                            setDraft((current) =>
                              current
                                ? { ...current, custom_touch_keyboard_skin: design }
                                : current,
                            )
                          }
                          onUse={() =>
                            setDraft((current) =>
                              current ? { ...current, touch_keyboard_skin: "custom" } : current,
                            )
                          }
                          onClose={() => setShowTouchSkinEditor(false)}
                        />
                      </div>
                    )}
                    <TouchKeyboardGeometrySection
                      heightAdjustment={touchKeyboardHeightAdjustment}
                      keySpacingTenths={touchKeySpacingTenths}
                      rowSpacingTenths={touchRowSpacingTenths}
                      touchVoiceShortcut={draft.touch_voice_shortcut ?? false}
                      toolbarComponents={Boolean(host?.touch_toolbar_components)}
                      toolbar={draft.touch_toolbar}
                      tabletFullKeys={mobileKeyboardFeedback?.tabletFullKeys}
                      tabletFullKeysBusy={mobileKeyboardFeedbackBusy}
                      onHeightAdjustmentChange={(touch_keyboard_height_adjustment) =>
                        setDraft({ ...draft, touch_keyboard_height_adjustment })
                      }
                      onKeySpacingChange={(touch_key_spacing_tenths) =>
                        setDraft({ ...draft, touch_key_spacing_tenths })
                      }
                      onRowSpacingChange={(touch_row_spacing_tenths) =>
                        setDraft({ ...draft, touch_row_spacing_tenths })
                      }
                      onTouchVoiceShortcutChange={(touch_voice_shortcut) =>
                        setDraft({ ...draft, touch_voice_shortcut })
                      }
                      onToolbarChange={(touch_toolbar) => setDraft({ ...draft, touch_toolbar })}
                      onTabletFullKeysChange={(tabletFullKeys) => {
                        if (mobileKeyboardFeedback) {
                          void saveMobileKeyboardFeedback({
                            ...mobileKeyboardFeedback,
                            tabletFullKeys,
                          });
                        }
                      }}
                      onReset={() => void resetTouchKeyboardSettings()}
                    />
                    <ScreenKeyboardLaunchSection
                      openScreenKeyboard={
                        client.openScreenKeyboard
                          ? () => void openPanel(client.openScreenKeyboard)
                          : undefined
                      }
                      theme={keyboardPreviewTheme}
                      skin={touchKeyboardSkin}
                      customDesign={customTouchKeyboardSkin}
                      keySpacingTenths={touchKeySpacingTenths}
                      rowSpacingTenths={touchRowSpacingTenths}
                      heightAdjustment={touchKeyboardHeightAdjustment}
                      onPointerDown={beginTouchGeometryDrag}
                      onPointerMove={updateTouchGeometryDrag}
                      onPointerUp={endTouchGeometryDrag}
                      onPointerCancel={endTouchGeometryDrag}
                    />
                  </fieldset>
                  <fieldset disabled={busy} hidden={page !== "handwriting"} aria-label="手写识别板">
                    <HandwritingSettingsSection
                      ios={iosPlatform}
                      android={androidPlatform}
                      harmony={harmonyPlatform}
                      macos={macosPlatform}
                      mobile={mobilePlatform}
                      openSystemKeyboardSettings={client.openSystemKeyboardSettings}
                      openHandwriting={client.openHandwriting}
                      onOpenHandwriting={() => void openPanel(client.openHandwriting)}
                    />
                  </fieldset>
                  <fieldset disabled={busy} hidden={page !== "voice"} aria-label="语音输入">
                    <VoiceInputIntroSection
                      localVoice={localVoice}
                      localVoiceModelsAvailable={Boolean(client.localVoiceModels)}
                      systemVoice={systemVoice}
                      systemVoiceHostName={systemVoiceHostName}
                      android={androidPlatform}
                      ios={iosPlatform}
                      macos={macosPlatform}
                      harmony={harmonyPlatform}
                      linux={linuxPlatform}
                      showVoiceProviderSettings={showVoiceProviderSettings}
                      onOpenVoice={
                        client.openVoice ? () => void openPanel(client.openVoice) : undefined
                      }
                    />
                    <VoiceInputCoreSection
                      enabled={voiceInput.enabled}
                      provider={String(voiceInput.asr_provider)}
                      language={voiceInput.language}
                      showProviderSettings={showVoiceProviderSettings}
                      systemVoice={systemVoice}
                      macos={macosPlatform}
                      harmony={harmonyPlatform}
                      android={androidPlatform}
                      localVoiceAvailable={localVoiceAvailable}
                      nativeVoicePlatform={nativeVoicePlatform}
                      harmonyUnsupportedAsr={harmonyUnsupportedAsr}
                      onEnabledChange={(enabled) => updateVoice({ enabled })}
                      onProviderChange={(provider) =>
                        updateVoice({
                          ...asrProviderUpdate(provider, voiceInput),
                          ...(provider === "system" && voiceInput.language === "auto"
                            ? { language: "zh-CN" }
                            : {}),
                          ...(linuxPlatform
                            ? { asr_resource_id: "", doubao_boosting_table_id: "" }
                            : {}),
                        })
                      }
                      onLanguageChange={(language) => updateVoice({ language })}
                    />
                    {localVoice && client.localVoiceModels && (
                      <LocalModelManager
                        client={client.localVoiceModels}
                        mobile={mobilePlatform}
                        modelPath={voiceInput.asr_model_path ?? ""}
                        onUse={(asr_model_path) => updateVoice({ asr_model_path })}
                        onRemoved={(model) =>
                          // Checked against the draft as it is once the removal lands.
                          setDraft((current) =>
                            current &&
                            localModelInUse(model, current.voice_input?.asr_model_path ?? "")
                              ? {
                                  ...current,
                                  voice_input: {
                                    ...defaultVoiceInput,
                                    ...current.voice_input,
                                    asr_model_path: "",
                                  },
                                }
                              : current,
                          )
                        }
                        confirm={confirm}
                        openExternalUrl={client.openExternalUrl ? openExternalUrl : undefined}
                      />
                    )}
                    {localVoice && client.localVoiceModels && (
                      <VoiceModelMirrorSection
                        value={voiceInput.asr_model_mirror ?? ""}
                        onChange={(asr_model_mirror) => updateVoice({ asr_model_mirror })}
                      />
                    )}
                    {localVoice && (
                      <VoiceModelPathDisclosure
                        disclosure={Boolean(client.localVoiceModels)}
                        path={voiceInput.asr_model_path ?? ""}
                        pickPath={client.pickVoiceModelPath}
                        onChange={(asr_model_path) => updateVoice({ asr_model_path })}
                      />
                    )}
                    {showVoiceProviderSettings &&
                      serviceVoice &&
                      providerPresetControls(
                        "识别服务",
                        ASR_PROVIDER_DEFAULTS[String(voiceInput.asr_provider)],
                        voiceInput.asr_model ?? "",
                        (asr_model) => updateVoice({ asr_model }),
                      )}
                    {showVoiceProviderSettings && serviceVoice && (
                      <VoiceModelSection
                        value={voiceInput.asr_model ?? ""}
                        onChange={(asr_model) => updateVoice({ asr_model })}
                      />
                    )}
                    {showVoiceProviderSettings && voiceInput.asr_provider === "doubao" && (
                      <DoubaoAuthModeSection
                        value={doubaoAuthMode}
                        linux={linuxPlatform}
                        onChange={(doubao_auth_mode) => updateVoice({ doubao_auth_mode })}
                      />
                    )}
                    {showVoiceProviderSettings && !linuxPlatform && serviceVoice && (
                      <>
                        {voiceInput.asr_provider === "doubao" && (
                          <DoubaoStreamEndpointSection
                            endpoint={voiceInput.asr_endpoint ?? ""}
                            onChange={(asr_endpoint) => updateVoice({ asr_endpoint })}
                          />
                        )}
                        <VoiceEndpointSection
                          value={voiceInput.asr_endpoint ?? ""}
                          onChange={(asr_endpoint) => updateVoice({ asr_endpoint })}
                        />
                        <VoiceCredentialFieldsSection
                          showAppKey={
                            voiceInput.asr_provider === "doubao" && doubaoAuthMode === "legacy"
                          }
                          appKey={voiceInput.asr_app_key ?? ""}
                          tokenLabel={
                            voiceInput.asr_provider === "doubao" && doubaoAuthMode !== "legacy"
                              ? "Doubao API Key"
                              : "识别 API Token"
                          }
                          token={voiceInput.asr_token ?? ""}
                          onAppKeyChange={(asr_app_key) => updateVoice({ asr_app_key })}
                          onTokenChange={(asr_token) => updateVoice({ asr_token })}
                        />
                      </>
                    )}
                    {showVoiceProviderSettings && serviceVoice && (
                      <DoubaoResourceIdSection
                        value={voiceInput.asr_resource_id ?? ""}
                        onChange={(asr_resource_id) => updateVoice({ asr_resource_id })}
                      />
                    )}
                    {linuxPlatform &&
                      client.providerCredentials &&
                      ["openai", "siliconflow", "groq", "everyapi", "mistral", "doubao"].includes(
                        voiceInput.asr_provider ?? "doubao",
                      ) &&
                      (() => {
                        const provider = voiceInput.asr_provider ?? "doubao";
                        return (
                          <VoiceCredentialSection
                            kind="asr"
                            provider={provider}
                            model={voiceInput.asr_model ?? ""}
                            resourceId={voiceInput.asr_resource_id}
                            authMode={doubaoAuthMode}
                            credentials={providerCredentials}
                            input={voiceCredentialInput.asr}
                            busy={providerCredentialBusy === "asr"}
                            message={providerCredentialMessages.asr}
                            onChange={(patch) =>
                              setVoiceCredentialInput((current) => ({
                                ...current,
                                asr: { ...current.asr, ...patch },
                              }))
                            }
                            onSave={(credential: VoiceCredentialSaveInput) =>
                              void runVoiceCredential(
                                "asr",
                                (credentials) => credentials.saveVoice(credential),
                                "凭据已保存，语音 provider 下次请求时生效。",
                              )
                            }
                            onClear={() =>
                              void runVoiceCredential(
                                "asr",
                                (credentials) => credentials.clearVoice("asr", provider),
                                "凭据已清除。",
                              )
                            }
                          />
                        );
                      })()}
                    {linuxPlatform &&
                      credentialTestControl("voice.asr", "测试语音识别配置", {
                        asr_provider: voiceInput.asr_provider ?? "doubao",
                        asr_model: voiceInput.asr_model ?? "",
                        asr_resource_id: voiceInput.asr_resource_id ?? "",
                        doubao_auth_mode: doubaoAuthMode,
                        doubao_enable_itn: voiceInput.doubao_enable_itn !== false,
                        doubao_enable_punc: voiceInput.doubao_enable_punc !== false,
                        doubao_enable_ddc: voiceInput.doubao_enable_ddc === true,
                      })}
                    {/*
                     * Doubao belongs in this list, not in a HarmonyOS-only arm: the probe is the
                     * shared one, and Windows and macOS have had it since it was added. Gating it
                     * on HarmonyOS alone silently dropped the button on the two hosts whose tests
                     * cover it.
                     */}
                    {(windowsPlatform || macosPlatform || harmonyPlatform) &&
                      ["openai", "siliconflow", "groq", "everyapi", "mistral", "doubao"].includes(
                        voiceInput.asr_provider ?? "",
                      ) && (
                        <>
                          <VoiceSyntheticSilenceNotice />
                          {credentialTestControl(
                            "voice.asr",
                            voiceInput.asr_provider === "doubao"
                              ? "测试豆包识别配置"
                              : "测试语音识别配置",
                            {
                              provider: voiceInput.asr_provider,
                              endpoint:
                                voiceInput.asr_endpoint?.trim() ||
                                ASR_PROVIDER_DEFAULTS[voiceInput.asr_provider ?? ""]?.endpoint ||
                                "",
                              model:
                                voiceInput.asr_model?.trim() ||
                                ASR_PROVIDER_DEFAULTS[voiceInput.asr_provider ?? ""]?.model ||
                                "",
                              token: voiceInput.asr_token ?? "",
                              ...(voiceInput.asr_provider === "doubao"
                                ? {
                                    auth_mode: doubaoAuthMode,
                                    app_id:
                                      doubaoAuthMode === "legacy"
                                        ? (voiceInput.asr_app_key ?? "")
                                        : "",
                                    resource_id:
                                      voiceInput.asr_resource_id ?? "volc.seedasr.sauc.duration",
                                    doubao_enable_itn: voiceInput.doubao_enable_itn !== false,
                                    doubao_enable_punc: voiceInput.doubao_enable_punc !== false,
                                    doubao_enable_ddc: voiceInput.doubao_enable_ddc === true,
                                    doubao_boosting_table_id:
                                      voiceInput.doubao_boosting_table_id ?? "",
                                  }
                                : {}),
                            },
                            !voiceInput.asr_token?.trim() ||
                              (voiceInput.asr_provider === "doubao" &&
                                doubaoAuthMode === "legacy" &&
                                !voiceInput.asr_app_key?.trim()),
                          )}
                        </>
                      )}
                    {showVoiceStreamPreedit && (
                      <VoiceStreamPreeditSection
                        enabled={voiceInput.stream_inline_preedit === true}
                        onChange={(stream_inline_preedit) => updateVoice({ stream_inline_preedit })}
                      />
                    )}
                    {showVoiceCommitMode && (
                      <VoiceCommitModeSection
                        macos={macosPlatform}
                        value={voiceInput.commit_mode ?? "tsf"}
                        onChange={(commit_mode) => updateVoice({ commit_mode })}
                      />
                    )}
                    {showVoiceCaptureDevices && (
                      <VoiceCaptureDevicesSection
                        windows={windowsPlatform}
                        harmony={harmonyPlatform}
                        backend={voiceInput.capture_backend ?? ""}
                        device={voiceInput.capture_device ?? ""}
                        backendOptions={captureBackendOptions}
                        readDevices={client.listVoiceCaptureDevices!}
                        onBackendChange={(capture_backend, capture_device) =>
                          updateVoice({ capture_backend, capture_device })
                        }
                        onDeviceChange={(capture_device) => updateVoice({ capture_device })}
                      />
                    )}
                    {/* Not provider configuration: these four are the host's own recording
                        behaviour, and the Android host plays no prompt tones and does not mute
                        system audio while it records. Four switches with nothing behind them is
                        what this page keeps being audited for. */}
                    {!androidPlatform && (
                      <VoiceRecordingBehaviorSection
                        linux={linuxPlatform}
                        soundEnabled={voiceInput.sound_enabled !== false}
                        startSound={voiceInput.start_sound !== false}
                        endSound={voiceInput.end_sound !== false}
                        muteSystemAudio={voiceInput.mute_system_audio === true}
                        onSoundEnabledChange={(sound_enabled) => updateVoice({ sound_enabled })}
                        onStartSoundChange={(start_sound) => updateVoice({ start_sound })}
                        onEndSoundChange={(end_sound) => updateVoice({ end_sound })}
                        onMuteSystemAudioChange={(mute_system_audio) =>
                          updateVoice({ mute_system_audio })
                        }
                      />
                    )}
                    {showVoiceProviderSettings && voiceInput.asr_provider === "doubao" && (
                      <DoubaoOptionsSection
                        linux={linuxPlatform}
                        enableItn={voiceInput.doubao_enable_itn !== false}
                        enablePunc={voiceInput.doubao_enable_punc !== false}
                        enableDdc={voiceInput.doubao_enable_ddc === true}
                        boostingTableId={voiceInput.doubao_boosting_table_id ?? ""}
                        onEnableItnChange={(doubao_enable_itn) =>
                          updateVoice({ doubao_enable_itn })
                        }
                        onEnablePuncChange={(doubao_enable_punc) =>
                          updateVoice({ doubao_enable_punc })
                        }
                        onEnableDdcChange={(doubao_enable_ddc) =>
                          updateVoice({ doubao_enable_ddc })
                        }
                        onBoostingTableIdChange={(doubao_boosting_table_id) =>
                          updateVoice({ doubao_boosting_table_id })
                        }
                      />
                    )}
                    {showVoiceProviderSettings && (
                      <VoicePolishSection
                        enabled={
                          voiceInput.polish_text === true || voiceInput.polish_enabled === true
                        }
                        provider={voiceInput.polish_provider ?? "siliconflow"}
                        model={voiceInput.polish_model ?? ""}
                        providerPreset={providerPresetControls(
                          "文本润色",
                          POLISH_PROVIDER_DEFAULTS[voiceInput.polish_provider ?? "siliconflow"],
                          voiceInput.polish_model ?? "",
                          (polish_model) => updateVoice({ polish_model }),
                          "provider-preset-section",
                        )}
                        onEnabledChange={(enabled) =>
                          updateVoice({ polish_text: enabled, polish_enabled: enabled })
                        }
                        onProviderChange={(provider) =>
                          updateVoice(polishProviderUpdate(provider, voiceInput))
                        }
                        onModelChange={(polish_model) => updateVoice({ polish_model })}
                      >
                        {!linuxPlatform && (
                          <PolishCredentialFieldsSection
                            endpoint={voiceInput.polish_endpoint ?? ""}
                            token={voiceInput.polish_token ?? ""}
                            onEndpointChange={(polish_endpoint) => updateVoice({ polish_endpoint })}
                            onTokenChange={(polish_token) => updateVoice({ polish_token })}
                          />
                        )}
                        <PolishPromptSection
                          promptId={voiceInput.polish_prompt_id}
                          prompt={voiceInput.polish_prompt ?? ""}
                          customPrompts={{
                            custom_1: voiceInput.polish_prompt_custom_1,
                            custom_2: voiceInput.polish_prompt_custom_2,
                            custom_3: voiceInput.polish_prompt_custom_3,
                          }}
                          onSelectPrompt={(polish_prompt_id, polish_prompt) =>
                            updateVoice({ polish_prompt_id, polish_prompt })
                          }
                          onPromptChange={(polish_prompt, customSlot) =>
                            updateVoice({
                              polish_prompt,
                              ...(customSlot
                                ? { [`polish_prompt_${customSlot}`]: polish_prompt }
                                : {}),
                            })
                          }
                          onRestore={(polish_prompt) => updateVoice({ polish_prompt })}
                        />
                        {linuxPlatform &&
                          client.providerCredentials &&
                          (() => {
                            const provider = voiceInput.polish_provider ?? "siliconflow";
                            return (
                              <VoiceCredentialSection
                                kind="polish"
                                provider={provider}
                                model={voiceInput.polish_model ?? ""}
                                credentials={providerCredentials}
                                input={voiceCredentialInput.polish}
                                busy={providerCredentialBusy === "polish"}
                                message={providerCredentialMessages.polish}
                                onChange={(patch) =>
                                  setVoiceCredentialInput((current) => ({
                                    ...current,
                                    polish: { ...current.polish, ...patch },
                                  }))
                                }
                                onSave={(credential: VoiceCredentialSaveInput) =>
                                  void runVoiceCredential(
                                    "polish",
                                    (credentials) => credentials.saveVoice(credential),
                                    "凭据已保存，语音 provider 下次请求时生效。",
                                  )
                                }
                                onClear={() =>
                                  void runVoiceCredential(
                                    "polish",
                                    (credentials) => credentials.clearVoice("polish", provider),
                                    "凭据已清除。",
                                  )
                                }
                              />
                            );
                          })()}
                        {linuxPlatform &&
                          credentialTestControl(
                            "voice.polish",
                            "测试语音润色配置",
                            {
                              polish_provider: voiceInput.polish_provider ?? "siliconflow",
                              polish_model: voiceInput.polish_model ?? "",
                            },
                            !(
                              voiceInput.polish_text === true || voiceInput.polish_enabled === true
                            ),
                          )}
                        {(windowsPlatform || macosPlatform || iosPlatform || harmonyPlatform) &&
                          credentialTestControl(
                            "voice.polish",
                            "测试语音润色配置",
                            {
                              provider: voiceInput.polish_provider ?? "siliconflow",
                              endpoint:
                                voiceInput.polish_endpoint?.trim() ||
                                POLISH_PROVIDER_DEFAULTS[
                                  voiceInput.polish_provider ?? "siliconflow"
                                ]?.endpoint ||
                                "",
                              model:
                                voiceInput.polish_model?.trim() ||
                                POLISH_PROVIDER_DEFAULTS[
                                  voiceInput.polish_provider ?? "siliconflow"
                                ]?.model ||
                                "",
                              token: voiceInput.polish_token ?? "",
                            },
                            !voiceInput.polish_token?.trim(),
                          )}
                      </VoicePolishSection>
                    )}
                    {desktopPanels && (
                      <VoiceHotkeysSection
                        platform={
                          macosPlatform
                            ? "macos"
                            : windowsPlatform
                              ? "windows"
                              : linuxPlatform
                                ? "linux"
                                : "other"
                        }
                        values={draft.voice_input ?? {}}
                        onChange={(key, enabled) =>
                          setDraft({
                            ...draft,
                            voice_input: {
                              ...draft.voice_input,
                              enabled: draft.voice_input?.enabled ?? true,
                              language: draft.voice_input?.language ?? "zh-CN",
                              [key]: enabled,
                            },
                          })
                        }
                      />
                    )}
                  </fieldset>
                  <fieldset disabled={busy} hidden={page !== "ai"} aria-label="AI 辅助">
                    <AiBasicSettingsSection
                      enabled={ai.enabled}
                      enabledDescription={
                        iosPlatform
                          ? "为键盘 AI 联想、回复与润色提供共享配置"
                          : androidPlatform
                            ? "为拼音联想和 Android 选中文字润色提供共享配置"
                            : "为拼音联想提供共享配置"
                      }
                      provider={ai.provider}
                      providerOptions={AI_PROVIDER_OPTIONS as readonly AiProviderOption[]}
                      model={ai.model}
                      endpoint={ai.endpoint}
                      providerPreset={providerPresetControls(
                        "AI ",
                        AI_PROVIDER_OPTIONS.find((option) => option.id === ai.provider),
                        ai.model,
                        (model) => updateAi({ model }),
                      )}
                      onEnabledChange={(enabled) => updateAi({ enabled })}
                      onProviderChange={(provider) => updateAi(aiProviderUpdate(provider, ai))}
                      onModelChange={(model) => updateAi({ model })}
                      onEndpointChange={(endpoint) => updateAi({ endpoint })}
                    />
                    {linuxPlatform && client.providerCredentials ? (
                      <AiCredentialSection
                        endpoint={ai.endpoint}
                        model={ai.model}
                        origin={aiOrigin}
                        token={aiCredentialInput}
                        stored={storedAiCredential}
                        invalid={providerCredentials?.aiInvalid === true}
                        busy={providerCredentialBusy === "ai"}
                        message={providerCredentialMessages.ai}
                        onTokenChange={setAiCredentialInput}
                        onSave={() =>
                          void runProviderCredential(
                            "ai",
                            (credentials) =>
                              credentials.saveAi({
                                provider: ai.provider,
                                endpoint: ai.endpoint,
                                model: ai.model,
                                ...(aiCredentialInput.trim() ? { token: aiCredentialInput } : {}),
                              }),
                            "凭据已保存，provider 服务下次请求时生效。",
                          )
                        }
                        onClear={() =>
                          void runProviderCredential(
                            "ai",
                            (credentials) => credentials.clearAi(ai.provider),
                            "凭据已清除。",
                          )
                        }
                      >
                        {credentialTestControl(
                          "ai.assistant",
                          "测试 AI 辅助配置",
                          { provider: ai.provider, endpoint: ai.endpoint, model: ai.model },
                          !ai.enabled || !aiOrigin || !ai.model.trim(),
                        )}
                      </AiCredentialSection>
                    ) : linuxPlatform ? (
                      <AiLinuxProviderSection>
                        {credentialTestControl(
                          "ai.assistant",
                          "测试 AI 辅助配置",
                          { provider: ai.provider, endpoint: ai.endpoint, model: ai.model },
                          !ai.enabled || !aiOrigin || !ai.model.trim(),
                        )}
                      </AiLinuxProviderSection>
                    ) : (
                      <AiApiTokenSection
                        origin={aiOrigin}
                        token={aiToken}
                        onTokenChange={updateAiToken}
                      />
                    )}
                    {(windowsPlatform || macosPlatform || iosPlatform) &&
                      credentialTestControl(
                        "ai.assistant",
                        "测试 AI 辅助配置",
                        {
                          provider: ai.provider,
                          endpoint: ai.endpoint,
                          model: ai.model,
                          token: aiToken,
                        },
                        !ai.enabled || !aiOrigin || !ai.model.trim() || !aiToken.trim(),
                      )}
                    {client.aiAssistant && (
                      <AiModelCatalogSection
                        busy={aiModelsBusy}
                        origin={aiOrigin ?? ""}
                        models={aiModels ?? undefined}
                        selectedModel={ai.model}
                        status={aiModelsStatus}
                        onFetch={() => void fetchAiModels()}
                        onSelect={(model) => updateAi({ model })}
                      />
                    )}
                    <AiCandidateLimitSection
                      value={ai.candidate_limit}
                      onChange={(candidate_limit) => updateAi({ candidate_limit })}
                    />
                    <AiPromptSettingsSection
                      promptId={ai.prompt_id}
                      prompt={ai.prompt}
                      promptCustom1={ai.prompt_custom_1 ?? ""}
                      promptCustom2={ai.prompt_custom_2 ?? ""}
                      promptCustom3={ai.prompt_custom_3 ?? ""}
                      fallbackPrompt={defaultAiAssistant.prompt ?? ""}
                      onPromptIdChange={(prompt_id) => updateAi({ prompt_id })}
                      onPromptChange={(prompt) => updateAi({ prompt })}
                      onPromptCustom1Change={(prompt_custom_1) => updateAi({ prompt_custom_1 })}
                      onPromptCustom2Change={(prompt_custom_2) => updateAi({ prompt_custom_2 })}
                      onPromptCustom3Change={(prompt_custom_3) => updateAi({ prompt_custom_3 })}
                    />
                    {client.aiAssistant && (
                      <AiTestToolsSection
                        input={aiTestInput}
                        busy={aiTestBusy}
                        status={aiTestStatus}
                        output={aiTestOutput}
                        onInputChange={setAiTestInput}
                        onTest={() => void testAi()}
                        onCopyOutput={
                          client.copyText ? () => void client.copyText!(aiTestOutput) : undefined
                        }
                      />
                    )}
                    {client.mcpServerStatus && (
                      <McpConnectSection
                        status={client.mcpServerStatus}
                        install={client.installMcpClient}
                        copyText={client.copyText}
                      />
                    )}
                  </fieldset>
                  <fieldset disabled={busy} hidden={page !== "feedback"} aria-label="反馈">
                    <FeedbackSettingsSection
                      hero={doc.hero}
                      eyebrow={doc.eyebrow}
                      heroTitle={doc.heroTitle}
                      note={doc.note}
                      feedbackList={doc.feedbackList}
                      feedbackCard={doc.feedbackCard}
                      feedbackIcon={doc.feedbackIcon}
                      feedbackBody={doc.feedbackBody}
                      feedbackTitle={doc.feedbackTitle}
                      serviceRow={settings.serviceRow}
                      kind={feedbackKind}
                      detail={feedbackDetail}
                      reportCopied={feedbackReportCopied}
                      feedbackCopied={feedbackCopied}
                      supportDiagnostics={supportDiagnostics}
                      issuesUrl={platformIssuesUrl}
                      copyText={client.copyText}
                      openExternalUrl={client.openExternalUrl}
                      onKindChange={setFeedbackKind}
                      onDetailChange={setFeedbackDetail}
                      onCopyReport={() => {
                        if (!client.copyText) return;
                        void client.copyText(feedbackReport).then(() => {
                          setFeedbackReportCopied(true);
                          window.setTimeout(() => setFeedbackReportCopied(false), 1600);
                        });
                      }}
                      onSubmitFeedback={submitFeedback}
                      onOpenIssues={() => void openExternalUrl(platformIssuesUrl)}
                      onCopyGroup={() => {
                        if (!client.copyText) return;
                        void client.copyText("829919142").then(() => {
                          setFeedbackCopied(true);
                          window.setTimeout(() => setFeedbackCopied(false), 1600);
                        });
                      }}
                      onOpenTelegram={() => void openExternalUrl("https://t.me/msimegroup")}
                    />
                  </fieldset>
                  {!validCandidateFonts(draft) && (
                    <p role="alert">
                      请在外观页修正字体：名称不能为空、不能含控制字符或超过 128 个 UTF-8
                      字节，补充字体最多 32 项。
                    </p>
                  )}
                  <SettingsActionsFooter
                    busy={busy}
                    dirty={dirty}
                    canSave={validCandidateFonts(draft)}
                    showRestoreDefaults={Boolean(client.loadDefaultPreferences)}
                    onRestoreDefaults={() => void restoreDefaults()}
                  />
                </form>
              )}
            {page !== "typing-statistics" &&
              page !== "vocabulary" &&
              page !== "account" &&
              page !== "chat" &&
              page !== "community" && (
                <button
                  className="secondary"
                  disabled={busy}
                  onClick={() => {
                    if (!dirty) {
                      void reload();
                      return;
                    }
                    void confirm({
                      title: "重新读取",
                      message: "尚未保存的修改会被放弃。",
                      confirmLabel: "放弃并重新读取",
                      danger: true,
                    }).then((confirmed) => {
                      if (confirmed) void reload();
                    });
                  }}
                >
                  重新读取
                </button>
              )}
          </div>
        </main>
      </div>
    </div>
  );
}
