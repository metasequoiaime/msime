import type { SettingsClient } from "./core/settings-client";
import type { Preferences, Snapshot } from "./core/preferences-model";

import { useConfirm } from "./core/confirm";
import { NavItem } from "./core/platform-controls";
import { ToastProvider } from "./core/toast";
import { appThemeStyle, seasonAttr } from "./core/app-theme-style";
import { SetupWarningStrip } from "./keyboard/setup-status-card";
import { KeyboardSwitchGuideHost } from "./keyboard/keyboard-switch-guide";
import { TryKeyboardPage } from "./keyboard/try-keyboard-page";
import { mobilePageTitle } from "./settings/mobile-tab-helpers";
import { desktopDownloadUrl, fallbackAppVersion } from "./settings/app-resources";
import { useSettingsWindowInteractions } from "./settings/use-settings-window-interactions";
import { useSettingsNavigation } from "./settings/use-settings-navigation";
import { useSettingsContentScrollReset } from "./settings/use-settings-content-scroll-reset";
import { MobileSettingsTabs } from "./settings/mobile-settings-tabs";
import { SettingsPageHeader } from "./settings/settings-page-header";
import {
  mobilePrimaryPageIds,
  mobileTabForPage,
  requestedPage,
  type MobilePrimaryPageId,
} from "./settings/settings-navigation-helpers";
import {
  desktopPage,
  desktopPageTitle,
  pages,
  subPageParents,
  type SettingsPageId,
} from "./settings/settings-page-registry";
import { settingsPageProjections } from "./settings/settings-page-projections";
import {
  canReloadSettingsPage,
  canRestoreDefaultsOnPage,
  isSettingsFormPage,
} from "./settings/settings-page-visibility";
import { settingsInputPreferences } from "./settings/settings-input-preferences";
import { aiSettingsPreferences } from "./settings/ai-settings-preferences";
import { diagnosticLogPreferences } from "./settings/diagnostic-log-preferences";
import { clipboardHistoryEnabled } from "./settings/clipboard-history-preferences";
import {
  hostDrawnCustomTheme,
  settingsThemePreferences,
} from "./settings/settings-theme-preferences";

import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
export type {
  DictionaryEntry,
  DictionaryPageFormat,
  LocalDictionaryFormat,
  LocalDictionaryKind,
  ScelText,
} from "./dictionary/dictionary-file";
export {
  MAX_DICTIONARY_EXPORT_BYTES,
  dictionaryExportName,
  dictionaryExportPayload,
  loadAllPersonalDictionaryEntries,
  personalDictionaryExportName,
  personalDictionaryExportPayload,
} from "./dictionary/dictionary-export";
export {
  DictionaryFormatOptions,
  type DictionaryFormatOptionsProps,
} from "./dictionary/dictionary-format-options";
export { dictionaryErrorMessage } from "./dictionary/dictionary-errors";

export {
  type TouchKeyboardScheme,
  type TouchKeyboardSchemePreferences,
  inferredTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  allTouchKeyboardSchemes,
  defaultTouchKeyboardSchemes,
} from "./settings/touch-keyboard-scheme-helpers";
import { settingsPlatformPresentation } from "./settings/settings-platform-presentation";
import { settingsPageEnvironment } from "./settings/settings-page-environment";
export {
  platformCopy,
  type PlatformCopyContext,
  type PlatformCopy,
} from "./settings/platform-copy";
import { schemeTitle } from "./settings/label-helpers";
export { schemeTitle } from "./settings/label-helpers";
import {
  useHarmonySystemBars,
  useResolvedAppTheme,
  useSettingsTheme,
} from "./settings/use-settings-theme";
export { useSettingsTheme } from "./settings/use-settings-theme";
export { updateCandidateColor, updateCustomKeyboard } from "./settings/theme-selection-updates";
import { useTouchKeyboardGeometryDrag } from "./settings/use-touch-keyboard-geometry-drag";
import { touchKeyboardGeometryPreferences } from "./settings/touch-keyboard-geometry-preferences";
export { defaultTouchKeyboardGeometry } from "./settings/touch-keyboard-geometry-defaults";
export { useTouchKeyboardGeometryDrag } from "./settings/use-touch-keyboard-geometry-drag";
import { useMobileKeyboardFeedback } from "./settings/use-mobile-keyboard-feedback";
export {
  useMobileKeyboardFeedback,
  type UseMobileKeyboardFeedbackOptions,
} from "./settings/use-mobile-keyboard-feedback";
import { useTranslationSettings } from "./settings/use-translation-settings";
export {
  useTranslationSettings,
  type UseTranslationSettingsOptions,
} from "./settings/use-translation-settings";
import { useDictionaryManager } from "./settings/use-dictionary-manager";
export {
  useDictionaryManager,
  type DictionaryManagerClient,
  type DictionaryConfirmOptions,
  type UseDictionaryManagerOptions,
} from "./settings/use-dictionary-manager";
import { SettingsFormFooter } from "./settings/settings-form-footer";
import { SettingsPageStatus } from "./settings/settings-page-status";
import { SettingsFormFrame } from "./settings/settings-form-frame";
import { NoticeBanner } from "./settings/notice-banner";
import { WindowTitlebar } from "./settings/window-titlebar";
import { useProviderCredentials } from "./settings/use-provider-credentials";
import { useFeedbackReport } from "./settings/use-feedback-report";
import { useDataDirectory } from "./settings/use-data-directory";
import { usePreferenceRecovery } from "./settings/use-preference-recovery";
import { useSettingsPersistence } from "./settings/use-settings-persistence";
import { useInputSourceUninstall } from "./settings/use-input-source-uninstall";
import { useTouchKeyboardSettingsReset } from "./settings/use-touch-keyboard-settings-reset";
import { useExternalUrl } from "./settings/use-external-url";
import { useUpdateCheck } from "./settings/use-update-check";
import { useOpenPanel } from "./settings/use-open-panel";
import { useClipboardHistoryToggle } from "./settings/use-clipboard-history-toggle";
import { useTouchKeyboardSchemeSelection } from "./settings/use-touch-keyboard-scheme-selection";
import { useSettingsDestinationActions } from "./settings/use-settings-destination-actions";
import {
  communityGalleryPage,
  communityTabCategories,
  useCommunityGallerySubview,
  type CommunityGalleryView,
} from "./settings/use-community-gallery-subview";
import { CommunityGallerySubview } from "./settings/community-gallery-subview";
import { useMacosSettings } from "./settings/use-macos-settings";
import { useWindowState } from "./settings/use-window-state";
import { useAppVersion } from "./settings/use-app-version";
import { supportDiagnostics } from "./settings/support-diagnostics";
import { useMountedRef } from "./settings/use-mounted-ref";
import { useCustomHelpcodeSchemas, useHelpcodePacks } from "./settings/use-helpcode-catalog";
export {
  useProviderCredentials,
  type ProviderCredentialBusy,
  type ProviderCredentialInput,
  type ProviderCredentialMessage,
  type ProviderCredentialsHost,
  type TencentCredentialInput,
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
  touchKeyboardResetSubjects,
  useTouchKeyboardSettingsReset,
  type TouchKeyboardSettingsResetConfirmOptions,
  type UseTouchKeyboardSettingsResetOptions,
} from "./settings/use-touch-keyboard-settings-reset";
export { useExternalUrl, type UseExternalUrlOptions } from "./settings/use-external-url";
export {
  useSettingsNavigation,
  type SettingsNavigationOptions,
} from "./settings/use-settings-navigation";
export { useSettingsContentScrollReset } from "./settings/use-settings-content-scroll-reset";
export { useUpdateCheck, type UseUpdateCheckOptions } from "./settings/use-update-check";
export type {
  HostReleaseUpdate,
  UpdateCheckRequest,
  UpdateCheckResult,
} from "./settings/update-manifest";
export {
  aiSettingsPreferences,
  type AiSettingsPreferences,
} from "./settings/ai-settings-preferences";
export { appearanceSettingsPreferences } from "./settings/appearance-settings-preferences";
export {
  clipboardHistoryEnabled,
  type ClipboardHistoryPreferencesSource,
} from "./settings/clipboard-history-preferences";
export {
  diagnosticLogPreferences,
  type DiagnosticLogPreferencesSource,
} from "./settings/diagnostic-log-preferences";
export { settingsThemePreferences } from "./settings/settings-theme-preferences";
export {
  settingsInputPreferences,
  type SettingsInputPreferences,
  type SettingsInputPreferencesSource,
} from "./settings/settings-input-preferences";
export {
  settingsPlatformContext,
  type SettingsPlatformContext,
} from "./settings/settings-platform-context";
export {
  platformResourceUrls,
  type PlatformResourceUrls,
  type PlatformResourceUrlsContext,
} from "./settings/platform-resource-urls";
export {
  canReloadSettingsPage,
  canRestoreDefaultsOnPage,
  isSettingsFormPage,
} from "./settings/settings-page-visibility";
export type { SettingsPageId } from "./settings/settings-page-registry";
export {
  useSettingsDictionaryState,
  type UseSettingsDictionaryStateOptions,
} from "./settings/use-settings-dictionary-state";
export { SettingsFormFrame, type SettingsFormFrameProps } from "./settings/settings-form-frame";
export {
  SettingsPageFieldset,
  type SettingsPageFieldsetProps,
} from "./settings/settings-page-fieldset";
export {
  NoticeBanner,
  noticeBodyHtml,
  type AppNotice,
  type NoticesClient,
} from "./settings/notice-banner";
export { settingsPageEnvironment } from "./settings/settings-page-environment";
export {
  settingsPlatformPresentation,
  type SettingsPlatformPresentationOptions,
} from "./settings/settings-platform-presentation";
export {
  settingsPageLinks,
  settingsPageTitle,
  type SettingsPageLinkItem,
  type SettingsPageTitleItem,
} from "./settings/settings-page-view-model";
export { SettingsPageStatus, type SettingsPageStatusProps } from "./settings/settings-page-status";
export { SettingsFormFooter, type SettingsFormFooterProps } from "./settings/settings-form-footer";
export { FeedbackChannels, type FeedbackChannelsProps } from "./settings/feedback-channels";
export {
  createAboutSettingsActions,
  type CreateAboutSettingsActionsOptions,
} from "./settings/about-settings-actions";
export {
  createAppearanceSettingsActions,
  type CreateAppearanceSettingsActionsOptions,
} from "./settings/appearance-settings-actions";
export {
  createFloatingToolbarSettingsActions,
  type CreateFloatingToolbarSettingsActionsOptions,
} from "./settings/floating-toolbar-settings-actions";
export {
  createDictionaryPanelActions,
  type CreateDictionaryPanelActionsOptions,
} from "./settings/dictionary-panel-actions";
export {
  createHelpcodeSettingsActions,
  type CreateHelpcodeSettingsActionsOptions,
} from "./settings/helpcode-settings-actions";
export {
  createSettingsDraftActions,
  type CreateSettingsDraftActionsOptions,
} from "./settings/settings-draft-actions";
export {
  createSettingsExternalActions,
  type CreateSettingsExternalActionsOptions,
} from "./settings/settings-external-actions";
export {
  createSettingsNavigationActions,
  type CreateSettingsNavigationActionsOptions,
} from "./settings/settings-navigation-actions";
export {
  createSettingsPageSelection,
  type CreateSettingsPageSelectionOptions,
} from "./settings/settings-page-selection";
export {
  createSettingsReloadAction,
  type CreateSettingsReloadActionOptions,
} from "./settings/settings-reload-action";
export {
  createSettingsSaveAction,
  type CreateSettingsSaveActionOptions,
} from "./settings/settings-save-action";
export {
  createSettingsStatusActions,
  type CreateSettingsStatusActionsOptions,
} from "./settings/settings-status-actions";
export {
  createShortcutsSettingsActions,
  type CreateShortcutsSettingsActionsOptions,
} from "./settings/shortcuts-settings-actions";
export {
  createUtilitiesSettingsActions,
  type CreateUtilitiesSettingsActionsOptions,
} from "./settings/utilities-settings-actions";
export {
  useAccountPageActions,
  type UseAccountPageActionsOptions,
} from "./settings/use-account-page-actions";
export {
  useWindowResizeCapture,
  type UseWindowResizeCaptureOptions,
} from "./settings/use-window-resize-capture";
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
export {
  communityGalleryPage,
  useCommunityGallerySubview,
  type CommunityGallerySubviewOptions,
  type CommunityGalleryView,
} from "./settings/use-community-gallery-subview";
export { useMacosSettings, type UseMacosSettingsOptions } from "./settings/use-macos-settings";
export {
  MacosInputModeEntriesSection,
  macosInputModeEntries,
  macosInputModeEntriesFor,
  type MacosInputModesClient,
} from "./settings/macos-input-mode-entries-section";
export { useWindowState, type UseWindowStateOptions } from "./settings/use-window-state";
export {
  useSettingsWindowInteractions,
  type UseSettingsWindowInteractionsOptions,
} from "./settings/use-settings-window-interactions";
export { useAppVersion, type UseAppVersionOptions } from "./settings/use-app-version";
export { useCustomHelpcodeSchemas, useHelpcodePacks } from "./settings/use-helpcode-catalog";
export {
  supportDiagnostics,
  type SupportDiagnosticsHost,
  type SupportDiagnosticsOptions,
} from "./settings/support-diagnostics";
export { useMountedRef } from "./settings/use-mounted-ref";
import { createProviderPresetControl } from "./settings/provider-preset-control";
export {
  windowResizeEdge,
  type WindowResizeBounds,
  type WindowResizePoint,
} from "./settings/window-resize";
import { useCandidatePreviewTheme } from "./candidate/candidate-preview-theme";
export {
  settingsCapabilities,
  type SettingsCapabilitiesInput,
} from "./settings/settings-capabilities";
import { useAiAssistant } from "./settings/use-ai-assistant";
export { useAiAssistant, type UseAiAssistantOptions } from "./settings/use-ai-assistant";
import { type GlobalTheme } from "./theme/global-theme";
export {
  candidatePaletteStyle,
  customCandidateStyle,
  customThemeBase,
  defaultGlobalTheme,
  globalThemeIds,
  isGlobalTheme,
  keyboardThemeId,
  themeCandidateStyle,
  themeCatalog,
  themeEntry,
  type BuiltinGlobalTheme,
  type CandidateThemePalette,
  type CustomCandidateColors,
  type CustomTheme,
  type GlobalTheme,
  type KeyboardThemePalette,
  type ResolveThemeRequest,
  type ResolvedTheme,
  type ThemeAppearance,
  type ThemeCatalog,
  type ThemeCatalogEntry,
  type ThemePreview,
  type ThemeSource,
} from "./theme/global-theme";
import { useVoiceInputSettings } from "./settings/use-voice-input-settings";
export {
  useVoiceInputSettings,
  type UseVoiceInputSettingsOptions,
} from "./settings/use-voice-input-settings";
export type {
  AiSkinClient,
  AiSkinProposal,
  AiSkinProgress,
  CustomSkinLibraryAction,
  CustomSkinLibraryClient,
  SavedTouchKeyboardSkin,
  TouchKeyboardSkinDesign,
} from "./keyboard/touch-keyboard-skin-design";
import { TypingStatisticsPage } from "./settings/typing-statistics";
import { VocabularyReviewPage } from "./settings/vocabulary-review";
import * as settings from "./settings/settings-style";
import { FluentIcon } from "./core/fluent-icons";
import { AccountPage } from "./account/account-page";
import type { AccountCommunityDestination } from "./account/account-model";
import { ChatPage } from "./chat/chat-page";
import { HomePage } from "./keyboard/home-page";
import {
  ScreenKeyboardLayoutContext,
  screenKeyboardLayoutFor,
} from "./keyboard/screen-keyboard-preview";
import { useSettingsPlatform } from "./theme/settings-platform";
import { SettingsFormContext } from "./settings/settings-form-context";
import { createSettingsReloadAction } from "./settings/settings-reload-action";
import { deepEqual } from "./core/deep-equal";
import { createSettingsPageSelection } from "./settings/settings-page-selection";
import { createSettingsDraftActions } from "./settings/settings-draft-actions";
import { createSettingsStatusActions } from "./settings/settings-status-actions";
import { createSettingsExternalActions } from "./settings/settings-external-actions";
import { createSettingsNavigationActions } from "./settings/settings-navigation-actions";
import { VoiceSettingsPage } from "./settings/pages/voice-page";
import { AiSettingsPage } from "./settings/pages/ai-page";
import { InputSettingsPage } from "./settings/pages/input-page";
import { ExpressionSettingsPage } from "./settings/pages/expression-page";
import { DeveloperSettingsPage, McpSettingsPage } from "./settings/pages/developer-page";
import { DictionarySettingsPage } from "./settings/pages/dictionary-page";
import { AppearanceSettingsPage } from "./settings/pages/appearance-page";
import { SkinSettingsPage } from "./settings/pages/skin-page";
import { skinGridTabs } from "./settings/skin-grid";
import { useSettingsTabs } from "./settings/use-settings-tabs";
import { FloatingToolbarSettingsPage } from "./settings/pages/floating-toolbar-page";
import { ScreenKeyboardSettingsPage } from "./settings/pages/screen-keyboard-page";
import { ShortcutSettingsPage } from "./settings/pages/shortcuts-page";
import { ToolsSettingsPage } from "./settings/pages/tools-page";
import { PluginsSettingsPage } from "./settings/pages/plugins-page";

