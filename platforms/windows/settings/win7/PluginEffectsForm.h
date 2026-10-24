#pragma once

#include "PluginPreferencesModel.h"
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class PluginEffectsForm {
public:
  enum Id {
    key_enabled = 2400, mode, key_volume, commit_enabled, achievements_enabled,
    effect_style, combo_counter, combo_tier, music_enabled, music_volume, selected_packs, intensity,
  };
  struct Command { bool handled = false; std::vector<Change> changes; };
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;
  int create(const Row& row, int top = 0) {
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      controls_[id] = row(type, title, style, id, top, height);
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    const auto check = [&](int id, const wchar_t* title) { add(id, L"BUTTON", title, WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY); };
    const auto choice = [&](int id, const wchar_t* title) {
      row(L"STATIC", title, SS_LEFT, -1, top, 28); top += 40;
      add(id, WC_COMBOBOXW, title, WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 300);
    };
    check(key_enabled, L"按键音（安全输入框中不发声）");
    choice(mode, L"发声方式");
    for (const auto* title : {L"按键音效", L"按键旋律"}) SendMessageW(get(mode), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title));
    choice(key_volume, L"音效音量（按键、旋律、上屏和成就音效共用）");
    choice(music_volume, L"背景音乐音量");
    choice(intensity, L"打字效果强度");
    for (int value = 0; value <= 100; ++value) {
      const auto title = std::to_wstring(value) + L"%";
      for (const auto id : {key_volume, music_volume, intensity}) SendMessageW(get(id), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str()));
    }
    check(commit_enabled, L"上屏音"); check(achievements_enabled, L"成就音效（需要打开打字统计）");
    choice(effect_style, L"打字效果样式（选择特效包后由特效包决定）");
    for (const auto& item : plugin_preferences::model::effect_style_options)
      SendMessageW(get(effect_style), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(item.label.data()));
    check(combo_counter, L"连击计数（停顿或退格后重新计数）");
    check(combo_tier, L"连击升档音（需要打开连击计数）");
    check(music_enabled, L"背景音乐（输入法处于活动状态时播放）");
    add(selected_packs, L"STATIC", L"", SS_LEFT, 120);
    return top;
  }
  void refresh(const Json& preferences) {
    preferences_ = plugin_preferences::read(preferences);
    if (!preferences_) { SetWindowTextW(get(selected_packs), L"插件设置无法读取，请重新读取设置后重试。"); update(); return; }
    const auto& value = *preferences_;
    for (const auto& field : std::vector<std::pair<int, bool>>{{key_enabled, value.key_sound_enabled}, {commit_enabled, value.commit_sound_enabled},
        {achievements_enabled, value.achievements_enabled}, {combo_counter, value.combo_counter}, {combo_tier, value.combo_tier_sound}, {music_enabled, value.music_enabled}})
      SendMessageW(get(field.first), BM_SETCHECK, field.second ? BST_CHECKED : BST_UNCHECKED, 0);
    SendMessageW(get(mode), CB_SETCURSEL, value.key_sound_mode == L"melody" ? 1 : 0, 0);
    SendMessageW(get(key_volume), CB_SETCURSEL, static_cast<int>(value.key_sound_volume), 0);
    SendMessageW(get(music_volume), CB_SETCURSEL, static_cast<int>(value.music_volume), 0);
    SendMessageW(get(intensity), CB_SETCURSEL, static_cast<int>(value.effect_intensity), 0);
    for (size_t index = 0; index < plugin_preferences::model::effect_style_options.size(); ++index)
      if (plugin_preferences::model::effect_style_options[index].value == value.effect_style) SendMessageW(get(effect_style), CB_SETCURSEL, index, 0);
    const auto label = L"音效包：" + value.key_sound_pack + L"\r\n旋律包：" + value.melody_pack +
        L"\r\n音乐包：" + value.music_pack + L"\r\n特效包：" + value.effect_pack;
    SetWindowTextW(get(selected_packs), label.c_str()); update();
  }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  void catalog(std::vector<plugin_preferences::model::Package> packages, bool loaded) { packages_ = std::move(packages); listed_ = loaded; update(); }
  void statistics(std::optional<bool> enabled) { statistics_ = enabled; update(); }
  const std::optional<plugin_preferences::model::Preferences>& preferences() const { return preferences_; }
  Command command(WPARAM event) const {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !preferences_ || !IsWindowEnabled(get(id))) return result;
    auto next = *preferences_;
    const bool checked = SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED;
    if (notification == BN_CLICKED) {
      if (id == key_enabled) next.key_sound_enabled = checked;
      else if (id == commit_enabled) next.commit_sound_enabled = checked;
      else if (id == achievements_enabled) next.achievements_enabled = checked;
      else if (id == combo_counter) next.combo_counter = checked;
      else if (id == combo_tier) next.combo_tier_sound = checked;
      else if (id == music_enabled) {
        next.music_enabled = checked;
        if (checked && next.music_pack.empty()) next.music_pack = plugin_preferences::model::music_pack_to_select(packages_, listed_);
      } else return result;
    } else if (notification == CBN_SELCHANGE) {
      const auto index = SendMessageW(get(id), CB_GETCURSEL, 0, 0);
      if (id == mode && index >= 0 && index < 2) next.key_sound_mode = index == 1 ? L"melody" : L"keys";
      else if (id == effect_style && index >= 0 && static_cast<size_t>(index) < plugin_preferences::model::effect_style_options.size())
        next.effect_style = plugin_preferences::model::effect_style_options[static_cast<size_t>(index)].value;
      else if ((id == key_volume || id == music_volume || id == intensity) && index >= 0 && index <= 100) {
        if (id == key_volume) next.key_sound_volume = static_cast<double>(index);
        else if (id == intensity) next.effect_intensity = static_cast<double>(index); else next.music_volume = static_cast<double>(index);
      } else return result;
    } else return result;
    result.changes = plugin_preferences::changes(*preferences_, next); return result;
  }
private:
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  void update() {
    const bool enabled = enabled_ && preferences_.has_value();
    for (const auto& control : controls_) EnableWindow(control.second, enabled);
    if (!preferences_) return;
    const auto& value = *preferences_;
    EnableWindow(get(mode), enabled && value.key_sound_enabled);
    EnableWindow(get(effect_style), enabled && value.effect_pack.empty());
    EnableWindow(get(intensity), enabled && value.effect_pack.empty());
    EnableWindow(get(combo_tier), enabled && value.combo_counter);
    EnableWindow(get(achievements_enabled), enabled && !plugin_preferences::model::achievements_blocked(statistics_, value.achievements_enabled));
    const auto music = value.music_pack.empty() ? plugin_preferences::model::music_pack_to_select(packages_, listed_) : value.music_pack;
    EnableWindow(get(music_enabled), enabled && (value.music_enabled || !music.empty()));
  }
  std::map<int, HWND> controls_;
  std::optional<plugin_preferences::model::Preferences> preferences_;
  std::vector<plugin_preferences::model::Package> packages_;
  std::optional<bool> statistics_;
  bool enabled_ = false, listed_ = false;
};
} // namespace msime::settings::win7
