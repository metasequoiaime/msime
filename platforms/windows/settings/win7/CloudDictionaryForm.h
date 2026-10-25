#pragma once

#include "ControlText.h"
#include "ThemeData.h"
#include <commctrl.h>
#include <algorithm>
#include <vector>
#include <charconv>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class CloudDictionaryForm {
public:
  enum Id { kind = 2800, view, search, scheme, profile, query, previous, next, items, detail, code, word,
    weight, new_entry, save, remove, rank_mode, rank_step, rank_trigger, rank_top, rank, position,
    position_save, position_clear, format, import_file, export_file, snapshot_status, snapshot_info,
    snapshot_preview, snapshot_apply, snapshot_cancel, snapshot_export, restore_choose, restore_apply,
    restore_discard, preview_info, notice };
  enum class File { none, import_words, export_words, export_snapshot, restore_snapshot };
  struct Command { bool handled = false; std::optional<Json> request; File file = File::none; std::wstring confirmation; };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      controls_[id] = row(type, title, style, id, top, height); top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    const auto edit = [&](int id, const wchar_t* title) { add(id, L"EDIT", title, WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL); };
    const auto button = [&](int id, const wchar_t* title) { add(id, L"BUTTON", title, WS_TABSTOP | BS_PUSHBUTTON, 32); };
    const auto combo = [&](int id, std::initializer_list<const wchar_t*> choices) {
      add(id, WC_COMBOBOXW, L"", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 240);
      for (const auto choice : choices) SendMessageW(get(id), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(choice));
      SendMessageW(get(id), CB_SETCURSEL, 0, 0);
    };
    add(notice, L"STATIC", L"管理当前账号的云端词条。", SS_LEFT, 72);
    combo(kind, {L"拼音", L"86 五笔", L"98 五笔", L"快捷短语", L"英文"});
    combo(view, {L"个人云词条", L"完整词库目录", L"候选与排序", L"固定排位"});
    add(-1, L"STATIC", L"查询文字或编码（固定排位时为上下文）", SS_LEFT); edit(search, L"");
    add(-2, L"STATIC", L"查询方案与双拼布局", SS_LEFT); edit(scheme, L"pinyin"); edit(profile, L"xiaohe");
    button(query, L"查询 / 刷新"); button(previous, L"上一页"); button(next, L"下一页");
    add(items, L"LISTBOX", L"云词库结果", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 200);
    add(detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 120);
    add(-3, L"STATIC", L"编码、词条与权重", SS_LEFT); edit(code, L""); edit(word, L""); edit(weight, L"100000");
    button(new_entry, L"新增个人云词条"); button(save, L"保存词条 / 完整目录覆盖"); button(remove, L"删除选中词条或目录记录");
    combo(rank_mode, {L"不调频", L"置顶", L"折半", L"线性", L"逐次前移"});
    add(-4, L"STATIC", L"线性步长与触发次数", SS_LEFT); edit(rank_step, L"1"); edit(rank_trigger, L"1");
    add(rank_top, L"BUTTON", L"强制置顶", WS_TABSTOP | BS_AUTOCHECKBOX); button(rank, L"调整选中候选排序");
    add(-5, L"STATIC", L"固定位置（从 1 开始）", SS_LEFT); edit(position, L"1");
    button(position_save, L"设置选中词条的固定位置"); button(position_clear, L"清除所选固定位置");
    combo(format, {L"标准 TSV", L"Windows TSV", L"汉字自动注音（仅拼音导入）"});
    button(import_file, L"选择词表并导入云端"); button(export_file, L"导出当前种类的云词库");
    add(-6, L"STATIC", L"完整快照（包含全部词库和排序记录）", SS_LEFT, 44);
    button(snapshot_status, L"刷新本机快照应用状态");
    add(snapshot_info, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 140);
    button(snapshot_preview, L"下载并预览云端完整快照"); button(snapshot_apply, L"确认替换本机词库"); button(snapshot_cancel, L"取消待应用快照");
    button(snapshot_export, L"备份完整云词库到文件"); button(restore_choose, L"选择备份文件并预览");
    add(preview_info, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 150);
    button(restore_apply, L"确认使用备份替换云端词库"); button(restore_discard, L"放弃恢复预览");
    enable(false); return top;
  }
  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto& item : controls_) if (item.first >= 0) EnableWindow(item.second, enabled);
    const bool bound = !binding_.empty(), selected = !selected_.is_null(), has_list = !confirmed_.is_null();
    for (const int id : {new_entry, import_file, export_file, snapshot_status, snapshot_preview, snapshot_cancel, snapshot_export, restore_choose}) EnableWindow(get(id), enabled && bound);
    EnableWindow(get(previous), enabled && has_list && offset_ > 0); EnableWindow(get(next), enabled && has_list && has_more_);
    EnableWindow(get(save), enabled && bound && (adding_ || (selected && mode_ <= 1)));
    EnableWindow(get(remove), enabled && selected && mode_ <= 2);
    EnableWindow(get(rank), enabled && selected && mode_ == 2);
    EnableWindow(get(position_save), enabled && selected && (mode_ == 3 || (mode_ == 2 && !context_.empty())));
    EnableWindow(get(position_clear), enabled && selected && mode_ == 3);
    EnableWindow(get(snapshot_apply), enabled && bound && !preview_token_.empty());
    EnableWindow(get(restore_apply), enabled && bound && restore_.is_object());
    EnableWindow(get(restore_discard), enabled && restore_.is_object());
  }
  Json query_request(size_t offset = 0) const {
    const auto index = SendMessageW(get(kind), CB_GETCURSEL, 0, 0), selected_view = SendMessageW(get(view), CB_GETCURSEL, 0, 0);
    const auto key = index >= 0 && index < 5 ? kinds_[static_cast<size_t>(index)] : "pinyin";
    Json action{{"operation", "list"}, {"kind", key}, {"offset", offset}, {"search", text(search)}};
    if (selected_view == 1) action = {{"operation", "catalog"}, {"kind", key}, {"code", text(search)}, {"scheme", text(scheme)}, {"profile", text(profile)}, {"offset", offset}};
    if (selected_view == 2) action = {{"operation", "candidates"}, {"kind", key}, {"text", text(search)}, {"scheme", text(scheme)}, {"profile", text(profile)}, {"limit", 50}};
    if (selected_view == 3) action = {{"operation", "fixed_positions"}, {"context", context_}, {"offset", offset}};
    return envelope(std::move(action));
  }
  bool can_poll() const { return !binding_.empty(); }
  Json status_request() const { return envelope({{"operation", "snapshot_status"}}); }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event); if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if ((id == kind || id == view) && notification == CBN_SELCHANGE) { clear_list(); enable(enabled_); return result; }
    if (id == items && notification == LBN_SELCHANGE) { select(); return result; }
    if (notification != BN_CLICKED) return result;
    Json action;
    if (id == query) { result.request = query_request(); return result; }
    if ((id == previous || id == next) && !confirmed_.is_null()) {
      auto request = confirmed_; auto& query_action = request["action"]; query_action["offset"] = id == previous ? (offset_ > page_size_ ? offset_ - page_size_ : 0) : offset_ + page_size_;
      result.request = std::move(request); return result;
    }
    if (id == new_entry) { adding_ = true; selected_ = Json(); SetWindowTextW(get(code), L""); SetWindowTextW(get(word), L""); SetWindowTextW(get(weight), L"100000"); enable(enabled_); return result; }
    if (id == restore_discard) { restore_ = Json(); SetWindowTextW(get(preview_info), L""); enable(enabled_); return result; }
    const auto current_kind = kinds_[static_cast<size_t>(std::clamp<LRESULT>(SendMessageW(get(kind), CB_GETCURSEL, 0, 0), 0, 4))];
    if (id == save) {
      const auto number = integer(weight); if (!number) { message(L"权重应为非负整数。"); return result; }
      action = {{"operation", "add"}, {"kind", current_kind}, {"code", text(code)}, {"word", text(word)}, {"weight", *number}};
      if (!adding_) {
        if (mode_ == 0) { action["operation"] = "update"; action["id"] = selected_.at("id"); action["revision"] = selected_.at("revision"); }
        else { action = catalog_edit(); action["replacement"] = {{"code", text(code)}, {"word", text(word)}, {"weight", *number}}; }
      }
    } else if (id == remove) {
      if (mode_ == 0) action = {{"operation", "delete"}, {"kind", confirmed_.at("action").at("kind")}, {"id", selected_.at("id")}, {"revision", selected_.at("revision")}};
      else if (mode_ == 1) { action = catalog_edit(); action["replacement"] = nullptr; }
      else { action = candidate_query("remove_candidate"); action["revision"] = revision_; }
      result.confirmation = L"删除所选云端词条或目录记录？本机词库不会立即改变。";
    } else if (id == rank) {
      const auto step = integer(rank_step), trigger = integer(rank_trigger); if (!step || !trigger) { message(L"请填写有效的步长与触发次数。"); return result; }
      action = candidate_query("rank"); action["revision"] = revision_; static constexpr const char* modes[] = {"disabled", "pin", "halve", "linear", "promote"}; action["mode"] = modes[static_cast<size_t>(std::clamp<LRESULT>(SendMessageW(get(rank_mode), CB_GETCURSEL, 0, 0), 0, 4))];
      action["linear_step"] = *step; action["trigger_count"] = *trigger; action["force_top"] = SendMessageW(get(rank_top), BM_GETCHECK, 0, 0) == BST_CHECKED; result.confirmation = L"调整所选云端候选的排序？";
    } else if (id == position_save || id == position_clear) {
      const auto place = integer(position); if (id == position_save && (!place || *place < 1)) { message(L"固定位置应为正整数。"); return result; }
      action = {{"operation", "set_fixed_position"}, {"context", mode_ == 3 ? confirmed_.at("action").at("context") : Json(context_)},
          {"code", mutation_code()}, {"word", selected_.at("word")}, {"position", id == position_clear ? Json() : Json(*place)}, {"revision", revision_}};
    } else if (id == import_file || id == export_file) {
      const auto selected_format = SendMessageW(get(format), CB_GETCURSEL, 0, 0);
      if (id == export_file && selected_format == 2) { message(L"汉字自动注音只用于拼音导入。"); return result; }
      action = {{"operation", id == import_file ? "import" : "export_file"}, {"kind", current_kind}, {"format", selected_format == 2 ? "hans" : selected_format == 1 ? "windows" : "standard"}};
      result.file = id == import_file ? File::import_words : File::export_words;
      if (id == import_file) result.confirmation = L"按当前种类和格式将所选词表导入账号云词库？导入不会立即改变本机输入。";
    } else if (id == snapshot_status) action = {{"operation", "snapshot_status"}};
    else if (id == snapshot_preview) { preview_token_.clear(); action = {{"operation", "snapshot_preview"}}; }
    else if (id == snapshot_apply) {
      action = {{"operation", "snapshot_enqueue"}, {"token", preview_token_}};
      result.confirmation = L"这份云端快照会替换本机个人词库和学习记录，期间输入会暂停。请保持面板打开，直到状态显示已应用。是否继续？";
    } else if (id == snapshot_cancel) { action = {{"operation", "snapshot_cancel"}}; result.confirmation = L"撤销待应用的云词库快照？"; }
    else if (id == snapshot_export) { action = {{"operation", "snapshot_export_file"}}; result.file = File::export_snapshot; }
    else if (id == restore_choose) { restore_ = Json(); action = {{"operation", "snapshot_restore_preview_file"}}; result.file = File::restore_snapshot; }
    else if (id == restore_apply) {
      action = {{"operation", "snapshot_restore_file"}, {"path", restore_.at("path")}, {"expected_sha256", restore_.at("snapshot").at("sha256")}, {"revision", restore_.at("expectedRevision")}};
      result.confirmation = L"此备份将替换全部云端词库和排序记录。确认使用已预览的备份恢复？";
    }
    if (action.is_object()) result.request = envelope(std::move(action)); enable(enabled_); return result;
  }
  void completed(const Json& request, const Json& completion) {
    try { complete(request, completion); }
    catch (const std::exception&) { clear_list(); binding_.clear(); preview_token_.clear(); restore_ = Json(); context_.clear(); SetWindowTextW(get(preview_info), L""); SetWindowTextW(get(snapshot_info), L""); message(L"返回内容无法读取，请重新查询。"); enable(enabled_); }
  }
