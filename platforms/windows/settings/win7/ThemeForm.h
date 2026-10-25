#pragma once

#include "ControlText.h"
#include "SettingsWindow.h"
#include "ThemeData.h"
#include "../ThemeSkinModel.h"
#include "../../src/candidate/CandidatePalette.h"
#include <commctrl.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <functional>
#include <map>

namespace msime::settings::win7 {

class ThemeForm {
public:
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    bool scan = false;
    bool import = false;
    bool directory = false;
    std::optional<int> preview;
    std::wstring message;
  };
  int create(const Row& row) {
    int top = 0;
    const auto note = [&](const wchar_t* title, int height = 28) {
      row(L"STATIC", title, SS_LEFT, -1, top, height); top += height + 10;
    };
    const auto control = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto handle = row(type, title, style, id, top, height);
      controls_[id] = handle; top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
      return handle;
    };
    const auto combo = [&](int id, const wchar_t* title, std::initializer_list<const wchar_t*> titles) {
      note(title);
      const auto handle = control(id, WC_COMBOBOXW, title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 220);
      for (const auto* text : titles) SendMessageW(handle, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text));
    };
    note(L"选择皮肤与配色；候选布局、字体和显示方式在「候选窗口」中设置。", 48);
    combo(theme_mode, L"颜色模式", {L"跟随系统", L"浅色", L"深色"});
    combo(theme_candidate_mode, L"候选窗口明暗", {L"跟随颜色模式", L"浅色", L"深色"});
    combo(theme_global, L"全局主题", {});
    combo(theme_preview_mode, L"预览明暗（仅影响此预览）", {L"跟随当前设置", L"预览浅色", L"预览深色"});
    SendMessageW(get(theme_preview_mode), CB_SETCURSEL, 0, 0);
    control(theme_preview, L"STATIC", L"主题候选预览", SS_OWNERDRAW | WS_BORDER, 228);
    control(theme_slot_summary, L"STATIC", L"", SS_LEFT, 64);
    note(L"本地皮肤");
    control(theme_skins, L"LISTBOX", L"本地皮肤", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 144);
    control(theme_skin_notice, L"STATIC", L"正在读取皮肤目录…", SS_LEFT, 112);
    control(theme_skin_use, L"BUTTON", L"使用选中的皮肤", WS_TABSTOP | BS_PUSHBUTTON, 32);
    control(theme_skin_stop, L"BUTTON", L"停用选中的皮肤", WS_TABSTOP | BS_PUSHBUTTON, 32);
    control(theme_skin_refresh, L"BUTTON", L"刷新皮肤", WS_TABSTOP | BS_PUSHBUTTON, 32);
    control(theme_skin_import, L"BUTTON", L"导入皮肤文件夹…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    control(theme_skin_directory, L"BUTTON", L"打开皮肤目录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    note(L"自定义配色");
    note(L"输入 #RRGGBB 后保存；“跟随主题”清除该项覆盖。未修改的皮肤与键盘设计会保留。", 48);
    for (size_t index = 0; index < std::size(skins::color_slots); ++index) {
      const auto& slot = skins::color_slots[index];
      note(slot.title.data());
      const int id = theme_color_first + static_cast<int>(index) * 3;
      const auto edit = control(id, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
      SendMessageW(edit, EM_LIMITTEXT, 7, 0);
      control(id + 1, L"BUTTON", L"保存颜色", WS_TABSTOP | BS_PUSHBUTTON, 32);
      control(id + 2, L"BUTTON", slot.reset_label.empty() ? L"跟随主题" : slot.reset_label.data(), WS_TABSTOP | BS_PUSHBUTTON, 32);
    }
    return top;
  }
  static bool valid(const Json& preferences) {
    for (const auto* key : {"theme", "candidate_theme", "global_theme"})
      if (preferences.contains(key) && !preferences.at(key).is_string()) return false;
    return !preferences.contains("custom_theme") || preferences.at("custom_theme").is_object();
  }
  void refresh(const Json& preferences) {
    const auto previous = preferences_;
    preferences_ = preferences;
    select(theme_mode, mode_values, preferences.value("theme", std::string("system")));
    select(theme_candidate_mode, candidate_values, preferences.value("candidate_theme", std::string("follow")));
    select_theme();
    const auto custom = preferences.value("custom_theme", Json::object());
    if (previous.is_null() || previous.value("custom_theme", Json::object()) != custom) {
      const auto colors = custom.value("candidate_colors", Json::object());
      for (size_t index = 0; index < std::size(skins::color_slots); ++index) {
        const auto key = control_text::utf8(skins::color_slots[index].key).value();
        const auto found = colors.find(key);
        const auto text = found != colors.end() && found->is_string() ? found->get<std::string>() : std::string{};
        SetWindowTextW(get(theme_color_first + static_cast<int>(index) * 3), control_text::wide(text).c_str());
      }
    }
    update_skin();
  }
  void catalogs(const Json& themes, const Json& response) {
    const auto theme_value = theme_data::value(themes);
    if (theme_value && theme_value->is_object() && theme_value->contains("themes") && theme_value->at("themes").is_array()) {
      themes_ = Json::array();
      for (const auto& theme : theme_value->at("themes"))
        if (theme.is_object() && theme.contains("id") && theme.at("id").is_string() && theme.contains("title") && theme.at("title").is_string()) themes_.push_back(theme);
      SendMessageW(get(theme_global), CB_RESETCONTENT, 0, 0);
      for (const auto& theme : themes_)
        SendMessageW(get(theme_global), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(control_text::wide(theme.at("title").get<std::string>()).c_str()));
      select_theme();
    }
    const auto catalog = theme_data::value(response);
    if (!catalog || !catalog->is_object() || !catalog->contains("packages") || !catalog->at("packages").is_array() ||
        !catalog->contains("issues") || !catalog->at("issues").is_array()) {
      catalog_failed_ = true; update_skin(); return;
    }
    catalog_failed_ = false;
    const auto selected = selected_skin();
    const auto id = selected ? selected->value("id", std::string{}) : std::string{};
    packages_ = catalog->at("packages");
    issues_ = catalog->at("issues");
    SendMessageW(get(theme_skins), LB_RESETCONTENT, 0, 0);
    int choose = packages_.empty() ? -1 : 0;
    for (size_t index = 0; index < packages_.size(); ++index) {
      const auto& package = packages_[index];
      const auto text = control_text::wide(package.at("name").get<std::string>());
      SendMessageW(get(theme_skins), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text.c_str()));
      if (package.at("id") == id) choose = static_cast<int>(index);
    }
    SendMessageW(get(theme_skins), LB_SETCURSEL, choose, 0);
    update_skin();
  }
  void preview(theme_data::Preview value) {
    preview_ = std::move(value);
    InvalidateRect(get(theme_preview), nullptr, TRUE);
    update_skin();
  }
  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto& item : controls_) EnableWindow(item.second, enabled);
    EnableWindow(get(theme_global), enabled && !themes_.empty());
    update_skin();
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notice = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_) return result;
    if (id == theme_skins && notice == LBN_SELCHANGE) { update_skin(); return result; }
    if (notice == CBN_SELCHANGE) {
      const auto selected = SendMessageW(get(id), CB_GETCURSEL, 0, 0);
      if (selected < 0) return result;
      if (id == theme_preview_mode && selected < 3) result.preview = static_cast<int>(selected);
      else if ((id == theme_mode || id == theme_candidate_mode) && selected < 3) {
        const auto key = id == theme_mode ? "theme" : "candidate_theme";
        const auto value = (id == theme_mode ? mode_values : candidate_values)[static_cast<size_t>(selected)];
        if (preferences_.value(key, std::string{}) != value) result.changes.push_back({{key}, value});
      } else if (id == theme_global && static_cast<size_t>(selected) < themes_.size()) {
        const auto value = themes_[static_cast<size_t>(selected)].at("id");
        if (preferences_.value("global_theme", Json()) != value) result.changes.push_back({{"global_theme"}, value});
        if (value == "custom") {
          auto custom = preferences_.value("custom_theme", Json::object());
          if (custom.contains("candidate_skin") || custom.contains("candidate_skin_dark")) {
            custom.erase("candidate_skin"); custom.erase("candidate_skin_dark");
            result.changes.push_back({{"custom_theme"}, std::move(custom)});
          }
        }
      }
    }
    if (notice != BN_CLICKED) return result;
    if (id == theme_skin_refresh) result.scan = true;
    if (id == theme_skin_import) result.import = true;
    if (id == theme_skin_directory) result.directory = true;
    if (id == theme_skin_use || id == theme_skin_stop) {
      const auto package = selected_skin();
      if (!package || (id == theme_skin_use && (catalog_failed_ || !compatible(*package)))) return result;
      const auto skin_id = control_text::wide(package->at("id").get<std::string>());
      const auto current = custom_skins();
      if (id == theme_skin_stop && !skins::skin_in_use(preferences_.value("global_theme", std::string{}) == "custom", current, skin_id)) return result;
      const auto next = id == theme_skin_stop ? skins::remove_candidate_skin(current, skin_id) :
          skins::apply_candidate_skin(current, skin_id, control_text::wide(package->at("base").get<std::string>()),
              slot(package->at("base").get<std::string>()), [this](const std::wstring& other) -> std::optional<skins::Slot> {
                for (const auto& entry : packages_)
                  if (control_text::wide(entry.at("id").get<std::string>()) == other) return slot(entry.at("base").get<std::string>());
                return std::nullopt;
              });
      auto custom = preferences_.value("custom_theme", Json::object());
      custom["base"] = control_text::utf8(next.base).value();
      custom["candidate_skin"] = next.light.empty() ? Json(nullptr) : Json(control_text::utf8(next.light).value());
      custom["candidate_skin_dark"] = next.dark.empty() ? Json(nullptr) : Json(control_text::utf8(next.dark).value());
      if (!(next == current) || preferences_.value("global_theme", std::string{}) != "custom")
        result.changes = {{{"custom_theme"}, std::move(custom)}, {{"global_theme"}, "custom"}};
    }
    if (id >= theme_color_first && id < theme_color_first + static_cast<int>(std::size(skins::color_slots)) * 3 && (id - theme_color_first) % 3 != 0) {
      const auto index = static_cast<size_t>((id - theme_color_first) / 3);
      const bool clearing = (id - theme_color_first) % 3 == 2;
      const auto text = control_text::read(get(theme_color_first + static_cast<int>(index) * 3));
      const auto color = text ? skins::candidate_text_color(control_text::wide(*text)) : std::nullopt;
      if (!clearing && !color) { result.message = L"请输入 #RRGGBB 格式的颜色，例如 #336699。"; return result; }
      auto custom = preferences_.value("custom_theme", Json::object());
      auto colors = custom.value("candidate_colors", Json::object());
      const auto key = control_text::utf8(skins::color_slots[index].key).value();
      if (clearing) {
        colors.erase(key);
        SetWindowTextW(get(theme_color_first + static_cast<int>(index) * 3), L"");
      } else colors[key] = control_text::utf8(*color).value();
      if (colors.empty()) custom.erase("candidate_colors"); else custom["candidate_colors"] = std::move(colors);
      const auto change = skins::candidate_color_change(control_text::wide(preferences_.value("global_theme", std::string("system"))),
          control_text::wide(custom.value("base", std::string("system"))), clearing);
      if (change.reset_skins) {
        custom["base"] = control_text::utf8(change.base).value();
        custom.erase("candidate_skin"); custom.erase("candidate_skin_dark");
      }
      if (custom != preferences_.value("custom_theme", Json::object())) result.changes.push_back({{"custom_theme"}, std::move(custom)});
      if (change.select_custom && preferences_.value("global_theme", std::string{}) != "custom") result.changes.push_back({{"global_theme"}, "custom"});
    }
    return result;
  }

  bool draw(const DRAWITEMSTRUCT& draw) const;

