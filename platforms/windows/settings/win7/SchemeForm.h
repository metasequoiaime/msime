#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "../InputSettingsModel.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <commctrl.h>
#include <algorithm>
#include <array>
#include <functional>
#include <tuple>

namespace msime::settings::win7 {

class SchemeForm {
public:
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    std::wstring message;
    std::optional<std::string> lookup;
  };
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;

  static bool offers(std::string_view value) {
    for (std::string_view scheme : {MSIME_EDITION_INPUT_SCHEMES}) if (scheme == value) return true;
    return false;
  }

  int create(const Row& row) {
    int top = 0;
    Section section = Section::common;
    const auto add = [&](const wchar_t* type, const wchar_t* title, DWORD style, int id, int position, int height) {
      auto control = row(type, title, style, id, position, height);
      rows_.push_back({control, position, section});
      return control;
    };
    const auto choice = [&](int id, const wchar_t* title, const std::vector<Option>& options) {
      add(L"STATIC", title, SS_LEFT, -1, top, 24);
      auto control = add(WC_COMBOBOXW, title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, id, top + 34, 240);
      fill(control, options);
      top += 90;
      return control;
    };
    const auto check = [&](int id, const wchar_t* title) {
      auto control = add(L"BUTTON", title, WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY, id, top, 28);
      top += 42;
      return control;
    };
    for (const auto& option : std::vector<Option>{{"quanpin", L"全拼"}, {"shuangpin", L"双拼"}, {"wubi", L"五笔"},
        {"cantonese", L"粤拼"}, {"zhuyin", L"注音"}, {"stroke", L"笔画"}})
      if (offers(option.value)) chinese_.push_back(option);
    if (!chinese_.empty()) languages_.push_back({"chinese", L"中文"});
    for (const auto& option : std::vector<Option>{{"japanese", L"日文"}, {"korean", L"韩文"},
        {"vietnamese", L"越南文"}, {"tibetan", L"藏文"}})
      if (offers(option.value)) languages_.push_back(option);
    language_ = choice(scheme_language, L"输入语言", languages_);
    if (!chinese_.empty()) {
      chinese_control_ = choice(scheme_chinese, L"中文方案", chinese_);
      chinese_notice_ = add(L"STATIC", L"", SS_LEFT, -1, top, 36);
      top += 48;
    }
    description_ = add(L"STATIC", L"", SS_LEFT, scheme_description, top, 100);
    top += 120;
    if (offers("shuangpin")) {
      section = Section::shuangpin;
      const int start = top;
      profile_ = choice(scheme_shuangpin_profile, L"双拼方案", profiles());
      hint_ = check(scheme_shuangpin_hint, L"输入时显示双拼键位提示");
      preedit_ = choice(scheme_shuangpin_preedit, L"双拼预编辑", {{"raw", L"原始按键"}, {"pinyin", L"拼音分词"}});
      heights_[static_cast<size_t>(section)] = top - start;
    }
    if (offers("wubi")) {
      section = Section::wubi;
      const int start = top;
      wubi_profile_ = choice(scheme_wubi_profile, L"五笔方案", {{"wubi86", L"86 五笔"}, {"wubi98", L"98 五笔"}});
      mixed_ = check(scheme_wubi_mixed, L"五笔拼音混输");
      code_hint_ = check(scheme_wubi_hint, L"候选显示剩余编码");
      unique_ = check(scheme_wubi_unique, L"五笔四码唯一候选自动上屏");
      selection_ = check(scheme_wubi_selection, L"五笔快捷选词");
      add(L"STATIC", L"组字时用两个不同的英文标点选当前页第 2、3 候选；候选不足时仍输入标点。两个键编辑后一起保存。", SS_LEFT, -1, top, 60);
      top += 72;
      add(L"STATIC", L"第 2 候选键", SS_LEFT, -1, top, 24);
      second_ = add(L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL, scheme_wubi_second, top + 32, 30);
      top += 76;
      add(L"STATIC", L"第 3 候选键", SS_LEFT, -1, top, 24);
      third_ = add(L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL, scheme_wubi_third, top + 32, 30);
      top += 76;
      for (const auto control : {second_, third_}) SendMessageW(control, EM_SETLIMITTEXT, 1, 0);
      keys_description_ = add(L"STATIC", L"", SS_LEFT, -1, top, 60);
      keys_save_ = add(L"BUTTON", L"保存选词键", WS_TABSTOP | BS_PUSHBUTTON, scheme_wubi_save, top + 72, 32);
      top += 122;
      add(L"STATIC", L"五笔查码：输入汉字或词语，同时查询 86 与 98 编码。", SS_LEFT, -1, top, 48);
      lookup_ = add(L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL, scheme_wubi_lookup_text, top + 56, 80);
      SendMessageW(lookup_, EM_SETLIMITTEXT, 1000, 0);
      lookup_button_ = add(L"BUTTON", L"查询编码", WS_TABSTOP | BS_PUSHBUTTON, scheme_wubi_lookup, top + 148, 32);
      lookup_result_ = add(L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL,
          scheme_wubi_result, top + 194, 260);
      top += 478;
      heights_[static_cast<size_t>(section)] = top - start;
    }
    if (offers("vietnamese")) {
      section = Section::vietnamese;
      const int start = top;
      vietnamese_method_ = choice(scheme_vietnamese_method, L"越南语方案", {{"telex", L"Telex"}, {"vni", L"VNI"}});
      vietnamese_tone_ = choice(scheme_vietnamese_tone, L"声调位置", {{"modern", L"新式 hoà"}, {"classic", L"旧式 hòa"}});
      heights_[static_cast<size_t>(section)] = top - start;
    }
    // 粤语双拼与粤拼共用同一个方案和词典，作为粤拼下的选项；缺省（文档里没有 cantonese）为粤拼。
    if (offers("cantonese")) {
      section = Section::cantonese;
      const int start = top;
      cantonese_method_ = choice(scheme_cantonese_method, L"粤拼方案", {{"jyutping", L"粤拼"}, {"shuangpin", L"粤语双拼"}});
      heights_[static_cast<size_t>(section)] = top - start;
    }
    height_ = top;
    return top;
  }

