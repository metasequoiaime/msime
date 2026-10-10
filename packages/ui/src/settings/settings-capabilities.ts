import type { HostCapabilities } from "../index";

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
  const showToolbarComponents = host ? host.floating_toolbar_components : true;
  const showCandidateFontControls = host ? host.candidate_font_controls : true;
  const showCandidatePreeditFont =
    showCandidateFontControls && (host ? host.candidate_preedit_font : true);
  const showCandidateEnglishFont = host ? host.candidate_english_font : false;
  const showEnglishSuggestions = host ? host.english_suggestions : false;
  const showHelpcodeShiftEntry = host ? host.helpcode_shift_entry : false;
  const showShuangpinPreedit = host ? host.shuangpin_preedit : false;
  // 只有画双拼键位图的宿主（macOS、Windows）给出「输入时显示双拼键位提示」。
  const showShuangpinKeymapHint = host ? host.shuangpin_keymap_hint === true : false;
  const showCharacterWidth = host ? host.character_width : true;
  const aiProviderCredentials = host ? host.ai_provider_credentials : false;
  const showVoiceCommitMode = host ? host.voice_commit_mode : true;
  const showVoiceProviderSettings = host ? host.voice_provider_settings : true;
  const showVoiceStreamPreedit = host ? host.voice_stream_preedit : true;
  const showCandidateRowColors = host ? host.candidate_row_colors : true;
  const showCandidateSelectionAppearance = host ? host.candidate_selection_appearance : true;
  const showCandidateBorderColor = host ? host.candidate_border_color : true;
  const showCandidateFollowCursor = host ? host.candidate_follow_cursor : false;
  const showCandidateWindowScale = host ? host.candidate_window_scale : true;
  const showCandidateWindowOpacity = host ? host.candidate_window_opacity : true;
  const showCandidateCornerRadius = host ? host.candidate_corner_radius : true;
  const showInputModeHUD = host ? host.input_mode_hud : false;
  const showVoiceCaptureDevices =
    !android && (host ? host.voice_capture_devices : false) && canListVoiceCaptureDevices;
  const showDesktopMaintenanceShortcuts = host
    ? host.maintenance_shortcuts || host.panel_windows
    : true;
  const showFullwidthChord = host ? host.fullwidth_chord : false;
  const clientHostedPlatform =
    windows || linux || android || macos || harmony || host?.platform === "ios";
  const desktopPanels = host ? host.panel_windows : true;
  const showVoiceHotkeys = desktopPanels || host?.voice_hotkeys === true;
  // Nothing plays a pack or routes / and @ without a host, so these switches stay hidden there rather than doing nothing.
  const showKeySound = host ? host.key_sound : false;
  const showMusic = host ? host.music : false;
  const showPluginTriggers = host ? host.plugin_triggers : false;
  const showTypingEffects = host ? host.typing_effects : false;
  // Linux shows the combo count as text in the candidate panel's aux line and draws no flash or sparks, so only the counter switch has an effect there.
  const showTypingEffectStyles = showTypingEffects && !linux;
  // An effect pack only sets a drawn style's parameters, so it is offered wherever a style is.
  const showTypingEffectPacks = showTypingEffectStyles;
  // HarmonyOS 没有粒子浮层：每种样式都是候选卡片闪一下，火花和 Power Mode 闪得更亮，所以那里的样式说明不能许诺火花。Windows 和 macOS 一样在光标处画火花。
  const typingEffectsFlashOnly = showTypingEffectStyles && harmony;
  const showWordbookPacks = host ? host.wordbook_packs : false;
  const showSymbolSetPacks = host ? host.symbol_set_packs : false;
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
    showToolbarComponents,
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
    showCandidateRowColors,
    showCandidateSelectionAppearance,
    showCandidateBorderColor,
    showCandidateFollowCursor,
    showCandidateWindowScale,
    showCandidateWindowOpacity,
    showCandidateCornerRadius,
    showInputModeHUD,
    showVoiceCaptureDevices,
    showDesktopMaintenanceShortcuts,
    showFullwidthChord,
    clientHostedPlatform,
    desktopPanels,
    showVoiceHotkeys,
    showKeySound,
    showMusic,
    showPluginTriggers,
    showTypingEffects,
    showTypingEffectStyles,
    showTypingEffectPacks,
    typingEffectsFlashOnly,
    showWordbookPacks,
    showSymbolSetPacks,
  } as const;
}