private:
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  template <size_t N> void select(int id, const std::array<const char*, N>& values, const std::string& value) {
    for (size_t index = 0; index < values.size(); ++index)
      if (value == values[index]) SendMessageW(get(id), CB_SETCURSEL, index, 0);
  }
  void select_theme() {
    for (size_t index = 0; index < themes_.size(); ++index)
      if (themes_[index].at("id") == preferences_.value("global_theme", std::string("system"))) SendMessageW(get(theme_global), CB_SETCURSEL, index, 0);
  }
  std::optional<Json> selected_skin() const {
    const auto selected = SendMessageW(get(theme_skins), LB_GETCURSEL, 0, 0);
    if (selected < 0 || static_cast<size_t>(selected) >= packages_.size()) return std::nullopt;
    return packages_[static_cast<size_t>(selected)];
  }
  skins::CustomSkins custom_skins() const {
    const auto custom = preferences_.value("custom_theme", Json::object());
    const auto string = [&](const char* key) { const auto value = custom.find(key); return value != custom.end() && value->is_string() ? control_text::wide(value->get<std::string>()) : std::wstring{}; };
    const auto base = string("base");
    return {base.empty() ? L"system" : base, string("candidate_skin"), string("candidate_skin_dark")};
  }
  skins::Slot slot(const std::string& base) const {
    for (const auto& theme : themes_) if (theme.at("id") == base) {
      const auto appearance = theme.find("appearance");
      if (appearance != theme.end() && appearance->is_string()) return *appearance == "dark" ? skins::Slot::dark : skins::Slot::light;
    }
    return skins::Slot::either;
  }
  static std::vector<std::wstring> strings(const Json& values) {
    std::vector<std::wstring> result;
    for (const auto& value : values) if (value.is_string()) result.push_back(control_text::wide(value.get<std::string>()));
    return result;
  }
  bool compatible(const Json& package) const {
    return skins::card_compatible(slot(package.at("base").get<std::string>()), strings(package.at("layouts")), strings(package.at("themes")),
        control_text::wide(preferences_.value("candidate_layout", std::string("vertical"))), theme_data::candidate_dark(preferences_));
  }
  void update_skin() {
    if (preferences_.is_null()) return;
    std::vector<skins::Listed> listed;
    for (const auto& package : packages_) listed.push_back({control_text::wide(package.at("id").get<std::string>()),
        control_text::wide(package.at("name").get<std::string>()), slot(package.at("base").get<std::string>())});
    auto summary = skins::slot_summary(preferences_.value("global_theme", std::string{}) == "custom", custom_skins(), listed);
    if (preview_.image_failed) summary += L"\r\n部分皮肤图片无法预览，已保留基础配色。";
    SetWindowTextW(get(theme_slot_summary), summary.c_str());
    const auto package = selected_skin();
    std::wstring text;
    if (package) {
      const auto description = package->find("description");
      text = skins::card_description(compatible(*package), description != package->end() && description->is_string() ? control_text::wide(description->get<std::string>()) : L"",
          control_text::wide(package->at("base").get<std::string>()), strings(package->at("layouts")), strings(package->at("themes")));
      text += L"\r\n"; text += skins::slot_label(slot(package->at("base").get<std::string>()));
    } else text = L"没有可用的本地皮肤，可以导入皮肤文件夹。";
    if (catalog_failed_) text += L"\r\n皮肤目录读取失败，请刷新后再试。";
    if (!issues_.empty()) text += L"\r\n有 " + std::to_wstring(issues_.size()) + L" 个皮肤包未通过校验。";
    SetWindowTextW(get(theme_skin_notice), text.c_str());
    EnableWindow(get(theme_skin_use), enabled_ && !catalog_failed_ && package && compatible(*package));
    EnableWindow(get(theme_skin_stop), enabled_ && package && skins::skin_in_use(preferences_.value("global_theme", std::string{}) == "custom",
        custom_skins(), control_text::wide(package->at("id").get<std::string>())));
  }
  inline static constexpr std::array<const char*, 3> mode_values{{"system", "light", "dark"}};
  inline static constexpr std::array<const char*, 3> candidate_values{{"follow", "light", "dark"}};
  std::map<int, HWND> controls_;
  Json preferences_;
  Json themes_ = Json::array();
  Json packages_ = Json::array();
  Json issues_ = Json::array();
  theme_data::Preview preview_;
  bool enabled_ = false;
  bool catalog_failed_ = false;
};

} // namespace msime::settings::win7