import { AboutSettingsPage } from "./settings/pages/about-page";
import { UsageReportingSettingsPage } from "./settings/pages/usage-reporting-page";
import { PrivacySettingsPage } from "./settings/pages/privacy-page";

import { floatingToolbarPreferences } from "./settings/floating-toolbar-preferences";

import { HelpSettingsPage } from "./settings/help-settings-page";
import { HandwritingSettingsPage } from "./settings/pages/handwriting-page";
import { FeedbackSettingsPage } from "./settings/pages/feedback-page";
import { defaultTouchKeyboardSchemes } from "./settings/touch-keyboard-scheme-helpers";
import { logo } from "./settings/settings-options";
import { CommunitySkinsPage } from "./community/community-skins";
import { communityDestinationView } from "./community/community-destination";
import type { CommunityResource } from "./community/community-resource-model";
import { CommunityResourcesPage } from "./community/community-resources";
import { CommunityPluginsPage } from "./community/community-plugins";
import * as communityStyle from "./community/community-style";
import { CommunityPage } from "./community/community-page";
export { useConfirm, type ConfirmRequest } from "./core/confirm";
export type * from "./core/host-contracts";
export { resolveSettingsTheme } from "./settings/theme-helpers";
export { ToastProvider, useToast } from "./core/toast";
export { subscribeMediaQuery } from "./core/media-query";
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
  dailySpeeds,
  formatActiveTime,
  longestStreak,
  statisticDayKeys,
  statisticsOverviewMetrics,
  statisticsOverviewDetails,
  usualHours,
  type ActivityMetrics,
  type DailyDetailRow,
  type TypingBreakdown,
  type TypingStatistics,
  type TypingStatisticsClient,
  type TypingStatisticsStatus,
  type StatisticsOverviewMetric,
} from "./settings/typing-statistics";
export {
  keyboardHeatmapLayout,
  keyboardHeatmapModel,
  keyHeatLevel,
  keyLabel,
  scopedKeyCounts,
  type DailyKeyCounts,
  type KeyboardHeatmapKey,
  type KeyboardHeatmapLayout,
  type KeyboardHeatmapModel,
  type KeyCount,
} from "./settings/keyboard-heatmap";
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
  type McpFlag,
  type McpInstallOutcome,
  type McpServerStatus,
} from "./settings/mcp-connect";
export { AccountPage } from "./account/account-page";
export type {
  AccountChallenge,
  AccountClient,
  AccountCommunityDestination,
  AccountPreferenceSchema,
  AccountPreferences,
  AccountPreferenceValue,
  AccountProfile,
  AccountProviders,
  AccountUser,
  AppIconClient,
  AppIconInfo,
  SettingsSyncClient,
} from "./account/account-model";
export {
  AccountConfirmation,
  type AccountConfirmationAction,
  type AccountConfirmationProps,
} from "./account/account-confirmation";
export {
  AccountIdentityDetails,
  type AccountIdentityDetailsProps,
} from "./account/account-identity-details";
export { AccountInputField, type AccountInputFieldProps } from "./account/account-input-field";
export {
  AccountStatusMessages,
  type AccountStatusMessagesProps,
} from "./account/account-status-messages";
export { copyAccountId, type AccountIdCopyOptions } from "./account/account-id-copy";
export {
  runAccountOperation,
  useAccountAction,
  type AccountActionState,
  type AccountOperationState,
} from "./account/account-operation";
export {
  runAsyncAction,
  type AsyncActionOptions,
  type AsyncActionState,
} from "./core/async-action";
export {
  useAsyncActionRunner,
  type AsyncActionOperation,
  type AsyncActionRunner,
} from "./core/use-async-action";
export { useInterval } from "./core/use-interval";
export { useDebouncedCallback, type DebouncedCallback } from "./core/use-debounced-callback";
export {
  subscribeToWindowBlur,
  subscribeToWindowEvent,
  subscribeToWindowFocus,
  subscribeToWindowResume,
  type WindowFocusListener,
} from "./core/window-focus";
export { ChatPage } from "./chat/chat-page";
export type { ChatClient, ChatMessage, ChatModel, ChatModels } from "./chat/chat-model";
export { CHAT_SYSTEM_PROMPT } from "./chat/chat-prompt";
export { HomePage } from "./keyboard/home-page";
export type { HomePageActions } from "./keyboard/home-model";
export { openKeyboardSwitchGuide } from "./keyboard/keyboard-switch-guide";
export {
  WelcomeFlowPage,
  type OnboardingActions,
  type OnboardingChoices,
  type OnboardingInputScheme,
} from "./account/onboarding-page";
export { completeOnboardingPreferences } from "./account/onboarding-preferences";
export {
  usePreferencesSnapshot,
  type PreferencesSnapshotClient,
} from "./settings/use-preferences-snapshot";
export {
  LinuxSetupPage,
  type LinuxSetupClient,
  type LinuxSetupLine,
  type LinuxSetupStatus,
} from "./account/linux-setup-page";
export {
  MacosInstallPage,
  nextSimulatedProgress,
  type MacosInstallClient,
} from "./account/macos-install-page";
export { SettingsStartupPage } from "./settings/settings-startup-page";
export {
  HelpcodeSettingsPage,
  HelpcodeSettingsGroup,
  type HelpcodeSettingsGroupProps,
} from "./settings/pages/helpcode-page";
export {
  helpcodeSchemaOptions,
  type CustomHelpcodeSchema,
  type HelpcodePackOption,
  type HelpcodePreferences,
  type HelpcodeSchema,
  type HelpcodeSettings,
} from "./settings/helpcode-preferences";
export type { AppInputModeRule, PickedApplication } from "./settings/app-input-mode-rules";
export {
  ClipboardHistorySection,
  type ClipboardHistoryClient,
  type ClipboardHistoryEntry,
} from "./settings/clipboard-history-section";
export {
  CloudPanelSessionNotice,
  CLOUD_PANEL_SESSION_NOTE,
} from "./settings/cloud-panel-session-notice";
export {
  CLOUD_CLIPBOARD_DISABLED,
  CLOUD_CLIPBOARD_MAX_UTF16,
  CLOUD_CLIPBOARD_PENDING,
  CLOUD_CLIPBOARD_RELAY_UNAUTHORIZED,
  CLOUD_CLIPBOARD_RELAY_UNSUPPORTED,
  CLOUD_CLIPBOARD_SIGNED_OUT,
  CLOUD_CLIPBOARD_UNAVAILABLE,
  cloudClipboardAvailabilityNote,
  cloudClipboardErrorMessage,
  cloudClipboardFailure,
  type CloudClipboardAvailability,
  type CloudClipboardRequest,
} from "./settings/cloud-clipboard-send";
export { FuzzyPinyinSection, type FuzzyPinyinPreferences } from "./settings/fuzzy-pinyin-section";
export {
  WordCharacterSection,
  type NavigationPreferences,
  type WordCharacterPreferences,
} from "./settings/word-character-section";
export { NavigationSection } from "./settings/navigation-section";
export {
  enabledPagingShortcuts,
  navigationShortcutItems,
  wordCharacterKeyOptions,
} from "./settings/candidate-paging-options";
export {
  handwritingPrivacyText,
  type HandwritingPlatform,
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
export {
  CandidateTranslationSettingsSection,
  type CandidateTranslationSettingsSectionProps,
} from "./settings/candidate-translation-settings-section";
export { PunctuationSection, type PunctuationPreferences } from "./settings/punctuation-section";
export { punctuationControlState } from "./settings/punctuation-preferences";
export {
  MixedInputSection,
  MixedInputMoreRows,
  defaultMixedInput,
  mixedInputDefaultsFor,
  type MixedInputPreferences,
} from "./settings/mixed-input-section";
export {
  InputLanguageOptionsSection,
  type InputLanguageOptionsSectionProps,
} from "./settings/input-language-options-section";
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
export { localModeRowsFor, type LocalModeRow } from "./settings/local-mode-options";
export {
  PluginsSection,
  MAX_MENTIONS,
  mentionListIssue,
  pluginErrorMessage,
  type MentionEntry,
  type PluginCatalogResult,
  type PluginClient,
  type PluginCommand,
  type PluginIssue,
  type PluginKind,
  type PluginPackage,
  type PluginsSectionProps,
  SCRIPT_CAPABILITIES,
  ScriptGrantDialog,
  capabilityLabels,
  SCRIPT_BUILTIN_NOTE,
  SCRIPT_COMMAND_MODE_NOTE,
  scriptCommandStatus,
  scriptGrantState,
  scriptResidualRisk,
  toggleCapability,
  type ScriptCapability,
  type ScriptCommand,
  type ScriptCommandState,
  type ScriptGrant,
  type ScriptGrantDialogProps,
  type ScriptGrantsResult,
  type ScriptHealth,
} from "./settings/plugins-section";
export {
  DEFAULT_MELODY_PACK,
  DEFAULT_SOUND_PACK,
  MAX_COMMAND_TABLES,
  defaultPluginPreferences,
  pluginPreferences,
  withPackSelected,
  withoutRemovedPack,
  type AchievementPreferences,
  type CommitSoundPreferences,
  type EffectStyle,
  type KeySoundMode,
  type KeySoundPreferences,
  type MelodyPreferences,
  type MusicPreferences,
  type PluginPreferences,
} from "./settings/plugin-preferences";
export { ThemeSettingsSection } from "./settings/theme-settings-section";
export type {
  SurfaceTheme,
  ThemeMode,
  ThemePreferenceKey,
  ThemePreferences,
} from "./settings/theme-preferences";
export { SurfaceThemeSelect, type SurfaceThemeSelectProps } from "./settings/surface-theme-select";
export { SurfaceThemeRow, type SurfaceThemeRowProps } from "./settings/surface-theme-row";
export { ShortcutRow, type ShortcutRowProps } from "./settings/shortcut-row";
export { EndpointInput, type EndpointInputProps } from "./settings/endpoint-input";
export { EndpointSettingRow, type EndpointSettingRowProps } from "./settings/endpoint-setting-row";
export { OpenPanelRow, type OpenPanelRowProps } from "./settings/open-panel-row";
export { ActionRow, type ActionRowProps } from "./settings/action-row";
export { ActionButton, type ActionButtonProps } from "./settings/action-button";
export { FeedbackKindOptions } from "./settings/feedback-kind-options";
export {
  feedbackDiagnostics,
  feedbackMaxText,
  feedbackTypes,
  type FeedbackDiagnosticsSource,
} from "./settings/feedback-contracts";
export { OpenPanelButton, type OpenPanelButtonProps } from "./settings/open-panel-button";
export {
  VoiceLanguageOptions,
  type VoiceLanguageOptionsProps,
} from "./voice/voice-language-options";
export { VoiceProviderRow, type VoiceProviderRowProps } from "./settings/voice-provider-row";
export {
  CloudPinyinSchemeOptions,
  CloudShuangpinProfileOptions,
} from "./keyboard/cloud-scheme-options";
export {
  PreeditStyleSelect,
  type PreeditStyle,
  type PreeditStyleSelectMode,
  type PreeditStyleSelectProps,
} from "./settings/preedit-style-select";
export { PreeditStyleRow, type PreeditStyleRowProps } from "./settings/preedit-style-row";
export { preeditControlState } from "./settings/preedit-preferences";
export {
  CandidateColorsSection,
  type CandidateColorKey,
  type CandidateColorPreferences,
} from "./settings/candidate-colors-section";
export { CandidatePaletteFallbackNotice } from "./settings/candidate-palette-fallback-notice";
export {
  CandidatePaletteSection,
  type CandidatePaletteSectionProps,
} from "./settings/candidate-palette-section";
export { VoicePolishSection, type VoicePolishSectionProps } from "./settings/voice-polish-section";
export {
  VoicePolishSettingsSection,
  type VoicePolishSettingsSectionProps,
} from "./settings/voice-polish-settings-section";
export { describeImportResult } from "./dictionary/dictionary-messages";
export { AiLinuxProviderSection } from "./settings/ai-linux-provider-section";
export {
  CandidateSizingSection,
  type CandidateSizingPreferences,
} from "./settings/candidate-sizing-section";
export {
  CandidateFontPresetRow,
  CandidateScaleRow,
  CandidateWindowStyleSection,
  type CandidateFontPresetRowProps,
  type CandidateScaleRowProps,
  type CandidateWindowStyleSectionPreferences,
  type CandidateWindowStyleSectionProps,
} from "./settings/candidate-window-style-section";
export { CandidatePageSizeSection } from "./settings/candidate-page-size-section";
export { CandidateLayoutSection, type CandidateLayout } from "./settings/candidate-layout-section";
export { candidateLayoutOptions } from "./settings/candidate-layout-options";
export { themeModeOptions } from "./settings/theme-mode-options";
export { candidateTabs, type CandidateTab } from "./settings/candidate-tabs";
export { expressionTabs, type ExpressionTab } from "./settings/expression-tabs";
export { inputTabs, type InputTab } from "./settings/input-tabs";
export {
  CandidateFollowCursorSection,
  type CandidateFollowCursorSectionProps,
} from "./settings/candidate-follow-cursor-section";
export {
  CandidatePageNumberSection,
  type CandidatePageNumberSectionProps,
} from "./settings/candidate-page-number-section";
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
  SingleCharacterOnlySection,
  type SingleCharacterOnlySectionProps,
} from "./settings/single-character-only-section";
export {
  LearningDataSection,
  type LearningDataSectionProps,
} from "./settings/learning-data-section";
export {
  SettingsActionsFooter,
  type SettingsActionsFooterProps,
} from "./settings/settings-actions-footer";
export {
  SETTINGS_AUTOSAVE_DELAY_MS,
  type SettingsSaveState,
} from "./settings/use-settings-persistence";
export { AboutHeroSection, type AboutHeroSectionProps } from "./settings/about-hero-section";
export { SkinPlatformNotice, type SkinPlatformNoticeProps } from "./settings/skin-platform-notice";
export { ThemeCarousel, type ThemeCarouselProps } from "./settings/theme-carousel";
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
  type ChineseOutputForm,
  type TraditionalChineseOutputSectionProps,
  type TraditionalChineseOutputValue,
  type TraditionalChineseVariant,
} from "./settings/traditional-chinese-output-section";
export {
  ChineseEnglishSpacingSection,
  type ChineseEnglishSpacingSectionProps,
} from "./settings/chinese-english-spacing-section";
export {
  CloudCandidatesSection,
  type CloudCandidatesSectionProps,
} from "./settings/cloud-candidates-section";
export {
  TelemetryRow,
  TelemetrySection,
  UsageReportingDetails,
  usageReportingEndpoint,
  usageReportingSummary,
  type TelemetrySectionProps,
} from "./settings/telemetry-section";
export { WubiSection, type WubiPreferences, type WubiSectionProps } from "./settings/wubi-section";
export { wubiControlState } from "./settings/wubi-preferences";
export {
  WubiCodeLookupSection,
  WUBI_CODE_LOOKUP_MAX_LENGTH,
  type WubiCodeLookupSectionProps,
} from "./settings/wubi-code-lookup-section";
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
  type WubiProfile,
} from "./settings/input-scheme-details-section";
export {
  ShuangpinCustomEditor,
  type ShuangpinCustomEditorProps,
} from "./settings/shuangpin-custom-editor";
export {
  parseShuangpinCustomProfile,
  shuangpinCustomProblemMessage,
  type BuiltinShuangpinProfile,
  type ShuangpinCustomProblem,
  type ShuangpinCustomProfile,
  type ShuangpinCustomProfileClient,
  type ShuangpinCustomTablePart,
} from "./settings/shuangpin-custom-profile";
export {
  TranslationProviderSettingsSection,
  type TranslationProviderSettingsSectionProps,
  type TranslationNiuTransSettings,
  type TranslationTencentSettings,
  type TranslationCustomSettings,
} from "./settings/translation-provider-settings-section";
export {
  InputModeShortcutsSection,
  type InputModeShortcutPreferences,
  type InputModeShortcutsSectionProps,
} from "./settings/input-mode-shortcuts-section";
export {
  languageSwitchChoice,
  languageSwitchPatch,
  type LanguageSwitchChoice,
} from "./settings/input-mode-shortcuts-model";
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
  InputMethodServiceSection,
  type InputMethodServiceSectionProps,
} from "./settings/input-method-service-section";
export {
  DataDirectorySection,
  type DataDirectoryInfo,
  type DataDirectorySectionProps,
} from "./settings/data-directory-section";
export { LicenseRows, type LicenseRowsProps } from "./settings/license-rows";
export { UninstallSection, type UninstallSectionProps } from "./settings/uninstall-section";
export {
  DiagnosticLogsSection,
  type DiagnosticLogPreferences,
  type DiagnosticLogsSectionProps,
} from "./settings/diagnostic-logs-section";
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
export {
  VoiceAsrProviderSettingsSection,
  type VoiceAsrProviderSettingsSectionProps,
} from "./settings/voice-asr-provider-settings-section";
export {
  VoiceLocalModelSettingsSection,
  type VoiceLocalModelSettingsSectionProps,
} from "./settings/voice-local-model-settings-section";
export {
  VoiceInputBasicsSection,
  type VoiceInputBasicsSectionProps,
} from "./settings/voice-input-basics-section";
export {
  VoiceRecordingBehaviorSettingsSection,
  type VoiceRecordingBehaviorSettingsSectionProps,
} from "./settings/voice-recording-behavior-settings-section";
export {
  VoiceCredentialControl,
  type VoiceCredentialControlProps,
} from "./settings/voice-credential-control";
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
  type PolishCustomSlot,
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
  VoiceAsrServiceTestSection,
  type VoiceAsrServiceTestSectionProps,
} from "./settings/voice-asr-service-test-section";
export {
  VoiceHotkeysSection,
  type VoiceHotkeysSectionProps,
  type VoiceHotkeyKey,
  type VoiceHotkeyPlatform,
} from "./settings/voice-hotkeys-section";
export { FloatingToolbarPlatformNotice } from "./settings/floating-toolbar-platform-notice";
export {
  floatingToolbarButtonOptions,
  floatingToolbarSize,
  floatingToolbarSizes,
  type FloatingToolbarSize,
} from "./settings/floating-toolbar-options";
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
  DoubaoOptionsRows,
  type DoubaoOptionsSectionProps,
} from "./settings/doubao-options-section";
export {
  DoubaoResourceIdSection,
  type DoubaoResourceIdSectionProps,
} from "./settings/doubao-resource-id-section";
export {
  VoiceRecordingBehaviorSection,
  type VoiceRecordingBehaviorSectionProps,
  VoiceCueSwitchesSection,
  type VoiceCueSwitchesSectionProps,
} from "./settings/voice-recording-behavior-section";
export {
  VoiceModelMirrorRow,
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
  VoiceProviderSelect,
  type VoiceProviderSelectProps,
} from "./settings/voice-provider-select";
export {
  CredentialTestSection,
  type CredentialTestSectionProps,
} from "./settings/credential-test-section";
export type { CredentialTestState } from "./settings/credential-test-state";
export {
  ProviderPresetSection,
  type ProviderPresetSectionProps,
} from "./settings/provider-preset-section";
export {
  CredentialStatusMessage,
  type CredentialStatusMessageProps,
} from "./settings/credential-status-message";
export type { CredentialStatusMessageValue } from "./settings/credential-status-message";
export { CredentialActions, type CredentialActionsProps } from "./settings/credential-actions";
export { SettingToggle, type SettingToggleProps } from "./settings/setting-toggle";
export { SettingField, type SettingFieldProps } from "./settings/setting-field";
export {
  SettingActionHeader,
  type SettingActionHeaderProps,
  SettingSectionHeader,
  type SettingSectionHeaderProps,
} from "./settings/setting-action-header";
export {
  SettingSectionTitle,
  type SettingSectionTitleProps,
} from "./settings/setting-section-title";
export {
  SettingsTextareaField,
  type SettingsTextareaFieldProps,
} from "./settings/settings-textarea-field";
export { SettingsInputField, type SettingsInputFieldProps } from "./settings/settings-input-field";
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
  TencentTranslationSection,
  type TencentTranslationSectionProps,
} from "./settings/tencent-translation-section";
export { SecretSettingRow, type SecretSettingRowProps } from "./settings/secret-setting-row";
export { TextInputRow, type TextInputRowProps } from "./settings/text-input-row";
export { SelectRow, type SelectRowProps } from "./settings/select-row";
export { SwitchRow, type SwitchRowProps } from "./settings/switch-row";
export {
  SegmentedRow,
  type SegmentedRowOption,
  type SegmentedRowProps,
} from "./settings/segmented-row";
export { SliderRow, type SliderRowProps } from "./settings/slider-row";
export { SummaryRow, type SummaryRowProps } from "./settings/summary-row";
export { SecretSettingField, type SecretSettingFieldProps } from "./settings/secret-setting-field";
export {
  PasswordSettingField,
  type PasswordSettingFieldProps,
} from "./settings/password-setting-field";
export { TextSettingField, type TextSettingFieldProps } from "./settings/text-setting-field";
export { SelectSettingField, type SelectSettingFieldProps } from "./settings/select-setting-field";
export { ModelPickerRows, type ModelPickerRowsProps } from "./settings/model-picker-rows";
export {
  EndpointSettingField,
  type EndpointSettingFieldProps,
} from "./settings/endpoint-setting-field";
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
  MobileKeyboardFeedbackSettings,
  type MobileKeyboardFeedbackSettingsProps,
} from "./settings/mobile-keyboard-feedback-settings";
export {
  PreeditSettingsSection,
  type CandidatePreeditStyle,
  type PreeditSettingsPreferences,
  type TsfPreeditStyle,
} from "./settings/preedit-settings-section";
export { CommunitySkinsPage } from "./community/community-skins";
export {
  CommunitySkinDetail,
  type CommunitySkinDetailProps,
} from "./community/community-skin-detail";
export { CommunitySkinCard, type CommunitySkinCardProps } from "./community/community-skin-card";
export {
  useCommunitySkinGallery,
  type CommunitySkinGallery,
  type UseCommunitySkinGalleryOptions,
} from "./community/use-community-skin-gallery";
export {
  CommunityHarmonySkinCard,
  type CommunityHarmonySkinCardProps,
} from "./community/community-harmony-skin-card";
export type { CommunitySkinPillState } from "./community/use-community-skin-library";
export {
  type CommunitySkin,
  type CommunitySkinClient,
  type CommunitySkinDownload,
  type CommunitySkinPage,
  type CommunitySkinTrial,
} from "./community/community-skin-model";
export {
  createCommunitySkinClient,
  type CommunitySkinOperation,
  type CommunitySkinRequest,
} from "./community/community-skin-client";
export {
  useCommunityGallery,
  type CommunityGalleryActionOptions,
  type CommunityGalleryClient,
  type CommunityGalleryOptions,
  type CommunityGalleryPage,
} from "./community/community-gallery";
export {
  CommunityRemovedBadge,
  CommunityReportSection,
  type CommunityReportSectionProps,
} from "./community/community-report";
export {
  communityReportDetailLimit,
  communityReportReasons,
  communityReportedNotice,
  type CommunityModeration,
  type CommunityReportKind,
  type CommunityReportReason,
} from "./community/community-report-model";
export {
  CommunityErrorAlert,
  type CommunityErrorAlertProps,
} from "./community/community-error-alert";
export {
  CommunityGalleryFeedback,
  type CommunityGalleryFeedbackProps,
} from "./community/community-gallery-feedback";
export {
  CommunityDialogActions,
  type CommunityDialogActionsProps,
  CommunityDialogFrame,
  type CommunityDialogFrameProps,
  CommunityDialogHeader,
  type CommunityDialogHeaderProps,
} from "./community/community-dialog";
export {
  CommunityConfirmation,
  type CommunityConfirmationProps,
} from "./community/community-confirmation";
export {
  CommunityDetailStatus,
  type CommunityDetailStatusProps,
} from "./community/community-detail-status";
export {
  CommunityRatingMetrics,
  type CommunityRatingMetricsProps,
} from "./community/community-rating-metrics";
export {
  CommunityDetailFrame,
  type CommunityDetailFrameProps,
} from "./community/community-detail-frame";
export {
  communityDestinationView,
  type CommunityDestination,
  type CommunityDestinationCategory,
  type CommunityDestinationScope,
  type CommunityDestinationView,
} from "./community/community-destination";
export { CommunityHomePage, type CommunityHomePageProps } from "./community/community-home-page";
export {
  CommunityCategoryTabs,
  type CommunityCategoryTabsProps,
} from "./community/community-category-tabs";
export {
  CommunityResourcesPage,
  type CommunityResourcesPageProps,
} from "./community/community-resources";
export {
  CommunityResourceCard,
  type CommunityResourceCardProps,
} from "./community/community-resource-card";
export {
  CommunityHarmonyResourceRow,
  type CommunityHarmonyResourceRowProps,
  type CommunityResourcePillState,
} from "./community/community-harmony-resource-row";
export {
  CommunityResourceDetail,
  type CommunityResourceDetailProps,
} from "./community/community-resource-detail";
export {
  CommunityResourceEditor,
  type CommunityResourceEditorProps,
} from "./community/community-resource-editor";
export type {
  CommunityLocalDictionaryClient,
  CommunityResource,
  CommunityResourceApplication,
  CommunityResourceClient,
  CommunityResourceContent,
  CommunityResourceKind,
  CommunityResourcePage,
  CommunityResourceScope,
  CommunitySharedWord,
} from "./community/community-resource-model";
export { CommunityCandidateSkinsPage } from "./community/community-candidate-skins";
export {
  candidateSkinCategories,
  candidateSkinCategoryLabels,
  type CandidateSkinCategory,
  type CandidateSkinCommunityClient,
  type CandidateSkinPackPreview,
  type CandidateSkinSyncReport,
  type CandidateSkinSyncSkip,
  type CandidateSkinVisibility,
  type CommunityCandidateSkin,
  type CommunityCandidateSkinLicense,
  type CommunityCandidateSkinPage,
} from "./community/community-candidate-skin-model";
export {
  CommunityScopeButtons,
  type CommunityScopeButtonsProps,
} from "./community/community-scope-buttons";
export {
  communityPublishFields,
  handleCommunityPublishKeyDown,
  type CommunityPublishFields,
} from "./community/community-publish-validation";
export {
  CommunityResourceScopeButtons,
  type CommunityResourceScopeButtonsProps,
} from "./community/community-resource-scope-buttons";
export {
  CommunitySearchForm,
  type CommunitySearchFormProps,
} from "./community/community-search-form";
export {
  CommunityGalleryLoadMore,
  type CommunityGalleryLoadMoreProps,
} from "./community/community-gallery-load-more";
export {
  CommunityGalleryGrid,
  type CommunityGalleryGridProps,
} from "./community/community-gallery-grid";
export { CommunityPageShell, type CommunityPageShellProps } from "./community/community-page-shell";
export {
  CommunityCardAuthor,
  type CommunityCardAuthorProps,
} from "./community/community-card-author";
export {
  CommunityConfirmationActions,
  type CommunityConfirmationActionsProps,
} from "./community/community-confirmation-actions";
export { CommunityMetrics, type CommunityMetricsProps } from "./community/community-metrics";
export { CommunityCard, type CommunityCardProps } from "./community/community-card";
export {
  CommunitySkinPublicationFields,
  type CommunitySkinPublicationFieldsProps,
} from "./community/community-skin-publication-fields";
export {
  CommunityPublicationMetadataFields,
  type CommunityPublicationMetadataFieldsProps,
} from "./community/community-publication-metadata-fields";
export {
  CommunityPublicationWarning,
  type CommunityPublicationWarningProps,
} from "./community/community-publication-warning";
export { CommunityNotice, type CommunityNoticeProps } from "./community/community-notice";
export { SettingsGroupNote, type SettingsGroupNoteProps } from "./settings/settings-group-note";
export { SettingsGroupBlock, type SettingsGroupBlockProps } from "./settings/settings-group-block";
export { SettingsRowStack, type SettingsRowStackProps } from "./settings/settings-row-stack";
export {
  SettingsManagerActions,
  type SettingsManagerActionsProps,
} from "./settings/settings-manager-actions";
export {
  SettingsManagerBlock,
  type SettingsManagerBlockProps,
} from "./settings/settings-manager-block";
export {
  SettingsPreviewBlock,
  type SettingsPreviewBlockProps,
} from "./settings/settings-preview-block";
export {
  SettingsPreviewLabel,
  type SettingsPreviewLabelProps,
} from "./settings/settings-preview-label";
export {
  SettingsEmptyMessage,
  type SettingsEmptyMessageProps,
} from "./settings/settings-empty-message";
export { SettingsServiceRow, type SettingsServiceRowProps } from "./settings/settings-service-row";
export { SettingsPhraseForm, type SettingsPhraseFormProps } from "./settings/settings-phrase-form";
export {
  SettingsShortcutKey,
  type SettingsShortcutKeyProps,
} from "./settings/settings-shortcut-key";
export { SkinCardHeader, type SkinCardHeaderProps } from "./skin/skin-card-header";
export { SkinPreviewStage, type SkinPreviewStageProps } from "./skin/skin-preview-stage";
export { SkinPreviewSurface, type SkinPreviewSurfaceProps } from "./skin/skin-preview-surface";
export {
  SettingsInputDescription,
  type SettingsInputDescriptionProps,
} from "./settings/settings-input-description";
export { SettingsWarning, type SettingsWarningProps } from "./settings/settings-warning";
export { ErrorAlert, type ErrorAlertProps } from "./core/error-alert";
export { formatDateTime, formatZhDate, formatZhMonthDay } from "./core/format-date";
export { formatZhNumber, formatZhPercent } from "./core/format-number";
export { StatusMessage, type StatusMessageProps } from "./core/status-message";
export { SettingsNotice, type SettingsNoticeProps } from "./settings/settings-notice";
export {
  SettingsManagerNote,
  type SettingsManagerNoteProps,
} from "./settings/settings-manager-note";
export {
  useCommunityPublicationDraft,
  type CommunityPublicationDraft,
} from "./community/use-community-publication-draft";
export {
  useCommunityClientLifecycle,
  type CommunityClientLifecycle,
} from "./community/use-community-client-lifecycle";
export {
  useCommunityDetailHistory,
  type CommunityDetailHistoryOptions,
} from "./community/use-community-detail-history";
export { useMobilePopState, type MobilePopStateHandler } from "./settings/use-mobile-pop-state";
export {
  CommunityRightsAgreement,
  type CommunityRightsAgreementProps,
} from "./community/community-rights-agreement";
export {
  CommunityTextareaField,
  type CommunityTextareaFieldProps,
} from "./community/community-textarea-field";
export { CommunityField, type CommunityFieldProps } from "./community/community-field";
export {
  CommunityActionNotice,
  type CommunityActionNoticeProps,
} from "./community/community-action-notice";
export {
  CommunityGalleryHeading,
  type CommunityGalleryHeadingProps,
} from "./community/community-gallery-heading";
export {
  CommunityInputField,
  type CommunityInputFieldProps,
} from "./community/community-input-field";
export {
  CommunitySelectField,
  type CommunitySelectFieldProps,
} from "./community/community-select-field";
export {
  CommunitySkinPublishDialog,
  type CommunitySkinPublishDialogProps,
} from "./community/community-skin-publish-dialog";
export {
  CandidateSkinPublishDialog,
  type CandidateSkinPublishDialogProps,
} from "./community/candidate-skin-publish-dialog";
export { CommunityPluginsPage } from "./community/community-plugins";
export {
  CommunityPluginPublishDialog,
  type CommunityPluginPublishDialogProps,
} from "./community/community-plugin-publish-dialog";
export {
  communityPluginKinds,
  type CommunityPlugin,
  type CommunityPluginClient,
  type CommunityPluginKind,
  type CommunityPluginPackPreview,
  type CommunityPluginPage,
} from "./community/community-plugin-model";
export { CommunityPage, type CommunityPageProps } from "./community/community-page";
export type { SkinCatalog, ExternalSkin } from "./skin/external-skins";
export type { SkinImage, SkinImageReader } from "./skin/skin-image";
export type { SkinFont, SkinFontReader } from "./skin/skin-font";
export {
  POLISH_CUSTOM_IDS,
  POLISH_PRESETS,
  POLISH_PRESET_IDS,
  POLISH_PRESET_NAMES,
  isPolishCustomSlot,
  polishPromptFor,
  polishPresetPrompt,
  polishSlotField,
  type PolishPresetId,
} from "./voice/polish-presets";
export {
  ASR_PROVIDER_DEFAULTS,
  ASR_SERVICE_PROVIDER_IDS,
  POLISH_PROVIDER_DEFAULTS,
  asrProviderUpdate,
  isAsrServiceProvider,
  providerSettingValue,
  polishProviderUpdate,
} from "./voice/voice-providers";
export {
  ASR_PROVIDER_OPTIONS,
  POLISH_PROVIDER_OPTIONS,
  VoiceProviderOptions,
  type VoiceProviderOption,
} from "./voice/voice-provider-options";
export {
  asrProviderCredentialTestConfig,
  asrProviderCredentialTestDisabled,
  asrServiceCredentialTestConfig,
  polishProviderCredentialTestConfig,
  polishServiceCredentialTestConfig,
} from "./settings/voice-credential-test-config";
export {
  candidateTemplate,
  type CandidateAppearance,
  type CandidateOrientation,
} from "./candidate/candidate-themes";
import { describeInstallerTrust } from "./settings/update-manifest";
import { editionUsesHelpcode } from "./settings/input-scheme-options";
export {
  serializeWindowHostMessage,
  type WindowControl,
  type WindowHostMessage,
  type WindowResizeEdge,
} from "./keyboard/window-host";
export { emojiDisplayName } from "./keyboard/panels";
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
} from "./keyboard/panels";
export type {
  CloudCandidate,
  CloudCandidateKind,
  CloudClipboardAction,
  CloudClipboardPanelClient,
  CloudDictionaryAction,
  CloudDictionaryCatalogEntry,
  CloudDictionaryEntry,
  CloudDictionaryFileFormat,
  CloudDictionaryKind,
  CloudDictionaryPanelClient,
  CloudDictionarySnapshotMetadata,
  CloudDictionarySnapshotRequest,
  CloudFixedPosition,
  CloudRankingMode,
  EmojiPanelClient,
  PanelClient,
  VoicePanelClient,
} from "./keyboard/panel-client";
export {
  usePanelAction,
  type PanelAction,
  type PanelActionState,
} from "./keyboard/use-panel-action";
export {
  CloudDictionaryEntryForm,
  type CloudDictionaryEntryFormProps,
  type CloudDictionaryEntryFormValue,
} from "./keyboard/cloud-dictionary-entry-form";
export {
  CloudDictionaryEntryCard,
  type CloudDictionaryEntryCardEntry,
  type CloudDictionaryEntryCardProps,
} from "./keyboard/cloud-dictionary-entry-card";
export {
  CloudDictionaryItem,
  type CloudDictionaryItemProps,
} from "./keyboard/cloud-dictionary-item";
export { CloudPanelHeader, type CloudPanelHeaderProps } from "./keyboard/cloud-panel-header";
export {
  CloudDictionaryPagination,
  type CloudDictionaryPaginationProps,
} from "./keyboard/cloud-dictionary-pagination";
export {
  CloudDictionaryKindTabs,
  type CloudDictionaryKindTabsProps,
} from "./keyboard/cloud-dictionary-kind-tabs";
export {
  CloudDictionaryKindSelect,
  type CloudDictionaryKindSelectProps,
} from "./keyboard/cloud-dictionary-kind-select";
export {
  CloudDictionarySelectField,
  type CloudDictionarySelectFieldProps,
} from "./keyboard/cloud-dictionary-select-field";
export {
  CloudDictionaryQueryToolbar,
  type CloudDictionaryQueryToolbarProps,
} from "./keyboard/cloud-dictionary-query-toolbar";
export type { EmojiCatalogGroup } from "./emoji/emoji-catalog";
export type { VoiceCaptureDevice, VoiceDeviceReader } from "./voice/voice-device-picker";
export {
  LocalModelManager,
  formatModelBytes,
  localModelErrorMessage,
  localModelImportErrorMessage,
  localModelInUse,
  localModelProgressPercent,
  validModelMirror,
  visibleLocalModels,
  type LocalVoiceModel,
  type LocalVoiceModelClient,
  type LocalVoiceModelImportFile,
  type LocalVoiceModelList,
  type LocalVoiceModelProgress,
} from "./voice/local-models";
export {
  ResourcePackRow,
  resourcePackForScheme,
  resourcePackTitles,
  savedModelMirror,
  useResourcePacks,
  type ModelMirrorClient,
  type ResourcePackClient,
  type ResourcePackId,
  type ResourcePackMirror,
  type ResourcePacks,
  type ResourcePackStatus,
} from "./settings/resource-packs";

