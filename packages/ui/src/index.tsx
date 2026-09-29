import { AiSettingsPanel } from "./settings/ai-settings-panel";
import { InputSettingsPanel } from "./settings/input-settings-panel";
import { VoiceSettingsPanel } from "./settings/voice-settings-panel";
import { useConfirm } from "./core/confirm";
import { errorMessage } from "./core/error-message";
import { clamp } from "./core/number";
import {
  inferredTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  type TouchKeyboardSchemePreferences,
} from "./settings/touch-keyboard-scheme-helpers";
export {
  type TouchKeyboardScheme,
  type TouchKeyboardSchemePreferences,
  inferredTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  allTouchKeyboardSchemes,
} from "./settings/touch-keyboard-scheme-helpers";
import { schemeTitle } from "./settings/label-helpers";
import { platformCopy, type PlatformCopyContext } from "./settings/platform-copy";
export {
  platformCopy,
  type PlatformCopyContext,
  type PlatformCopy,
} from "./settings/platform-copy";
import { settingsVisualPreferences } from "./settings/settings-visual-preferences";
export {
  settingsVisualPreferences,
  type SettingsVisualPreferences,
  type SettingsVisualPreferencesSource,
} from "./settings/settings-visual-preferences";
import { settingsInputPreferences } from "./settings/settings-input-preferences";
export {
  settingsInputPreferences,
  type SettingsInputPreferences,
  type SettingsInputPreferencesSource,
} from "./settings/settings-input-preferences";
import { platformResourceUrls } from "./settings/platform-resource-urls";
import {
  initialMobileTabPages,
  initialSettingsPage,
  mobileHeaderlessPageIds,
  splitMobilePages,
  type SettingsPageId,
} from "./settings/mobile-navigation";
export {
  initialMobileTabPages,
  initialSettingsPage,
  mobileHeaderlessPageIds,
  type InitialSettingsPageOptions,
} from "./settings/mobile-navigation";
import { useSettingsNavigation } from "./settings/use-settings-navigation";
export {
  useSettingsNavigation,
  type SettingsNavigationOptions,
} from "./settings/use-settings-navigation";
import { useSettingsContentScrollReset } from "./settings/use-settings-content-scroll-reset";
export { useSettingsContentScrollReset } from "./settings/use-settings-content-scroll-reset";
import { isLinuxDesktop } from "./settings/platform-helpers";
import { useSettingsTheme } from "./settings/use-settings-theme";
export { useSettingsTheme } from "./settings/use-settings-theme";
import { useTouchKeyboardGeometryDrag } from "./settings/use-touch-keyboard-geometry-drag";
export { useTouchKeyboardGeometryDrag } from "./settings/use-touch-keyboard-geometry-drag";
import { useMobileKeyboardFeedback } from "./settings/use-mobile-keyboard-feedback";
export {
  useMobileKeyboardFeedback,
  type UseMobileKeyboardFeedbackOptions,
} from "./settings/use-mobile-keyboard-feedback";
import { fallbackAppVersion, logo } from "./settings/app-resources";
export { AI_PROVIDER_OPTIONS } from "./settings/ai-provider-options";
import { defaultVoiceInput } from "./settings/voice-input-defaults";
import { settingsSidebarGroups } from "./settings/sidebar-groups";
export {
  settingsSidebarGroups,
  type SettingsSidebarGroupsOptions,
} from "./settings/sidebar-groups";
import { mobileHiddenPageIds as getMobileHiddenPageIds } from "./settings/mobile-hidden-pages";
import { defaultAiAssistant } from "./settings/ai-assistant-defaults";
import { useTranslationSettings } from "./settings/use-translation-settings";
export {
  useTranslationSettings,
  type UseTranslationSettingsOptions,
} from "./settings/use-translation-settings";
export { aiProviderUpdate } from "./settings/ai-provider-update";
import type { VoiceDeviceReader } from "./voice/voice-device-picker";
import {
  LocalModelManager,
  localModelInUse,
  validModelMirror,
  type LocalVoiceModelClient,
} from "./voice/local-models";
import { useRef, useState } from "react";
import type {
  DictionaryEntry,
  LocalDictionaryFormat,
  LocalDictionaryKind,
} from "./dictionary/dictionary-file";
export type {
  DictionaryEntry,
  LocalDictionaryFormat,
  LocalDictionaryKind,
} from "./dictionary/dictionary-file";
import { describeImportResult } from "./dictionary/dictionary-messages";
import {
  dictionaryExportName,
  dictionaryExportPayload,
  personalDictionaryExportName,
  personalDictionaryExportPayload,
  loadAllPersonalDictionaryEntries,
  dictionaryKindLabel,
} from "./dictionary/dictionary-export";
import { dictionaryErrorMessage } from "./dictionary/dictionary-errors";
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
import { AppearanceSettingsSection } from "./settings/appearance-settings-section";
import { createAppearanceSettingsActions } from "./settings/appearance-settings-actions";
import { appearanceSettingsPreferences } from "./settings/appearance-settings-preferences";
import {
  useSettingsPreviewThemes,
  type SettingsSkinPreviewThemes,
} from "./settings/use-settings-preview-themes";
import { LearningDataSection } from "./settings/learning-data-section";
import { DictionaryManagerHeader } from "./settings/dictionary-manager-header";
import { DictionaryFailuresNotice } from "./settings/dictionary-failures-notice";
import { DictionaryManagerControls } from "./settings/dictionary-manager-controls";
import { DictionaryEntries } from "./settings/dictionary-entries";
import { DictionaryPagination } from "./settings/dictionary-pagination";
import { useDictionaryManager } from "./settings/use-dictionary-manager";
export {
  useDictionaryManager,
  type DictionaryManagerClient,
  type DictionaryConfirmOptions,
  type UseDictionaryManagerOptions,
} from "./settings/use-dictionary-manager";
export {
  DictionaryFailuresNotice,
  type DictionaryFailuresNoticeProps,
  type DictionaryFailureNotice,
} from "./settings/dictionary-failures-notice";
export {
  DictionaryManagerControls,
  type DictionaryManagerControlsProps,
} from "./settings/dictionary-manager-controls";
export {
  DictionaryPagination,
  type DictionaryPaginationProps,
} from "./settings/dictionary-pagination";
export {
  createDictionaryPanelActions,
  type CreateDictionaryPanelActionsOptions,
} from "./settings/dictionary-panel-actions";
export {
  DictionaryEntries,
  type DictionaryEntriesProps,
  type DictionaryPhraseForm,
} from "./settings/dictionary-entries";
import { SettingsActionsFooter } from "./settings/settings-actions-footer";
import { AboutHeroSection } from "./settings/about-hero-section";
import { AboutSettingsSection } from "./settings/about-settings-section";
import { TelemetrySection } from "./settings/telemetry-section";
import { InputModeShortcutsSection } from "./settings/input-mode-shortcuts-section";
import { ShortcutsIntroSection } from "./settings/shortcuts-intro-section";
import { PanelShortcutsSection } from "./settings/panel-shortcuts-section";
import { CandidateShortcutsSection } from "./settings/candidate-shortcuts-section";
import { MaintenanceShortcutsSection } from "./settings/maintenance-shortcuts-section";
import { InputMethodServiceSection } from "./settings/input-method-service-section";
import { DataDirectorySection } from "./settings/data-directory-section";
import { LicenseUninstallSection } from "./settings/license-uninstall-section";
import {
  DiagnosticLogsSection,
  diagnosticLogPreferences,
} from "./settings/diagnostic-logs-section";
import { HelpFeedbackSection } from "./settings/help-feedback-section";
import { HelpSettingsPage } from "./settings/help-settings-page";
import { ScreenKeyboardSettingsSection } from "./settings/screen-keyboard-settings-section";
import type { TouchToolbarPreferences } from "./settings/touch-keyboard-geometry-section";
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
import { voiceCaptureBackendOptions } from "./settings/voice-capture-backend-options";
import { fullwidthShortcutChord, maintenanceShortcutChord } from "./settings/platform-shortcuts";
import { settingsDirty } from "./settings/settings-dirty";
import { settingsPlatformContext } from "./settings/settings-platform-context";
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
import { FeedbackPageSection } from "./settings/feedback-page-section";
import { HandwritingSettingsSection } from "./settings/handwriting-settings-section";
import {
  InputSourceStartupNotice,
  type InputSourceStartupStatus,
} from "./settings/input-source-startup-notice";
import { VoiceModelMirrorSection } from "./settings/voice-model-mirror-section";
import { useProviderCredentials } from "./settings/use-provider-credentials";
import { useFeedbackReport } from "./settings/use-feedback-report";
import { useDataDirectory } from "./settings/use-data-directory";
import { useCustomTranslations } from "./settings/use-custom-translations";
import { usePreferenceRecovery } from "./settings/use-preference-recovery";
import { useSettingsPersistence } from "./settings/use-settings-persistence";
import { useInputSourceUninstall } from "./settings/use-input-source-uninstall";
import { useTouchKeyboardSettingsReset } from "./settings/use-touch-keyboard-settings-reset";
import { useExternalUrl } from "./settings/use-external-url";
import { useUpdateCheck } from "./settings/use-update-check";
import { useOpenPanel } from "./settings/use-open-panel";
import { useClipboardHistoryToggle } from "./settings/use-clipboard-history-toggle";
import { clipboardHistoryEnabled } from "./settings/clipboard-history-preferences";
export {
  clipboardHistoryEnabled,
  type ClipboardHistoryPreferencesSource,
} from "./settings/clipboard-history-preferences";
import { useTouchKeyboardSchemeSelection } from "./settings/use-touch-keyboard-scheme-selection";
import { useSettingsDestinationActions } from "./settings/use-settings-destination-actions";
import { useMacosSettings } from "./settings/use-macos-settings";
import { useWindowState } from "./settings/use-window-state";
import { useWindowResizeCapture } from "./settings/use-window-resize-capture";
import { useAccountPageActions } from "./settings/use-account-page-actions";
import { createSettingsExternalActions } from "./settings/settings-external-actions";
import { createAboutSettingsActions } from "./settings/about-settings-actions";
import { createScreenKeyboardActions } from "./settings/screen-keyboard-actions";
import { createShortcutsSettingsActions } from "./settings/shortcuts-settings-actions";
import { createHelpcodeSettingsActions } from "./settings/helpcode-settings-actions";
import { createUtilitiesSettingsActions } from "./settings/utilities-settings-actions";
import { useAppVersion } from "./settings/use-app-version";
import { supportDiagnostics } from "./settings/support-diagnostics";
import { useMountedRef } from "./settings/use-mounted-ref";
export {
  useProviderCredentials,
  type ProviderCredentialBusy,
  type ProviderCredentialInput,
  type ProviderCredentialMessage,
  type ProviderCredentialsHost,
  type UseProviderCredentialsOptions,
} from "./settings/use-provider-credentials";
export { useFeedbackReport, type UseFeedbackReportOptions } from "./settings/use-feedback-report";
export {
  useDataDirectory,
  type DataDirectoryClient,
  type DataDirectoryConfirmOptions,
  type UseDataDirectoryOptions,
} from "./settings/use-data-directory";
export {
  useCustomTranslations,
  type CustomTranslationsClient,
  type UseCustomTranslationsOptions,
} from "./settings/use-custom-translations";
export {
  usePreferenceRecovery,
  type PreferenceRecoveryConfirmOptions,
  type UsePreferenceRecoveryOptions,
} from "./settings/use-preference-recovery";
export {
  useSettingsPersistence,
  type UseSettingsPersistenceOptions,
} from "./settings/use-settings-persistence";
export {
  useInputSourceUninstall,
  type UseInputSourceUninstallOptions,
} from "./settings/use-input-source-uninstall";
export {
  useTouchKeyboardSettingsReset,
  type TouchKeyboardSettingsResetConfirmOptions,
  type UseTouchKeyboardSettingsResetOptions,
} from "./settings/use-touch-keyboard-settings-reset";
export {
  isSafeExternalUrl,
  useExternalUrl,
  type UseExternalUrlOptions,
} from "./settings/use-external-url";
export { useUpdateCheck, type UseUpdateCheckOptions } from "./settings/use-update-check";
export { useOpenPanel, type UseOpenPanelOptions } from "./settings/use-open-panel";
export {
  useClipboardHistoryToggle,
  type UseClipboardHistoryToggleOptions,
} from "./settings/use-clipboard-history-toggle";
export {
  useTouchKeyboardSchemeSelection,
  type UseTouchKeyboardSchemeSelectionOptions,
} from "./settings/use-touch-keyboard-scheme-selection";
export {
  useSettingsDestinationActions,
  type UseSettingsDestinationActionsOptions,
} from "./settings/use-settings-destination-actions";
export { useMacosSettings, type UseMacosSettingsOptions } from "./settings/use-macos-settings";
export { useWindowState, type UseWindowStateOptions } from "./settings/use-window-state";
export { useAppVersion, type UseAppVersionOptions } from "./settings/use-app-version";
export {
  supportDiagnostics,
  type SupportDiagnosticsHost,
  type SupportDiagnosticsOptions,
} from "./settings/support-diagnostics";
export { useMountedRef } from "./settings/use-mounted-ref";
import { createProviderPresetControl } from "./settings/provider-preset-control";
export {
  tencentCredentialIssue,
  translationEndpointIssue,
} from "./settings/translation-validation";
export {
  aiCredentialOrigin,
  providerCredentialErrorMessage,
  tencentSecretConfigured,
} from "./settings/credential-utils";
import { settingsCapabilities } from "./settings/settings-capabilities";
export {
  settingsCapabilities,
  type SettingsCapabilitiesInput,
} from "./settings/settings-capabilities";
import { useAiAssistant } from "./settings/use-ai-assistant";
export { useAiAssistant, type UseAiAssistantOptions } from "./settings/use-ai-assistant";
import { aiSettingsPreferences } from "./settings/ai-settings-preferences";
export {
  aiSettingsPreferences,
  type AiSettingsPreferences,
} from "./settings/ai-settings-preferences";
export { appearanceSettingsPreferences } from "./settings/appearance-settings-preferences";
import {
  VoiceCredentialSection,
  type VoiceCredentialSaveInput,
} from "./settings/voice-credential-section";
import type { MobileKeyboardFeedbackClient } from "./settings/mobile-keyboard-feedback-section";
import { DictionaryManifestCard } from "./settings/dictionary-manifest-card";
import { PersonalDictionaryImportCard } from "./settings/personal-dictionary-import-card";
import { DictionarySettingsPanel } from "./settings/dictionary-settings-panel";
import { createDictionaryPanelActions } from "./settings/dictionary-panel-actions";
import { validCandidateFonts } from "./candidate/candidate-font-family";
import type { FontCatalogReader } from "./candidate/font-catalog";
import {
  asrProviderUpdate,
  polishProviderUpdate,
  ASR_PROVIDER_DEFAULTS,
  POLISH_PROVIDER_DEFAULTS,
} from "./voice/voice-providers";
import { PolishPromptSection } from "./settings/polish-prompt-section";
import { useVoiceInputSettings } from "./settings/use-voice-input-settings";
export {
  useVoiceInputSettings,
  type UseVoiceInputSettingsOptions,
} from "./settings/use-voice-input-settings";
import type { TouchKeyboardSkin } from "./keyboard/screen-keyboard-preview";
import * as skin from "./keyboard/touch-skin-style";
import type {
  AiSkinClient,
  CustomSkinLibraryClient,
  TouchKeyboardSkinDesign,
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
import type { SkinCatalog } from "./skin/external-skins";
import { TypingStatisticsPage, type TypingStatisticsClient } from "./settings/typing-statistics";
import { VocabularyReviewPage, type VocabularyReviewClient } from "./settings/vocabulary-review";
import type { McpClientId, McpInstallOutcome, McpServerStatus } from "./settings/mcp-connect";
import {
  HelpcodeSettingsPage,
  type HelpcodePreferences,
  type HelpcodeSchema,
} from "./settings/pages/helpcode-page";
import type { FuzzyPinyinPreferences } from "./settings/fuzzy-pinyin-section";
import {
  type NavigationPreferences,
  type WordCharacterPreferences,
} from "./settings/word-character-section";
import type { MixedInputPreferences } from "./settings/mixed-input-section";
import type { FrequencyPreferences } from "./settings/frequency-section";
import { LocalModesSection, type LocalModePreferences } from "./settings/local-modes-section";
import { type SurfaceTheme, type ThemeMode } from "./settings/theme-settings-section";
import {
  type CandidateColorKey,
  type CandidateColorPreferences,
} from "./settings/candidate-colors-section";
import {
  ClipboardHistorySection,
  type ClipboardHistoryClient,
} from "./settings/clipboard-history-section";
import { WindowTitlebar } from "./settings/window-titlebar";
export { WindowTitlebar, type WindowTitlebarProps } from "./settings/window-titlebar";
import { windowResizeEdge } from "./settings/window-resize";
export {
  windowResizeEdge,
  type WindowResizeBounds,
  type WindowResizePoint,
} from "./settings/window-resize";
export {
  useWindowResizeCapture,
  type UseWindowResizeCaptureOptions,
} from "./settings/use-window-resize-capture";
export {
  useAccountPageActions,
  type UseAccountPageActionsOptions,
} from "./settings/use-account-page-actions";
export {
  createSettingsExternalActions,
  type CreateSettingsExternalActionsOptions,
} from "./settings/settings-external-actions";
export {
  createAboutSettingsActions,
  type CreateAboutSettingsActionsOptions,
} from "./settings/about-settings-actions";
export {
  createScreenKeyboardActions,
  type CreateScreenKeyboardActionsOptions,
} from "./settings/screen-keyboard-actions";
export {
  createShortcutsSettingsActions,
  type CreateShortcutsSettingsActionsOptions,
} from "./settings/shortcuts-settings-actions";
export {
  createHelpcodeSettingsActions,
  type CreateHelpcodeSettingsActionsOptions,
} from "./settings/helpcode-settings-actions";
export {
  createUtilitiesSettingsActions,
  type CreateUtilitiesSettingsActionsOptions,
} from "./settings/utilities-settings-actions";
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
import { UtilitiesSettingsSection } from "./settings/utilities-settings-section";
export {
  UtilitiesSettingsSection,
  type UtilitiesSettingsSectionProps,
} from "./settings/utilities-settings-section";
import { ShortcutsSettingsSection } from "./settings/shortcuts-settings-section";
export {
  ShortcutsSettingsSection,
  type ShortcutsSettingsSectionProps,
} from "./settings/shortcuts-settings-section";
import { FloatingToolbarSettingsSection } from "./settings/floating-toolbar-settings-section";
import { createFloatingToolbarActions } from "./settings/floating-toolbar-actions";
export {
  FloatingToolbarSettingsSection,
  type FloatingToolbarSettingsSectionProps,
} from "./settings/floating-toolbar-settings-section";
export {
  createFloatingToolbarActions,
  type CreateFloatingToolbarActionsOptions,
} from "./settings/floating-toolbar-actions";
import { SkinSettingsSection } from "./settings/skin-settings-section";
import { createSkinSettingsActions } from "./settings/skin-settings-actions";
export {
  SkinSettingsSection,
  type SkinSettingsSectionProps,
} from "./settings/skin-settings-section";
export {
  createSkinSettingsActions,
  type CreateSkinSettingsActionsOptions,
} from "./settings/skin-settings-actions";
import { availableSettingsPages } from "./settings/available-pages";
export { availableSettingsPages, type AvailablePageCapabilities } from "./settings/available-pages";
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
import { communityDestinationView } from "./community/community-destination";
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
export { InputSettingsPanel, type InputSettingsPanelProps } from "./settings/input-settings-panel";
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
export {
  AppearanceSettingsSection,
  type AppearanceSettingsSectionProps,
} from "./settings/appearance-settings-section";
export {
  createAppearanceSettingsActions,
  type CreateAppearanceSettingsActionsOptions,
} from "./settings/appearance-settings-actions";
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
export {
  AboutSettingsSection,
  type AboutSettingsSectionProps,
} from "./settings/about-settings-section";
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
  diagnosticLogPreferences,
  type DiagnosticLogPreferences,
  type DiagnosticLogsSectionProps,
} from "./settings/diagnostic-logs-section";
export {
  HelpFeedbackSection,
  type HelpFeedbackSectionProps,
} from "./settings/help-feedback-section";
export {
  FeedbackPageSection,
  type FeedbackPageSectionProps,
} from "./settings/feedback-page-section";
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
  createProviderPresetControl,
  type ProviderPresetControlFactory,
} from "./settings/provider-preset-control";
export {
  TouchKeyboardGeometrySection,
  type TouchKeyboardGeometrySectionProps,
  type TouchToolbarPreferences,
} from "./settings/touch-keyboard-geometry-section";
export { VoiceSettingsPanel, type VoiceSettingsPanelProps } from "./settings/voice-settings-panel";
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
export {
  voiceCaptureBackendOptions,
  type VoiceCaptureBackendOptionsContext,
} from "./settings/voice-capture-backend-options";
export { fullwidthShortcutChord, maintenanceShortcutChord } from "./settings/platform-shortcuts";
export {
  platformResourceUrls,
  type PlatformResourceUrls,
  type PlatformResourceUrlsContext,
} from "./settings/platform-resource-urls";
export { settingsDirty, type SettingsDirtyOptions } from "./settings/settings-dirty";
export {
  settingsPlatformContext,
  type SettingsPlatformContext,
} from "./settings/settings-platform-context";
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
export { AiSettingsPanel, type AiSettingsPanelProps } from "./settings/ai-settings-panel";
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
  DictionarySettingsPanel,
  type DictionarySettingsPanelProps,
} from "./settings/dictionary-settings-panel";
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
  communityDestinationView,
  type CommunityDestination,
  type CommunityDestinationCategory,
  type CommunityDestinationScope,
  type CommunityDestinationView,
} from "./community/community-destination";
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
import { describeInstallerTrust } from "./settings/update-manifest";
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
  ScreenKeyboardSettingsSection,
  type ScreenKeyboardSettingsSectionProps,
} from "./settings/screen-keyboard-settings-section";
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
export {
  useSettingsPreviewThemes,
  type SettingsSkinPreviewThemes,
  type UseSettingsPreviewThemesOptions,
} from "./settings/use-settings-preview-themes";

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
  const {
    host,
    linux: linuxPlatform,
    android: androidPlatform,
    ios: iosPlatform,
    harmony: harmonyPlatform,
    mobile: mobilePlatform,
    windows: windowsPlatform,
    macos: macosPlatform,
    releasePlatform,
    diagnosticsFallbackPlatform,
    accountPlatform,
  } = settingsPlatformContext(client.host);
  const {
    nativeVoicePlatform,
    showModeScope,
    showModeSwitchShortcuts,
    showPanelShortcuts,
    showNumberRowSelection,
    showRestartInputMethod,
    showInstallInputSource,
    showFloatingToolbar,
    showToolbarAppearance,
    showToolbarComponents,
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showEnglishSuggestions,
    showHelpcodeShiftEntry,
    showShuangpinPreedit,
    showCharacterWidth,
    aiProviderCredentials,
    showVoiceCommitMode,
    showVoiceProviderSettings,
    showVoiceStreamPreedit,
    showCandidateRowColors,
    showCandidateSelectionAppearance,
    showCandidateBorderColor,
    showCandidateFollowCursor,
    showInputModeHUD,
    showVoiceCaptureDevices,
    showDesktopMaintenanceShortcuts,
    showFullwidthChord,
    clientHostedPlatform,
    desktopPanels,
  } = settingsCapabilities({
    host,
    linux: linuxPlatform,
    android: androidPlatform,
    ios: iosPlatform,
    harmony: harmonyPlatform,
    windows: windowsPlatform,
    macos: macosPlatform,
    mobile: mobilePlatform,
    canRestartInputMethod: Boolean(client.restartInputMethod),
    canInstallInputSource: Boolean(client.installInputSource),
    canListVoiceCaptureDevices: Boolean(client.listVoiceCaptureDevices),
  });
  const fullwidthChord = fullwidthShortcutChord(macosPlatform);
  const maintenanceChord = maintenanceShortcutChord(macosPlatform);
  const {
    releasesPageUrl: platformReleasesPageUrl,
    licenseUrl: platformLicenseUrl,
    issuesUrl: platformIssuesUrl,
    privacyUrl: platformPrivacyUrl,
  } = platformResourceUrls({ clientHostedPlatform, linux: linuxPlatform });
  const captureBackendOptions = voiceCaptureBackendOptions({
    linux: linuxPlatform,
    macos: macosPlatform,
    windows: windowsPlatform,
    harmony: harmonyPlatform,
  });
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
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [draft, setDraft] = useState<Preferences>();
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  /** The backup the last repair wrote, while its notice is showing. */
  const [recoveredBackup, setRecoveredBackup] = useState("");
  const {
    busy: uninstallBusy,
    cancelUninstall,
    confirmUninstall,
    confirmation: uninstallConfirmation,
    removeUserData: removeUserDataOnUninstall,
    requestUninstall,
    result: uninstallResult,
    setRemoveUserData: setRemoveUserDataOnUninstall,
  } = useInputSourceUninstall({ uninstallInputSource: client.uninstallInputSource });
  const {
    inputSourceStartup,
    onDeviceDownloadable,
    setInputSourceStartup,
    setSavedWubiAutoCommitUnique,
    setShuangpinKeymap,
    setWubiAutoCommitUnique,
    shuangpinKeymap: macosShuangpinKeymap,
    wubiAutoCommitUnique: macosWubiAutoCommitUnique,
    savedWubiAutoCommitUnique: savedMacosWubiAutoCommitUnique,
  } = useMacosSettings({ client, macos: macosPlatform, setError });
  const [page, setPage] = useState<SettingsPageId>(() =>
    initialSettingsPage({
      initialPage,
      mobilePlatform,
      mobileHistoryState: typeof window === "undefined" ? undefined : window.history.state,
      hasHomePage: Boolean(client.home),
    }),
  );
  const [accountLoginReturnPage, setAccountLoginReturnPage] = useState<SettingsPageId | null>(null);
  // Each bottom tab owns a navigation stack in the source app. This shared page has a flat route,
  // so remember the visible leaf for each tab: leaving 输入 for 社区 and returning to 键盘 must
  // restore 输入 rather than reset the first tab to 首页.
  const mobileLastPageByTab = useRef(initialMobileTabPages(page));
  // Every settings category shares this one scrolling surface. Reset it after
  // the new category is committed so sidebar clicks, in-page links and mobile
  // back navigation all open the destination at its beginning.
  const settingsContentRef = useSettingsContentScrollReset(page);
  const [communityDestination, setCommunityDestination] = useState<
    AccountCommunityDestination | "all"
  >("all");
  const currentAppVersion = useAppVersion({
    readAppVersion: client.readAppVersion,
    fallbackVersion: fallbackAppVersion,
  });
  const {
    value: mobileKeyboardFeedback,
    busy: mobileKeyboardFeedbackBusy,
    save: saveMobileKeyboardFeedback,
    preview: previewMobileKeyboardHaptics,
  } = useMobileKeyboardFeedback({
    mobile: mobilePlatform,
    client: client.mobileKeyboardFeedback,
    onError: setError,
  });
  const {
    text: customTranslationsText,
    setText: setCustomTranslationsText,
    notice: customTranslationsNotice,
    summary: customTranslationsSummary,
    busy: customTranslationsBusy,
    placeholder: customTranslationsPlaceholder,
    save: saveCustomTranslations,
  } = useCustomTranslations({ client: client.customTranslations });
  const {
    phrases,
    setPhrases,
    phrasePage,
    phraseBusy,
    phraseError,
    dictionaryPendingCount,
    dictionaryFailures,
    dictionarySnapshotError,
    phraseNotice,
    phraseSearch,
    setPhraseSearch,
    phraseForm,
    setPhraseForm,
    dictionaryKind,
    setDictionaryKind,
    dictionaryFormat,
    setDictionaryFormat,
    phraseListRef,
    loadPhrases,
    turnPhrasePage,
    removePhrase,
    savePhrase,
    importPhrases,
    retryDictionaryFailure,
    dismissDictionaryFailure,
    exportPhrases,
    exportAllPhrases,
    resetLearnedData,
  } = useDictionaryManager({ client, confirm });
  const dictionaryPanelActions = createDictionaryPanelActions({
    dictionaryKind,
    setPhraseForm,
    loadPhrases,
    exportPhrases,
    exportAllPhrases,
    importPhrases,
    retryDictionaryFailure,
    dismissDictionaryFailure,
    savePhrase,
    removePhrase,
    resetLearnedData,
  });
  const mounted = useMountedRef();
  const windowMaximized = useWindowState({ client, setError });
  const handleWindowResizeCapture = useWindowResizeCapture({
    resizeWindow: client.resizeWindow,
    windowMaximized,
    setError,
  });
  const [skinPreviewThemes, setSkinPreviewThemes] = useState<SettingsSkinPreviewThemes>({});
  const [showTouchSkinEditor, setShowTouchSkinEditor] = useState(false);
  const {
    onPointerDown: beginTouchGeometryDrag,
    onPointerMove: updateTouchGeometryDrag,
    onPointerUp: endTouchGeometryDrag,
    onPointerCancel: cancelTouchGeometryDrag,
  } = useTouchKeyboardGeometryDrag(draft, setDraft);
  const {
    providerCredentials,
    aiCredentialInput,
    setAiCredentialInput,
    tencentCredentialInput,
    setTencentCredentialInput,
    voiceCredentialInput,
    setVoiceCredentialInput,
    providerCredentialBusy,
    providerCredentialMessages,
    runProviderCredential,
    runVoiceCredential,
    credentialTestControl,
  } = useProviderCredentials({ client });
  const diagnosticsText = supportDiagnostics({
    version: currentAppVersion,
    host,
    scheme: draft ? schemeTitle(draft.scheme) : undefined,
    fallbackPlatform: diagnosticsFallbackPlatform,
    userAgent: typeof navigator === "undefined" ? undefined : navigator.userAgent,
  });
  const {
    kind: feedbackKind,
    setKind: setFeedbackKind,
    detail: feedbackDetail,
    setDetail: setFeedbackDetail,
    report: feedbackReport,
    reportCopied: feedbackReportCopied,
    feedbackCopied,
    copyReport,
    submit: submitFeedback,
    copyGroup,
  } = useFeedbackReport({
    supportDiagnostics: diagnosticsText,
    issuesUrl: platformIssuesUrl,
    copyText: client.copyText,
    openExternalUrl: client.openExternalUrl,
  });
  const {
    dataDirectory,
    busy: dataDirectoryBusy,
    result: dataDirectoryResult,
    choose: chooseDataDirectory,
  } = useDataDirectory({
    client: client.dataDirectory,
    enabled: macosPlatform || linuxPlatform,
    confirm,
  });

  const { draftRef, snapshotRef, reload, save } = useSettingsPersistence({
    client,
    mobile: mobilePlatform,
    macos: macosPlatform,
    mounted,
    snapshot,
    draft,
    setSnapshot,
    setDraft,
    setBusy,
    setError,
    setNotice,
    setRecoveredBackup,
    macosShuangpinKeymap,
    saveMacosShuangpinKeymap: client.saveMacosShuangpinKeymap,
    macosWubiAutoCommitUnique,
    saveMacosWubiAutoCommitUnique: client.saveMacosWubiAutoCommitUnique,
    setSavedMacosWubiAutoCommitUnique: setSavedWubiAutoCommitUnique,
  });

  const { restoreDefaults, recoverPreferences } = usePreferenceRecovery({
    client,
    busy,
    snapshotRef,
    draftRef,
    setSnapshot,
    setDraft,
    setBusy,
    setError,
    setNotice,
    setRecoveredBackup,
    confirm,
  });

  const resetTouchKeyboardSettings = useTouchKeyboardSettingsReset({
    draft,
    setDraft,
    setError,
    setNotice,
    confirm,
  });
  const openExternalUrl = useExternalUrl({
    openExternalUrl: client.openExternalUrl,
    setError,
  });
  const {
    available: availableUpdate,
    busy: updateBusy,
    checkForUpdate,
    status: updateStatus,
  } = useUpdateCheck({
    clientHostedPlatform,
    releasePlatform,
    releasePageUrl: platformReleasesPageUrl,
    currentAppVersion,
  });

  const openPanel = useOpenPanel({ setError });

  const dirty = settingsDirty({
    draft,
    snapshot,
    macosWubiAutoCommitUnique,
    savedMacosWubiAutoCommitUnique,
  });
  const { ai, storedAiCredential } = aiSettingsPreferences(
    draft?.ai_assistant,
    providerCredentials,
  );
  const {
    origin: aiOrigin,
    token: aiToken,
    models: aiModels,
    modelsStatus: aiModelsStatus,
    modelsBusy: aiModelsBusy,
    testInput: aiTestInput,
    setTestInput: setAiTestInput,
    testOutput: aiTestOutput,
    testStatus: aiTestStatus,
    testBusy: aiTestBusy,
    updateAi,
    updateToken: updateAiToken,
    fetchModels: fetchAiModels,
    test: testAi,
  } = useAiAssistant({
    client: client.aiAssistant,
    ai,
    providerCredentialAvailable: aiProviderCredentials,
    onChange: (patch) =>
      setDraft((current) =>
        current
          ? {
              ...current,
              ai_assistant: { ...(current.ai_assistant ?? defaultAiAssistant), ...patch },
            }
          : current,
      ),
  });
  const {
    wordCharacter,
    keybindings,
    frequency,
    mixedInput,
    fuzzyPinyin,
    localModes,
    navigation,
    numberRowSelection,
  } = settingsInputPreferences(draft);
  const {
    selected: selectedTouchKeyboardScheme,
    selectHome: selectHomeScheme,
    setEnabled: setTouchKeyboardSchemeEnabled,
  } = useTouchKeyboardSchemeSelection({ draft, setDraft });
  // Every platform sees every mode. macOS used to hide emoji, kaomoji and temporary Japanese on the
  // grounds that its bundle shipped only msime.db and english.db, but others.db and dict_japanese.dat have
  // been in resources/desktop-dictionary.lock.json since 780a9381b and tauri.macos.conf.json bundles the
  // whole verified set - so the switches were hidden for modes that worked. Temporary English, gated the
  // same way on english.db, was visible throughout, which is how inconsistent this had become.
  //
  // A host missing a catalog is still handled, and handled better than by hiding a switch: the runtime
  // turns that mode off when its resource is absent, so the trigger key inserts its capital instead of
  // being swallowed.
  const clipboardHistory = clipboardHistoryEnabled(iosPlatform, draft);
  const toggleClipboardHistory = useClipboardHistoryToggle({
    draft,
    enabled: clipboardHistory,
    clear: client.clipboard?.clear,
    setDraft,
    setError,
  });
  const diagnosticLog = diagnosticLogPreferences(draft?.diagnostic_log);
  const {
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
  } = useTranslationSettings({
    preferences: draft,
    mobile: mobilePlatform,
    macos: macosPlatform,
    linux: linuxPlatform,
    candidateEnglishGlossAvailable: Boolean(client.candidateEnglishGloss),
    onDeviceDownloadable,
    onChange: (next) => setDraft(next),
  });
  const {
    voiceInput,
    systemVoice,
    systemVoiceHostName,
    localVoiceAvailable,
    localVoice,
    serviceVoice,
    harmonyUnsupportedAsr,
    doubaoAuthMode,
    updateVoice,
  } = useVoiceInputSettings({
    preferences: draft?.voice_input,
    macos: macosPlatform,
    android: androidPlatform,
    harmony: harmonyPlatform,
    nativeVoicePlatform,
    localModelsAvailable: client.localVoiceModels !== undefined,
    onChange: (patch) =>
      setDraft((current) =>
        current
          ? {
              ...current,
              voice_input: { ...defaultVoiceInput, ...current.voice_input, ...patch },
            }
          : current,
      ),
  });
  const providerPresetControls = createProviderPresetControl(client.openExternalUrl);
  const {
    inputModeHUD,
    floatingToolbar,
    themeMode,
    settingsTheme,
    touchKeyboardSchemes,
    touchKeyboardSkin,
    customTouchKeyboardSkin,
    touchKeySpacingTenths,
    touchRowSpacingTenths,
    touchKeyboardHeightAdjustment,
  } = settingsVisualPreferences(draft);
  const floatingToolbarActions = createFloatingToolbarActions({
    draft,
    preferences: floatingToolbar,
    setDraft,
  });
  useSettingsTheme(themeMode, settingsTheme);
  const { candidatePreviewTheme, toolbarPreviewTheme, keyboardPreviewTheme } =
    useSettingsPreviewThemes({
      themeMode,
      candidateTheme: draft?.candidate_theme,
      toolbarTheme: draft?.toolbar_theme,
      screenKeyboardTheme: draft?.screen_keyboard_theme,
      setSkinPreviewThemes,
    });
  const skinSettingsActions = createSkinSettingsActions({
    draft,
    candidatePreviewTheme,
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
    setDraft,
    setSkinPreviewThemes,
  });
  const appearanceSettingsActions = createAppearanceSettingsActions({
    draft,
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
    setDraft,
  });
  const installerTrust = availableUpdate
    ? describeInstallerTrust(availableUpdate, releasePlatform)
    : null;
  const availablePages = availableSettingsPages({
    home: Boolean(client.home),
    typingStatistics: Boolean(client.typingStatistics),
    vocabularyReview: Boolean(client.vocabularyReview),
    account: Boolean(client.account || client.appIcon),
    chat: Boolean(client.chat),
    community: Boolean(client.communitySkins || client.communityResources),
    floatingToolbar: showFloatingToolbar,
    mobile: mobilePlatform,
  });
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
  const sidebarGroups = settingsSidebarGroups(availablePages, {
    mobile: mobilePlatform,
    hiddenPageIds: mobileHiddenPageIds,
    macos: macosPlatform,
  });
  // Walked in tab order rather than filtered out of `availablePages`, which is in the order the
  // pages happen to be declared in — that put 我的 second, and the bar read 键盘 / 我的 / 社区 / 统计
  // against the source's 键盘 / 社区 / 统计 / 我的.
  // A page without a tab of its own was reached from inside the 键盘 tab, so that is the tab still
  // standing on. Keyed off the page alone, the bar went blank the moment anyone opened one — nothing
  // lit, and no way to read where in the app you were.
  const { primary: mobilePrimaryPages, secondary: mobileSecondaryPages } = splitMobilePages(
    availablePages,
    mobileHiddenPageIds,
  );
  const { mobileActiveTab, selectPage, selectMobileTab, openAccountLogin, finishAccountLogin } =
    useSettingsNavigation({
      mobilePlatform,
      mobileHiddenPageIds,
      availablePages,
      page,
      setPage,
      mobileLastPageByTab,
      route,
      hasHomePage: Boolean(client.home),
      setCommunityDestination,
      setAccountLoginReturnPage,
      accountLoginReturnPage,
    });
  const { openCommunity, openLocalDesigns } = useSettingsDestinationActions({
    selectPage,
    setShowTouchSkinEditor,
    setCommunityDestination,
  });
  const accountPageActions = useAccountPageActions({
    hasAccountLoginReturnPage: Boolean(accountLoginReturnPage),
    finishAccountLogin,
    openLocalDesigns: client.customTouchKeyboardSkins ? openLocalDesigns : undefined,
    openCommunity: client.communitySkins && client.communityResources ? openCommunity : undefined,
    openCloudDictionary: client.openCloudDictionary,
    openCloudClipboard: client.openCloudClipboard,
    mobile: mobilePlatform,
    canOpenExternalUrl: Boolean(client.openExternalUrl),
    openExternalUrl,
    onReplayOnboarding,
    openAbout: () => selectPage("about"),
    setError,
  });
  const settingsExternalActions = createSettingsExternalActions({
    mobile: mobilePlatform,
    canOpenExternalUrl: Boolean(client.openExternalUrl),
    openExternalUrl,
    issuesUrl: platformIssuesUrl,
    openSystemKeyboardSettings: client.openSystemKeyboardSettings,
  });
  const aboutSettingsActions = createAboutSettingsActions({
    draft,
    diagnosticLog,
    checkForUpdate,
    chooseDataDirectory,
    confirmUninstall,
    selectPage,
    setDraft,
  });
  const screenKeyboardActions = createScreenKeyboardActions({
    draft,
    openCommunity: () => openCommunity("all"),
    setShowTouchSkinEditor,
    saveMobileKeyboardFeedback,
    mobileKeyboardFeedback,
    resetTouchKeyboardSettings,
    openScreenKeyboard: client.openScreenKeyboard,
    openPanel,
    setDraft,
  });
  const shortcutsSettingsActions = createShortcutsSettingsActions({
    draft,
    keybindings,
    setDraft,
  });
  const helpcodeSettingsActions = createHelpcodeSettingsActions({ draft, setDraft });
  const utilitiesSettingsActions = createUtilitiesSettingsActions({ draft, setDraft });
  const communityView = communityDestinationView(communityDestination);
  return (
    <div
      className={settings.shell}
      data-settings-shell=""
      // The phone hosts read as one product with the Apple app, which is where the palette below
      // comes from. The inherited one is the Windows settings accent.
      data-mobile={mobilePlatform ? "" : undefined}
      onPointerDownCapture={handleWindowResizeCapture}
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
              hiddenOnPhone={mobilePlatform && mobileHeaderlessPageIds.includes(page)}
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
                platform={accountPlatform}
                mobile={mobilePlatform}
                {...accountPageActions}
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
                initialMine={communityView.initialMine}
                initialCategory={communityView.category}
                initialScope={communityView.scope}
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
                initialMine={communityView.initialMine}
                mobile={mobilePlatform}
                onLogin={openAccountLogin}
              />
            )}
            {!client.communitySkins && client.communityResources && page === "community" && (
              <CommunityResourcesPage
                client={client.communityResources}
                kind={communityView.category === "reply" ? "reply" : "dictionary"}
                initialScope={communityView.scope}
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
                  <AppearanceSettingsSection
                    disabled={busy}
                    hidden={page !== "appearance"}
                    preferences={appearanceSettingsPreferences(draft, windowsPlatform)}
                    revision={snapshot?.revision ?? 0}
                    mobile={mobilePlatform}
                    linux={linuxPlatform}
                    windows={host?.platform === "windows"}
                    candidatePreviewTheme={candidatePreviewTheme}
                    candidatePanelLimit={host?.candidate_panel_limit}
                    showCandidateFollowCursor={showCandidateFollowCursor}
                    showCandidateFontControls={showCandidateFontControls}
                    showCandidateEnglishFont={showCandidateEnglishFont}
                    showCandidatePreeditFont={showCandidatePreeditFont}
                    showCandidateRowColors={showCandidateRowColors}
                    showCandidateSelectionAppearance={showCandidateSelectionAppearance}
                    showCandidateBorderColor={showCandidateBorderColor}
                    showShuangpinPreedit={showShuangpinPreedit}
                    fixedCandidatePageSize={host?.fixed_candidate_page_size !== undefined}
                    fixedCandidateLayout={host?.fixed_candidate_layout !== undefined}
                    floatingToolbar={showFloatingToolbar}
                    desktopPanels={desktopPanels}
                    candidatePaletteFollowsDesktop={
                      mobileKeyboardFeedback?.candidatePaletteFollowsDesktop
                    }
                    inlinePreedit={mobileKeyboardFeedback?.inlinePreedit}
                    inlinePreeditBusy={mobileKeyboardFeedbackBusy}
                    scan={client.scanSkinCatalog}
                    readImage={client.readSkinImage}
                    resolveFonts={client.resolveFontFamilies}
                    listFontFamilies={client.listFontFamilies}
                    {...appearanceSettingsActions}
                  />
                  <DictionarySettingsPanel
                    disabled={busy}
                    hidden={page !== "dictionary"}
                    dictionary={client.dictionary}
                    dictionaryManifest={client.dictionaryManifest}
                    platform={client.host?.platform}
                    macos={macosPlatform}
                    resetLearnedData={client.resetLearnedData}
                    phraseBusy={phraseBusy}
                    dictionaryFormat={dictionaryFormat}
                    setDictionaryFormat={setDictionaryFormat}
                    {...dictionaryPanelActions}
                    dictionaryPendingCount={dictionaryPendingCount}
                    dictionaryFailures={dictionaryFailures}
                    dictionarySnapshotError={dictionarySnapshotError}
                    canRetry={Boolean(client.dictionary?.retry)}
                    canDismiss={Boolean(client.dictionary?.dismissFailure)}
                    dictionaryKind={dictionaryKind}
                    phraseSearch={phraseSearch}
                    setDictionaryKind={setDictionaryKind}
                    setPhraseSearch={setPhraseSearch}
                    phraseError={phraseError}
                    phraseNotice={phraseNotice}
                    phrases={phrases}
                    setPhrases={setPhrases}
                    phraseForm={phraseForm}
                    phraseListRef={phraseListRef}
                    setPhraseForm={setPhraseForm}
                    onTurnPage={turnPhrasePage}
                    phrasePage={phrasePage}
                  />
                  <SkinSettingsSection
                    disabled={busy}
                    hidden={page !== "skin"}
                    mobile={mobilePlatform}
                    linux={linuxPlatform}
                    candidatePanelLimit={host?.candidate_panel_limit}
                    mobileKeyboardFeedback={mobileKeyboardFeedback}
                    mobileKeyboardFeedbackBusy={mobileKeyboardFeedbackBusy}
                    {...skinSettingsActions}
                    candidateSkinCatalog={snapshot?.candidate_skin_catalog}
                    selected={draft.candidate_skin ?? "willow_green"}
                    previewThemes={skinPreviewThemes}
                    defaultTheme={candidatePreviewTheme}
                    activeTheme={candidatePreviewTheme}
                    scan={client.scanSkinCatalog}
                    openDirectory={client.openSkinDirectory}
                    importsSkin={host?.skin_directory_import === true}
                    readImage={client.readSkinImage}
                    readFont={client.readSkinFont}
                    readToolbarCss={client.readSkinToolbarCss}
                    // A host that draws one layout judges a skin by that layout, not by a setting it ignores.
                    layout={host?.fixed_candidate_layout ?? draft.candidate_layout ?? "vertical"}
                    toolbarPreview={!linuxPlatform}
                  />
                  <FloatingToolbarSettingsSection
                    disabled={busy}
                    hidden={page !== "floating-toolbar"}
                    preferences={floatingToolbar}
                    skin={draft.candidate_skin ?? "willow_green"}
                    theme={toolbarPreviewTheme}
                    {...floatingToolbarActions}
                    showAppearance={showToolbarAppearance}
                    showComponents={showToolbarComponents}
                    capabilities={
                      host
                        ? {
                            floating_toolbar_handwriting: host.floating_toolbar_handwriting,
                            floating_toolbar_voice: host.floating_toolbar_voice,
                          }
                        : undefined
                    }
                  />
                  <InputSettingsPanel
                    disabled={busy}
                    hidden={page !== "input"}
                    client={client}
                    draft={draft}
                    setDraft={setDraft}
                    confirm={confirm}
                    onOpenAi={() => selectPage("ai")}
                    openExternalUrl={openExternalUrl}
                    onError={setError}
                    iosPlatform={iosPlatform}
                    harmonyPlatform={harmonyPlatform}
                    androidPlatform={androidPlatform}
                    mobilePlatform={mobilePlatform}
                    macosPlatform={macosPlatform}
                    linuxPlatform={linuxPlatform}
                    windowsPlatform={windowsPlatform}
                    touchKeyboardSchemes={touchKeyboardSchemes}
                    selectedTouchKeyboardScheme={selectedTouchKeyboardScheme}
                    setTouchKeyboardSchemeEnabled={setTouchKeyboardSchemeEnabled}
                    macosShuangpinKeymap={macosShuangpinKeymap}
                    setShuangpinKeymap={setShuangpinKeymap}
                    macosWubiAutoCommitUnique={macosWubiAutoCommitUnique}
                    setWubiAutoCommitUnique={setWubiAutoCommitUnique}
                    wordCharacter={wordCharacter}
                    candidateTranslations={candidateTranslations}
                    candidateGlossLanguagesEnabled={candidateGlossLanguagesEnabled}
                    translationTargetLanguage={translationTargetLanguage}
                    translationSecondaryLanguage={translationSecondaryLanguage}
                    visibleTranslationLanguages={visibleTranslationLanguages}
                    visibleSecondaryLanguages={visibleSecondaryLanguages}
                    customTranslation={customTranslation}
                    tencentTranslation={tencentTranslation}
                    niutrans={niutrans}
                    translationProvider={translationProvider}
                    onDeviceMissingLanguages={onDeviceMissingLanguages}
                    setTranslationProvider={setTranslationProvider}
                    providerCredentials={providerCredentials}
                    tencentCredentialInput={tencentCredentialInput}
                    setTencentCredentialInput={setTencentCredentialInput}
                    providerCredentialBusy={providerCredentialBusy}
                    providerCredentialMessages={providerCredentialMessages}
                    runProviderCredential={runProviderCredential}
                    credentialTestControl={credentialTestControl}
                    customTranslationsText={customTranslationsText}
                    setCustomTranslationsText={setCustomTranslationsText}
                    customTranslationsNotice={customTranslationsNotice}
                    customTranslationsSummary={customTranslationsSummary}
                    customTranslationsBusy={customTranslationsBusy}
                    customTranslationsPlaceholder={customTranslationsPlaceholder}
                    saveCustomTranslations={saveCustomTranslations}
                    fuzzyPinyin={fuzzyPinyin}
                    mixedInput={mixedInput}
                    frequency={frequency}
                    showCharacterWidth={showCharacterWidth}
                    showInputModeHUD={showInputModeHUD}
                    showEnglishSuggestions={showEnglishSuggestions}
                    showModeScope={showModeScope}
                    mobileKeyboardFeedback={mobileKeyboardFeedback}
                    mobileKeyboardFeedbackBusy={mobileKeyboardFeedbackBusy}
                    saveMobileKeyboardFeedback={saveMobileKeyboardFeedback}
                    previewMobileKeyboardHaptics={previewMobileKeyboardHaptics}
                  />
                  <HelpcodeSettingsPage
                    value={draft}
                    mobile={mobilePlatform}
                    showShiftEntry={showHelpcodeShiftEntry}
                    disabled={busy}
                    hidden={page !== "helpcode"}
                    {...helpcodeSettingsActions}
                  />
                  <ShortcutsSettingsSection
                    disabled={busy}
                    hidden={page !== "shortcuts"}
                    mobile={mobilePlatform}
                    keybindings={keybindings}
                    {...shortcutsSettingsActions}
                    showModeSwitchShortcuts={showModeSwitchShortcuts}
                    macos={macosPlatform}
                    showInputModeHUD={showInputModeHUD}
                    inputModeHUD={inputModeHUD}
                    showFullwidthChord={showFullwidthChord}
                    fullwidthChord={fullwidthChord}
                    windows={windowsPlatform}
                    navigation={navigation}
                    numberRowSelection={numberRowSelection}
                    showNumberRowSelection={showNumberRowSelection}
                    showPanelShortcuts={showPanelShortcuts}
                    harmony={harmonyPlatform}
                    showDesktopMaintenanceShortcuts={showDesktopMaintenanceShortcuts}
                    linux={linuxPlatform}
                    maintenanceChord={maintenanceChord}
                    showRestartInputMethod={Boolean(showRestartInputMethod)}
                    restartInputMethod={client.restartInputMethod}
                    installInputSource={
                      showInstallInputSource ? client.installInputSource : undefined
                    }
                  />
                  <UtilitiesSettingsSection
                    disabled={busy}
                    hidden={page !== "tools"}
                    clipboard={client.clipboard}
                    historyEnabled={clipboardHistory}
                    persistedHistoryEnabled={snapshot?.preferences.clipboard_history ?? false}
                    revision={snapshot?.revision}
                    page={page}
                    ios={iosPlatform}
                    macos={macosPlatform}
                    onToggleClipboard={toggleClipboardHistory}
                    onError={setError}
                    openCloudClipboard={client.openCloudClipboard}
                    openCloudDictionary={client.openCloudDictionary}
                    onOpenPanel={openPanel}
                    localModes={localModes}
                    {...utilitiesSettingsActions}
                  />
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
                    onOpenDocumentation={settingsExternalActions.onOpenDocumentation}
                    onOpenSystemKeyboardSettings={
                      settingsExternalActions.onOpenSystemKeyboardSettings
                    }
                  />
                  <AboutSettingsSection
                    disabled={busy}
                    hidden={page !== "about"}
                    logo={logo}
                    description={platformAboutDescription}
                    currentAppVersion={currentAppVersion}
                    updateStatus={updateStatus}
                    updateBusy={updateBusy}
                    availableUpdate={availableUpdate}
                    installerTrust={installerTrust}
                    licenseUrl={platformLicenseUrl}
                    privacyUrl={platformPrivacyUrl}
                    macos={macosPlatform}
                    linux={linuxPlatform}
                    windows={windowsPlatform || !client.host}
                    mobile={mobilePlatform}
                    dataDirectoryVisible={Boolean(
                      (macosPlatform || linuxPlatform) && client.dataDirectory,
                    )}
                    dataDirectory={dataDirectory}
                    dataDirectoryBusy={dataDirectoryBusy}
                    dataDirectoryResult={dataDirectoryResult}
                    {...aboutSettingsActions}
                    onOpenExternalUrl={openExternalUrl}
                    openThirdPartyLicenses={client.openThirdPartyLicenses}
                    uninstallInputSource={client.uninstallInputSource}
                    removeUserData={removeUserDataOnUninstall}
                    uninstallBusy={uninstallBusy}
                    uninstallConfirmation={uninstallConfirmation}
                    uninstallResult={uninstallResult}
                    onRemoveUserDataChange={setRemoveUserDataOnUninstall}
                    onRequestUninstall={requestUninstall}
                    onCancelUninstall={cancelUninstall}
                    diagnosticVisible={
                      !client.host || linuxPlatform || windowsPlatform || macosPlatform
                    }
                    diagnosticLog={diagnosticLog}
                    openDiagnosticLogDirectory={client.openDiagnosticLogDirectory}
                    onDiagnosticLogError={setError}
                    telemetryEnabled={draft.telemetry_enabled}
                  />
                  <ScreenKeyboardSettingsSection
                    disabled={busy}
                    hidden={page !== "screen-keyboard"}
                    mobile={mobilePlatform}
                    screenKeyboardTheme={draft.screen_keyboard_theme ?? "follow"}
                    previewTheme={keyboardPreviewTheme}
                    {...screenKeyboardActions}
                    selectedSkin={touchKeyboardSkin}
                    customDesign={customTouchKeyboardSkin}
                    customAvailable={Boolean(client.customTouchKeyboardSkins)}
                    editorOpen={showTouchSkinEditor}
                    communityAvailable={Boolean(mobilePlatform && client.communitySkins)}
                    library={client.customSkinLibrary}
                    aiSkins={client.aiSkins}
                    communitySkins={client.communitySkins}
                    heightAdjustment={touchKeyboardHeightAdjustment}
                    keySpacingTenths={touchKeySpacingTenths}
                    rowSpacingTenths={touchRowSpacingTenths}
                    touchVoiceShortcut={draft.touch_voice_shortcut ?? false}
                    toolbarComponents={Boolean(host?.touch_toolbar_components)}
                    toolbar={draft.touch_toolbar}
                    tabletFullKeys={mobileKeyboardFeedback?.tabletFullKeys}
                    tabletFullKeysBusy={mobileKeyboardFeedbackBusy}
                    onPointerDown={beginTouchGeometryDrag}
                    onPointerMove={updateTouchGeometryDrag}
                    onPointerUp={endTouchGeometryDrag}
                    onPointerCancel={cancelTouchGeometryDrag}
                  />
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
                  <VoiceSettingsPanel
                    disabled={busy}
                    hidden={page !== "voice"}
                    client={client}
                    draft={draft}
                    setDraft={setDraft}
                    confirm={confirm}
                    openExternalUrl={openExternalUrl}
                    openPanel={openPanel}
                    voiceInput={voiceInput}
                    systemVoice={systemVoice}
                    systemVoiceHostName={systemVoiceHostName}
                    localVoiceAvailable={localVoiceAvailable}
                    localVoice={localVoice}
                    serviceVoice={serviceVoice}
                    harmonyUnsupportedAsr={harmonyUnsupportedAsr}
                    doubaoAuthMode={doubaoAuthMode}
                    updateVoice={updateVoice}
                    providerCredentials={providerCredentials}
                    voiceCredentialInput={voiceCredentialInput}
                    setVoiceCredentialInput={setVoiceCredentialInput}
                    providerCredentialBusy={providerCredentialBusy}
                    providerCredentialMessages={providerCredentialMessages}
                    runVoiceCredential={runVoiceCredential}
                    credentialTestControl={credentialTestControl}
                    providerPresetControls={providerPresetControls}
                    androidPlatform={androidPlatform}
                    iosPlatform={iosPlatform}
                    macosPlatform={macosPlatform}
                    harmonyPlatform={harmonyPlatform}
                    linuxPlatform={linuxPlatform}
                    windowsPlatform={windowsPlatform}
                    mobilePlatform={mobilePlatform}
                    nativeVoicePlatform={nativeVoicePlatform}
                    desktopPanels={desktopPanels}
                    showVoiceProviderSettings={showVoiceProviderSettings}
                    showVoiceStreamPreedit={showVoiceStreamPreedit}
                    showVoiceCommitMode={showVoiceCommitMode}
                    showVoiceCaptureDevices={showVoiceCaptureDevices}
                    captureBackendOptions={captureBackendOptions}
                  />
                  <AiSettingsPanel
                    disabled={busy}
                    hidden={page !== "ai"}
                    client={client}
                    ai={ai}
                    updateAi={updateAi}
                    aiOrigin={aiOrigin}
                    aiToken={aiToken}
                    updateAiToken={updateAiToken}
                    aiModels={aiModels}
                    aiModelsStatus={aiModelsStatus}
                    aiModelsBusy={aiModelsBusy}
                    fetchAiModels={fetchAiModels}
                    aiTestInput={aiTestInput}
                    setAiTestInput={setAiTestInput}
                    aiTestOutput={aiTestOutput}
                    aiTestStatus={aiTestStatus}
                    aiTestBusy={aiTestBusy}
                    testAi={testAi}
                    providerPresetControls={providerPresetControls}
                    linuxPlatform={linuxPlatform}
                    windowsPlatform={windowsPlatform}
                    macosPlatform={macosPlatform}
                    iosPlatform={iosPlatform}
                    androidPlatform={androidPlatform}
                    providerCredentials={providerCredentials}
                    storedAiCredential={storedAiCredential}
                    aiCredentialInput={aiCredentialInput}
                    setAiCredentialInput={setAiCredentialInput}
                    providerCredentialBusy={providerCredentialBusy}
                    providerCredentialMessages={providerCredentialMessages}
                    runProviderCredential={runProviderCredential}
                    credentialTestControl={credentialTestControl}
                  />
                  <FeedbackPageSection
                    disabled={busy}
                    hidden={page !== "feedback"}
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
                    supportDiagnostics={diagnosticsText}
                    issuesUrl={platformIssuesUrl}
                    copyText={client.copyText}
                    openExternalUrl={client.openExternalUrl}
                    onKindChange={setFeedbackKind}
                    onDetailChange={setFeedbackDetail}
                    onCopyReport={copyReport}
                    onSubmitFeedback={submitFeedback}
                    onOpenIssues={settingsExternalActions.onOpenIssues}
                    onCopyGroup={copyGroup}
                    onOpenTelegram={settingsExternalActions.onOpenTelegram}
                  />
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
