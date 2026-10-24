#pragma once

#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "ControlText.h"
#include "../CandidateFontPresets.h"
#include "../GameProcessList.h"
#include <commctrl.h>
#include <array>
#include <functional>
#include <map>

namespace msime::settings::win7 {

class CandidateAppearanceForm {
public:
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;
  struct Command {
    bool handled = false;
    bool fonts = false;
    std::vector<Change> changes;
    std::wstring message;
    bool game_profile = false;
  };

  int create(const Row& row) {
    int top = 0;
    const auto note = [&](const wchar_t* title, int height = 42) {
      row(L"STATIC", title, SS_LEFT, -1, top, height); top += height + 12;
    };
    const auto control = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      auto handle = row(type, title, style, id, top, height);
      controls_[id] = handle; top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 14;
      return handle;
    };
    note(L"候选窗口布局", 28);
    control(appearance_preview, L"STATIC", L"", SS_OWNERDRAW | WS_BORDER, 156);
    note(L"预览使用固定样例展示候选排列、字号和预编辑位置；实际颜色与皮肤由候选宿主绘制。", 42);
    for (const auto& field : enum_fields) {
      if (field.id != appearance_layout) continue;
      note(field.title, 24);
      add_options(control(field.id, WC_COMBOBOXW, field.title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 180), field);
    }
    note(L"每页候选项数量", 24);
    control(appearance_page_size, WC_COMBOBOXW, L"每页候选项数量", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 180);
    control(appearance_logo, L"BUTTON", L"显示水杉 logo（候选窗与悬浮工具栏）", WS_TABSTOP | BS_AUTOCHECKBOX);
    control(appearance_follow, L"BUTTON", L"候选窗口跟随光标", WS_TABSTOP | BS_AUTOCHECKBOX);
    note(L"关闭跟随光标后，保持首次出现的位置，直到候选窗口消失。");
    note(L"字体与大小", 28);
    note(L"候选字体预设", 24);
    const auto presets = control(appearance_font_preset, WC_COMBOBOXW, L"候选字体预设", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 200);
    for (const auto& preset : candidate_font_presets) {
      if (!candidate_font_preset_available(preset.id)) continue;
      presets_.emplace_back(preset.id);
      SendMessageW(presets, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(preset.label.data()));
    }
    SendMessageW(presets, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(L"自定义"));
    note(L"Windows 没有自带圆体；也可输入已安装的字体名称。主字体同时加入汉字回退列表。");
    note(L"主字体", 24);
    control(appearance_font_family, WC_COMBOBOXW, L"主字体", WS_TABSTOP | CBS_DROPDOWN | CBS_AUTOHSCROLL | WS_VSCROLL, 240);
    note(L"英文字体（优先用于候选和预编辑，缺字时回退主字体）", 42);
    control(appearance_english_font, WC_COMBOBOXW, L"英文字体", WS_TABSTOP | CBS_DROPDOWN | CBS_AUTOHSCROLL | WS_VSCROLL, 240);
    for (const int id : {appearance_font_family, appearance_english_font}) SendMessageW(get(id), CB_LIMITTEXT, 128, 0);
    control(appearance_fonts_save, L"BUTTON", L"保存字体", WS_TABSTOP | BS_PUSHBUTTON, 32);
    control(appearance_fonts_refresh, L"BUTTON", L"刷新系统字体列表", WS_TABSTOP | BS_PUSHBUTTON, 32);
    control(appearance_fonts_status, L"STATIC", L"可以直接输入字体名，或刷新列表后选择。", SS_LEFT, 48);
    for (const auto& field : number_fields) {
      note(field.title, 24);
      control(field.id, WC_COMBOBOXW, field.title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 200);
    }
    note(L"不透明度只改变窗口背景、边框和皮肤图；文字、编号和高亮保持不透明。圆角选择“跟随皮肤”时不覆盖皮肤或主题的值。", 66);
    note(L"预编辑", 28);
    for (const auto& field : enum_fields) {
      if (field.id == appearance_layout) continue;
      note(field.title, 24);
      add_options(control(field.id, WC_COMBOBOXW, field.title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 180), field);
    }
    note(L"游戏", 28);
    control(appearance_game_overlay, L"BUTTON", L"在游戏中显示候选窗口", WS_TABSTOP | BS_AUTOCHECKBOX);
    note(L"游戏声明自己绘制候选却不绘制时，由水杉显示候选。总是显示和从不显示列表优先；对已打开的游戏切换一次输入法后生效。独占全屏请改为无边框窗口。", 78);
    note(L"CS2 / Dota 2 原生候选兼容：将系统登记名称改为“拼音输入法”，让游戏自行绘制候选；需要管理员授权并重启游戏。关闭后恢复本版本名称，用户的总是显示列表仍优先。", 78);
    control(appearance_game_profile, L"BUTTON", L"启用兼容名称", WS_TABSTOP | BS_PUSHBUTTON, 32);
    for (const auto& field : game_lists) {
      note(field.title, 24);
      control(field.id, L"LISTBOX", field.title, WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 120);
      control(field.remove, L"BUTTON", L"移除选中的程序", WS_TABSTOP | BS_PUSHBUTTON, 32);
    }
    note(L"添加程序文件名，例如 game.exe（不填写路径）", 42);
    control(appearance_game_name, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(appearance_game_name), EM_LIMITTEXT, 256, 0);
    const auto target = control(appearance_game_list, WC_COMBOBOXW, L"添加到", WS_TABSTOP | CBS_DROPDOWNLIST, 160);
    for (const auto* title : {L"总是显示候选窗口", L"从不显示候选窗口"}) SendMessageW(target, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title));
    SendMessageW(target, CB_SETCURSEL, 0, 0);
    control(appearance_game_add, L"BUTTON", L"添加程序", WS_TABSTOP | BS_PUSHBUTTON, 32);
    return top;
  }

  static bool valid(const Json& preferences) {
    for (const auto& field : enum_fields) {
      if (!preferences.contains(field.key) || !preferences.at(field.key).is_string()) return false;
      bool known = false;
      for (size_t index = 0; index < field.count; ++index) known = known || preferences.at(field.key) == field.values[index];
      if (!known) return false;
    }
    for (const auto& field : number_fields) {
      if (!preferences.contains(field.key)) { if (field.optional) continue; return false; }
      if (!preferences.at(field.key).is_number_integer()) return false;
    }
    for (const auto* key : {"show_app_logo", "candidate_follow_cursor"})
      if (!preferences.contains(key) || !preferences.at(key).is_boolean()) return false;
    if (!preferences.contains("candidate_page_size") || !preferences.at("candidate_page_size").is_number_integer()) return false;
    if (!preferences.contains("candidate_font_family") || !preferences.at("candidate_font_family").is_string()) return false;
    if (preferences.contains("candidate_english_font") && !preferences.at("candidate_english_font").is_string()) return false;
    if (!preferences.contains("candidate_fallback_fonts") || !preferences.at("candidate_fallback_fonts").is_array()) return false;
    for (const auto& name : preferences.at("candidate_fallback_fonts")) if (!name.is_string()) return false;
    if (!preferences.contains("game_compatibility")) return true;
    const auto& games = preferences.at("game_compatibility");
    if (!games.is_object()) return false;
    if (games.contains("candidate_overlay") && !games.at("candidate_overlay").is_boolean()) return false;
    for (const auto& field : game_lists) {
      if (!games.contains(field.key)) continue;
      if (!games.at(field.key).is_array()) return false;
      for (const auto& name : games.at(field.key)) if (!name.is_string()) return false;
    }
    return true;
  }

  void refresh(const Json& preferences) {
    refreshing_ = true;
    preferences_ = preferences;
    for (const auto& field : enum_fields)
      for (size_t index = 0; index < field.count; ++index)
        if (preferences.at(field.key) == field.values[index]) SendMessageW(get(field.id), CB_SETCURSEL, index, 0);
    numbers(appearance_page_size, 3, 9, 1, preferences.at("candidate_page_size").get<int>(), false);
    for (const auto& field : number_fields)
      numbers(field.id, field.low, field.high, field.step, preferences.value(field.key, -1), field.optional);
    for (const auto& field : std::array<std::pair<int, const char*>, 2>{{{appearance_logo, "show_app_logo"}, {appearance_follow, "candidate_follow_cursor"}}})
      SendMessageW(get(field.first), BM_SETCHECK, preferences.at(field.second).get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
    const auto configuration = Json::array({preferences.at("candidate_font_family"), preferences.at("candidate_fallback_fonts"),
        preferences.value("candidate_english_font", "Segoe UI")});
    if (font_configuration_ != configuration) {
      font_configuration_ = configuration;
      SetWindowTextW(get(appearance_font_family), control_text::wide(preferences.at("candidate_font_family").get<std::string>()).c_str());
      SetWindowTextW(get(appearance_english_font), control_text::wide(preferences.value("candidate_english_font", "Segoe UI")).c_str());
    }
    const auto preset = current_candidate_font_preset(control_text::wide(preferences.at("candidate_font_family").get<std::string>()), fallback());
    auto choice = presets_.end();
    if (preset) choice = std::find(presets_.begin(), presets_.end(), *preset);
    SendMessageW(get(appearance_font_preset), CB_SETCURSEL, static_cast<WPARAM>(std::distance(presets_.begin(), choice)), 0);
    const auto games = game_preferences();
    SendMessageW(get(appearance_game_overlay), BM_SETCHECK, games.value("candidate_overlay", true) ? BST_CHECKED : BST_UNCHECKED, 0);
    for (const auto& field : game_lists) {
      const auto selected = SendMessageW(get(field.id), LB_GETCURSEL, 0, 0);
      SendMessageW(get(field.id), LB_RESETCONTENT, 0, 0);
      for (const auto& name : games.value(field.key, Json::array()))
        SendMessageW(get(field.id), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(control_text::wide(name.get<std::string>()).c_str()));
      SendMessageW(get(field.id), LB_SETCURSEL, selected, 0);
    }
    refreshing_ = false;
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto& item : controls_) EnableWindow(item.second, enabled);
    for (const auto& field : game_lists)
      EnableWindow(get(field.remove), enabled && SendMessageW(get(field.id), LB_GETCURSEL, 0, 0) != LB_ERR);
  }

  void font_catalog(const Json& response) {
    if (!response.is_object() || !response.value("ok", false) || !response.contains("value") || !response["value"].is_array()) {
      SetWindowTextW(get(appearance_fonts_status), L"字体列表读取失败，可重试或直接输入字体名。"); return;
    }
    refreshing_ = true;
    for (const int id : {appearance_font_family, appearance_english_font}) {
      const auto draft = control_text::read(get(id));
      SendMessageW(get(id), CB_RESETCONTENT, 0, 0);
      for (const auto& name : response["value"]) {
        if (!name.is_string()) continue;
        const auto value = control_text::wide(name.get_ref<const std::string&>());
        SendMessageW(get(id), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(value.c_str()));
      }
      if (draft) SetWindowTextW(get(id), control_text::wide(*draft).c_str());
    }
    refreshing_ = false;
    const auto message = L"已读取 " + std::to_wstring(response["value"].size()) + L" 个系统字体。编辑后点击“保存字体”。";
    SetWindowTextW(get(appearance_fonts_status), message.c_str());
  }

  void game_profile(bool enabled) { SetWindowTextW(get(appearance_game_profile), enabled ? L"恢复产品名称" : L"启用兼容名称"); }

  Command command(WPARAM event) {
    const int id = LOWORD(event), notice = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result{true, false, {}, {}};
    if (!enabled_ || refreshing_) return result;
    if (notice == LBN_SELCHANGE && (id == appearance_game_always || id == appearance_game_never)) {
      enable(enabled_); return result;
    }
    if (notice == CBN_SELCHANGE) {
      const auto selected = SendMessageW(get(id), CB_GETCURSEL, 0, 0);
      for (const auto& field : enum_fields)
        if (id == field.id && selected >= 0 && static_cast<size_t>(selected) < field.count)
          result.changes.push_back({{field.key}, field.values[static_cast<size_t>(selected)]});
      if (id == appearance_page_size || std::any_of(number_fields.begin(), number_fields.end(), [&](const auto& field) { return id == field.id; })) {
        const auto value = SendMessageW(get(id), CB_GETITEMDATA, selected, 0);
        if (selected != CB_ERR) {
          const char* key = "candidate_page_size";
          for (const auto& field : number_fields) if (id == field.id) key = field.key;
          result.changes.push_back({{key}, value == -1 ? Json(nullptr) : Json(static_cast<int>(value))});
        }
      }
      if (id == appearance_font_preset && selected >= 0 && static_cast<size_t>(selected) < presets_.size())
        result.changes = font_patch(candidate_font_preset_patch(presets_[static_cast<size_t>(selected)], fallback()));
    } else if (notice == BN_CLICKED) {
      if (id == appearance_fonts_refresh) result.fonts = true;
      else if (id == appearance_game_profile) result.game_profile = true;
      else if (id == appearance_fonts_save) {
        const auto main = control_text::read(get(appearance_font_family));
        const auto english = control_text::read(get(appearance_english_font));
        if (!main || !english || !valid_font_family(control_text::wide(*main)) || !valid_font_family(control_text::wide(*english)))
          result.message = L"字体名不能为空、不能含控制字符，且 UTF-8 长度最多为 128 字节。";
        else {
          result.changes = font_patch(candidate_main_font_patch(control_text::wide(*main),
              control_text::wide(preferences_.at("candidate_font_family").get<std::string>()), fallback()));
          result.changes.push_back({{"candidate_english_font"}, *english});
        }
      } else if (id == appearance_logo || id == appearance_follow)
        result.changes.push_back({{id == appearance_logo ? "show_app_logo" : "candidate_follow_cursor"}, SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED});
      else if (id == appearance_game_overlay) {
        auto games = game_preferences();
        games["candidate_overlay"] = SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED;
        result.changes.push_back({{"game_compatibility"}, std::move(games)});
      } else if (id == appearance_game_add) {
        const auto raw = control_text::read(get(appearance_game_name));
        const auto name = raw ? normalize_game_process(control_text::wide(*raw)) : std::wstring{};
        auto games = game_preferences();
        std::array<std::vector<std::wstring>, 2> lists;
        for (size_t index = 0; index < game_lists.size(); ++index)
          for (const auto& entry : games.value(game_lists[index].key, Json::array())) lists[index].push_back(control_text::wide(entry.get<std::string>()));
        if (const auto error = validate_game_process(name, lists[0], lists[1])) result.message = game_error(*error);
        else if (const auto key = control_text::utf8(name)) {
          const auto target = SendMessageW(get(appearance_game_list), CB_GETCURSEL, 0, 0);
          if (target >= 0 && target <= 1) {
            const auto field = game_lists[static_cast<size_t>(target)].key;
            auto entries = games.value(field, Json::array()); entries.push_back(*key); games[field] = std::move(entries);
            result.changes.push_back({{"game_compatibility"}, std::move(games)});
          }
        }
      } else for (const auto& field : game_lists) if (id == field.remove) {
        const auto index = SendMessageW(get(field.id), LB_GETCURSEL, 0, 0);
        auto games = game_preferences();
        auto entries = games.value(field.key, Json::array());
        if (index >= 0 && static_cast<size_t>(index) < entries.size()) {
          entries.erase(entries.begin() + index); games[field.key] = std::move(entries);
          result.changes.push_back({{"game_compatibility"}, std::move(games)});
        }
      }
    }
    return result;
  }