private:
  void complete(const Json& request, const Json& completion) {
    const auto operation = request.at("action").value("operation", std::string{});
    const auto response = completion.find("response");
    const auto value = response != completion.end() ? theme_data::value(*response) : std::optional<Json>{};
    if (!value || !completion.contains("binding") || !completion.at("binding").is_string()) {
      const auto error = response != completion.end() && response->is_object() ? response->value("error", std::string{}) : std::string{};
      clear_list(); preview_token_.clear(); restore_ = Json(); binding_.clear(); context_.clear(); revision_ = 0;
      SetWindowTextW(get(preview_info), L""); SetWindowTextW(get(snapshot_info), L"");
      message(error == "account_conflict" || error == "snapshot_conflict" ? L"词库已变化，请重新查询并预览。" : error == "dictionary_account_changed" ? L"账号已变化，请重新查询；旧结果不能再操作。" : L"云词库请求失败，请检查登录状态和网络后重新查询。");
      enable(enabled_); return;
    }
    const auto binding = completion.at("binding").get<std::string>();
    if (binding_ != binding) { context_.clear(); revision_ = 0; clear_list(); preview_token_.clear(); restore_ = Json(); SetWindowTextW(get(preview_info), L""); }
    binding_ = binding;
    if (is_query(operation)) {
      if (!load_results(request, *value)) { clear_list(); message(L"查询结果无法读取，请重新查询。"); }
      else message(L"云词库结果已刷新。");
    } else if (operation == "snapshot_preview") {
      preview_token_.clear();
      if (value->is_object() && value->contains("snapshot") && value->at("snapshot").is_object() && value->contains("previewToken") && value->at("previewToken").is_string()) {
        restore_ = Json(); preview_token_ = value->at("previewToken").get<std::string>(); SetWindowTextW(get(preview_info), metadata(value->at("snapshot")).c_str()); message(L"云端快照已校验，确认后才会替换本机词库。");
      } else message(L"快照预览无法读取，请重新下载。");
    } else if (operation == "snapshot_restore_preview_file") {
      restore_ = Json();
      if (value->is_object() && value->contains("snapshot") && value->at("snapshot").is_object() && value->at("snapshot").contains("sha256") && value->at("snapshot").at("sha256").is_string() && value->contains("expectedRevision") && value->at("expectedRevision").is_number_integer()) {
        preview_token_.clear(); restore_ = *value; restore_["path"] = request.at("action").at("path"); SetWindowTextW(get(preview_info), (metadata(value->at("snapshot")) + L"\r\n恢复时核对云端修订：" + std::to_wstring(value->at("expectedRevision").get<int64_t>())).c_str()); message(L"备份已校验，确认后才会替换云端词库。");
      } else message(L"备份预览无法读取，云端词库未改变。");
    } else if (operation == "snapshot_status" || operation == "snapshot_enqueue" || operation == "snapshot_cancel") {
      SetWindowTextW(get(snapshot_info), snapshot_status_text(*value).c_str());
      if (operation != "snapshot_status") { preview_token_.clear(); message(operation == "snapshot_cancel" ? L"已取消待应用快照。" : L"快照已受理，请查看状态；尚在等待或准备时需保持面板打开。"); }
    } else {
      if (operation == "snapshot_restore_file") { restore_ = Json(); SetWindowTextW(get(preview_info), L""); }
      clear_list(); message(operation == "export_file" || operation == "snapshot_export_file" ? L"文件已保存。" : L"云词库操作已完成，请重新查询。" );
    }
    enable(enabled_);
  }
