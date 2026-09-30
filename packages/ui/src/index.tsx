import { useConfirm } from "./core/confirm";
import { NavItem } from "./core/platform-controls";
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
  pages,
  settingsPageAliases,
  subPageParents,
  type SettingsPageId,
} from "./settings/settings-page-registry";
import { settingsPageProjections } from "./settings/settings-page-projections";
import { canReloadSettingsPage, isSettingsFormPage } from "./settings/settings-page-visibility";
import { settingsInputPreferences } from "./settings/settings-input-preferences";
import { aiSettingsPreferences } from "./settings/ai-settings-preferences";
import { diagnosticLogPreferences } from "./settings/diagnostic-log-preferences";
import { clipboardHistoryEnabled } from "./settings/clipboard-history-preferences";
import { settingsThemePreferences } from "./settings/settings-theme-preferences";
import type { VoiceDeviceReader } from "./voice/voice-device-picker";
import type { LocalVoiceModelClient } from "./voice/local-models";
import { useEffect, useRef, useState } from "react";
import {
  type DictionaryEntry,
  type LocalDictionaryFormat,
  type LocalDictionaryKind,
} from "./dictionary/dictionary-file";
export type {
  DictionaryEntry,
  LocalDictionaryFormat,
  LocalDictionaryKind,
} from "./dictionary/dictionary-file";
export {
  dictionaryExportName,
  dictionaryExportPayload,
  loadAllPersonalDictionaryEntries,
  personalDictionaryExportName,
  personalDictionaryExportPayload,
} from "./dictionary/dictionary-export";
export { dictionaryErrorMessage } from "./dictionary/dictionary-errors";
import type { TouchKeyboardSchemePreferences } from "./settings/touch-keyboard-scheme-helpers";
export {
  type TouchKeyboardScheme,
  type TouchKeyboardSchemePreferences,
  inferredTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  allTouchKeyboardSchemes,
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
import { useSettingsTheme } from "./settings/use-settings-theme";
export { useSettingsTheme } from "./settings/use-settings-theme";
export { updateCandidateColor, updateCustomKeyboard } from "./settings/theme-selection-updates";
import { useTouchKeyboardGeometryDrag } from "./settings/use-touch-keyboard-geometry-drag";
import { defaultTouchKeyboardGeometry } from "./settings/touch-keyboard-geometry-defaults";
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
import type { InputSourceStartupStatus } from "./settings/input-source-startup-notice";
import { SettingsFormFrame } from "./settings/settings-form-frame";
import { WindowTitlebar } from "./settings/window-titlebar";
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
import { useTouchKeyboardSchemeSelection } from "./settings/use-touch-keyboard-scheme-selection";
import { useSettingsDestinationActions } from "./settings/use-settings-destination-actions";
import { useMacosSettings } from "./settings/use-macos-settings";
import { useWindowState } from "./settings/use-window-state";
import { useAppVersion } from "./settings/use-app-version";
import { supportDiagnostics } from "./settings/support-diagnostics";
import { useMountedRef } from "./settings/use-mounted-ref";
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
export { useExternalUrl, type UseExternalUrlOptions } from "./settings/use-external-url";
export {
  useSettingsNavigation,
  type SettingsNavigationOptions,
} from "./settings/use-settings-navigation";
export { useSettingsContentScrollReset } from "./settings/use-settings-content-scroll-reset";
export { useUpdateCheck, type UseUpdateCheckOptions } from "./settings/use-update-check";
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
  settingsSidebarGroups,
  type SettingsSidebarGroupsOptions,
} from "./settings/sidebar-groups";
export { canReloadSettingsPage, isSettingsFormPage } from "./settings/settings-page-visibility";
export type { SettingsPageId } from "./settings/mobile-navigation";
export {
  useSettingsDictionaryState,
  type UseSettingsDictionaryStateOptions,
} from "./settings/use-settings-dictionary-state";
export { SettingsFormFrame, type SettingsFormFrameProps } from "./settings/settings-form-frame";
export { SettingsInputPage, type SettingsInputPageProps } from "./settings/settings-input-page";
export {
  settingsPageCatalog,
  type SettingsPageCatalog,
  type SettingsPageCatalogOptions,
} from "./settings/settings-page-catalog";
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
export {
  SettingsFeedbackPage,
  type SettingsFeedbackPageProps,
} from "./settings/settings-feedback-page";
export {
  SettingsUtilityPages,
  type SettingsUtilityPagesProps,
} from "./settings/settings-utility-pages";
export {
  SettingsVoiceAiPages,
  type SettingsVoiceAiPagesProps,
} from "./settings/settings-voice-ai-pages";
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
export { useMacosSettings, type UseMacosSettingsOptions } from "./settings/use-macos-settings";
export { useWindowState, type UseWindowStateOptions } from "./settings/use-window-state";
export {
  useSettingsWindowInteractions,
  type UseSettingsWindowInteractionsOptions,
} from "./settings/use-settings-window-interactions";
export { useAppVersion, type UseAppVersionOptions } from "./settings/use-app-version";
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
import type { FontCatalogReader } from "./candidate/font-catalog";
import {
  isPolishCustomSlot,
  normalizePolishSlot,
  polishPromptFor,
  polishPresetPrompt,
  polishSlotField,
} from "./voice/polish-presets";
import {
  customThemeBase,
  type CustomTheme,
  type GlobalTheme,
  type ResolveThemeRequest,
  type ResolvedTheme,
} from "./theme/global-theme";
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
import {
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
import { type SkinCatalog } from "./skin/external-skins";
import { TypingStatisticsPage, type TypingStatisticsClient } from "./settings/typing-statistics";
import { VocabularyReviewPage, type VocabularyReviewClient } from "./settings/vocabulary-review";
import {
  type McpClientId,
  type McpInstallOutcome,
  type McpServerStatus,
} from "./settings/mcp-connect";
import * as settings from "./settings/settings-style";
import {
  AccountPage,
  type AccountClient,
  type AccountCommunityDestination,
  type AppIconClient,
} from "./account/account-page";
import { ChatPage, type ChatClient } from "./chat/chat-page";
import { HomePage, MoreSettingsPage, type HomePageActions } from "./keyboard/home-page";
import { useSettingsPlatform } from "./theme/settings-platform";
import { SettingsFormContext } from "./settings/settings-form-context";
import { createSettingsReloadAction } from "./settings/settings-reload-action";
import { deepEqual } from "./core/deep-equal";
import { createSettingsPageSelection } from "./settings/settings-page-selection";
import { createSettingsDraftActions } from "./settings/settings-draft-actions";
import { createHelpcodeSettingsActions } from "./settings/helpcode-settings-actions";
import { createSettingsStatusActions } from "./settings/settings-status-actions";
import { createSettingsExternalActions } from "./settings/settings-external-actions";
import { createSettingsNavigationActions } from "./settings/settings-navigation-actions";
import { VoiceSettingsPage } from "./settings/pages/voice-page";
import { AiSettingsPage } from "./settings/pages/ai-page";
import { InputSettingsPage } from "./settings/pages/input-page";
import { ExpressionSettingsPage } from "./settings/pages/expression-page";
import { DeveloperSettingsPage } from "./settings/pages/developer-page";
import { DownloadSettingsPage } from "./settings/pages/download-page";
import { DictionarySettingsPage } from "./settings/pages/dictionary-page";
import { AppearanceSettingsPage } from "./settings/pages/appearance-page";
import { SkinSettingsPage } from "./settings/pages/skin-page";
import { FloatingToolbarSettingsPage } from "./settings/pages/floating-toolbar-page";
import { ScreenKeyboardSettingsPage } from "./settings/pages/screen-keyboard-page";
import { ShortcutSettingsPage } from "./settings/pages/shortcuts-page";
import { ToolsSettingsPage } from "./settings/pages/tools-page";
import { AboutSettingsPage } from "./settings/pages/about-page";
import { HelpcodeSettingsPage } from "./settings/pages/helpcode-page";
import type { CustomHelpcodeSchema, HelpcodePreferences } from "./settings/pages/helpcode-page";
import type { ClipboardHistoryClient } from "./settings/clipboard-history-section";
import { defaultFuzzyPinyin, type FuzzyPinyinPreferences } from "./settings/fuzzy-pinyin-section";
import {
  defaultWordCharacter,
  type NavigationPreferences,
} from "./settings/word-character-section";
import { defaultMixedInput, type MixedInputPreferences } from "./settings/mixed-input-section";
import { defaultFrequency, type FrequencyPreferences } from "./settings/frequency-section";
import { defaultLocalModes, type LocalModePreferences } from "./settings/local-modes-section";
import { floatingToolbarPreferences } from "./settings/floating-toolbar-preferences";
import { defaultKeybindings } from "./settings/keybinding-defaults";
import type { SurfaceTheme, ThemeMode } from "./settings/theme-settings-section";
import type { TouchToolbarPreferences } from "./settings/touch-keyboard-geometry-section";
import { HelpSettingsPage } from "./settings/help-settings-page";
import { VoiceCredentialControl } from "./settings/voice-credential-control";
import type { MobileKeyboardFeedbackClient } from "./settings/mobile-keyboard-feedback-section";
import { HandwritingSettingsPage } from "./settings/pages/handwriting-page";
import { FeedbackSettingsPage } from "./settings/pages/feedback-page";
import { allTouchKeyboardSchemes } from "./settings/touch-keyboard-scheme-helpers";
import { logo } from "./settings/settings-options";
import type { CommunitySkinClient } from "./community/community-skins";
import { communityDestinationView } from "./community/community-destination";
import type { CommunityResourceClient } from "./community/community-resources";
import type { CandidateSkinCommunityClient } from "./community/community-candidate-skins";
import { CommunityPage } from "./community/community-page";
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
  type OnboardingChoices,
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
  type CustomHelpcodeSchema,
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
export {
  CandidateTranslationSettingsSection,
  type CandidateTranslationSettingsSectionProps,
} from "./settings/candidate-translation-settings-section";
export { PunctuationSection, type PunctuationPreferences } from "./settings/punctuation-section";
export {
  MixedInputSection,
  defaultMixedInput,
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
  CandidatePaletteSection,
  type CandidatePaletteSectionProps,
} from "./settings/candidate-palette-section";
export { VoicePolishSection, type VoicePolishSectionProps } from "./settings/voice-polish-section";
export {
  VoicePolishSettingsSection,
  type VoicePolishSettingsSectionProps,
} from "./settings/voice-polish-settings-section";
export { availableSettingsPages, type AvailablePageCapabilities } from "./settings/available-pages";
export { describeImportResult } from "./dictionary/dictionary-messages";
export {
  AiLinuxProviderSection,
  type AiLinuxProviderSectionProps,
} from "./settings/ai-linux-provider-section";
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
  MobileKeyboardFeedbackSettings,
  type MobileKeyboardFeedbackSettingsProps,
} from "./settings/mobile-keyboard-feedback-settings";
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
  useCommunityGallery,
  type CommunityGalleryClient,
  type CommunityGalleryOptions,
  type CommunityGalleryPage,
} from "./community/community-gallery";
export {
  CommunityErrorAlert,
  type CommunityErrorAlertProps,
} from "./community/community-error-alert";
export {
  CommunityDetailStatus,
  type CommunityDetailStatusProps,
} from "./community/community-detail-status";
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
export {
  CommunityCandidateSkinsPage,
  type CandidateSkinCommunityClient,
  type CandidateSkinPackPreview,
  type CommunityCandidateSkin,
  type CommunityCandidateSkinLicense,
  type CommunityCandidateSkinPage,
} from "./community/community-candidate-skins";
export {
  CommunityScopeButtons,
  type CommunityScopeButtonsProps,
} from "./community/community-scope-buttons";
export {
  CommunityResourceScopeButtons,
  type CommunityResourceScopeButtonsProps,
} from "./community/community-resource-scope-buttons";
export {
  CommunitySkinPublicationFields,
  type CommunitySkinPublicationFieldsProps,
} from "./community/community-skin-publication-fields";
export { CandidateSkinPublishDialog } from "./community/candidate-skin-publish-dialog";
export { CommunityPage, type CommunityPageProps } from "./community/community-page";
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
  type ProviderDefaults,
} from "./voice/voice-providers";
export {
  asrProviderCredentialTestConfig,
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
export {
  CloudDictionaryEntryForm,
  type CloudDictionaryEntryFormProps,
  type CloudDictionaryEntryFormValue,
} from "./keyboard/cloud-dictionary-entry-form";
export { CloudPanelHeader, type CloudPanelHeaderProps } from "./keyboard/cloud-panel-header";
export {
  CloudDictionaryKindTabs,
  type CloudDictionaryKindTabsProps,
} from "./keyboard/cloud-dictionary-kind-tabs";
export {
  CloudDictionaryKindSelect,
  type CloudDictionaryKindSelectProps,
} from "./keyboard/cloud-dictionary-kind-select";
export type { EmojiCatalogGroup } from "./emoji/emoji-catalog";
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
  /** The host answers the voice recording shortcuts from an attached hardware keyboard without having desktop panel windows (a HarmonyOS phone), so the page offers their switches there too. */
  voice_hotkeys?: boolean;
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

/** Local whole-sentence candidate sources and optional neural reranking. */
export type SentenceAssociationPreferences = {
  word_lattice?: boolean;
  neural_desktop?: boolean;
  neural_keyboard?: boolean;
  show_next_on_duplicate?: boolean;
};

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
  sentence_association?: SentenceAssociationPreferences;
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
  candidate_follow_cursor?: boolean;
  /** macOS-only non-activating badge shown after switching Chinese/English input. */
  input_mode_hud?: boolean;
  candidate_font_family?: string;
  candidate_english_font?: string | null;
  candidate_fallback_fonts?: string[];
  candidate_layout?: "horizontal" | "vertical";
  /** Inline (host-drawn) preedit. Linux applies it via ClientEngine preedit_style(). */
  tsf_preedit_style?: "raw" | "pinyin" | "empty";
  candidate_preedit_style?: "pinyin" | "empty";
  /** The one theme for the candidate window, floating toolbar, menus and touch keyboard. `theme` stays the light/dark mode that `system` and the settings window follow. */
  global_theme?: GlobalTheme;
  /** What the `custom` global theme is made of: an external candidate skin package, the seven candidate colour pickers and the keyboard editor design. */
  custom_theme?: CustomTheme;
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
  /** Absolute path the `local` provider loads: an installed model directory (one holding msime-model.json). */
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
export { AI_PROVIDER_OPTIONS, aiProviderOption } from "./settings/ai-provider-options";
export { aiProviderUpdate } from "./settings/ai-provider-update";

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
export { dictionaryKindKeyHint } from "./settings/pages/dictionary-page";

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
  /** Desktop community commands that publish, install and rate candidate-window skin packages. */
  communityCandidateSkins?: CandidateSkinCommunityClient;
  listVoiceCaptureDevices?: VoiceDeviceReader;
  /**
   * The user's own candidate glosses. Windows delivers these as a file dropped in the profile
   * directory; a host whose user data lives in an app sandbox has to offer a way in instead.
   */
  customTranslations?: { load(): Promise<string>; save(text: string): Promise<void> };
  listFontFamilies?: FontCatalogReader;
  resolveFontFamilies?: (names: string[]) => Promise<string[]>;
  scanSkinCatalog?: () => Promise<SkinCatalog>;
  /** Custom helper-code tables found below the host's verified resource directory. */
  listHelpcodeSchemas?: () => Promise<CustomHelpcodeSchema[]>;
  /**
   * The colours a host draws for a theme, from the same `resolve` the input method runs (`msime_client_resolve_theme`), with the custom theme's package read from the host's own skin directory. Absent on hosts whose bridge has no theme call; the picker and built-in previews read `themeCatalog` and need no host.
   */
  resolveTheme?: (request: ResolveThemeRequest) => Promise<ResolvedTheme>;
  readSkinImage?: SkinImageReader;
  readSkinFont?: SkinFontReader;
  readSkinToolbarCss?: (id: string, relative?: string) => Promise<string | null>;
  openSkinDirectory?: () => Promise<void>;
  /**
   * Shows the input method's diagnostic log in the file manager so it can be sent after a reproduction. The host resolves the location itself; absent on hosts without a reachable file manager, which then show no button.
   */
  openDiagnosticLogDirectory?: () => Promise<void>;
  /**
   * Write an exported document into the user's Downloads folder and resolve to the absolute path written, which may carry a " (2)" suffix when the name was taken. A host that asks where to save (the Harmony save picker) resolves null when the user closes the picker, and may reject with an Error whose message is shown as is. A host whose webview drops download links (the macOS WKWebView cancels them) offers this; without it the page falls back to a download link.
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
   * Ask the host for an installed local speech model directory (one holding msime-model.json), resolving to null when the user cancels. The model is loaded by path and a file input hands back contents instead, so only the host can answer this.
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

/** What a load or save that failed on an unreadable document says; the repair button sits beside exactly this message. */
/**
 * The 设置 tab draws a gear, not the app.
 *
 * Its page icon is the app logo, which the design does not put in the bar either — its first tab is the `settings` glyph. Three of the four tabs would otherwise be a subject and the fourth a brand.
 */
export {
  tencentCredentialIssue,
  translationEndpointIssue,
} from "./settings/translation-validation";
export {
  aiCredentialOrigin,
  providerCredentialErrorMessage,
  tencentSecretConfigured,
} from "./settings/credential-utils";

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
  const settingsPlatform = useSettingsPlatform(host, linuxPlatform);
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
    showVoiceHotkeys,
  } = capabilities;
  const {
    fullwidthChord,
    maintenanceChord,
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
    inputSourceStartup,
    onDeviceDownloadable,
    setInputSourceStartup,
    setSavedShuangpinKeymap,
    setSavedWubiAutoCommitUnique,
    setShuangpinKeymap,
    setWubiAutoCommitUnique,
    shuangpinKeymap: macosShuangpinKeymap,
    wubiAutoCommitUnique: macosWubiAutoCommitUnique,
    savedShuangpinKeymap: savedMacosShuangpinKeymap,
    savedWubiAutoCommitUnique: savedMacosWubiAutoCommitUnique,
  } = useMacosSettings({ client, macos: macosPlatform, setError });
  const restoredMobilePage =
    mobilePlatform &&
    typeof window !== "undefined" &&
    window.history.state?.msimeSettings === true &&
    typeof window.history.state.page === "string"
      ? window.history.state.page
      : undefined;
  const [page, setPage] = useState<SettingsPageId>(() =>
    requestedPage(
      initialPage ?? restoredMobilePage ?? (client.home ? "home" : undefined),
      pages,
      settingsPageAliases,
      "appearance",
    ),
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
    saveState: customTranslationsSaveState,
    saveError: customTranslationsSaveError,
    placeholder: customTranslationsPlaceholder,
    flush: flushCustomTranslations,
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
    savedMacosShuangpinKeymap,
    saveMacosShuangpinKeymap: client.saveMacosShuangpinKeymap,
    setSavedMacosShuangpinKeymap: setSavedShuangpinKeymap,
    macosWubiAutoCommitUnique,
    savedMacosWubiAutoCommitUnique,
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
    releasePlatform: client.host?.platform ?? (linuxPlatform ? "linux" : null),
    releasePageUrl: platformReleasesPageUrl,
    currentAppVersion,
  });

  const openPanel = useOpenPanel({ setError });

  // Only a failed save leaves changes unsaved for long; 重新读取 then asks before discarding them.
  const dirty =
    (!!draft && !!snapshot && !deepEqual(draft, snapshot.preferences)) ||
    (macosShuangpinKeymap !== undefined && macosShuangpinKeymap !== savedMacosShuangpinKeymap) ||
    (macosWubiAutoCommitUnique !== undefined &&
      macosWubiAutoCommitUnique !== savedMacosWubiAutoCommitUnique);
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
  } = settingsInputPreferences(draft);
  const touchKeyboardSchemes = draft?.touch_keyboard_schemes ?? {
    enabled: allTouchKeyboardSchemes,
  };
  const {
    selected: selectedTouchKeyboardScheme,
    select: selectTouchKeyboardScheme,
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
    nativeVoicePlatform,
    localModelsAvailable: client.localVoiceModels !== undefined,
    onChange: onVoiceChange,
  });
  const providerPresetControls = createProviderPresetControl(client.openExternalUrl);
  const floatingToolbar = floatingToolbarPreferences(draft);
  const { themeMode, settingsTheme, globalTheme, customColors, customTouchKeyboardSkin } =
    settingsThemePreferences(draft);
  useSettingsTheme(themeMode, settingsTheme);
  const candidatePreviewTheme = useCandidatePreviewTheme(themeMode, draft?.candidate_theme);
  const toolbarPreviewTheme = useCandidatePreviewTheme(themeMode, draft?.toolbar_theme);
  const keyboardPreviewTheme = useCandidatePreviewTheme(themeMode, draft?.screen_keyboard_theme);
  // A picker colour is part of the custom theme, so choosing one selects that theme; clearing one leaves the selection alone. Choosing one while another theme is selected customizes that theme: it becomes the custom theme's base, and a package, whose own base would replace it, is dropped.
  // A custom theme without a keyboard design draws its base's keyboard, so the custom keyboard card is selected only when the theme carries one.
  const customKeyboardSelected = globalTheme === "custom" && Boolean(draft?.custom_theme?.keyboard);
  useEffect(() => setSkinPreviewThemes({}), [candidatePreviewTheme]);
  const touchKeySpacingTenths =
    draft?.touch_key_spacing_tenths ?? defaultTouchKeyboardGeometry.keySpacingTenths;
  const touchRowSpacingTenths =
    draft?.touch_row_spacing_tenths ?? defaultTouchKeyboardGeometry.rowSpacingTenths;
  const touchKeyboardHeightAdjustment =
    draft?.touch_keyboard_height_adjustment ?? defaultTouchKeyboardGeometry.heightAdjustment;
  const installerTrust = availableUpdate
    ? describeInstallerTrust(
        availableUpdate,
        client.host?.platform ?? (linuxPlatform ? "linux" : null),
      )
    : null;
  // Helper codes are per-host rather than per-form-factor. The Android keyboard sends them: Shift during a quanpin or shuangpin composition passes the next letter to the Engine as a helper code, and the Engine reads the schema and the candidate-row hint from these very preferences. Hiding the group left that shipping feature with no way to pick a schema or turn it off. The iOS keyboard extension marks a helper code the same way, so the group also follows the host's `helpcode_shift_entry`; the platform names stay for hosts that predate the capability. HarmonyOS ships the same input: its ChineseHelpcodePolicy is the Android one, ported, and the session calls it on every shifted key.
  const showHelpcode =
    !mobilePlatform || showHelpcodeShiftEntry || androidPlatform || harmonyPlatform;
  // The local MCP server, the diagnostic logs and the data directory are what 开发者选项 holds; a host with none of them has no such page.
  const showDeveloperPage =
    Boolean(client.mcpServerStatus) ||
    !client.host ||
    linuxPlatform ||
    windowsPlatform ||
    macosPlatform;
  // Physical-keyboard shortcuts and a desktop floating toolbar have no phone surface. HarmonyOS keeps those controls in the input-method panel on a 2-in-1, but its phone panel is still a touch keyboard, so the settings entry must not leak the PC key descriptions into the phone's "全部设置" list.
  //
  // The shortcuts page carries the hardware-keyboard chords, so it is hidden where the host does not route any of them rather than where the platform happens to be a phone. Any of these devices can have a keyboard attached, and its owner has to be able to reach the switches the host already reads; hiding the page by platform name left them unreachable on Android.
  const mobileHiddenPageIds: readonly SettingsPageId[] = [
    ...(showModeSwitchShortcuts || showPanelShortcuts || showDesktopMaintenanceShortcuts
      ? []
      : (["shortcuts"] as const)),
    "floating-toolbar",
  ];
  const { availablePages, sidebarGroups, mobilePrimaryPages, mobileSecondaryGroups } =
    settingsPageProjections({
      mobilePlatform,
      hasHomePage: Boolean(client.home),
      hasTypingStatistics: Boolean(client.typingStatistics),
      hasVocabularyReview: Boolean(client.vocabularyReview),
      hasAccount: Boolean(client.account || client.appIcon),
      hasChat: Boolean(client.chat),
      hasCommunity: Boolean(
        client.communitySkins || client.communityResources || client.communityCandidateSkins,
      ),
      showFloatingToolbar,
      showDeveloperPage,
      mobileHiddenPageIds,
      mobilePageTitle,
    });
  // A sub-page lights its parent in the navigation and offers the way back to it.
  const navigationPage: SettingsPageId = subPageParents[page] ?? page;
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
  const untitledOnPhone: readonly SettingsPageId[] = ["home", "typing-statistics", "account"];
  // The design's row that opens a page from inside another, e.g. AI 辅助 on 表达.
  const pageEntry = (id: SettingsPageId) => availablePages.find((item) => item.id === id);
  const { openCommunity, openLocalDesigns } = useSettingsDestinationActions({
    selectPage,
    setShowTouchSkinEditor,
    setCommunityDestination,
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
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showEnglishSuggestions,
    showHelpcodeShiftEntry,
    showHelpcode,
    showShuangpinPreedit,
    showCharacterWidth,
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
    fullwidthChord,
    maintenanceChord,
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
    setInputSourceStartup,
    page,
    accountLoginReturnPage,
    settingsContentRef,
    communityDestination,
    updateStatus,
    updateBusy,
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
    customTranslationsText,
    setCustomTranslationsText,
    customTranslationsNotice,
    customTranslationsPlaceholder,
    customTranslationsSaveState,
    customTranslationsSaveError,
    customTranslationsSummary,
    macosShuangpinKeymap,
    setShuangpinKeymap,
    macosWubiAutoCommitUnique,
    setWubiAutoCommitUnique,
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
    flushCustomTranslations,
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
  };
}

