#pragma once

#include "PluginPreferencesModel.h"
#include "ThemeData.h"
#include "../PluginEditorModel.h"
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {
namespace plugin_editor = msime::settings::plugins;
// 插件编辑器：共享页 plugin-editor-view.tsx 的 Win32 简化版。选一种类型新建，或从下拉框里选一个已安装的用户短语表、指令表、符号集或单词本读出来编辑；基本信息各一个输入框，内容是一个多行文本框，写法见 PluginEditorModel.h 的「Win7 设置的整段文本」。读写经 msime_client_plugins 的 authoring_load 和 authoring_save，与现代设置和共享页同一份规则；保存前按 `draft_issue` 先查一遍，只报第一处问题，宿主拒绝时报它的说明，表单里的内容都留着。
class PluginEditorForm {
public:
  enum Id { kind = 2900, kind_note, new_pack, packs, open_pack, name, author, version, license, description, format, content, save_pack, notice };
  struct Command {
    bool handled = false;
    std::optional<Json> request;
    std::wstring confirmation;
    // 确认之后才做的改动（放弃没保存的内容、换成新的草稿）。
    std::function<void()> after;
  };
  // 保存之后的提醒要用到的设置：哪些表已经启用、K 模式和 / 指令开没开。
  struct Usage {
    std::optional<plugin_preferences::model::Preferences> preferences;
    bool quick_phrase = true, command = false;
  };
  using Package = plugin_preferences::model::Package;
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row, int top) {
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      controls_[id] = row(type, title, style, id, top, height);
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    const auto label = [&](const wchar_t* text, int height = 24) { row(L"STATIC", text, SS_LEFT, -1, top, height); top += height + 12; };
    label(L"新建和编辑插件（短语表、指令表、符号集、单词本）", 28);
    label((std::wstring(plugin_editor::create_description) + L"。").c_str(), 48);
    add(kind, WC_COMBOBOXW, L"插件类型", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 200);
    for (const auto& entry : plugin_editor::authored_kinds) SendMessageW(get(kind), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(std::wstring(entry.title).c_str()));
    SendMessageW(get(kind), CB_SETCURSEL, 0, 0);
    add(kind_note, L"STATIC", L"", SS_LEFT, 48);
    add(new_pack, L"BUTTON", L"新建插件", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(packs, WC_COMBOBOXW, L"可以编辑的插件", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 240);
    add(open_pack, L"BUTTON", L"编辑所选插件", WS_TABSTOP | BS_PUSHBUTTON, 32);
    for (const auto& field : std::vector<std::pair<int, const wchar_t*>>{{name, L"名字"}, {author, L"作者（可不填）"}, {version, L"版本"}, {license, L"许可证"}}) {
      label(field.second);
      add(field.first, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
      SendMessageW(get(field.first), EM_SETLIMITTEXT, field_limit, 0);
    }
    label(std::wstring(plugin_editor::license_note).c_str(), 62);
    label(L"介绍（可不填）");
    add(description, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN, 72);
    SendMessageW(get(description), EM_SETLIMITTEXT, description_limit, 0);
    add(format, L"STATIC", L"", SS_LEFT, 120);
    add(content, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | WS_HSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_AUTOHSCROLL | ES_WANTRETURN, 240);
    SendMessageW(get(content), EM_SETLIMITTEXT, content_limit, 0);
    add(save_pack, L"BUTTON", L"保存并安装", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(notice, L"STATIC", L"", SS_LEFT, 90);
    start(plugin_editor::empty_draft(plugin_editor::authored_kinds[0].kind));
    return top;
  }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  // 插件目录读到之后，列出能编辑的用户包；原来选着的那个还在时留着。
  void catalog(const std::vector<Package>& packages) {
    std::optional<std::pair<std::wstring, std::wstring>> previous;
    if (const auto* pack = selected()) previous = std::make_pair(pack->kind, pack->id);
    editable_.clear(); SendMessageW(get(packs), CB_RESETCONTENT, 0, 0);
    for (const auto& pack : packages) {
      if (!plugin_editor::pack_editable(pack)) continue;
      const auto title = plugin_editor::kind_label(pack.kind) + L" · " + pack.name;
      SendMessageW(get(packs), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str()));
      if (previous && pack.kind == previous->first && pack.id == previous->second) SendMessageW(get(packs), CB_SETCURSEL, editable_.size(), 0);
      editable_.push_back(pack);
    }
    if (!editable_.empty() && !selected()) SendMessageW(get(packs), CB_SETCURSEL, 0, 0);
    update();
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == kind && notification == CBN_SELCHANGE) { show_kind(chosen_kind()); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == new_pack) {
      if (dirty()) result.confirmation = L"放弃还没有保存的修改，新建一个插件？";
      result.after = [this, chosen = std::wstring(chosen_kind())] { start(plugin_editor::empty_draft(chosen)); };
    } else if (id == open_pack) {
      const auto* pack = selected(); if (!pack) return result;
      if (dirty()) result.confirmation = L"放弃还没有保存的修改，读出所选插件？";
      result.request = Json{{"operation", "authoring_load"}, {"kind", control_text::utf8(pack->kind).value()}, {"id", control_text::utf8(pack->id).value()}};
    } else if (id == save_pack) {
      auto draft = read();
      if (!draft) { SetWindowTextW(get(notice), L"表单里的文字无法读取，请检查长度。"); return result; }
      if (const auto issue = plugin_editor::read_authored_content(*draft, read_text(content, content_limit).value_or(L""))) { SetWindowTextW(get(notice), issue->c_str()); return result; }
      if (const auto issue = plugin_editor::draft_issue(*draft)) { SetWindowTextW(get(notice), issue->c_str()); return result; }
      result.request = Json{{"operation", "authoring_save"}, {"pack", pack_json(plugin_editor::pack_from_draft(*draft))}, {"replace", draft->id.has_value()}};
    }
    return result;
  }
  // authoring_load 和 authoring_save 的回答。读出的包换掉表单；保存成功后这一份就是已安装的那个包，再保存时替换它。失败时表单原样留着。
  void completed(const Json& response, std::string_view operation, const Usage& usage) {
    const auto value = theme_data::value(response);
    if (operation == "authoring_load") {
      const auto pack = value ? authored_pack(*value) : std::nullopt;
      if (!pack) { SetWindowTextW(get(notice), failure(response, L"无法读取这个插件，请重试。").c_str()); return; }
      start(plugin_editor::draft_from_pack(*pack));
      return;
    }
    if (operation != "authoring_save") return;
    if (!value || !value->is_object() || !value->contains("id") || !value->at("id").is_string()) {
      SetWindowTextW(get(notice), failure(response, L"保存失败，请重试。").c_str()); return;
    }
    Package saved;
    saved.kind = control_text::wide(value->value("kind", std::string{})); saved.id = control_text::wide(value->at("id").get<std::string>());
    saved.name = control_text::wide(value->value("name", std::string{}));
    if (auto draft = read()) {
      // 新建的包这时才有 id；文本框里的内容按读出的样子留着，「有没有改动」从这里重新算起。
      draft->id = saved.id; draft_ = std::move(*draft);
    }
    baseline_ = snapshot();
    SetWindowTextW(get(save_pack), L"保存修改");
    SetWindowTextW(get(notice), saved_note(saved, usage).c_str());
    update();
  }
  void failed() { SetWindowTextW(get(notice), L"插件操作失败，请重试。表单里的内容仍保留。"); }
private:
  static constexpr int field_limit = 256, description_limit = 1024, content_limit = 4 * 1024 * 1024;
  HWND get(int id) const { const auto item = controls_.find(id); return item == controls_.end() ? nullptr : item->second; }
  const Package* selected() const {
    const auto index = SendMessageW(get(packs), CB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < editable_.size() ? &editable_[static_cast<size_t>(index)] : nullptr;
  }
  std::wstring_view chosen_kind() const {
    const auto index = SendMessageW(get(kind), CB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < plugin_editor::authored_kinds.size() ? plugin_editor::authored_kinds[static_cast<size_t>(index)].kind : plugin_editor::authored_kinds[0].kind;
  }
  std::optional<std::wstring> read_text(int id, int limit) const {
    const auto text = control_text::read(get(id), limit);
    return text ? std::make_optional(control_text::wide(*text)) : std::nullopt;
  }
  // 换成一份新的草稿：类型、各字段和内容都按它写进控件，「有没有改动」从这里算起。
  void start(plugin_editor::AuthoringDraft draft) {
    for (size_t index = 0; index < plugin_editor::authored_kinds.size(); ++index)
      if (plugin_editor::authored_kinds[index].kind == draft.kind) SendMessageW(get(kind), CB_SETCURSEL, index, 0);
    show_kind(draft.kind);
    SetWindowTextW(get(name), draft.name.c_str()); SetWindowTextW(get(author), draft.author.c_str());
    SetWindowTextW(get(version), draft.version.c_str()); SetWindowTextW(get(license), draft.license.c_str());
    std::wstring text;
    for (size_t index = 0; index < draft.description.size(); ++index) text += draft.description[index] == L'\n' ? std::wstring(L"\r\n") : std::wstring(1, draft.description[index]);
    SetWindowTextW(get(description), text.c_str());
    SetWindowTextW(get(content), plugin_editor::authored_content_text(draft).c_str());
    SetWindowTextW(get(save_pack), draft.id ? L"保存修改" : L"保存并安装");
    SetWindowTextW(get(notice), draft.id ? (L"正在编辑「" + draft.name + L"」。" + std::wstring(plugin_editor::save_note(true))).c_str() : std::wstring(plugin_editor::save_note(false)).c_str());
    draft_ = std::move(draft); baseline_ = snapshot(); update();
  }
  // 类型下拉框和它的说明、内容的写法。编辑已安装的包时类型跟着包，不能改。
  void show_kind(std::wstring_view chosen) {
    for (const auto& entry : plugin_editor::authored_kinds)
      if (entry.kind == chosen) SetWindowTextW(get(kind_note), std::wstring(entry.description).c_str());
    auto instructions = std::wstring(plugin_editor::authored_text_format(chosen));
    if (chosen == L"phrase_table") instructions += L"\r\n" + std::wstring(plugin_editor::phrase_cursor_note);
    SetWindowTextW(get(format), instructions.c_str());
  }
  // 表单读成草稿：类型和 id 是当前这一份的，内容留给 `read_authored_content`。介绍的换行写成 LF，与共享页的 textarea 相同。
  std::optional<plugin_editor::AuthoringDraft> read() const {
    if (!draft_) return std::nullopt;
    auto draft = plugin_editor::empty_draft(draft_->kind);
    draft.id = draft_->id;
    const auto take = [&](int id, int limit, std::wstring& output) { const auto text = read_text(id, limit); if (text) output = *text; return text.has_value(); };
    std::wstring text;
    if (!take(name, field_limit, draft.name) || !take(author, field_limit, draft.author) || !take(version, field_limit, draft.version) ||
        !take(license, field_limit, draft.license) || !take(description, description_limit, text)) return std::nullopt;
    for (size_t index = 0; index < text.size(); ++index) if (text[index] != L'\r') draft.description += text[index];
    return draft;
  }
  std::vector<std::wstring> snapshot() const {
    std::vector<std::wstring> values;
    for (const int id : {name, author, version, license, description, content}) values.push_back(read_text(id, id == content ? content_limit : id == description ? description_limit : field_limit).value_or(L""));
    return values;
  }
  bool dirty() const { return draft_ && snapshot() != baseline_; }
  void update() {
    for (const auto& control : controls_) EnableWindow(control.second, enabled_);
    // 编辑已安装的包时类型跟着包。
    EnableWindow(get(kind), enabled_ && draft_ && !draft_->id);
    EnableWindow(get(open_pack), enabled_ && selected() != nullptr);
    EnableWindow(get(packs), enabled_ && !editable_.empty());
  }
  static std::wstring failure(const Json& response, const wchar_t* fallback) {
    const auto text = [&](const char* key) { return response.is_object() && response.contains(key) && response.at(key).is_string() ? control_text::wide(response.at(key).get<std::string>()) : std::wstring{}; };
    return plugin_editor::error_message(text("error"), text("detail"), fallback);
  }
  // 保存之后的一句：已保存什么，短语表和指令表还要在插件库里启用、打开对应的快捷模式，别的类型装上就能用。Win7 没有详情页，启用在上面的插件库里。
  static std::wstring saved_note(const Package& pack, const Usage& usage) {
    auto note = plugin_editor::saved_notice(pack);
    if (pack.kind == L"phrase_table" || pack.kind == L"command_table") {
      const bool phrase = pack.kind == L"phrase_table";
      const bool enabled = usage.preferences && plugin_editor::contains(phrase ? usage.preferences->phrase_tables : usage.preferences->command_tables, pack.id);
      if (!enabled) note += L"在上面的插件库里选中它，点「选用 / 启用所选插件」后才会生效。";
      if (phrase && !usage.quick_phrase) note += L"快捷短语（K 模式）现在关着，在「输入 → 快捷模式」打开后才会出候选。";
      if (!phrase && !usage.command) note += L"/ 指令现在关着，在「输入 → 快捷模式」打开后才能用。";
    } else if (pack.kind == L"symbol_set") {
      note += L"符号面板里会显示这些符号。";
    } else if (pack.kind == L"wordbook") {
      note += L"这本书出现在背单词的词书里。";
    }
    return note;
  }
  static Json text_json(const std::wstring& value) { return control_text::utf8(value).value_or(std::string{}); }
  static Json nullable_json(const std::optional<std::wstring>& value) { return value ? text_json(*value) : Json(nullptr); }
  // authoring_save 的 pack：可选的字段为空时写 null，只带与 kind 对应的那一个内容字段。
  static Json pack_json(const plugin_editor::AuthoredPack& pack) {
    Json output{{"kind", text_json(pack.kind)}, {"id", nullable_json(pack.id)}, {"name", text_json(pack.name)}, {"version", text_json(pack.version)},
        {"license", text_json(pack.license)}, {"author", nullable_json(pack.author)}, {"description", nullable_json(pack.description)}};
    Json rows = Json::array();
    if (pack.kind == L"phrase_table") { for (const auto& row : pack.phrases) rows.push_back({{"key", text_json(row.key)}, {"text", text_json(row.text)}}); output["phrases"] = std::move(rows); }
    else if (pack.kind == L"command_table") { for (const auto& row : pack.commands) rows.push_back({{"trigger", text_json(row.trigger)}, {"title", text_json(row.title)}, {"template", text_json(row.template_text)}}); output["commands"] = std::move(rows); }
    else if (pack.kind == L"symbol_set") {
      for (const auto& group : pack.groups) {
        Json items = Json::array(); for (const auto& item : group.items) items.push_back(text_json(item));
        rows.push_back({{"tab", text_json(group.tab)}, {"title", text_json(group.title)}, {"keywords", nullable_json(group.keywords)}, {"items", std::move(items)}});
      }
      output["groups"] = std::move(rows);
    } else if (pack.kind == L"wordbook") { for (const auto& row : pack.words) rows.push_back({{"word", text_json(row.word)}, {"phonetic", nullable_json(row.phonetic)}, {"gloss", text_json(row.gloss)}}); output["words"] = std::move(rows); }
    return output;
  }
  // authoring_load 的 value（client-services 的 AuthoredPack）；形状不对时为空。
  static std::optional<plugin_editor::AuthoredPack> authored_pack(const Json& value) {
    if (!value.is_object()) return std::nullopt;
    bool valid = true;
    const auto text = [&](const Json& item, const char* key) {
      if (!item.is_object() || !item.contains(key) || !item.at(key).is_string()) { valid = false; return std::wstring{}; }
      return control_text::wide(item.at(key).get<std::string>());
    };
    const auto nullable = [&](const Json& item, const char* key) -> std::optional<std::wstring> {
      if (!item.is_object() || !item.contains(key) || item.at(key).is_null()) return std::nullopt;
      if (!item.at(key).is_string()) { valid = false; return std::nullopt; }
      return control_text::wide(item.at(key).get<std::string>());
    };
    const auto rows = [&](const char* key) -> const Json* {
      if (!value.contains(key) || !value.at(key).is_array()) { valid = false; return nullptr; }
      return &value.at(key);
    };
    plugin_editor::AuthoredPack pack;
    pack.kind = text(value, "kind"); pack.id = nullable(value, "id"); pack.name = text(value, "name"); pack.version = text(value, "version");
    pack.license = text(value, "license"); pack.author = nullable(value, "author"); pack.description = nullable(value, "description");
    if (!valid || !plugin_editor::is_authored_kind(pack.kind)) return std::nullopt;
    if (pack.kind == L"phrase_table") { if (const auto* list = rows("phrases")) for (const auto& item : *list) pack.phrases.push_back({text(item, "key"), text(item, "text")}); }
    else if (pack.kind == L"command_table") { if (const auto* list = rows("commands")) for (const auto& item : *list) pack.commands.push_back({text(item, "trigger"), text(item, "title"), text(item, "template")}); }
    else if (pack.kind == L"symbol_set") {
      if (const auto* list = rows("groups")) for (const auto& item : *list) {
        plugin_editor::AuthoredGroup group{text(item, "tab"), text(item, "title"), nullable(item, "keywords"), {}};
        if (!item.is_object() || !item.contains("items") || !item.at("items").is_array()) { valid = false; break; }
        for (const auto& symbol : item.at("items")) { if (!symbol.is_string()) { valid = false; break; } group.items.push_back(control_text::wide(symbol.get<std::string>())); }
        pack.groups.push_back(std::move(group));
      }
    } else if (const auto* list = rows("words")) for (const auto& item : *list) pack.words.push_back({text(item, "word"), nullable(item, "phonetic"), text(item, "gloss")});
    return valid ? std::make_optional(std::move(pack)) : std::nullopt;
  }
  std::map<int, HWND> controls_;
  std::vector<Package> editable_;
  std::optional<plugin_editor::AuthoringDraft> draft_;
  std::vector<std::wstring> baseline_;
  bool enabled_ = false;
};
} // namespace msime::settings::win7