export type { KeybindingPreferences } from "./settings/keybinding-preferences";
export type { HostPlatform, HostCapabilities, EditionInfo } from "./core/host-capabilities";
export type { InputScheme, ChineseScheme } from "./core/input-scheme";

export { useCandidatePreviewTheme } from "./candidate/candidate-preview-theme";

export type { SentenceAssociationPreferences } from "./settings/sentence-association-preferences";
export type { VoiceInputPreferences } from "./settings/voice-input-preferences";
export type { AiAssistantPreferences } from "./settings/ai-assistant-preferences";
export type { VietnamesePreferences } from "./settings/vietnamese-preferences";

export type { Preferences, Snapshot, ExternalSkinCatalog } from "./core/preferences-model";
export type {
  ProviderCredentialStatus,
  VoiceProviderCredential,
  VoiceCredentialKind,
  VoiceCredentialSaveResult,
  ProviderCredentialClient,
} from "./core/provider-credential-contracts";
export type { AiAssistantClient } from "./core/ai-assistant-client";
export type { ApiCredentialTestService, ApiCredentialTestResult } from "./core/api-credential-test";
export type { ProviderPreset, ProviderDefaults } from "./core/provider-preset";
export {
  AI_PROVIDER_OPTIONS,
  aiProviderOption,
  type AiProviderOption,
  type AiProviderCatalogEntry,
} from "./settings/ai-provider-options";
export { aiProviderUpdate } from "./settings/ai-provider-update";

