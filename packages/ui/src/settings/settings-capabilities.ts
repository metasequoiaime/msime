import type { HostCapabilities } from "../core/host-capabilities";

export interface SettingsCapabilitiesInput {
  host?: HostCapabilities;
  linux: boolean;
  android: boolean;
  ios: boolean;
  harmony: boolean;
  windows: boolean;
  macos: boolean;
  mobile: boolean;
  canRestartInputMethod: boolean;
  canInstallInputSource: boolean;
  canListVoiceCaptureDevices: boolean;
}

/** Derives UI capability flags from the host contract. Without a host (the browser preview and tests) the desktop defaults below apply. */
export function settingsCapabilities({
  host,
  linux,
  android,
  harmony,
  windows,
  macos,
  canRestartInputMethod,
  canInstallInputSource,
  canListVoiceCaptureDevices,
}: SettingsCapabilitiesInput) {
  // 宿主自带不需要 API Key 的系统识别器：macOS 的 Speech、HarmonyOS 的 CoreSpeechKit、Android 的 SpeechRecognizer、Windows 的 SAPI 听写。
  const nativeVoicePlatform = macos || harmony || android || windows;
  const showModeScope = host ? host.ime_mode_scope : false;
  const showModeSwitchShortcuts = host ? host.mode_switch_shortcuts : false;
  const showPanelShortcuts = host ? host.panel_shortcuts : false;
  const showNumberRowSelection = host ? host.number_row_selection : false;
  const showRestartInputMethod =
    (host ? host.restart_input_method : false) && canRestartInputMethod;
  const showInstallInputSource = macos && canInstallInputSource;
  const showFloatingToolbar = host ? host.floating_toolbar : true;
  const showToolbarAppearance = host ? host.floating_toolbar_appearance : true;
  // 没有宿主消费自动隐藏/延时/阴影时不列这三行：改了没有效果的开关比没有开关更糟。
  const showToolbarIdleBehavior = host ? host.floating_toolbar_idle_behavior === true : true;
  const showToolbarComponents = host ? host.floating_toolbar_components : true;
  // 屏幕键盘的高度和间距：没有宿主（网页预览）或宿主没报这两项时照旧给出。
  const showTouchKeyboardHeight = host?.touch_keyboard_height !== false;
  const showTouchKeyboardSpacing = host?.touch_keyboard_spacing !== false;
  const showCandidateFontControls = host ? host.candidate_font_controls : true;
  const showCandidatePreeditFont =
    showCandidateFontControls && (host ? host.candidate_preedit_font : true);
  const showCandidateEnglishFont = host ? host.candidate_english_font : false;
  const showEnglishSuggestions = host ? host.english_suggestions : false;
  const showHelpcodeTriggerKey = host?.helpcode_trigger_key === true;
  const showHelpcodeShiftEntry = host ? host.helpcode_shift_entry : false;
  const showShuangpinPreedit = host ? host.shuangpin_preedit : false;
  // 只有画双拼键位图的宿主（macOS、Windows 和 Linux 的 X11 客户端）给出「输入时显示双拼键位提示」。
  const showShuangpinKeymapHint = host ? host.shuangpin_keymap_hint === true : false;
  const showCharacterWidth = host ? host.character_width : true;
  const aiProviderCredentials = host ? host.ai_provider_credentials : false;
  // AI 生成类功能的总闸（`HostCapabilities.ai_generation`）：宿主明确报 false（HarmonyOS）时不给 AI 辅助页、AI 对话、语音润色、AI 翻译、AI 皮肤和回复模板；没有宿主或宿主没报时照旧给出。
  const showAiGeneration = host?.ai_generation !== false;
  // 社区投稿类功能的总闸（`HostCapabilities.community_contribution`）：宿主明确报 false（HarmonyOS）时社区页不画发布、改分类、发布新版本、评分和举报；没有宿主或宿主没报时照旧给出。
  const showCommunityContribution = host?.community_contribution !== false;
  const showVoiceCommitMode = host ? host.voice_commit_mode : true;
  const showVoiceProviderSettings = host ? host.voice_provider_settings : true;
  const showVoiceStreamPreedit = host ? host.voice_stream_preedit : true;
  // 宿主没有报这三项时（旧宿主、网页预览）照旧给出控件；只有明确报 false 的宿主把值固定下来（见 `HostCapabilities` 各字段说明）。
  const showVoiceSpaceLockSwitch = host?.voice_space_lock_switch !== false;
  const showVoiceCueSwitches = host?.voice_cue_switches !== false;
  const showVoiceCaptureBackends = host?.voice_capture_backends !== false;
  // 本地识别的停顿时间和判停灵敏度：只有明确报告 `voice_pause_settings` 的宿主（HarmonyOS 手机和平板）给出。
  const showVoicePauseSettings = host?.voice_pause_settings === true;
  const showCandidateRowColors = host ? host.candidate_row_colors : true;
  const showCandidateSelectionAppearance = host ? host.candidate_selection_appearance : true;
  const showCandidateBorderColor = host ? host.candidate_border_color : true;
  // 缺省（宿主还没报告这一项）按读取处理，与加这一项之前一样给出取色器。
  const showCandidateNumberColor = host ? host.candidate_number_color !== false : true;
  // 宿主让各界面一律跟随颜色模式时，主题页只留候选窗口的单独明暗，设置窗口和各面板也不再读其余几项。
  const surfaceThemesFollow = host?.surface_themes_follow === true;
  const showCandidateFollowCursor = host ? host.candidate_follow_cursor : false;
  const showCandidateWindowScale = host ? host.candidate_window_scale : true;
  const showCandidateWindowOpacity = host ? host.candidate_window_opacity : true;
  const showCandidateCornerRadius = host ? host.candidate_corner_radius : true;
  // 卡片内边距目前只有 macOS 候选窗和 Linux 的 Fcitx5 主题读；没有宿主时（网页预览）照样给出，与圆角一致。
  const showCandidatePadding = host ? host.candidate_padding : true;
  const showCandidateItemGeometry = host ? host.candidate_item_geometry === true : macos;
  const showInputModeHUD = host ? host.input_mode_hud : false;
  const showInputModeStatusItem = host ? host.input_mode_status_item : false;
  const showVoiceCaptureDevices =
    !android && (host ? host.voice_capture_devices : false) && canListVoiceCaptureDevices;
  const showDesktopMaintenanceShortcuts = host
    ? host.maintenance_shortcuts || host.panel_windows
    : true;
  const showFullwidthChord = host ? host.fullwidth_chord : false;
  // 宿主把 Tab、PageUp/PageDown 两组翻页键（macOS、Windows、HarmonyOS）或中英混输的触发字符数（macOS、Windows、Linux）固定下来时不再给开关；没有宿主或宿主没写这两项时照旧显示。
  const showTabPagePagingSwitches = host?.tab_page_paging_switches ?? true;
  const showMixedInputMinimumPrefix = host?.mixed_input_minimum_prefix ?? true;
  const clientHostedPlatform =
    windows || linux || android || macos || harmony || host?.platform === "ios";
  const desktopPanels = host ? host.panel_windows : true;
  const showVoiceHotkeys = desktopPanels || host?.voice_hotkeys === true;
  // Nothing plays a pack or routes / and @ without a host, so these switches stay hidden there rather than doing nothing.
  const showKeySound = host ? host.key_sound : false;
  const showMusic = host ? host.music : false;
  const showPluginTriggers = host ? host.plugin_triggers : false;
  const showTypingEffects = host ? host.typing_effects : false;
  // Linux 只在候选辅助行显示连击数，不画闪光和火花，所以样式、特效包和强度都不起作用；连击开关和 host-api 从按键音效包播放的升档音仍然有效。
  const showTypingEffectStyles = showTypingEffects && !linux;
  // An effect pack only sets a drawn style's parameters, so it is offered wherever a style is.
  const showTypingEffectPacks = showTypingEffectStyles;
  // 宿主固定了效果强度（`HostCapabilities.fixed_effect_intensity`，macOS、Windows、鸿蒙）时不给「效果强度」；没有宿主时（网页预览）照样给出。
  const showEffectIntensity = host ? host.fixed_effect_intensity === undefined : true;
  // 宿主固定了 AI 联想的候选数量（`HostCapabilities.fixed_ai_candidate_limit`，macOS、Windows、Linux、鸿蒙）时不给「候选数量」。
  const showAiCandidateLimit = host ? host.fixed_ai_candidate_limit === undefined : true;
  // HarmonyOS 没有粒子浮层：每种样式都是候选卡片闪一下，火花和 Power Mode 闪得更亮，所以那里的样式说明不能许诺火花。Windows 和 macOS 一样在光标处画火花。
  const typingEffectsFlashOnly = showTypingEffectStyles && harmony;
  const showWordbookPacks = host ? host.wordbook_packs : false;
  const showSymbolSetPacks = host ? host.symbol_set_packs : false;
  // 触屏设置里的插件页（`HostCapabilities.touch_plugin_packs`）：没有宿主或宿主没报时不给。
  const showTouchPluginPacks = host?.touch_plugin_packs === true;
  // 没有宿主时不显示：脚本插件只有运行它们的桌面输入法进程才有意义。
  const showScriptPlugins = host?.script_plugins === true;
  // 更新通道只有用这份页面当设置界面的桌面宿主给：Linux 的设置应用按它检查、提醒，macOS 的 Sparkle 按它选 appcast。HarmonyOS 也加载这份页面，但它的发布不分通道，不给；Windows 的设置界面是原生 WinUI，自己画这一行。
  const showUpdateChannel = macos || linux;
  return {
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
    showAiGeneration,
    showCommunityContribution,
    showVoiceCommitMode,
    showVoiceProviderSettings,
    showVoiceStreamPreedit,
    showVoiceSpaceLockSwitch,
    showVoiceCueSwitches,
    showVoiceCaptureBackends,
    showVoicePauseSettings,
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
    showDesktopMaintenanceShortcuts,
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
    showTouchPluginPacks,
    showScriptPlugins,
    showUpdateChannel,
  } as const;
}