private:
  struct EnumField { int id; const char* key; const wchar_t* title; size_t count; std::array<const char*, 3> values; std::array<const wchar_t*, 3> titles; };
  static constexpr std::array<EnumField, 3> enum_fields{{
      {appearance_layout, "candidate_layout", L"候选项排列方式", 2, {"horizontal", "vertical"}, {L"横向", L"纵向"}},
      {appearance_preedit, "candidate_preedit_style", L"候选窗口预编辑", 2, {"pinyin", "empty"}, {L"拼音分词", L"不显示"}},
      {appearance_inline, "tsf_preedit_style", L"行内预编辑", 3, {"raw", "pinyin", "empty"}, {L"原始按键", L"拼音分词", L"不显示"}},
  }};
  struct NumberField { int id; const char* key; const wchar_t* title; int low; int high; int step; bool optional = false; };
  static constexpr std::array<NumberField, 5> number_fields{{
      {appearance_font_size, "candidate_font_size", L"候选字号", 12, 32, 1},
      {appearance_preedit_size, "candidate_preedit_font_size", L"预编辑字号", 12, 32, 1},
      {appearance_scale, "candidate_scale_percent", L"整体大小（%）", 75, 150, 5},
      {appearance_opacity, "candidate_opacity_percent", L"不透明度（%）", 50, 100, 5},
      {appearance_radius, "candidate_corner_radius", L"圆角大小（DIP）", 0, 16, 1, true},
  }};
  struct GameList { int id; int remove; const char* key; const wchar_t* title; };
  static constexpr std::array<GameList, 2> game_lists{{
      {appearance_game_always, appearance_game_always_remove, "overlay_processes", L"总是显示候选窗口的程序"},
      {appearance_game_never, appearance_game_never_remove, "excluded_processes", L"从不显示候选窗口的程序"},
  }};
  static void add_options(HWND control, const EnumField& field) {
    for (size_t index = 0; index < field.count; ++index) SendMessageW(control, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(field.titles[index]));
  }
  void numbers(int id, int low, int high, int step, int current, bool optional) {
    const auto control = get(id);
    SendMessageW(control, CB_RESETCONTENT, 0, 0);
    const auto add = [&](int value, const std::wstring& title) {
      const auto index = SendMessageW(control, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str()));
      SendMessageW(control, CB_SETITEMDATA, index, value);
      if (current == value) SendMessageW(control, CB_SETCURSEL, index, 0);
    };
    if (optional) add(-1, L"跟随皮肤");
    for (int value = low; value <= high; value += step) add(value, std::to_wstring(value));
    if (current != -1 && (current < low || current > high || (current - low) % step != 0))
      add(current, std::to_wstring(current) + L"（当前值）");
  }
  std::vector<std::wstring> fallback() const {
    std::vector<std::wstring> names;
    for (const auto& name : preferences_.at("candidate_fallback_fonts")) names.push_back(control_text::wide(name.get<std::string>()));
    return names;
  }
  static std::vector<Change> font_patch(const CandidateFontFamilies& patch) {
    const auto family = control_text::utf8(patch.family);
    if (!family) return {};
    Json fonts = Json::array();
    for (const auto& name : patch.fallback) { const auto value = control_text::utf8(name); if (!value) return {}; fonts.push_back(*value); }
    return {{{"candidate_font_family"}, *family}, {{"candidate_fallback_fonts"}, std::move(fonts)}};
  }
  Json game_preferences() const { return preferences_.value("game_compatibility", Json::object()); }
  static const wchar_t* game_error(GameProcessError error) {
    switch (error) {
    case GameProcessError::Empty: return L"请填写程序文件名，例如 game.exe。";
    case GameProcessError::TooLong: return L"程序文件名不能超过 64 个字符。";
    case GameProcessError::InvalidCharacter: return L"只填写程序文件名，不能含路径或非法字符。";
    case GameProcessError::NotExe: return L"程序文件名必须以 .exe 结尾。";
    case GameProcessError::Duplicate: return L"这个程序已经在总是显示或从不显示列表里。";
    case GameProcessError::TooMany: return L"两张程序列表合计最多 32 项，请先移除不再使用的程序。";
    }
    return L"无法添加这个程序。";
  }
  HWND get(int id) const { return controls_.at(id); }
  std::map<int, HWND> controls_;
  std::vector<std::wstring> presets_;
  Json preferences_;
  Json font_configuration_;
  bool enabled_ = false;
  bool refreshing_ = false;
};

} // namespace msime::settings::win7