export type {
  DictionaryClient,
  DictionaryFailure,
  DictionaryImportResult,
  DictionaryManifest,
  WubiTextCodes,
  WubiCodeLookup,
  WubiCodeLookupClient,
} from "./dictionary/dictionary-contracts";
export { dictionaryKindKeyHint } from "./settings/pages/dictionary-page";
export type {
  DictionaryCollection,
  DictionaryCollectionKind,
  DictionaryCollectionSource,
  DictionaryCollectionWord,
  DictionaryCollectionImportReport,
  DictionaryCollectionsView,
  DictionaryCollectionsClient,
} from "./dictionary/dictionary-collections";

export type { FloatingToolbarPreferences } from "./settings/floating-toolbar-preferences";
export { InputSourceStartupNotice } from "./settings/input-source-startup-notice";
/** macOS 设置宿主启动时对内嵌输入法的安装与启用状态。 */
export type { InputSourceStartupStatus } from "./settings/input-source-startup-model";
export type { SettingsClient, PreferencesRecovery } from "./core/settings-client";

/** What a load or save that failed on an unreadable document says; the repair button sits beside exactly this message. */
/**
 * The 设置 tab draws a gear, not the app.
 *
 * Its page icon is the app logo, which the design does not put in the bar either — its first tab is the `settings` glyph. Three of the four tabs would otherwise be a subject and the fourth a brand.
 */
export {
  customTranslationKeyIssue,
  isDeepLApiEndpoint,
  tencentCredentialIssue,
  translationEndpointIssue,
} from "./settings/translation-validation";
export {
  aiCredentialOrigin,
  providerCredentialErrorMessage,
  tencentSecretConfigured,
} from "./settings/credential-utils";
export {
  aiEndpointHint,
  aiEndpointProblem,
  type AiEndpointProblem,
} from "./settings/ai-endpoint-policy";

type SettingsPageProps = {
  client: SettingsClient;
  /** The section the page opens on. Read once, at mount. */
  initialPage?: string;
  /** A section requested after mount; a new `nonce` navigates there and keeps the draft. */
  route?: { page: string; nonce: number };
  onReplayOnboarding?: () => void;
};