  std::optional<int> position(HWND control) const {
    for (const auto& row : rows_) {
      if (row.control != control) continue;
      if (!visible(row.section)) return std::nullopt;
      int top = row.top;
      for (size_t section = 1; section < static_cast<size_t>(row.section); ++section)
        if (!visible(static_cast<Section>(section))) top -= heights_[section];
      return top;
    }
    return std::nullopt;
  }

  int height() const {
    int result = height_;
    for (size_t section = 1; section < heights_.size(); ++section)
      if (!visible(static_cast<Section>(section))) result -= heights_[section];
    return result;
  }

  static bool valid(const Json& preferences) {
    for (const auto* key : {"scheme", "shuangpin_profile", "wubi_profile"})
      if (!preferences.contains(key) || !preferences.at(key).is_string()) return false;
    for (const auto* key : {"shuangpin_preedit_uses_raw", "wubi_mixed_pinyin", "wubi_code_hint", "wubi_auto_commit_unique"})
      if (!preferences.contains(key) || !preferences.at(key).is_boolean()) return false;
    if (preferences.contains("last_chinese_scheme") && !preferences.at("last_chinese_scheme").is_string()) return false;
    if (preferences.contains("shuangpin_keymap_hint") && !preferences.at("shuangpin_keymap_hint").is_boolean()) return false;
    if (preferences.contains("wubi_candidate_selection")) {
      const auto& group = preferences.at("wubi_candidate_selection");
      if (!group.is_object() || !group.contains("enabled") || !group.at("enabled").is_boolean()) return false;
      for (const auto* key : {"second_key", "third_key"})
        if (!group.contains(key) || !group.at(key).is_string()) return false;
    }
    if (!preferences.contains("vietnamese") || !preferences.at("vietnamese").is_object()) return false;
    for (const auto* key : {"input_method", "tone_style"})
      if (!preferences.at("vietnamese").contains(key) || !preferences.at("vietnamese").at(key).is_string()) return false;
    if (preferences.contains("cantonese")) {
      const auto& cantonese = preferences.at("cantonese");
      if (!cantonese.is_object() || (cantonese.contains("input_method") && !cantonese.at("input_method").is_string())) return false;
    }
    return true;
  }

