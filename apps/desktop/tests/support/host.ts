import type { HostCapabilities } from "@msime/ui";

/** A complete host for a settings test that names only the platform and the fields it is about. Every other field takes a default for that platform: off for an opt-in surface, and the platform's usual answer where the field tells the platforms apart. */
export function testHost(
  fields: Omit<Partial<HostCapabilities>, "platform"> & { platform: string },
): HostCapabilities {
  const platform = fields.platform as HostCapabilities["platform"];
  const android = platform === "android";
  const harmony = platform === "harmony";
  const linux = platform === "linux";
  const macos = platform === "macos";
  const windows = platform === "windows";
  const mobile = android || harmony || platform === "ios";
  const candidateSelectionAppearance = fields.candidate_selection_appearance ?? false;
  return {
    mobile_settings: mobile,
    restart_input_method: false,
    panel_windows: !mobile,
    ime_mode_scope: false,
    typing_statistics: false,
    vocabulary_review: false,
    fuzzy_pinyin: false,
    system_fonts: false,
    window_chrome: false,
    floating_toolbar: false,
    floating_toolbar_appearance: false,
    floating_toolbar_components: false,
    floating_toolbar_handwriting: false,
    floating_toolbar_voice: false,
    floating_toolbar_input_scheme: false,
    mode_switch_shortcuts: false,
    panel_shortcuts: false,
    number_row_selection: false,
    voice_capture_devices: false,
    candidate_font_controls: false,
    candidate_preedit_font: true,
    candidate_page_number: false,
    app_logo: macos || platform === "windows" || linux,
    expand_candidates: macos,
    // 桌面宿主（macOS、Windows、Linux、HarmonyOS）的各界面一律跟随颜色模式，只有候选窗口仍能单独设明暗（host_surface.rs）。
    surface_themes_follow: !android && platform !== "ios",
    candidate_row_colors: false,
    candidate_selection_appearance: candidateSelectionAppearance,
    // 桌面宿主的编号色和边框色固定跟随主题，宿主不读这两个槽位，只有 Android 和 iOS 照旧读（host_surface.rs）。
    candidate_border_color: candidateSelectionAppearance && (android || platform === "ios"),
    candidate_number_color: android || platform === "ios",
    candidate_window_scale: false,
    candidate_window_opacity: false,
    candidate_corner_radius: false,
    candidate_padding: false,
    candidate_follow_cursor: false,
    input_mode_hud: macos,
    input_mode_status_item: macos,
    candidate_english_font: platform === "windows" || macos || android,
    english_suggestions: android,
    helpcode_shift_entry: android || harmony,
    skin_directory_import: false,
    touch_toolbar_components: false,
    // 与 client-core 一致：macOS 和 Linux 的键盘窗口高度固定，macOS 的间距固定成默认值。
    touch_keyboard_height: !macos && !linux,
    touch_keyboard_spacing: !macos,
    shuangpin_preedit: macos,
    shuangpin_keymap_hint: macos || windows || linux,
    maintenance_shortcuts: false,
    fullwidth_chord: macos,
    voice_provider_settings: !android,
    voice_stream_preedit: !android,
    // 空格锁定和提示音细分只有 Android、iOS 还读存下的值，录音后端只有 Linux 有得选，与 `HostCapabilities::for_platform` 一致。
    voice_space_lock_switch: android || platform === "ios",
    voice_cue_switches: android || platform === "ios",
    voice_capture_backends: !(macos || platform === "windows" || harmony),
    character_width: !mobile,
    ai_provider_credentials: linux,
    voice_commit_mode: !android && !linux,
    key_sound: false,
    plugin_triggers: false,
    music: false,
    typing_effects: false,
    wordbook_packs: false,
    symbol_set_packs: false,
    input_schemes: ["quanpin", "shuangpin", "wubi", "japanese", "korean"],
    // 与 client-core 的 `HostPlatform::fixed_*` 一致：macOS、Windows、Linux、鸿蒙固定 AI 候选数量，macOS、Windows、鸿蒙固定打字效果强度，Android、iOS 都按偏好。
    ...(macos || windows || linux || harmony ? { fixed_ai_candidate_limit: 3 } : {}),
    ...(macos || windows || harmony ? { fixed_effect_intensity: 50 } : {}),
    ...fields,
    platform,
  };
}