namespace msime::settings::win7 {
inline bool ThemeForm::draw(const DRAWITEMSTRUCT& draw) const {
  if (draw.CtlID != theme_preview) return false;
  const auto dc = draw.hDC;
  const int saved = SaveDC(dc);
  const auto font = reinterpret_cast<HFONT>(SendMessageW(get(theme_preview), WM_GETFONT, 0, 0));
  if (font) SelectObject(dc, font);
  SetBkMode(dc, TRANSPARENT);
  const auto fill = [&](RECT rect, COLORREF color) {
    const auto brush = CreateSolidBrush(color); FillRect(dc, &rect, brush); DeleteObject(brush);
  };
  const auto mix = [](COLORREF front, COLORREF back, int alpha) {
    const auto channel = [alpha](int a, int b) { return (a * alpha + b * (255 - alpha) + 127) / 255; };
    return RGB(channel(GetRValue(front), GetRValue(back)), channel(GetGValue(front), GetGValue(back)), channel(GetBValue(front), GetBValue(back)));
  };
  const auto image = [&](const theme_data::Image& frame, RECT target, const std::string& fit, double opacity) {
    if (frame.bgra.empty() || target.right <= target.left || target.bottom <= target.top) return;
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = static_cast<LONG>(frame.width);
    info.bmiHeader.biHeight = -static_cast<LONG>(frame.height);
    info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 32; info.bmiHeader.biCompression = BI_RGB;
    void* bits = nullptr;
    const auto bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &bits, nullptr, 0);
    if (!bitmap || !bits) { if (bitmap) DeleteObject(bitmap); return; }
    std::memcpy(bits, frame.bgra.data(), frame.bgra.size());
    const auto memory = CreateCompatibleDC(dc);
    if (!memory) { DeleteObject(bitmap); return; }
    const auto previous = SelectObject(memory, bitmap);
    const int clipped = SaveDC(dc);
    IntersectClipRect(dc, target.left, target.top, target.right, target.bottom);
    int width = target.right - target.left, height = target.bottom - target.top;
    if (fit != "stretch") {
      const double x = static_cast<double>(width) / frame.width, y = static_cast<double>(height) / frame.height;
      const double scale = fit == "cover" ? std::max(x, y) : std::min(x, y);
      width = std::max(1, static_cast<int>(frame.width * scale)); height = std::max(1, static_cast<int>(frame.height * scale));
    }
    const int x = target.left + (target.right - target.left - width) / 2;
    const int y = target.top + (target.bottom - target.top - height) / 2;
    const BLENDFUNCTION blend{AC_SRC_OVER, 0, static_cast<BYTE>(std::clamp(opacity, 0.0, 1.0) * 255), AC_SRC_ALPHA};
    AlphaBlend(dc, x, y, width, height, memory, 0, 0, static_cast<int>(frame.width), static_cast<int>(frame.height), blend);
    RestoreDC(dc, clipped);
    SelectObject(memory, previous); DeleteDC(memory); DeleteObject(bitmap);
  };
  const auto text = [&](const wchar_t* title, RECT rect, COLORREF foreground, UINT flags = DT_SINGLELINE | DT_VCENTER | DT_LEFT) {
    SetTextColor(dc, foreground); DrawTextW(dc, title, -1, &rect, flags | DT_END_ELLIPSIS | DT_NOPREFIX);
  };
  RECT rect = draw.rcItem;
  fill(rect, GetSysColor(COLOR_BTNFACE));
  if (preview_.resolved.is_null()) {
    text(L"主题预览暂不可用，请刷新后再试。", rect, GetSysColor(COLOR_WINDOWTEXT)); RestoreDC(dc, saved); return true;
  }
  const double scale = static_cast<double>(rect.bottom - rect.top) / 228;
  const auto dip = [scale](int value) { return static_cast<int>(value * scale); };
  const auto palette = theme_data::palette(preview_);
  const auto flatten = [&](const msime::windows::CandidateColor& value, COLORREF background) {
    return mix(RGB(static_cast<int>(value.r * 255 + 0.5f), static_cast<int>(value.g * 255 + 0.5f), static_cast<int>(value.b * 255 + 0.5f)),
        background, static_cast<int>(value.a * 255 + 0.5f));
  };
  const auto surface = flatten(palette.surface, GetSysColor(COLOR_BTNFACE));
  const auto foreground = flatten(palette.text, surface);
  const auto selected = flatten(palette.selected, surface);
  const auto selected_text = flatten(palette.selected_text, selected);
  const auto accent = flatten(palette.accent, surface);
  RECT label{rect.left + dip(12), rect.top + dip(5), rect.right - dip(12), rect.top + dip(30)};
  text(preview_.dark ? L"候选预览 · 深色" : L"候选预览 · 浅色", label, GetSysColor(COLOR_WINDOWTEXT));
  const int decoration = preview_.decoration.bgra.empty() ? 0 : std::clamp(static_cast<int>(preview_.package.value("decorationTopDip", 0.0)), 0, 64);
  RECT panel{rect.left + dip(12), rect.top + dip(36 + decoration), rect.right - dip(12), rect.bottom - dip(12)};
  double radius_value = palette.radius;
  if (preview_.package.contains("cornerRadiusDip") && preview_.package.at("cornerRadiusDip").is_number())
    radius_value = preview_.package.at("cornerRadiusDip").get<double>();
  if (preferences_.contains("candidate_corner_radius") && preferences_.at("candidate_corner_radius").is_number())
    radius_value = preferences_.at("candidate_corner_radius").get<double>();
  const int radius = std::clamp(static_cast<int>(radius_value), 0, 32);
  const auto region = CreateRoundRectRgn(panel.left, panel.top, panel.right + 1, panel.bottom + 1, dip(radius * 2), dip(radius * 2));
  const int panel_saved = SaveDC(dc);
  ExtSelectClipRgn(dc, region, RGN_AND); DeleteObject(region);
  const int opacity = preferences_.value("candidate_opacity_percent", 100) * 255 / 100;
  fill(panel, mix(surface, GetSysColor(COLOR_BTNFACE), opacity));
  if (!preview_.background.bgra.empty()) {
    const auto background = preview_.package.value("background", Json::object());
    image(preview_.background, panel, background.value("fit", std::string("cover")), background.value("opacity", 1.0) * opacity / 255.0);
  }
  RECT preedit{panel.left + dip(12), panel.top + dip(7), panel.right - dip(12), panel.top + dip(33)};
  text(L"ni'mf", preedit, flatten(palette.translation.value_or(palette.number), surface));
  const std::array<const wchar_t*, 3> samples{{L"1  你们", L"2  你们的", L"3  你们好"}};
  const int row_height = dip(34);
  for (size_t index = 0; index < samples.size(); ++index) {
    const int width = (panel.right - panel.left - dip(20)) / 3;
    RECT choice{panel.left + dip(8), panel.top + dip(40), panel.right - dip(8), panel.top + dip(40) + row_height};
    if (preview_.horizontal) { choice.left += width * static_cast<int>(index); choice.right = choice.left + width; }
    else { choice.top += row_height * static_cast<int>(index); choice.bottom += row_height * static_cast<int>(index); }
    if (index == 0) {
      fill(choice, selected);
      if (palette.show_selected_bar) {
        RECT bar = choice; bar.right = bar.left + dip(3); fill(bar, accent);
      }
    }
    choice.left += dip(8);
    text(samples[index], choice, index == 0 ? selected_text : foreground);
  }
  const auto border_pen = CreatePen(PS_SOLID, std::max(1, dip(1)), flatten(palette.border, surface));
  const auto old_pen = SelectObject(dc, border_pen);
  const auto old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
  RoundRect(dc, panel.left, panel.top, panel.right, panel.bottom, dip(radius * 2), dip(radius * 2));
  SelectObject(dc, old_brush); SelectObject(dc, old_pen); DeleteObject(border_pen);
  RestoreDC(dc, panel_saved);
  if (decoration) {
    const auto alignment = preview_.package.value("decorationAlign", std::string("center"));
    const int width = std::min(static_cast<int>(panel.right - panel.left), dip(static_cast<int>(preview_.package.value("decorationWidthDip", 60.0))));
    const int x = alignment == "left" ? panel.left : alignment == "right" ? panel.right - width : panel.left + (panel.right - panel.left - width) / 2;
    const RECT target{x, panel.top - dip(decoration), x + width, panel.top};
    image(preview_.decoration, target, "contain", opacity / 255.0);
  }
  RestoreDC(dc, saved);
  return true;
}
} // namespace msime::settings::win7
