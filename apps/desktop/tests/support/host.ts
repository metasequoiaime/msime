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
    app_logo: macos,
    candidate_row_colors: false,
    candidate_selection_appearance: candidateSelectionAppearance,
    candidate_border_color: candidateSelectionAppearance,
    candidate_window_scale: false,
    candidate_window_opacity: false,
    candidate_corner_radius: false,
    candidate_follow_cursor: false,
    input_mode_hud: macos,
    candidate_english_font: platform === "windows" || macos || android,
    english_suggestions: android,
    helpcode_shift_entry: android || harmony,
    skin_directory_import: false,
    touch_toolbar_components: false,
    shuangpin_preedit: macos,
    maintenance_shortcuts: false,
    fullwidth_chord: macos,
    voice_provider_settings: !android,
    voice_stream_preedit: !android,
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
    ...fields,
    platform,
  };
}
