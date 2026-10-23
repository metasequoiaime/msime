import type { SettingsClient } from "../core/settings-client";
import type { Preferences, Snapshot } from "../core/preferences-model";
import { useConfirm } from "../core/confirm";
import { mobilePageTitle } from "./mobile-tab-helpers";
import { desktopDownloadUrl, fallbackAppVersion } from "./app-resources";
import { useSettingsNavigation } from "./use-settings-navigation";
import { useSettingsPageRouting } from "./use-settings-page-routing";
import type { SettingsPageId } from "./settings-page-registry";
import { settingsPageProjections } from "./settings-page-projections";
import { mobileHiddenPageIds, settingsPageModelVisibility } from "./settings-page-model-visibility";
import { settingsInputPreferences } from "./settings-input-preferences";
import { aiSettingsPreferences } from "./ai-settings-preferences";
import { diagnosticLogPreferences } from "./diagnostic-log-preferences";
import { clipboardHistoryEnabled } from "./clipboard-history-preferences";
import { useSettingsThemePreviews } from "./use-settings-theme-previews";
import { useState } from "react";
import { settingsPlatformPresentation } from "./settings-platform-presentation";
import { settingsPageEnvironment } from "./settings-page-environment";
import { schemeTitle } from "./label-helpers";
import { useHarmonySystemBars, useResolvedAppTheme, useSettingsTheme } from "./use-settings-theme";
import { useTouchKeyboardGeometryDrag } from "./use-touch-keyboard-geometry-drag";
import { touchKeyboardGeometryPreferences } from "./touch-keyboard-geometry-preferences";
import { useMobileKeyboardFeedback } from "./use-mobile-keyboard-feedback";
import { useTranslationSettings } from "./use-translation-settings";
import { useDictionaryManager } from "./use-dictionary-manager";
import { useProviderCredentials } from "./use-provider-credentials";
import { useFeedbackReport } from "./use-feedback-report";
import { useDataDirectory } from "./use-data-directory";
import { usePreferenceRecovery } from "./use-preference-recovery";
import { useSettingsPersistence, useTransientSyncNotice } from "./use-settings-persistence";
import { useInputSourceUninstall } from "./use-input-source-uninstall";
import { useTouchKeyboardSettingsReset } from "./use-touch-keyboard-settings-reset";
import { useExternalUrl } from "./use-external-url";
import { useUpdateCheck } from "./use-update-check";
import { useOpenPanel } from "./use-open-panel";
import { useClipboardHistoryToggle } from "./use-clipboard-history-toggle";
import { useTouchKeyboardSchemeSelection } from "./use-touch-keyboard-scheme-selection";
import { useSettingsDestinationActions } from "./use-settings-destination-actions";
import { useCommunityGallerySubview } from "./use-community-gallery-subview";
import { useMacosSettings } from "./use-macos-settings";
import { useWindowState } from "./use-window-state";
import { useAppVersion } from "./use-app-version";
import { useAppReleaseDate } from "./use-app-release-date";
import { supportDiagnostics } from "./support-diagnostics";
import { useMountedRef } from "./use-mounted-ref";
import { useHelpcodeCatalog } from "./use-helpcode-catalog";
import { createProviderPresetControl } from "./provider-preset-control";
import { useAiAssistant } from "./use-ai-assistant";
import { useVoiceInputSettings } from "./use-voice-input-settings";
import { useSettingsPlatform } from "../theme/settings-platform";
import { deepEqual } from "../core/deep-equal";
import { createSettingsDraftActions } from "./settings-draft-actions";
import { useCommunityGalleryNavigation } from "./use-community-gallery-navigation";
import { floatingToolbarPreferences } from "./floating-toolbar-preferences";
import { defaultTouchKeyboardSchemes } from "./touch-keyboard-scheme-helpers";
import { communityDestinationView } from "../community/community-destination";
import { describeInstallerTrust } from "./update-manifest";
import { editionUsesHelpcode } from "./input-scheme-options";
import { useSettingsIntroTabs } from "./use-settings-intro-tabs";
import { useAutomaticDictionaryUpdates } from "./use-automatic-dictionary-updates";

export type SettingsPageProps = {
  client: SettingsClient;
  /** 页面首次挂载时读取的初始设置页。 */
  initialPage?: string;
  /** 挂载后的页面请求；新的 `nonce` 触发导航并保留当前草稿。 */
  route?: { page: string; nonce: number };
  onReplayOnboarding?: () => void;
};