// The state, effects and handlers behind the settings window. The shell below and every page component read the same values - the pages through `SettingsFormContext` - so splitting the page into files changed where the markup lives, not what it closes over.
function useSettingsPageModel({ client, initialPage, route }: SettingsPageProps) {
  const { confirm, confirmation } = useConfirm();
  const {
    host,
    linux: linuxPlatform,
    android: androidPlatform,
    ios: iosPlatform,
    harmony: harmonyPlatform,
    mobile: mobilePlatform,
    windows: windowsPlatform,
    macos: macosPlatform,
    ...capabilities
  } = settingsPageEnvironment(client);
  // Which of the redesign's eight settings looks the root takes; see `theme/platform-tokens.ts`.
  const settingsPlatform = useSettingsPlatform(host);
  // HarmonyOS 2-in-1 是桌面窗口：侧栏从 输入 开始，启动时也打开 输入，所以即使宿主仍然提供 `home`（设置引导警示条会读取它），手机的 设置 根页面也不是它的页面。
  const hasHomePage = Boolean(client.home) && settingsPlatform !== "hm2";
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
    showTouchKeyboardHeight,
    showTouchKeyboardSpacing,
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showEnglishSuggestions,
    showHelpcodeShiftEntry,
    showShuangpinPreedit,
    showShuangpinKeymapHint,
    showCharacterWidth,
    aiProviderCredentials,
    showVoiceCommitMode,
    showVoiceProviderSettings,
    showVoiceStreamPreedit,
    showVoiceSpaceLockSwitch,
    showVoiceCueSwitches,
    showVoiceCaptureBackends,
    showCandidateRowColors,
    showCandidateSelectionAppearance,
    showCandidateBorderColor,
    showCandidateNumberColor,
    surfaceThemesFollow,
    showCandidateFollowCursor,
    showCandidateWindowScale,
    showCandidateWindowOpacity,
    showCandidateCornerRadius,
    showCandidatePadding,
    showInputModeHUD,
    showInputModeStatusItem,
    showVoiceCaptureDevices,
    showFullwidthChord,
    showTabPagePagingSwitches,
    showMixedInputMinimumPrefix,
    clientHostedPlatform,
    desktopPanels,
    showVoiceHotkeys,
    showKeySound,
    showMusic,
    showPluginTriggers,
    showTypingEffects,
    showTypingEffectStyles,
    showTypingEffectPacks,
    showEffectIntensity,
    showAiCandidateLimit,
    typingEffectsFlashOnly,
    showWordbookPacks,
    showSymbolSetPacks,
    showScriptPlugins,
  } = capabilities;
  const {
    fullwidthChord,
    releasesPageUrl: platformReleasesPageUrl,
    licenseUrl: platformLicenseUrl,
    issuesUrl: platformIssuesUrl,
    captureBackendOptions,
    helpIntro: platformHelpIntro,
    quickStart: platformQuickStart,
    networkDescription: platformNetworkDescription,
    aboutDescription: platformAboutDescription,
  } = settingsPlatformPresentation({
    android: androidPlatform,
    linux: linuxPlatform,
    macos: macosPlatform,
    harmony: harmonyPlatform,
    ios: iosPlatform,
    mobile: mobilePlatform,
    windows: windowsPlatform,
    clientHostedPlatform,
  });
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [draft, setDraft] = useState<Preferences>();
  const {
    onAiChange,
    onCandidateColorChange,
    onCustomKeyboardChange,
    onTranslationChange,
    onUseCustomKeyboard,
    onVoiceChange,
  } = createSettingsDraftActions({ setDraft });
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
    dismissInputSourceStartup,
    inputSourceStartup,
    onDeviceDownloadable,
    shuangpinKeymap: macosShuangpinKeymap,
  } = useMacosSettings({ client, macos: macosPlatform, setError });
  const restoredMobilePage =
    mobilePlatform &&
    typeof window !== "undefined" &&
    window.history.state?.msimeSettings === true &&
    typeof window.history.state.page === "string"
      ? window.history.state.page
      : undefined;
  const [requestedSettingsPage, setPage] = useState<SettingsPageId>(() =>
    requestedPage(
      initialPage ?? restoredMobilePage ?? (hasHomePage ? "home" : undefined),
      pages,
      "input",
    ),
  );
  // 桌面上并进别的页的页面（`desktopMergedPages`，例如「帮助与反馈」并进「关于」）照样可以路由打开，显示的是承载它的那一页。
  const page = mobilePlatform ? requestedSettingsPage : desktopPage(requestedSettingsPage);
  const [accountLoginReturnPage, setAccountLoginReturnPage] = useState<SettingsPageId | null>(null);
  // 源应用里每个底部标签页各有一个导航栈，这个共享页面只有一条扁平路由，所以为每个标签页记住它显示着的那一页：从「输入」去「统计」再回到「设置」，要回到「输入」，而不是把第一个标签页重置为首页。
  const mobileInitialTab = mobileTabForPage(page);
  const mobileLastPageByTab = useRef<Record<MobilePrimaryPageId, SettingsPageId>>({
    home: mobileInitialTab === "home" ? page : "home",
    community: mobileInitialTab === "community" ? page : "community",
    "typing-statistics": mobileInitialTab === "typing-statistics" ? page : "typing-statistics",
    account: mobileInitialTab === "account" ? page : "account",
  });
  const settingsContentRef = useSettingsContentScrollReset(page);
  const [communityDestination, setCommunityDestination] = useState<
    AccountCommunityDestination | "all"
  >("all");
  const currentAppVersion = useAppVersion({
    readAppVersion: client.readAppVersion,
    fallbackVersion: fallbackAppVersion,
  });
  const customHelpcodeSchemas = useCustomHelpcodeSchemas(client.listHelpcodeSchemas);
  const helpcodePacks = useHelpcodePacks(client.plugins?.catalog, page === "input");
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
  const mounted = useMountedRef();
  const windowMaximized = useWindowState({ client, setError });
  const [skinPreviewThemes, setSkinPreviewThemes] = useState<
    Partial<Record<GlobalTheme, "light" | "dark">>
  >({});
  const [showTouchSkinEditor, setShowTouchSkinEditor] = useState(false);
  const {
    onPointerDown: beginTouchGeometryDrag,
    onPointerMove: updateTouchGeometryDrag,
    onPointerUp: endTouchGeometryDrag,
  } = useTouchKeyboardGeometryDrag(draft, setDraft);
  const {
    providerCredentials,
    aiCredentialGeneration,
    aiCredentialGenerationRef,
    aiCredentialInput,
    setAiCredentialInput,
    tencentCredentialInput,
    setTencentCredentialInput,
    updateTencentCredentialInput,
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
    fallbackPlatform: androidPlatform ? "android" : iosPlatform ? "ios" : "desktop",
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
    copyReport: copyFeedbackReport,
    submit: submitFeedback,
    copyGroup: copyFeedbackGroup,
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

  const {
    draftRef,
    snapshotRef,
    reload,
    flush: retrySave,
    saveState,
    saveError,
    loadFailed,
  } = useSettingsPersistence({
    client,
    mobile: mobilePlatform,
    mounted,
    snapshot,
    draft,
    setSnapshot,
    setDraft,
    setBusy,
    setError,
    setNotice,
    setRecoveredBackup,
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
    supported: updateCheckSupported,
  } = useUpdateCheck({
    checkUpdate: client.checkUpdate,
    platform: client.host?.platform ?? null,
    edition: client.host?.edition?.id,
    arch: client.host?.arch,
    currentAppVersion,
  });

  const openPanel = useOpenPanel({ setError });

  // Only a failed save leaves changes unsaved for long; 重新读取 then asks before discarding them.
  const dirty = !!draft && !!snapshot && !deepEqual(draft, snapshot.preferences);
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
    credentialGeneration: aiCredentialGeneration,
    credentialGenerationRef: aiCredentialGenerationRef,
    onChange: onAiChange,
  });
  const {
    wordCharacter,
    keybindings,
    frequency,
    mixedInput,
    fuzzyPinyin,
    localModes,
    candidateEnglishGloss,
    englishSuggestions,
    inputModeHUD,
  } = settingsInputPreferences(draft, client.host?.platform);
  const touchKeyboardSchemes = draft?.touch_keyboard_schemes ?? {
    enabled: defaultTouchKeyboardSchemes,
  };
  const {
    selected: selectedTouchKeyboardScheme,
    select: selectTouchKeyboardScheme,
    selectHome: selectHomeScheme,
    setEnabled: setTouchKeyboardSchemeEnabled,
  } = useTouchKeyboardSchemeSelection({
    draft,
    setDraft,
    handwritingScheme: client.host?.edition?.default_scheme,
  });
  // 每个平台都显示全部快捷模式的开关。macOS 以前以发布包只带 msime-pinyin.db 和 msime-english.db 为由隐藏 Emoji、颜文字和临时日语，但 msime-others.db 早已在 resources/desktop-dictionary.lock.json 里并随包发布，隐藏开关只是藏起了能用的功能；同样依赖 msime-english.db 的临时英文却一直显示，前后并不一致。
  //
  // 现在 macOS 发布包不再内置 msime-japanese.dat，改为按需下载（输入页「临时日语」开关下方提供下载）。缺资源的情况仍由运行时处理，而且比隐藏开关处理得更好：资源不在时运行时关闭对应模式（临时日语在日文词库下载前不可用），触发键照常输入大写字母而不是被吞掉。
  const clipboardHistory = clipboardHistoryEnabled(iosPlatform, draft);
  const toggleClipboardHistory = useClipboardHistoryToggle({
    draft,
    enabled: clipboardHistory,
    clear: client.clipboard?.clear,
    setDraft,
    setError,
  });
  const diagnosticLog = diagnosticLogPreferences(draft);
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
    windows: windowsPlatform,
    harmony: harmonyPlatform,
    candidateEnglishGlossAvailable: Boolean(client.candidateEnglishGloss),
    onDeviceDownloadable,
    onChange: onTranslationChange,
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
    windows: windowsPlatform,
    nativeVoicePlatform,
    localModelsAvailable: client.localVoiceModels !== undefined,
    onChange: onVoiceChange,
  });
  const providerPresetControls = createProviderPresetControl(client.openExternalUrl);
  const floatingToolbar = floatingToolbarPreferences(draft);
  const { themeMode, settingsTheme, globalTheme, customColors, customTouchKeyboardSkin } =
    settingsThemePreferences(draft, host?.platform, surfaceThemesFollow);
  // 预览画的自定义主题只含宿主会读的槽位：桌面宿主不读编号色和边框色，存下的旧值不能在预览里画出来。只给预览用，不写回草稿。
  const previewCustomTheme = hostDrawnCustomTheme(
    draft?.custom_theme,
    showCandidateNumberColor,
    showCandidateBorderColor,
  );
  const dark = useSettingsTheme(themeMode, settingsTheme);
  // 两种 HarmonyOS 外观都用用户的应用主题（水杉四季或某个固定季节）着色；每种外观取哪些 token 由 `appThemeStyle` 决定。
  const resolvedAppTheme = useResolvedAppTheme(
    harmonyPlatform || settingsPlatform === "hm2" ? client.appTheme : undefined,
    dark,
  );
  useHarmonySystemBars(
    settingsPlatform === "harmony" ? client.chrome : undefined,
    resolvedAppTheme,
    dark,
  );
  const candidatePreviewTheme = useCandidatePreviewTheme(themeMode, draft?.candidate_theme);
  // 各界面一律跟随颜色模式的宿主不读工具栏和屏幕键盘的单独明暗，预览也照颜色模式画。
  const toolbarPreviewTheme = useCandidatePreviewTheme(
    themeMode,
    surfaceThemesFollow ? undefined : draft?.toolbar_theme,
  );
  const keyboardPreviewTheme = useCandidatePreviewTheme(
    themeMode,
    surfaceThemesFollow ? undefined : draft?.screen_keyboard_theme,
  );
  // A picker colour is part of the custom theme, so choosing one selects that theme; clearing one leaves the selection alone. Choosing one while another theme is selected customizes that theme: it becomes the custom theme's base, and a package, whose own base would replace it, is dropped.
  // A custom theme without a keyboard design draws its base's keyboard, so the custom keyboard card is selected only when the theme carries one.
  const customKeyboardSelected = globalTheme === "custom" && Boolean(draft?.custom_theme?.keyboard);
  useEffect(() => setSkinPreviewThemes({}), [candidatePreviewTheme]);
  const {
    keySpacingTenths: touchKeySpacingTenths,
    rowSpacingTenths: touchRowSpacingTenths,
    heightAdjustment: touchKeyboardHeightAdjustment,
  } = touchKeyboardGeometryPreferences(draft);
  const installerTrust = availableUpdate
    ? describeInstallerTrust(
        availableUpdate,
        client.host?.platform ?? null,
        client.host?.edition?.id,
      )
    : null;
  // Helper codes are per-host rather than per-form-factor. The Android keyboard sends them: Shift during a quanpin or shuangpin composition passes the next letter to the Engine as a helper code, and the Engine reads the schema and the candidate-row hint from these very preferences. Hiding the group left that shipping feature with no way to pick a schema or turn it off. The iOS keyboard extension marks a helper code the same way, and HarmonyOS ships the same input (its ChineseHelpcodePolicy is the Android one, ported), so on a mobile host the group follows the host's `helpcode_shift_entry`.
  // 只有五笔的版本里辅助码没有用处（五笔不用辅助码），不显示这一组；模糊音照常显示，五笔混拼查全拼时会用到。
  const showHelpcode =
    (!mobilePlatform || showHelpcodeShiftEntry) && editionUsesHelpcode(client.host?.edition);
  // 维护与诊断页收纳输入法服务（重启，macOS 上是修复输入源）、诊断日志、数据目录、本地 MCP 服务的入口和 macOS 的卸载；这些一样都没有的宿主不显示这一页。
  const showDeveloperPage =
    Boolean(client.mcpServerStatus) ||
    Boolean(showRestartInputMethod) ||
    !client.host ||
    linuxPlatform ||
    windowsPlatform ||
    macosPlatform;
  // 手机上不提供实体键盘快捷键页和桌面悬浮工具栏。HarmonyOS 在 2-in-1 上把这些控件放在输入法面板里，手机的面板仍是触屏键盘，设置里不能把电脑按键的说明带进手机的列表。
  //
  // 快捷键页以前只在宿主一个快捷键都不路由时隐藏，好让接了实体键盘的手机用户改得到宿主读取的开关（Android 曾因按平台隐藏而改不到）。现在手机上一律不提供：接实体键盘的手机用户很少，这一页对其余用户是噪音；宿主照旧按默认值处理这些按键。
  const mobileHiddenPageIds: readonly SettingsPageId[] = [
    "shortcuts",
    "floating-toolbar",
    // Sound packs, music and the / and @ modes all belong to a physical keyboard and a desktop candidate window; a phone keeps its own keyboard feedback settings.
    "plugins",
  ];
  const {
    availablePages,
    sidebarGroups,
    mobilePrimaryPages,
    mobileSecondaryGroups,
    pageAliases,
    handwritingOnScreenKeyboardPage,
  } = settingsPageProjections({
    mobilePlatform,
    hasHomePage,
    hasTypingStatistics: Boolean(client.typingStatistics),
    hasVocabularyReview: Boolean(client.vocabularyReview),
    hasAccount: Boolean(client.account || client.appIcon),
    hasChat: Boolean(client.chat),
    hasCommunity: Boolean(client.communitySkins || client.communityResources),
    showFloatingToolbar,
    showDeveloperPage,
    // A host with a pack store, or one that plays or routes something the page switches.
    hasPlugins:
      !mobilePlatform &&
      (Boolean(client.plugins) ||
        Boolean(client.communityPlugins) ||
        showKeySound ||
        showMusic ||
        showPluginTriggers ||
        showTypingEffects),
    // full 不带版本信息，手写一直都在。
    hasHandwriting: client.host?.edition?.handwriting ?? true,
    // 桌面上的手写页只有一个打开按钮（HarmonyOS 2-in-1 是一段说明），并进屏幕键盘页；Windows 的原生设置窗口也并成了一页（`SettingsNavigation.h`）。触屏宿主的手写页另有内容，投影本身就不并。
    handwritingOnScreenKeyboardPage:
      macosPlatform || linuxPlatform || windowsPlatform || harmonyPlatform,
    hasMcp: Boolean(client.mcpServerStatus),
    mobileHiddenPageIds,
    mobilePageTitle,
    desktopPageTitle: (id, title) => desktopPageTitle(id, title, client.host?.platform),
  });
  // 子页面在导航里点亮它的上级页面，并提供回到上级的入口。触屏宿主上「AI 对话」和桌面一样是「AI 辅助」的子页面；底部标签页（设置、社区、统计、我的）的根页面没有上级。
  // 桌面上子页的上级如果并进了别的页（「帮助」的上级「帮助与反馈」在「关于」里），点亮的是承载它的那一页。
  const navigationPage: SettingsPageId =
    mobilePlatform && mobilePrimaryPageIds.includes(page as MobilePrimaryPageId)
      ? page
      : mobilePlatform
        ? (subPageParents[page] ?? page)
        : desktopPage(subPageParents[page] ?? page);
  const { mobileActiveTab, selectPage, selectMobileTab, openAccountLogin, finishAccountLogin } =
    useSettingsNavigation({
      mobilePlatform,
      mobileHiddenPageIds,
      availablePages,
      pageAliases,
      page,
      setPage,
      mobileLastPageByTab,
      route,
      hasHomePage,
      setCommunityDestination,
      setAccountLoginReturnPage,
      accountLoginReturnPage,
    });
  // HarmonyOS 手机设计稿给每个页面都加大标题（包括 设置、统计、我的）；其他触屏宿主的三个标签页一打开就是已经表明自身名称的内容。
  const untitledOnPhone: readonly SettingsPageId[] =
    settingsPlatform === "harmony" ? [] : ["home", "typing-statistics", "account"];
  // 设计稿里从一个页面内部打开另一个页面的行，例如「词库」上的「背单词」。
  const pageEntry = (id: SettingsPageId) => availablePages.find((item) => item.id === id);
  const {
    communityGallery,
    openCommunityGallery: openCommunityGallerySubview,
    closeCommunityGallery,
  } = useCommunityGallerySubview({
    mobile: mobilePlatform,
    page,
    selectPage,
    hasSkins: Boolean(client.communitySkins),
  });
  // 触屏「社区」标签根页顶部的「皮肤 / 词库」分类，只给宿主有社区服务的那几类；只有一类时不画分类标签。选择放在页面模型里而不是社区页里：功能页上的「浏览更多」要从外面切到对应分类，离开「社区」标签再回来也还是上次的分类。
  const communityTabChoices = communityTabCategories.filter((tab) =>
    tab.id === "skin" ? Boolean(client.communitySkins) : Boolean(client.communityResources),
  );
  const communityTabs = useSettingsTabs(
    communityTabChoices.length > 0 ? communityTabChoices : communityTabCategories,
    "社区分类",
    mobilePlatform && communityTabChoices.length > 1,
  );
  const communityTabShown =
    mobilePlatform && availablePages.some((item) => item.id === "community");
  // 浏览整类皮肤或词库的入口（皮肤页的「在线皮肤」、词库页的「浏览更多词库」等）切到「社区」标签的对应分类，不再在功能页上盖一层图库；「我的」里的作品与收藏、回复模板仍在功能页上打开完整图库。
  const openCommunityGallery = (view: CommunityGalleryView) => {
    const category =
      view.category === "skin" && !client.communitySkins ? "dictionary" : view.category;
    if (
      communityTabShown &&
      category !== "reply" &&
      view.scope === "" &&
      !view.initialMine &&
      communityTabChoices.some((tab) => tab.id === category)
    ) {
      communityTabs.select(category);
      selectPage("community");
      return;
    }
    openCommunityGallerySubview(view);
  };
  const { openCommunity, openLocalDesigns } = useSettingsDestinationActions({
    selectPage,
    setShowTouchSkinEditor,
    setCommunityDestination,
    // 触屏宿主去社区的入口按类别去「社区」标签，或在功能页里打开完整图库（见上面的 `openCommunityGallery`）；桌面去「社区」页。
    openCommunityGallery: mobilePlatform ? openCommunityGallery : undefined,
  });
  const {
    category: initialCommunityCategory,
    scope: initialCommunityScope,
    initialMine: initialCommunityMine,
  } = communityDestinationView(communityDestination);
  return {
    client,
    confirmation,
    confirm,
    linuxPlatform,
    androidPlatform,
    iosPlatform,
    harmonyPlatform,
    mobilePlatform,
    windowsPlatform,
    macosPlatform,
    settingsPlatform,
    dark,
    resolvedAppTheme,
    nativeVoicePlatform,
    host,
    showModeScope,
    showModeSwitchShortcuts,
    showPanelShortcuts,
    showNumberRowSelection,
    showRestartInputMethod,
    showInstallInputSource,
    showFloatingToolbar,
    showToolbarAppearance,
    showToolbarComponents,
    showTouchKeyboardHeight,
    showTouchKeyboardSpacing,
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showEnglishSuggestions,
    showHelpcodeShiftEntry,
    showHelpcode,
    customHelpcodeSchemas,
    helpcodePacks,
    showShuangpinPreedit,
    showCharacterWidth,
    showVoiceCommitMode,
    showVoiceProviderSettings,
    showVoiceStreamPreedit,
    showVoiceSpaceLockSwitch,
    showVoiceCueSwitches,
    showVoiceCaptureBackends,
    showCandidateRowColors,
    showCandidateSelectionAppearance,
    showCandidateBorderColor,
    showCandidateNumberColor,
    surfaceThemesFollow,
    showCandidateFollowCursor,
    showCandidateWindowScale,
    showCandidateWindowOpacity,
    showCandidateCornerRadius,
    showCandidatePadding,
    showInputModeHUD,
    showInputModeStatusItem,
    showVoiceCaptureDevices,
    showFullwidthChord,
    showTabPagePagingSwitches,
    showMixedInputMinimumPrefix,
    fullwidthChord,
    clientHostedPlatform,
    platformLicenseUrl,
    platformReleasesPageUrl,
    desktopDownloadUrl,
    platformIssuesUrl,
    captureBackendOptions,
    platformHelpIntro,
    platformQuickStart,
    platformNetworkDescription,
    platformAboutDescription,
    desktopPanels,
    showVoiceHotkeys,
    showKeySound,
    showMusic,
    showPluginTriggers,
    showTypingEffects,
    showTypingEffectStyles,
    showTypingEffectPacks,
    showEffectIntensity,
    showAiCandidateLimit,
    typingEffectsFlashOnly,
    showWordbookPacks,
    showSymbolSetPacks,
    showScriptPlugins,
    snapshot,
    draft,
    setDraft,
    busy,
    setError,
    error,
    notice,
    recoveredBackup,
    removeUserDataOnUninstall,
    setRemoveUserDataOnUninstall,
    uninstallConfirmation,
    uninstallBusy,
    uninstallResult,
    requestUninstall,
    confirmUninstall,
    cancelUninstall,
    dataDirectory,
    dataDirectoryBusy,
    dataDirectoryResult,
    inputSourceStartup,
    dismissInputSourceStartup,
    page,
    accountLoginReturnPage,
    settingsContentRef,
    communityDestination,
    updateStatus,
    updateBusy,
    updateCheckSupported,
    availableUpdate,
    currentAppVersion,
    copyFeedbackGroup,
    feedbackCopied,
    feedbackKind,
    setFeedbackKind,
    feedbackDetail,
    setFeedbackDetail,
    copyFeedbackReport,
    feedbackReportCopied,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    macosShuangpinKeymap,
    showShuangpinKeymapHint,
    setPhrases,
    phrases,
    phrasePage,
    phraseBusy,
    phraseError,
    dictionaryPendingCount,
    dictionaryFailures,
    dictionarySnapshotError,
    phraseNotice,
    phraseSearch,
    setPhraseSearch,
    setPhraseForm,
    phraseForm,
    dictionaryKind,
    setDictionaryKind,
    dictionaryFormat,
    setDictionaryFormat,
    phraseListRef,
    windowMaximized,
    skinPreviewThemes,
    setSkinPreviewThemes,
    showTouchSkinEditor,
    setShowTouchSkinEditor,
    aiModels,
    aiModelsStatus,
    aiModelsBusy,
    aiTestInput,
    setAiTestInput,
    aiTestOutput,
    aiTestStatus,
    aiTestBusy,
    providerCredentials,
    aiCredentialInput,
    setAiCredentialInput,
    tencentCredentialInput,
    setTencentCredentialInput,
    updateTencentCredentialInput,
    voiceCredentialInput,
    setVoiceCredentialInput,
    providerCredentialBusy,
    providerCredentialMessages,
    runVoiceCredential,
    supportDiagnostics: diagnosticsText,
    feedbackReport,
    submitFeedback,
    reload,
    retrySave,
    saveState,
    saveError,
    loadFailed,
    saveMobileKeyboardFeedback,
    chooseDataDirectory,
    previewMobileKeyboardHaptics,
    resetTouchKeyboardSettings,
    beginTouchGeometryDrag,
    updateTouchGeometryDrag,
    endTouchGeometryDrag,
    openExternalUrl,
    openPanel,
    checkForUpdate,
    loadPhrases,
    turnPhrasePage,
    removePhrase,
    savePhrase,
    importPhrases,
    retryDictionaryFailure,
    dismissDictionaryFailure,
    exportPhrases,
    exportAllPhrases,
    restoreDefaults,
    recoverPreferences,
    resetLearnedData,
    dirty,
    ai,
    aiOrigin,
    aiToken,
    storedAiCredential,
    updateAi,
    updateAiToken,
    fetchAiModels,
    testAi,
    wordCharacter,
    keybindings,
    frequency,
    mixedInput,
    fuzzyPinyin,
    touchKeyboardSchemes,
    selectedTouchKeyboardScheme,
    selectTouchKeyboardScheme,
    setTouchKeyboardSchemeEnabled,
    selectHomeScheme,
    localModes,
    clipboardHistory,
    toggleClipboardHistory,
    diagnosticLog,
    candidateTranslations,
    candidateEnglishGloss,
    englishSuggestions,
    candidateGlossLanguagesEnabled,
    translationTargetLanguage,
    translationSecondaryLanguage,
    visibleTranslationLanguages,
    visibleSecondaryLanguages,
    voiceInput,
    systemVoice,
    systemVoiceHostName,
    localVoiceAvailable,
    localVoice,
    serviceVoice,
    harmonyUnsupportedAsr,
    doubaoAuthMode,
    updateVoice,
    customTranslation,
    tencentTranslation,
    niutrans,
    translationProvider,
    onDeviceMissingLanguages,
    setTranslationProvider,
    runProviderCredential,
    credentialTestControl,
    providerPresetControls,
    inputModeHUD,
    floatingToolbar,
    themeMode,
    settingsTheme,
    candidatePreviewTheme,
    toolbarPreviewTheme,
    keyboardPreviewTheme,
    globalTheme,
    customColors,
    previewCustomTheme,
    customTouchKeyboardSkin,
    onCandidateColorChange,
    onCustomKeyboardChange,
    onUseCustomKeyboard,
    customKeyboardSelected,
    touchKeySpacingTenths,
    touchRowSpacingTenths,
    touchKeyboardHeightAdjustment,
    installerTrust,
    availablePages,
    handwritingOnScreenKeyboardPage,
    sidebarGroups,
    mobileActiveTab,
    untitledOnPhone,
    mobilePrimaryPages,
    mobileSecondaryGroups,
    navigationPage,
    pageEntry,
    selectPage,
    selectMobileTab,
    openAccountLogin,
    finishAccountLogin,
    openLocalDesigns,
    openCommunity,
    initialCommunityCategory,
    initialCommunityScope,
    initialCommunityMine,
    communityGallery,
    openCommunityGallery,
    closeCommunityGallery,
    communityTabs,
  };
}