  void refresh(const Json& preferences) {
    preferences_ = preferences;
    const auto scheme = preferences.at("scheme").get<std::string>();
    select(language_, languages_, chinese(scheme) ? "chinese" : scheme);
    select(chinese_control_, chinese_, chinese(scheme) ? scheme : std::string{});
    if (chinese_notice_) SetWindowTextW(chinese_notice_, chinese(scheme) ? L"" : L"当前输入语言不是中文，中文方案仅在选择中文时可用。");
    SetWindowTextW(description_, scheme == "cantonese" && cantonese_shuangpin(preferences)
        ? L"粤语双拼：两键一个音节，第一键是声母、第二键是韵母，ing 和 ik 在 ; 键上；从第一键起按双拼解释，与粤拼同形的码（如 oi）也按双拼。数字键选词，候选为繁体字词。"
        : note(scheme));
    if (profile_) {
      auto options = profiles();
      if (preferences.at("shuangpin_profile") == "custom") options.push_back({"custom", L"自定义双拼（当前值）"});
      fill(profile_, options);
      select(profile_, options, preferences.at("shuangpin_profile").get<std::string>());
      SendMessageW(hint_, BM_SETCHECK, preferences.value("shuangpin_keymap_hint", false) ? BST_CHECKED : BST_UNCHECKED, 0);
      SendMessageW(preedit_, CB_SETCURSEL, preferences.at("shuangpin_preedit_uses_raw").get<bool>() ? 0 : 1, 0);
    }
    if (wubi_profile_) {
      SendMessageW(wubi_profile_, CB_SETCURSEL, preferences.at("wubi_profile") == "wubi98" ? 1 : 0, 0);
      for (const auto& field : std::vector<std::pair<HWND, const char*>>{{mixed_, "wubi_mixed_pinyin"},
          {code_hint_, "wubi_code_hint"}, {unique_, "wubi_auto_commit_unique"}})
        SendMessageW(field.first, BM_SETCHECK, preferences.at(field.second).get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
      const auto keys = preferences.value("wubi_candidate_selection", default_keys());
      SendMessageW(selection_, BM_SETCHECK, keys.at("enabled").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
      // 无关设置刷新不能抹掉尚未保存的按键草稿。
      if (!saved_keys_ || *saved_keys_ != keys) {
        saved_keys_ = keys;
        SetWindowTextW(second_, control_text::wide(keys.at("second_key").get<std::string>()).c_str());
        SetWindowTextW(third_, control_text::wide(keys.at("third_key").get<std::string>()).c_str());
      }
    }
    if (vietnamese_method_) {
      SendMessageW(vietnamese_method_, CB_SETCURSEL, preferences.at("vietnamese").at("input_method") == "vni" ? 1 : 0, 0);
      SendMessageW(vietnamese_tone_, CB_SETCURSEL, preferences.at("vietnamese").at("tone_style") == "classic" ? 1 : 0, 0);
    }
    if (cantonese_method_) SendMessageW(cantonese_method_, CB_SETCURSEL, cantonese_shuangpin(preferences) ? 1 : 0, 0);
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    const auto scheme = preferences_.value("scheme", std::string{});
    EnableWindow(language_, enabled && languages_.size() > 1);
    EnableWindow(chinese_control_, enabled && chinese(scheme) && chinese_.size() > 1);
    for (const auto control : {profile_, hint_, preedit_}) if (control) EnableWindow(control, enabled && scheme == "shuangpin");
    for (const auto control : {wubi_profile_, mixed_, code_hint_, unique_, selection_, lookup_, lookup_button_, lookup_result_})
      if (control) EnableWindow(control, enabled && scheme == "wubi");
    const bool keys = enabled && scheme == "wubi" && preferences_.value("wubi_candidate_selection", default_keys()).at("enabled").get<bool>();
    for (const auto control : {second_, third_}) if (control) EnableWindow(control, keys);
    for (const auto control : {vietnamese_method_, vietnamese_tone_}) if (control) EnableWindow(control, enabled && scheme == "vietnamese");
    if (cantonese_method_) EnableWindow(cantonese_method_, enabled && scheme == "cantonese");
    update_keys();
  }

  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (id < scheme_language || id > scheme_cantonese_method) return {};
    Command result{true, {}, {}, std::nullopt};
    if (!enabled_) return result;
    if ((id == scheme_wubi_second || id == scheme_wubi_third) && notification == EN_CHANGE) { update_keys(); return result; }
    if (id == scheme_wubi_lookup && notification == BN_CLICKED && IsWindowEnabled(lookup_button_)) {
      result.lookup = control_text::read(lookup_, 1000);
      if (!result.lookup || result.lookup->empty()) result.message = L"请输入要查询的汉字或词语。";
      return result;
    }
    if (notification == CBN_SELCHANGE) {
      if (id == scheme_language || id == scheme_chinese) {
        const auto control = id == scheme_language ? language_ : chinese_control_;
        if (!IsWindowEnabled(control)) return result;
        const auto& options = id == scheme_language ? languages_ : chinese_;
        const auto selected = SendMessageW(control, CB_GETCURSEL, 0, 0);
        if (selected < 0 || static_cast<size_t>(selected) >= options.size()) return result;
        auto next = options[static_cast<size_t>(selected)].value;
        if (next == "chinese") {
          next = preferences_.value("last_chinese_scheme", std::string{});
          if (!chinese(next) || !offers(next)) next = MSIME_EDITION_DEFAULT_SCHEME;
          if (!chinese(next) || !offers(next)) next = chinese_.front().value;
        }
        const auto current = preferences_.at("scheme").get<std::string>();
        if (current == next) return result;
        result.changes.push_back({{"scheme"}, next});
        if (chinese(next)) result.changes.push_back({{"last_chinese_scheme"}, next});
        else if (chinese(current)) result.changes.push_back({{"last_chinese_scheme"}, current});
      } else if (id == scheme_shuangpin_profile && IsWindowEnabled(profile_)) {
        const auto selected = SendMessageW(profile_, CB_GETCURSEL, 0, 0);
        // 自定义表只展示已有值，不能从内置选项切入尚无编辑入口的方案。
        if (selected >= 0 && static_cast<size_t>(selected) < profiles().size())
          result.changes.push_back({{"shuangpin_profile"}, profiles()[static_cast<size_t>(selected)].value});
      } else if (id == scheme_shuangpin_preedit && IsWindowEnabled(preedit_)) {
        const auto selected = SendMessageW(preedit_, CB_GETCURSEL, 0, 0);
        if (selected == 0 || selected == 1) result.changes.push_back({{"shuangpin_preedit_uses_raw"}, selected == 0});
      } else if (id == scheme_wubi_profile && IsWindowEnabled(wubi_profile_)) {
        const auto selected = SendMessageW(wubi_profile_, CB_GETCURSEL, 0, 0);
        if (selected == 0 || selected == 1) result.changes.push_back({{"wubi_profile"}, selected == 0 ? "wubi86" : "wubi98"});
      } else if ((id == scheme_vietnamese_method || id == scheme_vietnamese_tone) && IsWindowEnabled(vietnamese_method_)) {
        const auto selected = SendMessageW(id == scheme_vietnamese_method ? vietnamese_method_ : vietnamese_tone_, CB_GETCURSEL, 0, 0);
        if (selected == 0 || selected == 1) result.changes.push_back({{"vietnamese", id == scheme_vietnamese_method ? "input_method" : "tone_style"},
            id == scheme_vietnamese_method ? (selected == 0 ? "telex" : "vni") : (selected == 0 ? "modern" : "classic")});
      } else if (id == scheme_cantonese_method && IsWindowEnabled(cantonese_method_)) {
        const auto selected = SendMessageW(cantonese_method_, CB_GETCURSEL, 0, 0);
        // 文档在默认值时没有 cantonese 这一组，整组写入，保留其中别的成员。
        if (selected == 0 || selected == 1) {
          auto cantonese = preferences_.value("cantonese", Json::object());
          cantonese["input_method"] = selected == 0 ? "jyutping" : "shuangpin";
          result.changes.push_back({{"cantonese"}, std::move(cantonese)});
        }
      }
    } else if (notification == BN_CLICKED) {
      for (const auto& field : std::vector<std::tuple<int, HWND, const char*>>{
          {scheme_shuangpin_hint, hint_, "shuangpin_keymap_hint"}, {scheme_wubi_mixed, mixed_, "wubi_mixed_pinyin"},
          {scheme_wubi_hint, code_hint_, "wubi_code_hint"}, {scheme_wubi_unique, unique_, "wubi_auto_commit_unique"}})
        if (id == std::get<0>(field) && IsWindowEnabled(std::get<1>(field)))
          result.changes.push_back({{std::get<2>(field)}, SendMessageW(std::get<1>(field), BM_GETCHECK, 0, 0) == BST_CHECKED});
      if (id == scheme_wubi_selection && IsWindowEnabled(selection_)) {
        auto keys = preferences_.value("wubi_candidate_selection", default_keys());
        keys["enabled"] = SendMessageW(selection_, BM_GETCHECK, 0, 0) == BST_CHECKED;
        result.changes.push_back({{"wubi_candidate_selection"}, std::move(keys)});
      } else if (id == scheme_wubi_save && IsWindowEnabled(keys_save_)) {
        auto keys = preferences_.value("wubi_candidate_selection", default_keys());
        const auto second = control_text::read(second_, 1), third = control_text::read(third_, 1);
        if (second && third) {
          keys["second_key"] = *second;
          keys["third_key"] = *third;
          result.changes.push_back({{"wubi_candidate_selection"}, std::move(keys)});
        }
      }
    }
    return result;
  }

  void lookup_result(const Json& response) {
    std::wstring text;
    if (!response.is_object() || !response.value("ok", false) || !response.contains("value") || !response.at("value").is_object()) {
      text = L"查码失败，请确认词库已准备好后重试。";
    } else {
      const auto& value = response.at("value");
      const auto append = [&](const Json& item) {
        if (!item.is_object()) return;
        text += control_text::wide(item.value("text", std::string{}));
        for (const auto* profile : {"wubi86", "wubi98"}) {
          text += std::string_view(profile) == "wubi86" ? L"\r\n86 五笔：" : L"\r\n98 五笔：";
          const auto codes = item.value(profile, Json::array());
          if (codes.empty()) text += L"—";
          for (const auto& code : codes) if (code.is_string()) text += control_text::wide(code.get<std::string>()) + L" ";
        }
        text += L"\r\n\r\n";
      };
      if (value.contains("phrase")) append(value.at("phrase"));
      if (value.contains("characters") && value.at("characters").is_array())
        for (const auto& item : value.at("characters")) append(item);
      if (text.empty()) text = L"没有找到可查询的汉字。";
      if (value.value("truncated", false)) text += L"输入较长，仅显示前面的汉字。";
    }
    SetWindowTextW(lookup_result_, text.c_str());
  }

private:
  enum class Section { common, shuangpin, wubi, vietnamese, cantonese };
  struct RowPosition { HWND control; int top; Section section; };
  bool visible(Section section) const {
    const auto scheme = preferences_.value("scheme", std::string{});
    return section == Section::common || (section == Section::shuangpin && scheme == "shuangpin") ||
        (section == Section::wubi && scheme == "wubi") || (section == Section::vietnamese && scheme == "vietnamese") ||
        (section == Section::cantonese && scheme == "cantonese");
  }
  std::vector<RowPosition> rows_;
  std::array<int, 5> heights_{};
  int height_ = 0;
  struct Option { std::string value; const wchar_t* label; };
  static bool chinese(const std::string& scheme) {
    return scheme != "japanese" && scheme != "korean" && scheme != "vietnamese" && scheme != "tibetan";
  }
  static bool cantonese_shuangpin(const Json& preferences) {
    if (!preferences.contains("cantonese") || !preferences.at("cantonese").is_object()) return false;
    return preferences.at("cantonese").value("input_method", std::string{}) == "shuangpin";
  }
  static Json default_keys() { return {{"enabled", false}, {"second_key", ";"}, {"third_key", "'"}}; }
  static std::vector<Option> profiles() { return {{"xiaohe", L"小鹤双拼"}, {"ziranma", L"自然码双拼"},
      {"shoudao", L"首道双拼"}, {"microsoft", L"微软双拼"}, {"smart_abc", L"智能 ABC 双拼"}}; }
  static void fill(HWND control, const std::vector<Option>& options) {
    SendMessageW(control, CB_RESETCONTENT, 0, 0);
    for (const auto& option : options) SendMessageW(control, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(option.label));
  }
  static void select(HWND control, const std::vector<Option>& options, const std::string& value) {
    if (!control) return;
    SendMessageW(control, CB_SETCURSEL, static_cast<WPARAM>(-1), 0);
    for (size_t index = 0; index < options.size(); ++index)
      if (options[index].value == value) SendMessageW(control, CB_SETCURSEL, index, 0);
  }
  static const wchar_t* note(const std::string& scheme) {
    if (scheme == "japanese") return L"罗马音：直接输入罗马音，提供平假名、片假名及日语词库候选。";
    if (scheme == "korean") return L"两套式：按两套式（두벌식）键位输入韩文字母，自动拼成音节，标点为半角。";
    if (scheme == "cantonese") return L"粤拼：按不带声调的粤拼输入，用 ' 分隔音节，数字键选词，候选为繁体字词。";
    if (scheme == "zhuyin") return L"大千：按大千键位输入注音符号，空格为一声，6 3 4 7 为二、三、四声和轻声，候选为繁体字词。";
    if (scheme == "stroke") return L"横竖撇点折：按 h s p n z 依次输入横、竖、撇、点、折，x 代替不确定的一笔，数字键选词。";
    if (scheme == "vietnamese") return L"Telex 用字母、VNI 用数字标注声调和变音，Esc 恢复原始按键。";
    if (scheme == "tibetan") return L"威利转写：按 EWTS 威利转写输入，区分大小写；空格加音节点 ་ 上屏，/ 加垂符 ། 上屏，回车只上屏藏文，Esc 恢复原始按键。";
    return L"各方案的选项在选用对应方案时可修改，切换方案保留原有配置。";
  }
  void update_keys() {
    if (!keys_save_) return;
    const auto second = control_text::read(second_, 1), third = control_text::read(third_, 1);
    const bool valid = second && third && input::valid_wubi_candidate_keys(control_text::wide(*second), control_text::wide(*third));
    SetWindowTextW(keys_description_, input::wubi_candidate_keys_description(valid).data());
    const bool changed = saved_keys_ && second && third &&
        (*second != saved_keys_->at("second_key") || *third != saved_keys_->at("third_key"));
    EnableWindow(keys_save_, enabled_ && IsWindowEnabled(second_) && valid && changed);
  }
  HWND language_ = nullptr, chinese_control_ = nullptr, chinese_notice_ = nullptr, description_ = nullptr;
  HWND profile_ = nullptr, hint_ = nullptr, preedit_ = nullptr;
  HWND wubi_profile_ = nullptr, mixed_ = nullptr, code_hint_ = nullptr, unique_ = nullptr;
  HWND selection_ = nullptr, second_ = nullptr, third_ = nullptr, keys_save_ = nullptr, keys_description_ = nullptr;
  HWND lookup_ = nullptr, lookup_button_ = nullptr, lookup_result_ = nullptr;
  HWND vietnamese_method_ = nullptr, vietnamese_tone_ = nullptr;
  HWND cantonese_method_ = nullptr;
  std::vector<Option> languages_, chinese_;
  Json preferences_ = Json::object();
  std::optional<Json> saved_keys_;
  bool enabled_ = false;
};

} // namespace msime::settings::win7