/** 设置窗口的状态、副作用和处理器；外壳与各设置页面消费同一份模型。 */
export function useSettingsPageModel({ client, initialPage, route }: SettingsPageProps) {
  const { introTabs, selectIntroTab } = useSettingsIntroTabs();
  useAutomaticDictionaryUpdates(client);
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
  // 根页面使用的八套平台外观之一，见 `../theme/platform-tokens.ts`。
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
    showToolbarIdleBehavior,
    showToolbarComponents,
    showTouchKeyboardHeight,
    showTouchKeyboardSpacing,
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showEnglishSuggestions,
    showHelpcodeShiftEntry,
    showHelpcodeTriggerKey,
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
    showCandidateItemGeometry,
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
  useTransientSyncNotice(notice, setNotice);
  const dismissNotice = () => setNotice("");
  /** 最近一次修复生成的备份路径，在修复提示可见期间保留。 */
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
    refreshInputSourceStartup,
    inputSourceStartup,
    onDeviceDownloadable,
    shuangpinKeymap: macosShuangpinKeymap,
  } = useMacosSettings({ client, macos: macosPlatform, setError });
  const {
    requestedSettingsPage,
    setPage,
    page,
    dictionaryTab,
    setDictionaryTab,
    aboutTab,
    setAboutTab,
    aiTab,
    setAiTab,
    accountLoginReturnPage,
    setAccountLoginReturnPage,
    mobileLastPageByTab,
    settingsContentRef,
    communityDestination,
    setCommunityDestination,
  } = useSettingsPageRouting({
    initialPage,
    mobilePlatform,
    hasHomePage,
  });
  const currentAppVersion = useAppVersion({
    readAppVersion: client.readAppVersion,
    fallbackVersion: fallbackAppVersion,
  });
  const appReleaseDate = useAppReleaseDate(client.readAppReleaseDate);
  const { customHelpcodeSchemas, helpcodePacks, refreshHelpcodePacks } = useHelpcodeCatalog({
    schemaReader: client.listHelpcodeSchemas,
    catalog: client.plugins?.catalog,
    inputPageOpen: page === "input",
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
    phrases,
    setPhrases,
    phrasePage,
    phraseBusy,
    phraseError,
    dictionaryPendingCount,
    dictionaryFailures,
    dictionarySnapshotError,
    phraseNotice,
    learnedDataError,
    learnedDataNotice,
    clearPhraseFeedback,
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
    setError,
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
    checkForApplicationUpdate: client.checkForApplicationUpdate,
    pendingUpdateCheck: client.pendingUpdateCheck,
    platform: client.host?.platform ?? null,
    edition: client.host?.edition?.id,
    arch: client.host?.arch,
    currentAppVersion,
  });

  const openPanel = useOpenPanel({ setError });

  // 保存失败才会长时间留下未保存修改，重新读取前需要确认是否丢弃。
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
    aiAssistant,
    aiTranslationReady,
    aiToken: aiTranslationToken,
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
  const providerPresetControls = createProviderPresetControl(
    client.openExternalUrl ? openExternalUrl : undefined,
  );
  const floatingToolbar = floatingToolbarPreferences(draft);
  const {
    themeMode,
    settingsTheme,
    globalTheme,
    customColors,
    customTouchKeyboardSkin,
    skinPreviewThemes,
    setSkinPreviewThemes,
    candidatePreviewTheme,
    toolbarPreviewTheme,
    keyboardPreviewTheme,
    previewCustomTheme,
    customKeyboardSelected,
  } = useSettingsThemePreviews({
    draft,
    platform: host?.platform,
    surfaceThemesFollow,
    showCandidateNumberColor,
    showCandidateBorderColor,
  });
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
  const { showHelpcode, showDeveloperPage, untitledOnPhone } = settingsPageModelVisibility({
    mobilePlatform,
    settingsPlatform,
    hasHost: Boolean(client.host),
    linuxPlatform,
    windowsPlatform,
    macosPlatform,
    showRestartInputMethod: Boolean(showRestartInputMethod),
    hasMcpServerStatus: Boolean(client.mcpServerStatus),
    showHelpcodeShiftEntry,
    editionUsesHelpcode: editionUsesHelpcode(client.host?.edition),
  });
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
    // 宿主有资源包商店，或能播放、路由本页开关控制的功能。
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
  });
  const {
    mobileActiveTab,
    navigationPage,
    selectPage,
    selectMobileTab,
    openAccountLogin,
    finishAccountLogin,
  } = useSettingsNavigation({
    mobilePlatform,
    mobileHiddenPageIds,
    availablePages,
    pageAliases,
    page: requestedSettingsPage,
    setPage,
    mobileLastPageByTab,
    route,
    hasHomePage,
    setCommunityDestination,
    setAccountLoginReturnPage,
    accountLoginReturnPage,
  });
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
  const { communityTabs, openCommunityGallery } = useCommunityGalleryNavigation({
    mobile: mobilePlatform,
    hasCommunityPage: availablePages.some((item) => item.id === "community"),
    hasSkins: Boolean(client.communitySkins),
    hasResources: Boolean(client.communityResources),
    selectPage,
    openSubview: openCommunityGallerySubview,
  });
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
    introTabs,
    selectIntroTab,
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
    showToolbarIdleBehavior,
    showToolbarComponents,
    showTouchKeyboardHeight,
    showTouchKeyboardSpacing,
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showEnglishSuggestions,
    showHelpcodeShiftEntry,
    showHelpcodeTriggerKey,
    showHelpcode,
    customHelpcodeSchemas,
    helpcodePacks,
    refreshHelpcodePacks,
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
    showCandidateItemGeometry,
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
    dismissNotice,
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
    refreshInputSourceStartup,
    dismissInputSourceStartup,
    page,
    requestedSettingsPage,
    dictionaryTab,
    setDictionaryTab,
    aboutTab,
    setAboutTab,
    aiTab,
    setAiTab,
    accountLoginReturnPage,
    settingsContentRef,
    communityDestination,
    updateStatus,
    updateBusy,
    updateCheckSupported,
    availableUpdate,
    currentAppVersion,
    appReleaseDate,
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
    learnedDataError,
    learnedDataNotice,
    clearPhraseFeedback,
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
    aiAssistant,
    aiTranslationReady,
    aiTranslationToken,
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

/** 设置外壳和表单页面共用的完整模型。 */
export type SettingsPageModel = ReturnType<typeof useSettingsPageModel>;
