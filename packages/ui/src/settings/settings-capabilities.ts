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

/** Derives UI capability flags from the host contract and legacy platform fallbacks. */
export function settingsCapabilities({
  host,
  linux,
  android,
  harmony,
  windows,
  macos,
  mobile,
  canRestartInputMethod,
  canInstallInputSource,
  canListVoiceCaptureDevices,
}: SettingsCapabilitiesInput) {
  const nativeVoicePlatform = macos || harmony || android;
  const showModeScope = host ? host.ime_mode_scope : linux;
  const showModeSwitchShortcuts = host ? host.mode_switch_shortcuts : linux;
  const showPanelShortcuts = host ? host.panel_shortcuts : linux;
  const showNumberRowSelection = host ? host.number_row_selection === true : linux;
  const showRestartInputMethod =
    (host ? host.restart_input_method : linux) && canRestartInputMethod;
  const showInstallInputSource = macos && canInstallInputSource;
  const showFloatingToolbar = host ? host.floating_toolbar : true;
  const showToolbarAppearance = host ? host.floating_toolbar_appearance : true;
  const showToolbarComponents = host ? host.floating_toolbar_components : true;
  const showCandidateFontControls = host ? host.candidate_font_controls : true;
  const showCandidatePreeditFont =
    showCandidateFontControls && (host?.candidate_preedit_font ?? true);
  const showCandidateEnglishFont = host?.candidate_english_font ?? (windows || macos || android);
  const showEnglishSuggestions = host?.english_suggestions ?? android;
  const showHelpcodeShiftEntry = host?.helpcode_shift_entry ?? android;
  const showShuangpinPreedit = host?.shuangpin_preedit ?? macos;
  const showCharacterWidth = host?.character_width ?? !mobile;
  const aiProviderCredentials = host?.ai_provider_credentials ?? linux;
  const showVoiceCommitMode = host?.voice_commit_mode ?? (!android && !linux);
  const showVoiceProviderSettings = host?.voice_provider_settings ?? !android;
  const showVoiceStreamPreedit = host?.voice_stream_preedit ?? !android;
  const showCandidateRowColors = host ? host.candidate_row_colors : true;
  const showCandidateSelectionAppearance = host ? host.candidate_selection_appearance : true;
  const showCandidateBorderColor = host
    ? (host.candidate_border_color ?? host.candidate_selection_appearance)
    : true;
  const showCandidateFollowCursor = host ? host.candidate_follow_cursor : false;
  // A host older than these fields cannot draw them, so an absent flag hides the control.
  const showCandidateWindowScale = host ? host.candidate_window_scale === true : true;
  const showCandidateWindowOpacity = host ? host.candidate_window_opacity === true : true;
  const showCandidateCornerRadius = host ? host.candidate_corner_radius === true : true;
  const showInputModeHUD = host?.input_mode_hud ?? macos;
  const showVoiceCaptureDevices =
    !android && (host ? host.voice_capture_devices : linux) && canListVoiceCaptureDevices;
  const showDesktopMaintenanceShortcuts =
    (host?.maintenance_shortcuts ?? false) || !host || host.panel_windows;
  const showFullwidthChord = host?.fullwidth_chord ?? macos;
  const clientHostedPlatform =
    windows || linux || android || macos || harmony || host?.platform === "ios";
  const desktopPanels = host ? (host.panel_windows ?? !mobile) : true;
  const showVoiceHotkeys = desktopPanels || host?.voice_hotkeys === true;
  // A host older than these fields sends none of them, and nothing there plays a pack or routes / and @: the switches stay hidden rather than doing nothing.
  const showKeySound = host?.key_sound === true;
  const showMusic = host?.music === true;
  const showPluginTriggers = host?.plugin_triggers === true;
  const showTypingEffects = host?.typing_effects === true;
  // Linux shows the combo count as text in the candidate panel's aux line and draws no flash or sparks, so only the counter switch has an effect there.
  const showTypingEffectStyles = showTypingEffects && !linux;
  // An effect pack only sets a drawn style's parameters, so it is offered wherever a style is.
  const showTypingEffectPacks = showTypingEffectStyles;
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
  } as const;
}