/** What `SettingsPage` computes for its shell and page components. */
export type SettingsPageModel = ReturnType<typeof useSettingsPageModel>;

export function SettingsPage(props: SettingsPageProps) {
  const { onReplayOnboarding } = props;
  const model = useSettingsPageModel(props);
  const [customHelpcodeSchemas, setCustomHelpcodeSchemas] = useState<CustomHelpcodeSchema[]>([]);
  useEffect(() => {
    let active = true;
    const reader = props.client.listHelpcodeSchemas;
    if (!reader) {
      setCustomHelpcodeSchemas([]);
      return () => {
        active = false;
      };
    }
    void reader()
      .then((schemas) => {
        if (active) setCustomHelpcodeSchemas(schemas);
      })
      .catch(() => {
        if (active) setCustomHelpcodeSchemas([]);
      });
    return () => {
      active = false;
    };
  }, [props.client.listHelpcodeSchemas]);
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
    draft,
    busy,
    setError,
    error,
    notice,
    recoveredBackup,
    inputSourceStartup,
    setInputSourceStartup,
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
    selectHomeScheme,
    keyboardPreviewTheme,
    availablePages,
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
  } = model;
  const { onChange: onHelpcodeChange } = createHelpcodeSettingsActions({
    setDraft: model.setDraft,
  });
  const reloadSettings = createSettingsReloadAction({ dirty, reload, confirm });
  const { onOpenPage } = createSettingsPageSelection({ selectPage });
  const statusActions = createSettingsStatusActions({
    recoverPreferences,
    inputSourceStartup: client.inputSourceStartup,
    setInputSourceStartup,
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
  const pageTitle = availablePages.find((item) => item.id === page)?.title ?? "候选窗口";
  // A sub-page (AI 辅助 under 表达, 背单词 under 词库, 帮助 under 反馈) names its parent on the way back.
  const parentPage =
    navigationPage !== page ? availablePages.find((item) => item.id === navigationPage) : undefined;
  // A phone collapses the large title into a compact bar on the 设置 tab's pages, the way the design does; the other tabs and the untitled pages have no large title to collapse.
  const collapsingTitle =
    mobilePlatform && mobileActiveTab === "home" && !untitledOnPhone.includes(page);
  // The design searches in the Windows caption and at the top of the macOS and HarmonyOS 2-in-1 sidebars; GNOME has none. A Windows window without a caption (a browser preview) keeps the search in the sidebar so the filter is still reachable.
  const searchInTitlebar = winShell && titlebarShown;
  const searchInSidebar =
    macShell || settingsPlatform === "hm2" || ipadShell || (winShell && !titlebarShown);
  const navNeedle = navQuery.trim().toLocaleLowerCase();
  // The iPad's tab bar carries 社区, 统计 and 我的, so its settings sidebar does not list them a second time.
  const splitSidebarGroups = ipadShell
    ? sidebarGroups
        .map((group) =>
          group.filter(
            (item) =>
              item.id === "home" || !mobilePrimaryPageIds.includes(item.id as MobilePrimaryPageId),
          ),
        )
        .filter((group) => group.length > 0)
    : sidebarGroups;
  const shownSidebarGroups = navNeedle
    ? splitSidebarGroups
        .map((group) => group.filter((item) => item.title.toLocaleLowerCase().includes(navNeedle)))
        .filter((group) => group.length > 0)
    : splitSidebarGroups;
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
          if (event.key === "Escape") setNavQuery("");
          if (event.key !== "Enter") return;
          const first = shownSidebarGroups[0]?.[0];
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
      // Marks the phone navigation. It no longer carries a palette: the `[data-platform]` rules in styles.css cover the phone hosts too.
      data-mobile={mobilePlatform ? "" : undefined}
      onPointerDownCapture={onWindowPointerDownCapture}
    >
      {confirmation}
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
        {mobilePlatform && (
          <MobileSettingsTabs
            tabs={mobilePrimaryPages}
            activeTab={mobileActiveTab}
            onSelect={selectMobileTab}
          />
        )}
        <nav
          className={`${settings.sidebar} ${ipadSidebarShown ? "" : "ipad:hidden"}`}
          aria-label="设置分类"
        >
          {macShell && (
            <div className={settings.macosDragRow} data-window-drag="" {...windowDragHandlers} />
          )}
          {/* The brand heads the column only where no caption carries it: a desktop window without window commands, or a phone host wide enough for the sidebar. The macOS and HarmonyOS 2-in-1 designs open the column on the search instead. */}
          {ipadShell && <h2 className={settings.sidebarTitle}>设置</h2>}
          {!titlebarShown && !macShell && !ipadShell && settingsPlatform !== "hm2" && (
            <div className={settings.sidebarHeader}>
              <img src={logo} alt="" />
              <span>水杉输入法</span>
            </div>
          )}
          {searchInSidebar && <label className={settings.sidebarSearch}>{navSearchField}</label>}
          {shownSidebarGroups.map((group, index) => (
            <div
              key={group[0].id}
              className={settings.sidebarSection(index === 0)}
              data-sidebar-section=""
            >
              {group.map((item) => (
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
          className={`${settings.content} ${ipadSidebarShown ? "" : "ipad:col-span-2"}`}
          aria-labelledby="page-title"
          onScroll={
            collapsingTitle
              ? (event) => setTitleCollapsedOn(event.currentTarget.scrollTop > 28 ? page : null)
              : undefined
          }
        >
          {collapsingTitle && (
            // The large title's compact stand-in once it scrolls away. Decorative: the `h1` below still names the page.
            <div className={settings.collapsedTitle(titleCollapsedOn === page)} aria-hidden="true">
              {pageTitle}
            </div>
          )}
          {macShell && (
            <header className={settings.macosToolbar} data-window-drag="" {...windowDragHandlers}>
              <h1 id="page-title">{pageTitle}</h1>
            </header>
          )}
          <div className={settings.contentColumn}>
            {/* Three of the four tabs open on something that already names them — a headline, a
                profile card, a row of figures — and the source prints no page title over any of
                them. 社区 is the one that does. Hidden rather than dropped: it labels `main`. */}
            {parentPage && (
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
              />
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
            />
            {client.home && draft && page === "home" && (
              <HomePage
                preferences={draft}
                actions={client.home}
                onOpenPage={onOpenPage}
                onSelectScheme={selectHomeScheme}
                onOpenChat={onOpenChat}
                touchLayout={mobilePlatform}
              />
            )}
            {page === "more" && (
              <MoreSettingsPage
                groups={mobileSecondaryGroups.map((group) =>
                  group.map((item) => ({ id: item.id, title: item.title, icon: item.icon })),
                )}
                onOpenPage={onOpenPage}
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
            {page === "community" && (
              <CommunityPage
                destinationKey={communityDestination}
                skins={client.communitySkins}
                resources={client.communityResources}
                candidateSkins={client.communityCandidateSkins}
                localSkins={client.scanSkinCatalog}
                openSkinDirectory={client.openSkinDirectory}
                importsSkin={client.host?.skin_directory_import === true}
                onOpenSkinPage={() => selectPage("skin")}
                theme={keyboardPreviewTheme}
                initialMine={initialCommunityMine}
                initialCategory={initialCommunityCategory}
                initialScope={initialCommunityScope}
                localDictionary={client.dictionary}
                localSkinLibrary={client.customSkinLibrary}
                mobile={mobilePlatform}
                onLogin={openAccountLogin}
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
            {draft && isSettingsFormPage(page) && (
              <SettingsFormFrame showReload={false} busy={busy}>
                <SettingsFormContext.Provider value={{ ...model, draft }}>
                  <SkinSettingsPage />
                  <AppearanceSettingsPage />
                  <FloatingToolbarSettingsPage />
                  <InputSettingsPage />
                  <HelpcodeSettingsPage
                    value={draft}
                    customSchemas={customHelpcodeSchemas}
                    mobile={mobilePlatform}
                    showShiftEntry={model.showHelpcodeShiftEntry}
                    disabled={busy}
                    hidden={page !== "input" || !model.showHelpcode}
                    onChange={onHelpcodeChange}
                  />
                  <ExpressionSettingsPage />
                  <AiSettingsPage />
                  <ShortcutSettingsPage />
                  <DictionarySettingsPage />
                  <ScreenKeyboardSettingsPage />
                  <VoiceSettingsPage />
                  <HandwritingSettingsPage />
                  <ToolsSettingsPage />
                  <DownloadSettingsPage />
                  <DeveloperSettingsPage />
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
                </SettingsFormContext.Provider>
                <SettingsFormFooter
                  draft={draft}
                  busy={busy}
                  saveState={saveState}
                  saveError={saveError}
                  showRestoreDefaults={Boolean(client.loadDefaultPreferences)}
                  onRestoreDefaults={onRestoreDefaults}
                  onRetry={() => void retrySave()}
                  // Settings save themselves, so reading them again is only a way out of a failure: a save or load that failed, or an error the page is showing.
                  onReload={
                    canReloadSettingsPage(page) && (saveState === "failed" || loadFailed || !!error)
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
    </div>
  );
}
