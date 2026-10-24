#pragma once

#include <windows.h>
#include "McpPage.h"
#include <filesystem>
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <string_view>

namespace msime::settings::win7 {

// 控件标识也用于原生验收，操作实际 HWND 和工作线程，不绕过窗口事件。
enum ControlId {
  page_body = 99, language_switch = 100, caps_lock, japanese_toggle, traditional_toggle, fullwidth_toggle,
  reload_settings, page_tabs, status_text, reference_text, page_selector,
  page_navigation = 120, page_heading, page_description,
  toolbar_enabled = 200, toolbar_language, toolbar_scheme, toolbar_fullwidth,
  toolbar_punctuation, toolbar_character_set, toolbar_emoji, toolbar_handwriting,
  toolbar_keyboard, toolbar_voice, toolbar_settings, toolbar_size, toolbar_summary,
  toolbar_preview, toolbar_preview_description, toolbar_theme,
  diagnostic_server = 300, diagnostic_tsf, restart_service, open_data_directory,
  open_logs_directory, export_settings, import_settings, data_directory_text,
  word_character_keys = 400, backspace_reverts, navigation_minus_equal, navigation_comma_period,
  navigation_brackets, navigation_mouse_wheel, navigation_arrows, navigation_left_right,
  traditional_output = 500, output_description, chinese_english_spacing, fullwidth_output,
  punctuation_mode = 600, quote_style, bracket_style, paired_punctuation, smart_punctuation,
  smart_description, smart_repeat, smart_space_convert, smart_direct_digit, smart_direct_letter,
  cloud_candidates = 700, cloud_source, cloud_endpoint, cloud_method, cloud_headers, cloud_body,
  cloud_response_format, cloud_response_path, cloud_timeout, cloud_cache, cloud_save,
  mixed_enabled = 750, mixed_english, mixed_first_page_count, mixed_emoji, mixed_symbols,
  mixed_kaomoji, mixed_japanese, learn_candidates, single_character_only, word_lattice,
  default_ime_mode = 800, ime_mode_scope, input_mode_hud, app_rule_name, app_rule_add,
  app_rule_list, app_rule_mode, app_rule_remove,
  shift_letter_modes = 900, local_mode_first = 910, fuzzy_enabled = 930, fuzzy_reset,
  fuzzy_rule_first = 940, frequency_mode = 960, frequency_trigger, frequency_step,
  helpcode_first = 980,
  scheme_language = 1000, scheme_chinese, scheme_description, scheme_shuangpin_profile,
  scheme_shuangpin_hint, scheme_shuangpin_preedit, scheme_wubi_profile, scheme_wubi_mixed,
  scheme_wubi_hint, scheme_wubi_unique, scheme_wubi_selection, scheme_wubi_second,
  scheme_wubi_third, scheme_wubi_save, scheme_vietnamese_method, scheme_vietnamese_tone,
  scheme_wubi_lookup_text, scheme_wubi_lookup, scheme_wubi_result,
  gloss_english = 1100, gloss_translations, gloss_pronunciation, gloss_target, gloss_secondary,
  translation_provider_control = 1200, translation_tencent_first = 1210,
  translation_niutrans_first = 1230, translation_custom_first = 1250,
  appearance_layout = 1300, appearance_page_size, appearance_logo, appearance_follow,
  appearance_font_preset, appearance_font_family, appearance_english_font, appearance_fonts_save,
  appearance_fonts_refresh, appearance_fonts_status, appearance_font_size, appearance_preedit_size,
  appearance_scale, appearance_opacity, appearance_radius, appearance_preedit, appearance_inline,
  appearance_game_overlay, appearance_game_always, appearance_game_always_remove,
  appearance_game_never, appearance_game_never_remove, appearance_game_name, appearance_game_list,
  appearance_game_add, appearance_game_profile, appearance_preview,
  ai_enabled = 1400, ai_provider, ai_endpoint, ai_endpoint_hint, ai_token,
  ai_token_reveal, ai_token_save, ai_model, ai_service_save, ai_models_fetch,
  ai_models_status, ai_prompt, ai_prompt_save, ai_test, ai_test_result,
  account_status = 1500, account_refresh, account_email, account_phone, account_target,
  account_request_code, account_code, account_login, account_google, account_logout,
  account_logout_all, account_notice,
  account_profile, account_profile_text, account_name, account_rename, account_forget,
  account_delete, account_sync_load, account_sync_upload, account_sync_apply, account_sync_notice,
  account_google_cancel,
  account_avatar, account_avatar_load, account_avatar_upload, account_avatar_remove,
  theme_mode = 1600, theme_candidate_mode, theme_global, theme_preview_mode,
  theme_preview, theme_skins, theme_skin_use, theme_skin_stop, theme_skin_refresh,
  theme_skin_import, theme_skin_directory, theme_skin_notice, theme_slot_summary,
  theme_color_first = 1640,
  community_search = 1700, community_filter, community_mine, community_reload,
  community_list, community_more, community_notice, community_detail_text, community_detail,
  community_install, community_local, community_login, community_stars, community_rate,
  community_visibility, community_visibility_save, community_category, community_category_save,
  community_unpublish, community_sync, community_report_reason, community_report_detail,
  community_report, community_preview_image, community_preview_load, community_publish_skin,
  community_publish_name, community_publish_description, community_publish_visibility,
  community_publish_category, community_publish_license, community_license_save,
  community_preview_add, community_pack_check, community_publish_agree, community_publish,
  statistics_enabled = 1800, statistics_privacy, statistics_retention, statistics_upload,
  statistics_upload_notice, statistics_overview, statistics_scope, statistics_chart_kind,
  statistics_chart, statistics_chart_text, statistics_days, statistics_day_detail,
  statistics_notice, statistics_reset, statistics_calendar, statistics_speed,
  account_qq_refresh = 1900, account_qq_list, account_qq_code, account_qq_redeem,
  account_qq_remove, account_qq_notice, account_devices_refresh, account_devices_list,
  account_device_detail, account_device_revoke, account_devices_notice,
  account_data_refresh, account_data_summary, account_data_export, account_data_delete,
  account_voice_retry, account_data_notice, account_data_section_first = 1930,
  clipboard_enabled = 2000, clipboard_limit, clipboard_refresh, clipboard_page,
  clipboard_previous, clipboard_next, clipboard_list, clipboard_detail,
  clipboard_pin, clipboard_copy, clipboard_remove, clipboard_clear, clipboard_notice,
  clipboard_timestamp,
  cloud_clipboard_source = 2100, cloud_clipboard_relay_url, cloud_clipboard_relay_token,
  cloud_clipboard_relay_save, cloud_clipboard_relay_clear, cloud_clipboard_refresh,
  cloud_clipboard_status, cloud_clipboard_enabled, cloud_clipboard_retention,
  cloud_clipboard_search, cloud_clipboard_find, cloud_clipboard_items, cloud_clipboard_detail,
  cloud_clipboard_copy, cloud_clipboard_pin, cloud_clipboard_delete, cloud_clipboard_clear,
  cloud_clipboard_text, cloud_clipboard_add, cloud_clipboard_devices, cloud_clipboard_device_detail,
  cloud_clipboard_approve, cloud_clipboard_reject, cloud_clipboard_remove,
  cloud_clipboard_reset, cloud_clipboard_notice, cloud_clipboard_metadata,
  dictionary_kind = 2200, dictionary_user_only, dictionary_query, dictionary_search,
  dictionary_page, dictionary_previous, dictionary_next, dictionary_entries,
  dictionary_entry_source, dictionary_key, dictionary_value, dictionary_weight,
  dictionary_new, dictionary_save, dictionary_delete, dictionary_format,
  dictionary_import, dictionary_export, dictionary_reset, dictionary_notice, dictionary_export_all,
  dictionary_updates_enabled = 2250, dictionary_updates_version, dictionary_updates_refresh,
  dictionary_updates_check, dictionary_updates_rollback, dictionary_updates_notice,
};

class SettingsWindow {
public:
  // 默认使用系统文件选择器；原生集成测试可注入合成路径，继续走正式文件与存储流程。
  using FilePicker = std::function<std::optional<std::filesystem::path>(HWND, bool)>;
  using Confirmation = std::function<bool(HWND, const wchar_t*, const wchar_t*)>;
  using FolderPicker = std::function<std::optional<std::filesystem::path>(HWND)>;
  using SurfaceLauncher = std::function<bool(std::string_view)>;
  // 可注入社区传输以验证真实窗口与 worker；默认仍调用共享 Rust 服务。
  using CommunityTransport = std::function<std::string(const std::string&)>;
  struct ClipboardServices {
    CommunityTransport transport;
    std::function<bool(HWND, std::wstring_view)> copy;
  };
  struct CloudClipboardServices {
    CommunityTransport account;
    CommunityTransport relay;
  };
  explicit SettingsWindow(FilePicker file_picker = {}, Confirmation confirmation = {}, FolderPicker folder_picker = {},
      CommunityTransport community_transport = {}, CommunityTransport statistics_upload_transport = {}, CommunityTransport account_transport = {}, ClipboardServices clipboard = {}, CloudClipboardServices cloud_clipboard = {}, CommunityTransport dictionary_transport = {}, CommunityTransport update_transport = {}, CommunityTransport voice_transport = {}, SurfaceLauncher surface_launcher = {}, McpPage::Services mcp_services = {});
  ~SettingsWindow();
  SettingsWindow(const SettingsWindow&) = delete;
  SettingsWindow& operator=(const SettingsWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  bool open_route(std::string_view route);
  HWND handle() const;

private:
  struct State;
  std::unique_ptr<State> state_;
};

} // namespace msime::settings::win7