private:
  static std::wstring metadata(const Json& value) {
    std::wstring result;
    for (const auto& field : {std::pair<const char*, const wchar_t*>{"cloudRevision", L"云端修订"}, {"bytes", L"字节数"}, {"records", L"记录数"}, {"entries", L"词条数"}, {"overlays", L"覆盖记录"}, {"positions", L"固定排位"}, {"selections", L"选择记录"}}) {
      const auto found = value.find(field.first); if (found != value.end() && found->is_number_integer()) result += std::wstring(field.second) + L"：" + std::to_wstring(found->get<int64_t>()) + L"\r\n";
    }
    const auto sha = value.find("sha256"); if (sha != value.end() && sha->is_string()) result += L"SHA-256：" + control_text::wide(sha->get<std::string>());
    return result;
  }
  static std::wstring snapshot_status_text(const Json& value) {
    std::wstring result;
    const auto version = value.find("localVersion"); result = version != value.end() && version->is_string() ? L"本机词库版本：" + control_text::wide(version->get<std::string>()) : L"尚未读取本机词库版本，请先启用输入法。";
    const auto request = value.find("request");
    if (request != value.end() && request->is_object()) {
      const auto status = request->value("status", std::string{});
      const wchar_t* label = status == "queued" ? L"等待应用，请保持面板打开" : status == "preparing" ? L"正在准备，请保持面板打开" : status == "applied" ? L"已应用" : status == "conflict" ? L"因冲突作废，本机词库未改变" : status == "cancelled" ? L"已取消" : L"未能应用，请重新预览";
      result += std::wstring(L"\r\n快照状态：") + label;
    } else result += L"\r\n没有待应用快照。";
    return result;
  }
  static bool is_query(std::string_view operation) { return operation == "list" || operation == "catalog" || operation == "candidates" || operation == "fixed_positions"; }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  std::string text(int id) const { return control_text::read(get(id)).value_or(""); }
  std::optional<int64_t> integer(int id) const { const auto bytes = text(id); int64_t result = 0; const auto parsed = std::from_chars(bytes.data(), bytes.data() + bytes.size(), result); return parsed.ec == std::errc{} && parsed.ptr == bytes.data() + bytes.size() && result >= 0 ? std::make_optional(result) : std::nullopt; }
  Json envelope(Json action) const { Json result{{"action", std::move(action)}}; if (!binding_.empty()) result["binding"] = binding_; return result; }
  Json catalog_edit() const { return {{"operation", "edit_catalog"}, {"kind", confirmed_.at("action").at("kind")}, {"code", selected_.at("code")}, {"word", selected_.at("word")}, {"revision", revision_}}; }
  Json candidate_query(const char* operation) const { auto action = confirmed_.at("action"); action["operation"] = operation; action["code"] = mutation_code(); action["word"] = selected_.at("word"); return action; }
  std::string mutation_code() const { const auto canonical = selected_.find("canonical_pinyin"); return canonical != selected_.end() && canonical->is_string() && !canonical->get_ref<const std::string&>().empty() ? canonical->get<std::string>() : selected_.at("code").get<std::string>(); }
  void message(const wchar_t* text) { SetWindowTextW(get(notice), text); }
  void clear_list() { entries_.clear(); selected_ = Json(); confirmed_ = Json(); adding_ = false; offset_ = 0; has_more_ = false; SendMessageW(get(items), LB_RESETCONTENT, 0, 0); SetWindowTextW(get(detail), L""); }
  bool load_results(const Json& request, const Json& value) {
    const auto operation = request.at("action").at("operation").get<std::string>();
    const char* field = operation == "catalog" ? "catalog_entries" : operation == "candidates" ? "candidates" : operation == "fixed_positions" ? "positions" : "entries";
    if (!value.is_object() || !value.contains(field) || !value.at(field).is_array()) return false;
    if (operation != "candidates" && (!value.contains("offset") || !value.at("offset").is_number_unsigned() || !value.contains("has_more") || !value.at("has_more").is_boolean())) return false;
    if ((operation == "catalog" || operation == "candidates") && (!value.contains("revision") || !value.at("revision").is_number_integer())) return false;
    for (const auto& item : value.at(field)) {
      if (!item.is_object() || !item.contains("code") || !item.at("code").is_string() || !item.contains("word") || !item.at("word").is_string() ||
          (operation == "list" && (!item.contains("id") || !item.at("id").is_string() || !item.contains("revision") || !item.at("revision").is_number_integer()))) return false;
    }
    clear_list(); confirmed_ = request; entries_ = value.at(field).get<std::vector<Json>>();
    mode_ = operation == "catalog" ? 1 : operation == "candidates" ? 2 : operation == "fixed_positions" ? 3 : 0;
    offset_ = value.value("offset", request.at("action").value("offset", size_t{0}));
    has_more_ = value.value("has_more", false); if (mode_ != 3) revision_ = value.value("revision", int64_t{0}); if (mode_ == 2) context_ = value.value("context", std::string{});
    for (const auto& item : entries_) { const auto title = control_text::wide(item.at("code").get<std::string>()) + L" · " + control_text::wide(item.at("word").get<std::string>()); SendMessageW(get(items), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str())); }
    if (!entries_.empty()) { SendMessageW(get(items), LB_SETCURSEL, 0, 0); select(); } return true;
  }
  void select() {
    const auto index = SendMessageW(get(items), LB_GETCURSEL, 0, 0); selected_ = Json(); adding_ = false;
    if (index >= 0 && static_cast<size_t>(index) < entries_.size()) {
      selected_ = entries_[static_cast<size_t>(index)];
      std::wstring description = L"编码：" + control_text::wide(selected_.at("code").get<std::string>()) + L"\r\n词条：" + control_text::wide(selected_.at("word").get<std::string>());
      if (selected_.contains("weight") && selected_.at("weight").is_number_integer()) description += L"\r\n权重：" + std::to_wstring(selected_.at("weight").get<int64_t>());
      if (selected_.contains("position") && selected_.at("position").is_number_integer()) description += L"\r\n固定位置：" + std::to_wstring(selected_.at("position").get<int64_t>());
      SetWindowTextW(get(detail), description.c_str());
      SetWindowTextW(get(code), control_text::wide(selected_.at("code").get<std::string>()).c_str()); SetWindowTextW(get(word), control_text::wide(selected_.at("word").get<std::string>()).c_str());
      SetWindowTextW(get(weight), std::to_wstring(selected_.value("weight", int64_t{100000})).c_str());
    }
    enable(enabled_);
  }
  static constexpr const char* kinds_[] = {"pinyin", "wubi", "wubi98", "quick", "english"};
  static constexpr size_t page_size_ = 100;
  std::map<int, HWND> controls_;
  std::vector<Json> entries_;
  Json selected_, confirmed_, restore_;
  std::string binding_, preview_token_;
  size_t offset_ = 0; int64_t revision_ = 0; int mode_ = 0; std::string context_;
  bool enabled_ = false, adding_ = false, has_more_ = false;
};
} // namespace msime::settings::win7