/** 设计稿的 toast 只属于 HarmonyOS 外观。其他平台的 `useToast` 保持它在 provider 之外时的空操作，不会多出第二个常驻的状态区域。 */
function ShellToasts({ enabled, children }: { enabled: boolean; children: ReactNode }) {
  return enabled ? <ToastProvider>{children}</ToastProvider> : children;
}

/** What `SettingsPage` computes for its shell and page components. */
export type SettingsPageModel = ReturnType<typeof useSettingsPageModel>;

export function SettingsPage(props: SettingsPageProps) {
  // 宿主自己的屏幕键盘布局：没写 `layout` 的键盘预览按它画，手机上不画桌面键盘。
  return (
    <ScreenKeyboardLayoutContext.Provider
      value={screenKeyboardLayoutFor(props.client.host?.platform)}
    >
      <SettingsShell {...props} />
    </ScreenKeyboardLayoutContext.Provider>
  );
}

function SettingsShell(props: SettingsPageProps) {
  const { onReplayOnboarding } = props;
  const model = useSettingsPageModel(props);
  // The 插件 page shows either the installed packs (a page of the settings form) or the community gallery, which has its own search form and so is drawn outside the settings one.
  const [pluginView, setPluginView] = useState<"mine" | "community">("mine");
  // 主题 does the same on desktop: the installed candidate-window skins with the theme settings, or the community gallery of them.
  const [skinView, setSkinView] = useState<"mine" | "community">("mine");
  // Filters the sidebar by page name; the model does not need it, since it never leaves the shell.
  const [navQuery, setNavQuery] = useState("");
  // The phone page that has scrolled its large title away, which brings in the compact bar. Keyed by page so that arriving on another page, which opens at its top, never inherits the bar.
  const [titleCollapsedOn, setTitleCollapsedOn] = useState<string | null>(null);
  const {
    client,
    confirmation,
    confirm,
    androidPlatform,
    iosPlatform,
    harmonyPlatform,
    mobilePlatform,
    macosPlatform,
    settingsPlatform,
    dark,
    resolvedAppTheme,
    draft,
    setDraft,
    busy,
    setError,
    error,
    notice,
    recoveredBackup,
    inputSourceStartup,
    dismissInputSourceStartup,
    page,
    accountLoginReturnPage,
    settingsContentRef,
    communityDestination,
    windowMaximized,
    reload,
    retrySave,
    saveState,
    saveError,
    loadFailed,
    openExternalUrl,
    platformIssuesUrl,
    openPanel,
    restoreDefaults,
    recoverPreferences,
    dirty,
    keyboardPreviewTheme,
    surfaceThemesFollow,
    availablePages,
    handwritingOnScreenKeyboardPage,
    sidebarGroups,
    mobileActiveTab,
    untitledOnPhone,
    mobilePrimaryPages,
    mobileSecondaryGroups,
    navigationPage,
    selectPage,
    selectMobileTab,
    openAccountLogin,
    finishAccountLogin,
    openLocalDesigns,
    openCommunity,
    initialCommunityCategory,
    initialCommunityScope,
    initialCommunityMine,
    communityGallery,
    closeCommunityGallery,
    communityTabs,
    currentAppVersion,
  } = model;
  // 对话页第一次打开后就一直挂着（见下方渲染处），切走只隐藏。
  const [chatMounted, setChatMounted] = useState(page === "chat");
  // 首页「试用键盘」打开对话页时加一，对话页随即聚焦输入框、弹出键盘；从「AI 辅助」进入不加，不弹键盘。
  const [chatFocusRequest, setChatFocusRequest] = useState(0);
  useEffect(() => {
    if (page === "chat") setChatMounted(true);
  }, [page]);
  // 任一种 HarmonyOS 外观（手机或 2-in-1）：两者都用应用主题着色，并带有设计稿的 toast。
  const harmonyLook = settingsPlatform === "harmony" || settingsPlatform === "hm2";
  // HarmonyOS 手机外观：推入标签页的每个页面都带返回按钮并会滑入，另外三个标签页使用更紧凑的标签页列表。
  const harmonyPhone = settingsPlatform === "harmony" && mobilePlatform;
  const tabRootPage = mobilePrimaryPageIds.includes(page as MobilePrimaryPageId);
  const pushedPage = harmonyPhone && !tabRootPage;
  // 「试用键盘」是一页对话：对话区在页面内滚动，输入栏贴着内容区底部，和 Android 的试用页一样不带底部标签栏，键盘弹起时输入栏直接落在键盘上方。
  const fillsContent = mobilePlatform && page === "try-keyboard";
  // 触屏上的 AI 对话同样是一页对话，内容区撑满、输入栏贴底，但它是「AI 辅助」的二级页，底部标签栏照常显示。
  const contentFills = fillsContent || (mobilePlatform && page === "chat");
  const pageColumnRef = useRef<HTMLDivElement>(null);
  const launchPage = useRef(page);
  // 从一个推入页面再推入的页面（反馈 到 帮助）会保留内容列的动画类，所以要重启动画，而不是让它停在已结束的状态。用重启动画而不是按页面给内容列设 key，能让表单页面像在其他宿主上一样保持挂载、保留状态。
  useLayoutEffect(() => {
    const column = pageColumnRef.current;
    if (!pushedPage || !column || typeof column.getAnimations !== "function") return;
    for (const animation of column.getAnimations()) {
      animation.cancel();
      animation.play();
    }
  }, [page, pushedPage]);
  const reloadSettings = createSettingsReloadAction({ dirty, reload, confirm });
  // 社区图库「添加」词库的去处：宿主有命名词库时装成一个独立的词库，否则导入本机词库。「社区」页和功能页里的图库共用。
  const communityLocalDictionary = client.dictionaryCollections
    ? {
        ...client.dictionary,
        installCollection: async (resource: CommunityResource) => {
          await client.dictionaryCollections?.installCommunity(resource);
        },
      }
    : client.dictionary;
  const recordCommunitySkin = (id: string) => {
    void client.typingStatistics?.recordSkin?.(id);
  };
  // HarmonyOS 手机的皮肤页与 Android 皮肤页一样在顶部分「内置 / 我的 / 社区」。「社区」是完整的社区皮肤图库，自带搜索和发布表单，所以和桌面的「社区皮肤」一样画在设置表单之外，标签的选择也就放在这里；前两个分类的网格由皮肤页自己画。没有社区服务的宿主不给「社区」。
  const skinTabs = useSettingsTabs(
    skinGridTabs.filter((tab) => tab.id !== "community" || Boolean(client.communitySkins)),
    "皮肤分类",
    harmonyPlatform && mobilePlatform,
  );
  const phoneSkinCommunityShown =
    page === "skin" && Boolean(client.communitySkins) && skinTabs.selected === "community";
  const { onOpenPage } = createSettingsPageSelection({ selectPage });
  const statusActions = createSettingsStatusActions({
    recoverPreferences,
    inputSourceStartup: client.inputSourceStartup,
    dismissInputSourceStartup,
  });
  const externalActions = createSettingsExternalActions({
    mobile: mobilePlatform,
    canOpenExternalUrl: Boolean(client.openExternalUrl),
    openExternalUrl,
    issuesUrl: platformIssuesUrl,
    openSystemKeyboardSettings: client.openSystemKeyboardSettings,
  });
  const { onOpenChat, onRestoreDefaults } = createSettingsNavigationActions({
    selectPage,
    chatAvailable: Boolean(client.chat),
    openPanel,
    openHandwriting: client.openHandwriting,
    restoreDefaults,
  });
  const winShell = settingsPlatform === "win";
  const macShell = settingsPlatform === "mac";
  const linuxShell = settingsPlatform === "linux";
  const ipadShell = settingsPlatform === "ipad";
  const { onWindowPointerDownCapture, windowDragHandlers, keepPointer } =
    useSettingsWindowInteractions({
      windowControl: client.windowControl,
      beginWindowDrag: client.beginWindowDrag,
      resizeWindow: client.resizeWindow,
      windowMaximized,
      macShell,
      onError: setError,
    });
  // An iPad shows the settings split only on the 设置 tab; the other three tabs take the whole width.
  const ipadSidebarShown = mobileActiveTab === "home";
  // macOS keeps its native traffic lights over the page (an overlay title bar), and a phone's frame belongs to the OS, so only the platforms that draw their own caption get one. The host still exposes the window commands on mobile because the same Tauri app binary backs both, so the presence of a command is not the question -- the platform is.
  const titlebarShown =
    !mobilePlatform && !macShell && Boolean(client.windowControl || client.beginWindowDrag);
  const pageTitle = availablePages.find((item) => item.id === page)?.title ?? "输入";
  // A community gallery stands in for the page's own settings, so the form's page-level actions do not apply while it is shown.
  const skinCommunityShown =
    (page === "skin" && Boolean(client.communityCandidateSkins) && skinView === "community") ||
    phoneSkinCommunityShown;
  const pluginCommunityShown =
    page === "plugins" && Boolean(client.communityPlugins) && pluginView === "community";
  // 子页面（「词库」下的「背单词」、「帮助与反馈」下的「帮助」、「AI 辅助」下的「AI 对话」）在返回时写出父页面的名字。
  const parentPage =
    navigationPage !== page ? availablePages.find((item) => item.id === navigationPage) : undefined;
  // HarmonyOS 手机和边缘滑动一样，沿导航推入的 WebView 历史返回。设置打开时所在的页面身后没有设置的历史条目（最多只有宿主自己的，比如引导页的），所以从那里按钮改为向上，以替换当前条目的方式去到上级页面或所在标签页的根页面：如果推入新条目，刚离开的页面会留在上级页面后面，系统返回手势会回到它。此后上级页面占据最底部的条目，所以它自己的返回按钮也同样向上。
  const goBack = () => {
    if (page !== launchPage.current) {
      window.history.back();
      return;
    }
    const up = parentPage?.id ?? mobileTabForPage(page);
    launchPage.current = up;
    selectPage(up, true);
  };
  // A phone collapses the large title into a compact bar on the 设置 tab's pages, the way the design does; the other tabs and the untitled pages have no large title to collapse.
  const collapsingTitle =
    mobilePlatform &&
    mobileActiveTab === "home" &&
    !untitledOnPhone.includes(page) &&
    !fillsContent;
  // The design searches in the Windows caption and at the top of the macOS and HarmonyOS 2-in-1 sidebars; GNOME has none. A Windows window without a caption (a browser preview) keeps the search in the sidebar so the filter is still reachable.
  const searchInTitlebar = winShell && titlebarShown;
  const searchInSidebar =
    macShell || settingsPlatform === "hm2" || ipadShell || (winShell && !titlebarShown);
  const sidebarBrandShown = !mobilePlatform && !titlebarShown;
  const navNeedle = navQuery.trim().toLocaleLowerCase();
  // iPad 的标签栏已有社区、统计和我的；`settingsPageProjections` 在手机和 iPad 上都不把这几个标签页列进侧栏，这里不必再过滤一次。
  const shownSidebarGroups = navNeedle
    ? sidebarGroups
        .map((group) => ({
          ...group,
          pages: group.pages.filter((item) => item.title.toLocaleLowerCase().includes(navNeedle)),
        }))
        .filter((group) => group.pages.length > 0)
    : sidebarGroups;
  const navSearchField = (
    <>
      <svg
        className={settings.searchGlyph}
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        aria-hidden="true"
      >
        <circle cx="7" cy="7" r="5" />
        <path d="m11 11 3.5 3.5" />
      </svg>
      <input
        type="search"
        className={settings.searchInput}
        aria-label="搜索设置"
        placeholder="搜索"
        value={navQuery}
        onChange={(event) => setNavQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing || event.keyCode === 229) return;
          if (event.key === "Escape") setNavQuery("");
          if (event.key !== "Enter") return;
          const first = shownSidebarGroups[0]?.pages[0];
          if (first) selectPage(first.id);
        }}
      />
    </>
  );
  return (
    <div
      className={settings.shell}
      data-settings-shell=""
      // The platform's accent and the `--p-*` tokens the platform primitives read hang off this attribute.
      data-platform={settingsPlatform}
      // HarmonyOS 应用主题以内联方式在同一个元素上为 token 表重新着色，优先级高于 `[data-platform]` 规则；没有解析出主题时这里为空，沿用 token 表自己的颜色。
      style={harmonyLook ? appThemeStyle(resolvedAppTheme, dark, settingsPlatform) : undefined}
      data-season={seasonAttr(resolvedAppTheme)}
      // Marks the phone navigation. It no longer carries a palette: the `[data-platform]` rules in styles.css cover the phone hosts too.
      data-mobile={mobilePlatform ? "" : undefined}
      onPointerDownCapture={onWindowPointerDownCapture}
    >
      <ShellToasts enabled={harmonyLook}>
        {confirmation}
        <KeyboardSwitchGuideHost setup={client.home?.setup} />
        {titlebarShown && (
          <WindowTitlebar
            linux={linuxShell}
            logo={logo}
            pageTitle={pageTitle}
            search={searchInTitlebar ? navSearchField : undefined}
            maximized={windowMaximized}
            windowControl={client.windowControl}
            dragHandlers={windowDragHandlers}
            keepPointer={keepPointer}
          />
        )}
        <div className={settings.body} data-settings-body="">
          {/* A bottom tab bar. `order-2` seats it below the content while the DOM keeps it ahead, so assistive technology and keyboard focus still reach the navigation first, and the bottom padding clears the gesture inset. Hidden above phone width, where the sidebar serves. */}
          {mobilePlatform && !fillsContent && (
            <MobileSettingsTabs
              tabs={mobilePrimaryPages}
              activeTab={mobileActiveTab}
              onSelect={selectMobileTab}
              platform={settingsPlatform}
            />
          )}
          <nav
            className={`${settings.sidebar} ${ipadSidebarShown ? "" : "ipad:hidden"}`}
            aria-label="设置分类"
          >
            {macShell && (
              <div className={settings.macosDragRow} data-window-drag="" {...windowDragHandlers} />
            )}
            {ipadShell && <h2 className={settings.sidebarTitle}>设置</h2>}
            {/* 品牌（图标加名称）放在侧栏顶部、搜索框之上，只在没有自绘标题栏的桌面窗口出现：macOS 排在红绿灯那一行下面，并和那一行一样可以拖动窗口；HarmonyOS 2in1 与没有窗口命令的桌面窗口直接起头。Windows 和 Linux 的标题栏已经写着品牌，手机和 iPad 的界面由系统和标签栏承担，都不再画一份。图标是装饰，名称就是文字本身。 */}
            {sidebarBrandShown && (
              <div
                className={settings.sidebarHeader}
                data-sidebar-brand=""
                data-window-drag={macShell ? "" : undefined}
                {...(macShell ? windowDragHandlers : {})}
              >
                <img src={logo} alt="" draggable={false} />
                <span>水杉输入法</span>
              </div>
            )}
            {searchInSidebar && <label className={settings.sidebarSearch}>{navSearchField}</label>}
            {shownSidebarGroups.map((group, index) => (
              <div
                key={group.pages[0].id}
                className={settings.sidebarSection(index === 0)}
                data-sidebar-section=""
                role={group.title ? "group" : undefined}
                aria-label={group.title}
              >
                {group.title && (
                  <div className={settings.sidebarGroupTitle} aria-hidden="true">
                    {group.title}
                  </div>
                )}
                {group.pages.map((item) => (
                  <NavItem
                    key={item.id}
                    label={item.title}
                    icon={<img className={settings.sidebarGlyph} src={item.icon} alt="" />}
                    selected={navigationPage === item.id}
                    controls="settings-content"
                    onSelect={() => selectPage(item.id)}
                  />
                ))}
              </div>
            ))}
            {shownSidebarGroups.length === 0 && (
              <p className={settings.sidebarEmpty} role="status">
                没有匹配的设置
              </p>
            )}
            <p className={settings.previewLabel}>客户端预览版</p>
          </nav>
          <main
            ref={settingsContentRef}
            id="settings-content"
            className={`${settings.content} ${ipadSidebarShown ? "" : "ipad:col-span-2"} ${contentFills ? "flex flex-col" : ""}`}
            aria-labelledby="page-title"
            onScroll={
              collapsingTitle
                ? (event) => setTitleCollapsedOn(event.currentTarget.scrollTop > 28 ? page : null)
                : undefined
            }
          >
            {collapsingTitle && (
              // The large title's compact stand-in once it scrolls away. The title is decorative: the `h1` below still names the page. The back button is not: the large header's own scrolls away with the title, and a pushed page that has been scrolled had no way back but the system gesture.
              <div className={settings.collapsedTitle(titleCollapsedOn === page)}>
                {pushedPage && titleCollapsedOn === page && (
                  <button
                    type="button"
                    className={settings.collapsedBackButton}
                    aria-label="返回"
                    onClick={goBack}
                  >
                    <FluentIcon name="arrow_left" size={22} />
                  </button>
                )}
                <span aria-hidden="true">{pageTitle}</span>
              </div>
            )}
            {macShell && (
              <header className={settings.macosToolbar} data-window-drag="" {...windowDragHandlers}>
                <h1 id="page-title">{pageTitle}</h1>
              </header>
            )}
            <div
              ref={pageColumnRef}
              className={`${settings.contentColumn} ${pushedPage ? "animate-ms-push-in motion-reduce:animate-none" : ""} ${contentFills ? "flex min-h-0 flex-1 flex-col" : ""}`}
              // 统计 和 我的 使用更紧凑的标签页列表，样式表依据这个属性区分。社区虽是标签页的根页，内容是和皮肤页「社区」分类同样的图库而不是设置列表，保持图库原来的版式。
              data-tab-page={
                harmonyPhone && tabRootPage && page !== "home" && page !== "community"
                  ? ""
                  : undefined
              }
            >
              {/* 三个标签页一打开都是已经表明自身的内容（标题行、资料卡、一排数字），源应用不在它们上面印页面标题。标题是隐藏而不是去掉：它为 `main` 提供名称。 */}
              {parentPage && !harmonyPhone && (
                <button
                  type="button"
                  className={settings.backLink}
                  // Named for where it goes, so it is not a second button called just 反馈 next to the sidebar's.
                  aria-label={`返回${parentPage.title}`}
                  onClick={() => selectPage(parentPage.id)}
                >
                  <span aria-hidden="true">‹ </span>
                  {parentPage.title}
                </button>
              )}
              {!macShell && (
                <SettingsPageHeader
                  title={pageTitle}
                  hiddenOnPhone={mobilePlatform && untitledOnPhone.includes(page)}
                  onBack={pushedPage ? goBack : undefined}
                  home={page === "home"}
                />
              )}
              {/* 2-in-1 的根页面上没有设置引导卡片，所以输入法还需要添加或设为默认时，由 输入 和 关于 提示。 */}
              {settingsPlatform === "hm2" &&
                client.home &&
                (page === "input" || page === "about") && (
                  <SetupWarningStrip actions={client.home} />
                )}
              <SettingsPageStatus
                error={error}
                notice={notice}
                busy={busy}
                recoveredBackup={recoveredBackup}
                canRecover={Boolean(client.recoverPreferences)}
                onRecover={statusActions.onRecover}
                openPreferencesDirectory={client.openPreferencesDirectory}
                macos={macosPlatform}
                onError={setError}
                draft={draft}
                inputSourceStartup={inputSourceStartup}
                onOpenSettings={statusActions.onOpenSettings}
                onDismiss={statusActions.onDismiss}
                saveError={
                  saveState === "failed" && draft && isSettingsFormPage(page) ? saveError : ""
                }
                onRetrySave={() => void retrySave()}
              />
              {client.notices && (
                <NoticeBanner client={client.notices} openExternalUrl={openExternalUrl} />
              )}
              {client.home && draft && page === "home" && (
                <HomePage
                  preferences={draft}
                  actions={client.home}
                  onOpenPage={onOpenPage}
                  onOpenChat={
                    onOpenChat &&
                    (() => {
                      setChatFocusRequest((count) => count + 1);
                      onOpenChat();
                    })
                  }
                  touchLayout={mobilePlatform}
                  ios={iosPlatform}
                  keyboardSurfaceTheme={
                    surfaceThemesFollow ? undefined : draft.screen_keyboard_theme
                  }
                  rootPages={mobileSecondaryGroups
                    .flatMap((group) => group.pages)
                    .map((item) => ({ id: item.id, title: item.title }))}
                />
              )}
              {client.home && page === "try-keyboard" && (
                <TryKeyboardPage
                  actions={client.home}
                  switchAway={!harmonyPlatform}
                  chat={client.chat}
                  onLogin={openAccountLogin}
                  onOpenUrl={
                    client.openExternalUrl
                      ? (url: string) => {
                          void openExternalUrl(url);
                        }
                      : undefined
                  }
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
                  onOpenLocalDesigns={
                    client.customTouchKeyboardSkins ? openLocalDesigns : undefined
                  }
                  // Windows 的社区页只有词包与回复模板，没有皮肤画廊；它的候选窗口皮肤社区在主题页。
                  onOpenCommunity={client.communityResources ? openCommunity : undefined}
                  communityHasSkins={Boolean(client.communitySkins)}
                  onOpenCloudDictionary={
                    client.openCloudDictionary
                      ? () => {
                          void client.openCloudDictionary!().catch(() =>
                            setError("无法打开云词库，请重试。"),
                          );
                        }
                      : undefined
                  }
                  // macOS opens the cloud clipboard only from the IME menu, which supplies the input session it pastes into.
                  onOpenCloudClipboard={
                    client.openCloudClipboard && !macosPlatform
                      ? () => {
                          void client.openCloudClipboard!().catch(() =>
                            setError("无法打开云剪贴板，请重试。"),
                          );
                        }
                      : undefined
                  }
                  onOpenAbout={mobilePlatform ? () => selectPage("about") : undefined}
                  onOpenFeedback={
                    mobilePlatform && availablePages.some((item) => item.id === "feedback")
                      ? () => selectPage("feedback")
                      : undefined
                  }
                  onOpenDesktopDownload={
                    mobilePlatform && client.openExternalUrl
                      ? () => {
                          void openExternalUrl(desktopDownloadUrl);
                        }
                      : undefined
                  }
                  onReplayOnboarding={mobilePlatform ? onReplayOnboarding : undefined}
                  preferences={draft}
                  onOpenPage={selectPage}
                  appTheme={client.appTheme}
                  onOpenUrl={
                    client.openExternalUrl
                      ? (url: string) => {
                          void openExternalUrl(url);
                        }
                      : undefined
                  }
                  copyText={client.copyText}
                  desktopDownloadUrl={desktopDownloadUrl}
                  appVersion={currentAppVersion}
                />
              )}
              {/* 触屏上 AI 对话从「AI 辅助」二级页进入：第一次打开后一直挂着，切换标签只是隐藏，对话记录和进行中的回复都不丢。 */}
              {client.chat && (page === "chat" || (mobilePlatform && chatMounted)) && (
                <div className={page === "chat" ? "contents" : undefined} hidden={page !== "chat"}>
                  <ChatPage
                    client={client.chat}
                    touch={mobilePlatform}
                    active={page === "chat"}
                    focusRequest={chatFocusRequest}
                    setup={client.home}
                    onLogin={openAccountLogin}
                  />
                </div>
              )}
              {page === "community" && !mobilePlatform && (
                <CommunityPage
                  destinationKey={communityDestination}
                  skins={client.communitySkins}
                  resources={client.communityResources}
                  theme={keyboardPreviewTheme}
                  initialMine={initialCommunityMine}
                  initialCategory={initialCommunityCategory}
                  initialScope={initialCommunityScope}
                  localDictionary={communityLocalDictionary}
                  localSkinLibrary={client.customSkinLibrary}
                  mobile={mobilePlatform}
                  onLogin={openAccountLogin}
                  preferences={draft}
                  // 从皮肤库应用的皮肤和其他偏好修改一样：先进入草稿，再由表单的自动保存写入。
                  onApplyPreferences={setDraft}
                  onSkinApplied={recordCommunitySkin}
                  look={harmonyPlatform && mobilePlatform ? "harmony" : undefined}
                />
              )}
              {/* 触屏的「社区」标签根页：顶部「皮肤 / 词库」分类，每类是完整的社区图库（搜索、分类、详情、获取或添加、举报、发布都在里面），只挂载选中的那一类。和皮肤页的「社区」分类一样画在设置表单之外：图库自带搜索和发布表单，嵌在设置的 <form> 里会被 WebView 当成外层表单提交。详情的历史记录留在这一页，返回先关详情。 */}
              {page === "community" && mobilePlatform && communityTabs.tabList}
              {page === "community" && mobilePlatform && (
                <div className={communityTabs.tabList ? "mt-6" : undefined}>
                  {communityTabs.selected === "skin" &&
                    client.communitySkins &&
                    communityTabs.panel(
                      "skin",
                      <CommunitySkinsPage
                        client={client.communitySkins}
                        theme={keyboardPreviewTheme}
                        localSkinLibrary={client.customSkinLibrary}
                        mobile
                        onLogin={openAccountLogin}
                        look={harmonyPlatform ? "harmony" : undefined}
                        preferences={draft}
                        // 「使用」和皮肤页的网格一样先进入草稿，再由表单的自动保存写入。
                        onApplyPreferences={setDraft}
                        onSkinApplied={recordCommunitySkin}
                        historyPage="community"
                      />,
                    )}
                  {communityTabs.selected === "dictionary" &&
                    client.communityResources &&
                    communityTabs.panel(
                      "dictionary",
                      <CommunityResourcesPage
                        client={client.communityResources}
                        kind="dictionary"
                        localDictionary={communityLocalDictionary}
                        mobile
                        look={harmonyPlatform ? "harmony" : undefined}
                        historyPage="community"
                      />,
                    )}
                </div>
              )}
              {/* 触屏宿主在功能页里打开的完整社区图库。画在设置表单之外：图库自带搜索和发布表单，嵌在设置的 <form> 里会被 WebView 当成外层表单提交。 */}
              {communityGallery && page === communityGalleryPage(communityGallery.category) && (
                <CommunityGallerySubview
                  key={`${communityGallery.category}:${communityGallery.scope}:${communityGallery.initialMine}`}
                  view={communityGallery}
                  skins={client.communitySkins}
                  resources={client.communityResources}
                  theme={keyboardPreviewTheme}
                  localDictionary={communityLocalDictionary}
                  localSkinLibrary={client.customSkinLibrary}
                  onLogin={openAccountLogin}
                  preferences={draft}
                  onApplyPreferences={setDraft}
                  onSkinApplied={recordCommunitySkin}
                  look={harmonyPlatform && mobilePlatform ? "harmony" : undefined}
                  onClose={closeCommunityGallery}
                />
              )}
              {client.typingStatistics && page === "typing-statistics" && (
                <TypingStatisticsPage
                  client={client.typingStatistics}
                  mobile={mobilePlatform}
                  platform={client.host?.platform}
                  openSystemSettings={client.openSystemKeyboardSettings}
                  look={
                    settingsPlatform === "harmony"
                      ? "harmony"
                      : settingsPlatform === "hm2"
                        ? "hm2"
                        : undefined
                  }
                />
              )}
              {client.vocabularyReview && page === "vocabulary" && (
                <VocabularyReviewPage
                  client={client.vocabularyReview}
                  mobile={mobilePlatform}
                  openPanel={client.openVocabulary}
                />
              )}
              {page === "skin" && client.communityCandidateSkins && (
                <div
                  className={communityStyle.categoryTabsPair}
                  role="tablist"
                  aria-label="皮肤来源"
                >
                  <button
                    type="button"
                    role="tab"
                    aria-selected={skinView === "mine"}
                    onClick={() => setSkinView("mine")}
                  >
                    我的皮肤
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={skinView === "community"}
                    onClick={() => setSkinView("community")}
                  >
                    社区皮肤
                  </button>
                </div>
              )}
              {page === "skin" && skinTabs.tabList}
              {phoneSkinCommunityShown && client.communitySkins && (
                <div className="mt-6">
                  {skinTabs.panel(
                    "community",
                    <CommunitySkinsPage
                      client={client.communitySkins}
                      theme={keyboardPreviewTheme}
                      localSkinLibrary={client.customSkinLibrary}
                      mobile
                      onLogin={openAccountLogin}
                      look="harmony"
                      preferences={draft}
                      // 「使用」和网格里的卡片一样先进入草稿，再由表单的自动保存写入。
                      onApplyPreferences={setDraft}
                      onSkinApplied={recordCommunitySkin}
                      // 详情的历史记录留在皮肤页上，返回先关详情、再离开这一页。
                      historyPage="skin"
                    />,
                  )}
                </div>
              )}
              {skinCommunityShown && client.communityCandidateSkins && (
                <div className="mt-4">
                  <CommunityPage
                    candidateSkins={client.communityCandidateSkins}
                    localSkins={client.scanSkinCatalog}
                    openSkinDirectory={client.openSkinDirectory}
                    readSkinImage={client.readSkinImage}
                    onOpenSkinPage={() => setSkinView("mine")}
                    theme={keyboardPreviewTheme}
                    onLogin={openAccountLogin}
                  />
                </div>
              )}
              {page === "plugins" && client.communityPlugins && (
                <div
                  className={communityStyle.categoryTabsPair}
                  role="tablist"
                  aria-label="插件来源"
                >
                  <button
                    type="button"
                    role="tab"
                    aria-selected={pluginView === "mine"}
                    onClick={() => setPluginView("mine")}
                  >
                    我的插件
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={pluginView === "community"}
                    onClick={() => setPluginView("community")}
                  >
                    社区插件
                  </button>
                </div>
              )}
              {client.communityPlugins && pluginCommunityShown && (
                <div className="mt-4">
                  <CommunityPluginsPage
                    client={client.communityPlugins}
                    localPlugins={client.plugins?.catalog}
                    onLogin={openAccountLogin}
                  />
                </div>
              )}
              {draft && isSettingsFormPage(page) && (
                <SettingsFormFrame showReload={false} busy={busy}>
                  <SettingsFormContext.Provider value={{ ...model, draft }}>
                    <SkinSettingsPage hidden={skinCommunityShown} tabs={skinTabs} />
                    <AppearanceSettingsPage />
                    <FloatingToolbarSettingsPage />
                    <InputSettingsPage />
                    <ExpressionSettingsPage />
                    <AiSettingsPage />
                    <ShortcutSettingsPage />
                    <DictionarySettingsPage />
                    <ScreenKeyboardSettingsPage />
                    <VoiceSettingsPage />
                    {/* 手写并进屏幕键盘页时，那一页自己画手写的分组。 */}
                    {!handwritingOnScreenKeyboardPage && <HandwritingSettingsPage />}
                    <ToolsSettingsPage />
                    <PluginsSettingsPage hidden={pluginCommunityShown} />
                    <DeveloperSettingsPage />
                    <McpSettingsPage />
                    <FeedbackSettingsPage />
                    <HelpSettingsPage
                      busy={busy}
                      hidden={page !== "help"}
                      macos={macosPlatform}
                      mobile={mobilePlatform}
                      ios={iosPlatform}
                      android={androidPlatform}
                      platformHelpIntro={model.platformHelpIntro}
                      platformQuickStart={model.platformQuickStart}
                      platformNetworkDescription={model.platformNetworkDescription}
                      onOpenDocumentation={externalActions.onOpenDocumentation}
                      onOpenSystemKeyboardSettings={externalActions.onOpenSystemKeyboardSettings}
                    />
                    <AboutSettingsPage />
                    <UsageReportingSettingsPage />
                    {/* 只在打开着时渲染：它和「关于」用同一个统计开关，常驻的话桌面上会有两个。 */}
                    {page === "privacy" && <PrivacySettingsPage />}
                  </SettingsFormContext.Provider>
                  <SettingsFormFooter
                    draft={draft}
                    busy={busy}
                    // 保存失败由页面顶部的提示说明并重试（SettingsPageStatus），这一行不再重复同一个错误和按钮。
                    saveState={saveState === "failed" ? "idle" : saveState}
                    saveError={saveError}
                    showRestoreDefaults={
                      Boolean(client.loadDefaultPreferences) &&
                      canRestoreDefaultsOnPage(page) &&
                      // macOS 的屏幕键盘页（并进手写之后）只剩两个打开按钮，没有一项偏好，页脚的全局「恢复默认设置」在那里只会让人以为它恢复这一页。
                      !(
                        page === "screen-keyboard" &&
                        !mobilePlatform &&
                        !model.showTouchKeyboardHeight &&
                        !model.showTouchKeyboardSpacing
                      ) &&
                      !skinCommunityShown &&
                      !pluginCommunityShown
                    }
                    onRestoreDefaults={onRestoreDefaults}
                    onRetry={() => void retrySave()}
                    // Settings save themselves, so reading them again is only a way out of a failure: a save or load that failed, or an error the page is showing.
                    onReload={
                      canReloadSettingsPage(page) &&
                      (saveState === "failed" || loadFailed || !!error)
                        ? () => void reloadSettings()
                        : undefined
                    }
                  />
                </SettingsFormFrame>
              )}
              {/* With the form on screen 重新读取 sits in its action row once loading or saving failed; without it (a page with no form, or settings that failed to load) this is the only way back. */}
              {canReloadSettingsPage(page) && !(draft && isSettingsFormPage(page)) && (
                <button className="secondary" disabled={busy} onClick={() => void reloadSettings()}>
                  重新读取
                </button>
              )}
            </div>
          </main>
        </div>
      </ShellToasts>
    </div>
  );
}
