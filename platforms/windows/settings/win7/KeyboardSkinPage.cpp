#include "KeyboardSkinPage.h"
#include "KeyboardSkin.h"
#include "ControlText.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <commdlg.h>
#include <array>
#include <fstream>
#include <thread>
#include <map>

namespace msime::settings::win7 {
namespace {
constexpr UINT completed = WM_APP + 92;
constexpr UINT ai_progress = WM_APP + 93;
constexpr wchar_t page_class[] = L"MSIME.Windows7Settings.KeyboardSkin" MSIME_EDITION_NAME_SUFFIX;
struct Field { const char* key; const wchar_t* title; double minimum, maximum; };
const std::array<Field,7> colors{{{"background",L"背景色",0,0},{"keyBackground",L"按键底色",0,0},
  {"keyForeground",L"按键文字",0,0},{"accent",L"强调色",0,0},{"actionBackground",L"功能键底色",0,0},
  {"gradientEnd",L"渐变终点颜色",0,0},{"customBorderColor",L"自定义边框颜色",0,0}}};
const std::array<Field,7> numbers{{{"cornerRadius",L"圆角（0–20）",0,20},{"borderWidth",L"边框宽度（0–2）",0,2},
  {"shadow",L"阴影（0–0.4）",0,.4},{"keyOpacity",L"按键不透明度（0.25–1）",.25,1},
  {"patternOpacity",L"图案不透明度（0–0.5）",0,.5},{"photoShade",L"照片遮罩（0–0.8）",0,.8},
  {"photoPosition",L"照片位置（0–1）",0,1}}};
constexpr std::array<const char*,4> shapes{"rounded","capsule","ticket","pebble"};
constexpr std::array<const char*,4> materials{"flat","raised","glass","paper"};
std::wstring error_text(const Json& response) {
  const auto code = response.is_object() ? response.value("error",std::string{}) : std::string{};
  if (code == "community_skin_library_full") return L"皮肤库已满，最多保存 12 个设计。";
  if (code == "community_skin_invalid_name") return L"名称无效，请填写不超过 32 个字符的名称。";
  if (code == "community_skin_duplicate_name") return L"已有同名皮肤，请换一个名称。";
  if (code == "community_not_found") return L"该皮肤已被其他窗口移除，请重新读取。";
  return L"皮肤未能保存，请检查数据目录或重新读取；已有文件保持原样。";
}
}
struct KeyboardSkinPage::State {
  std::function<std::filesystem::path()> directory;
  Services services;
  HWND window = nullptr; HINSTANCE instance = nullptr; HFONT font = nullptr; UINT dpi = 96;
  struct Row { HWND handle; int top, height; };
  std::vector<Row> rows; std::thread worker;
  bool busy = false, filling = false, dirty = false;
  Json items = Json::array(), draft; std::string selected;
  Json catalog, proposals = Json::array();
  std::vector<Json> past, future;
  std::string ai_request;
  std::optional<Snapshot> snapshot;
  keyboard_skin::Look look;
  enum class Operation { read, select, mutate, apply, preview, photo, transform, back, forward, generate };
  struct Job { Operation operation; Json action, design; std::string id; std::optional<Snapshot> previous; std::filesystem::path photo_file; };
  struct Completion { Json items; Json design; Json catalog, proposals; std::string id; std::optional<Snapshot> snapshot; keyboard_skin::Look look; std::wstring message; Operation operation = Operation::read; bool success = false; };
  std::optional<Completion> result;
  int dip(int value) const { return MulDiv(value,static_cast<int>(dpi),96); }
  HWND control(int id) const { return GetDlgItem(window,id); }
  void text(int id, const std::wstring& value) { SetWindowTextW(control(id),value.c_str()); }
  bool checked(int id) const { return SendMessageW(control(id),BM_GETCHECK,0,0) == BST_CHECKED; }
  void check(int id, bool value) { SendMessageW(control(id),BM_SETCHECK,value ? BST_CHECKED : BST_UNCHECKED,0); }
  bool confirm(const wchar_t* title,const wchar_t* text) const { return services.confirm ? services.confirm(window,title,text) : MessageBoxW(window,text,title,MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES; }
  bool discard() const { return !dirty || confirm(L"切换皮肤",L"放弃当前未保存的皮肤修改？"); }
  void row(const wchar_t* type, const wchar_t* title, DWORD style, int id, int top, int row_height) {
    const auto child = CreateWindowExW(lstrcmpW(type,L"EDIT") == 0 ? WS_EX_CLIENTEDGE : 0,type,title,WS_CHILD | WS_VISIBLE | style |
      (lstrcmpW(type,L"BUTTON") == 0 ? BS_NOTIFY : 0),0,0,1,1,window,reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)),instance,nullptr);
    if (!child) throw std::runtime_error("keyboard_skin_control");
    SendMessageW(child,WM_SETFONT,reinterpret_cast<WPARAM>(font),FALSE); rows.push_back({child,top,row_height});
  }
  void layout() { RECT area{}; GetClientRect(window,&area); for (const auto& row : rows) MoveWindow(row.handle,0,dip(row.top),std::max(1L,area.right - dip(8)),dip(row.height),TRUE); }
  void enable() {
    for (const auto& row : rows) EnableWindow(row.handle,!busy && (draft.is_object() || GetDlgCtrlID(row.handle) == refresh || GetDlgCtrlID(row.handle) == create_skin || GetDlgCtrlID(row.handle) == name || GetDlgCtrlID(row.handle) == library));
    for (int id : {save_skin,rename_skin,delete_skin}) EnableWindow(control(id),!busy && !selected.empty());
    EnableWindow(control(apply_skin),!busy && draft.is_object() && snapshot.has_value());
    EnableWindow(control(undo),!busy && !past.empty());
    EnableWindow(control(redo),!busy && !future.empty());
    for (int id : {ai_prompt,ai_generate,templates,use_template,reset_design}) EnableWindow(control(id),!busy);
    EnableWindow(control(ai_cancel),busy && !ai_request.empty());
    EnableWindow(control(ai_proposals),!busy && !proposals.empty());
    EnableWindow(control(ai_use),!busy && !proposals.empty());
  }
  void cancel_generation() {
    if (ai_request.empty()) return;
    if (services.cancel) services.cancel(ai_request);
    else (void)theme_data::call(msime_client_ai_skin_cancel,{{"request_id",ai_request}});
  }
  bool fields() {
    if (!draft.is_object()) return false;
    auto value = draft;
    for (size_t index = 0; index < 7; ++index) {
      const auto text = control_text::read(control(first_number + static_cast<int>(index)),64);
      if (!text) return false;
      try {
        size_t count = 0; const auto number = std::stod(*text,&count);
        if (count != text->size() || !std::isfinite(number) || number < numbers[index].minimum || number > numbers[index].maximum) throw std::invalid_argument("range");
        const std::array<double,7> defaults{8,0,0,1,.15,.25,.5};
        const auto current = value.find(numbers[index].key);
        if (index < 3 || (current != value.end() && current->is_number()) || number != defaults[index]) value[numbers[index].key] = number;
      } catch (...) { text_notice(L"数值无效，请按字段旁的范围填写。"); return false; }
    }
    for (const auto& entry : {std::pair<int,const char*>{shape,"keyShape"},{material,"keyMaterial"}}) {
      const auto selection = SendMessageW(control(entry.first),CB_GETCURSEL,0,0); if (selection < 0 || selection > 3) return false;
      const auto current = value.find(entry.second);
      if (selection != 0 || (current != value.end() && current->is_string())) value[entry.second] = entry.first == shape ? shapes[static_cast<size_t>(selection)] : materials[static_cast<size_t>(selection)];
    }
    value["pattern"] = static_cast<int>(SendMessageW(control(pattern),CB_GETCURSEL,0,0));
    value["monospaced"] = checked(monospaced);
    if (checked(horizontal) || (value.contains("gradientHorizontal") && value.at("gradientHorizontal").is_boolean())) value["gradientHorizontal"] = checked(horizontal);
    if (!checked(gradient)) value["gradientEnd"] = nullptr;
    else if (!value.contains("gradientEnd") || value.at("gradientEnd").is_null()) value["gradientEnd"] = look.background;
    if (!checked(border_color)) value["customBorderColor"] = nullptr;
    else if (!value.contains("customBorderColor") || value.at("customBorderColor").is_null()) value["customBorderColor"] = look.accent;
    draft = std::move(value); return true;
  }
  void text_notice(const std::wstring& value) { text(notice,value); }
  void fill() {
    filling = true;
    for (size_t index = 0; index < 7; ++index) {
      const auto fallback = index == 3 ? 1.0 : index == 4 ? .15 : index == 5 ? .25 : index == 6 ? .5 : 0.0;
      auto value = draft.find(numbers[index].key); text(first_number + static_cast<int>(index),std::to_wstring(value != draft.end() && value->is_number() ? value->get<double>() : fallback));
    }
    const auto select = [&](int id, const auto& choices, const char* key) {
      auto value = draft.find(key); const auto word = value != draft.end() && value->is_string() ? value->get<std::string>() : choices[0];
      const auto item = std::find(choices.begin(),choices.end(),word); SendMessageW(control(id),CB_SETCURSEL,item == choices.end() ? 0 : item - choices.begin(),0);
    };
    select(shape,shapes,"keyShape"); select(material,materials,"keyMaterial");
    SendMessageW(control(pattern),CB_SETCURSEL,draft.is_object() ? draft.value("pattern",0) : 0,0);
    check(monospaced,look.mono()); check(gradient,draft.contains("gradientEnd") && draft.at("gradientEnd").is_number());
    check(border_color,draft.contains("customBorderColor") && draft.at("customBorderColor").is_number()); check(horizontal,look.flag("gradientHorizontal"));
    for (size_t index = 0; index < colors.size(); ++index) {
      const auto color = static_cast<uint32_t>(look.number(colors[index].key,index == 5 ? look.background : look.accent));
      wchar_t label[96]{}; swprintf_s(label,L"%s  #%06X",colors[index].title,color); text(first_color + static_cast<int>(index),label);
    }
    InvalidateRect(control(preview),nullptr,TRUE); filling = false; enable();
  }
  void start(Job job) {
    if (busy) return;
    if (worker.joinable()) worker.join();
    busy = true; enable(); text_notice(L"正在处理…");
    worker = std::thread([this,job = std::move(job)] {
      Completion out; out.items = items; out.design = job.design; out.id = job.id; out.snapshot = snapshot;
      out.operation = job.operation; out.catalog = catalog; out.proposals = proposals;
      try {
        const auto root = directory(); if (!root.is_absolute()) throw std::runtime_error("directory");
        if (job.operation == Operation::read) {
          const auto value = theme_data::value(theme_data::call(msime_client_keyboard_skin_design,{{"operation","catalog"}}));
          if (!value || !value->is_object()) throw std::runtime_error("catalog"); out.catalog = *value;
        }
        if (job.operation == Operation::transform) {
          const auto value = theme_data::value(theme_data::call(msime_client_keyboard_skin_design,job.action));
          if (!value || !value->is_object()) throw std::runtime_error("design"); out.design = *value;
        }
        if (job.operation == Operation::generate) {
          const auto body = Json{{"directory",root.u8string()},{"request_id",job.action.at("request_id")},{"prompt",job.action.at("prompt")}}.dump();
          Json response;
          if (services.generate) response = Json::parse(services.generate(root,job.action.at("request_id").get<std::string>(),job.action.at("prompt").get<std::string>(),[this](uint32_t count) { PostMessageW(window,ai_progress,count,0); }),nullptr,false);
          else response = theme_data::response(msime_client_ai_skin_generate(reinterpret_cast<const uint8_t*>(body.data()),body.size(),
            [](void* context,uint32_t count) { auto* self = static_cast<State*>(context); PostMessageW(self->window,ai_progress,count,0); },this));
          const auto value = theme_data::value(response);
          if (!value || !value->is_array() || value->empty()) {
            const auto code = response.is_object() ? response.value("error",std::string{}) : std::string{};
            out.message = code == "ai_skin_cancelled" ? L"已取消生成，当前设计保持原样。" : L"生成未完成，请检查账号、网络或稍后重试。当前设计保持原样。";
            throw std::runtime_error("ai");
          }
          out.proposals = *value;
        }
        if (job.operation == Operation::read || job.operation == Operation::mutate) {
          Json request{{"directory",root.u8string()}}; if (job.operation == Operation::mutate) request["action"] = job.action;
          const auto response = theme_data::call(msime_client_custom_skin_library,request); const auto value = theme_data::value(response);
          if (!value || !value->is_array()) { out.message = error_text(response); throw std::runtime_error("library"); }
          out.items = *value;
          if (job.operation == Operation::read) {
            auto loaded = PreferencesStore(root.u8string()).load(); if (!loaded.snapshot) throw std::runtime_error("preferences"); out.snapshot = std::move(loaded.snapshot);
          }
          if (job.operation == Operation::mutate && job.action.value("operation",std::string{}) == "create" && !out.items.empty()) out.id = out.items.back().at("id").get<std::string>();
          if (job.operation == Operation::mutate && job.action.value("operation",std::string{}) == "delete") out.id.clear();
          if (out.id.empty() && !out.items.empty()) out.id = out.items.front().at("id").get<std::string>();
          out.design = Json();
          for (const auto& item : out.items) if (item.at("id") == out.id) out.design = item.at("design");
        }
        if (job.operation == Operation::photo) {
          std::ifstream file(job.photo_file,std::ios::binary); std::vector<uint8_t> bytes(512001); file.read(reinterpret_cast<char*>(bytes.data()),static_cast<std::streamsize>(bytes.size())); bytes.resize(static_cast<size_t>(file.gcount()));
          if (file.bad() || bytes.empty() || bytes.size() > 512000) { out.message = L"照片必须是小于 512 KB 的 PNG 或 JPEG。"; throw std::runtime_error("photo"); }
          theme_data::Image decoded; if (msime_client_skin_image_decode(bytes.data(),bytes.size(),theme_data::Image::accept,&decoded) != 1) throw std::runtime_error("photo");
          DWORD length = 0; const DWORD flags = CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF;
          if (!CryptBinaryToStringA(bytes.data(),static_cast<DWORD>(bytes.size()),flags,nullptr,&length)) throw std::runtime_error("photo");
          std::string encoded(length,'\0'); if (!CryptBinaryToStringA(bytes.data(),static_cast<DWORD>(bytes.size()),flags,encoded.data(),&length)) throw std::runtime_error("photo");
          encoded.resize(length); while (!encoded.empty() && encoded.back() == '\0') encoded.pop_back(); out.design["photo"] = encoded;
        }
        if (job.operation == Operation::apply) {
          if (!job.previous) throw std::runtime_error("snapshot");
          auto custom = job.previous->preferences().value("custom_theme",Json::object()); custom["keyboard"] = job.design;
          const auto saved = PreferencesStore(root.u8string()).save(*job.previous,{{{"global_theme"},"custom"},{{"custom_theme"},custom}});
          if (saved.outcome != Outcome::saved) {
            out.snapshot = PreferencesStore(root.u8string()).load().snapshot;
            out.message = saved.outcome == Outcome::conflict ? L"设置已被其他窗口修改，本次未应用；已重新读取，可核对后再次应用。" : L"设计未能应用，已有设置保持原样。";
            throw std::runtime_error("apply");
          }
          out.snapshot = saved.snapshot;
        }
        out.look = keyboard_skin::Look::custom(out.design);
        if (out.look.image_failed) { out.message = L"照片无法解码，未替换当前预览。"; throw std::runtime_error("decode"); }
        out.success = true;
        out.message = job.operation == Operation::generate ? L"生成完成，选择方案后可载入预览，再保存或应用。" : job.operation == Operation::apply ? L"已应用到屏幕键盘。打开的键盘可点击「刷新」读取。" : L"已读取设计；「保存到皮肤库」和「应用到键盘」分别生效。";
      } catch (...) { if (out.message.empty()) out.message = L"无法读取皮肤，请重新读取后重试。"; }
      result = std::move(out); PostMessageW(window,completed,0,0);
    });
  }
  void accept() {
    if (worker.joinable()) worker.join(); busy = false; if (!result) return;
    auto out = std::move(*result); result.reset(); snapshot = std::move(out.snapshot); ai_request.clear();
    if (out.success) {
      const auto remember = [](std::vector<Json>& history,const Json& design) { if (design.is_object()) { history.push_back(design); if (history.size() > 30) history.erase(history.begin()); } };
      if (out.operation == Operation::read || out.operation == Operation::select) { past.clear(); future.clear(); }
      else if (out.operation == Operation::back) { if (!past.empty()) past.pop_back(); remember(future,look.design); }
      else if (out.operation == Operation::forward) { if (!future.empty()) future.pop_back(); remember(past,look.design); }
      else if (look.design.is_object() && look.design != out.design) { remember(past,look.design); future.clear(); }
      catalog = std::move(out.catalog); proposals = std::move(out.proposals);
      items = std::move(out.items); draft = std::move(out.design); selected = std::move(out.id); look = std::move(out.look); dirty = draft.is_object();
      for (const auto& item : items) if (item.at("id") == selected && item.at("design") == draft) dirty = false;
      filling = true; SendMessageW(control(library),CB_RESETCONTENT,0,0);
      int index = 0, chosen = -1;
      for (const auto& item : items) { const auto title = control_text::wide(item.at("name").get<std::string>()); SendMessageW(control(library),CB_ADDSTRING,0,reinterpret_cast<LPARAM>(title.c_str())); if (item.at("id") == selected) { chosen = index; text(name,title); } ++index; }
      SendMessageW(control(library),CB_SETCURSEL,chosen,0); filling = false; fill();
      for (const auto& choice : {std::pair<int,const char*>{templates,"templates"},{backgrounds,"backgrounds"}}) {
        if (SendMessageW(control(choice.first),CB_GETCOUNT,0,0) == 0 && catalog.is_object()) {
          for (const auto& item : catalog.at(choice.second)) { const auto title = control_text::wide(item.at("title").get<std::string>()); SendMessageW(control(choice.first),CB_ADDSTRING,0,reinterpret_cast<LPARAM>(title.c_str())); }
          SendMessageW(control(choice.first),CB_SETCURSEL,0,0);
        }
      }
      if (out.operation == Operation::generate) {
        SendMessageW(control(ai_proposals),CB_RESETCONTENT,0,0);
        for (const auto& item : proposals) { const auto title = control_text::wide(item.at("name").get<std::string>()); SendMessageW(control(ai_proposals),CB_ADDSTRING,0,reinterpret_cast<LPARAM>(title.c_str())); }
        SendMessageW(control(ai_proposals),CB_SETCURSEL,0,0);
      }
    }
    text(notice,out.message); enable();
  }
  void command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (id == ai_cancel && notification == BN_CLICKED) { cancel_generation(); return; }
    if (busy || filling) return;
    if (notification == EN_CHANGE || notification == CBN_SELCHANGE || notification == BN_CLICKED) dirty = dirty || (id >= first_color || id == shape || id == material || id == pattern || id == monospaced || id == gradient || id == horizontal || id == border_color);
    if (id == library && notification == CBN_SELCHANGE) {
      if (!discard()) { for (size_t i = 0; i < items.size(); ++i) if (items[i].at("id") == selected) SendMessageW(control(library),CB_SETCURSEL,i,0); return; }
      const auto index = SendMessageW(control(library),CB_GETCURSEL,0,0);
      if (index >= 0 && static_cast<size_t>(index) < items.size()) start({Operation::select,{},items[static_cast<size_t>(index)].at("design"),items[static_cast<size_t>(index)].at("id").get<std::string>(),snapshot,{}});
      return;
    }
    if (notification != BN_CLICKED) return;
    if (id == undo || id == redo) {
      auto& history = id == undo ? past : future; if (!history.empty()) start({id == undo ? Operation::back : Operation::forward,{},history.back(),selected,snapshot,{}}); return;
    }
    if (id == reset_design || id == use_template) {
      Json action{{"operation",id == reset_design ? "reset" : "template"}};
      if (id == use_template) { const auto index = SendMessageW(control(templates),CB_GETCURSEL,0,0); if (index < 0) return; action["index"] = index; }
      start({Operation::transform,std::move(action),draft,selected,snapshot,{}}); return;
    }
    if (id == ai_generate) {
      if (draft.is_object() && !fields()) return;
      const auto prompt = control_text::read(control(ai_prompt),2000); if (!prompt || prompt->empty()) { text_notice(L"请填写希望生成的皮肤风格。"); return; }
      ai_request = "win7-skin-" + std::to_string(GetCurrentProcessId()) + "-" + std::to_string(GetTickCount64());
      start({Operation::generate,{{"request_id",ai_request},{"prompt",*prompt}},draft,selected,snapshot,{}}); return;
    }
    if (id == ai_use) {
      const auto index = SendMessageW(control(ai_proposals),CB_GETCURSEL,0,0);
      if (index >= 0 && static_cast<size_t>(index) < proposals.size()) { const auto& proposal = proposals.at(static_cast<size_t>(index)); text(name,control_text::wide(proposal.at("name").get<std::string>())); start({Operation::preview,{},proposal.at("design"),selected,snapshot,{}}); } return;
    }
    if (id == refresh) { if (discard()) start({Operation::read,{},draft,selected,snapshot,{}}); return; }
    const auto title = control_text::read(control(name),256).value_or("");
    if (id == create_skin) {
      if (draft.is_object() && !fields()) return;
      start({Operation::mutate,{{"operation","create"},{"name",title},{"design",draft.is_object() ? draft : Json::object()}},{},"",snapshot,{}}); return;
    }
    if (id == rename_skin || id == delete_skin) {
      if (selected.empty() || !discard()) return;
      if (id == delete_skin && !confirm(L"删除皮肤",L"删除所选命名皮肤？已经应用的外观保持原样。")) return;
      Json action{{"operation",id == rename_skin ? "rename" : "delete"},{"id",selected}}; if (id == rename_skin) action["name"] = title;
      start({Operation::mutate,std::move(action),draft,selected,snapshot,{}}); return;
    }
    if (!fields()) return;
    if (id == optimize_contrast || id == use_background) {
      Json action{{"operation",id == optimize_contrast ? "optimize_contrast" : "background"},{"design",draft}};
      if (id == use_background) { const auto index = SendMessageW(control(backgrounds),CB_GETCURSEL,0,0); if (index < 0) return; action["index"] = index; }
      start({Operation::transform,std::move(action),draft,selected,snapshot,{}});
    } else if (id >= first_color && id < first_color + static_cast<int>(colors.size())) {
      const auto index = static_cast<size_t>(id - first_color); const auto current = static_cast<uint32_t>(look.number(colors[index].key,look.accent));
      static COLORREF custom[16]{}; CHOOSECOLORW dialog{}; dialog.lStructSize = sizeof(dialog); dialog.hwndOwner = window; dialog.lpCustColors = custom; dialog.rgbResult = RGB((current >> 16) & 255,(current >> 8) & 255,current & 255); dialog.Flags = CC_FULLOPEN | CC_RGBINIT;
      const auto choice = services.pick_color ? services.pick_color(window,dialog.rgbResult) : ChooseColorW(&dialog) ? std::optional<COLORREF>(dialog.rgbResult) : std::nullopt;
      if (choice) { draft[colors[index].key] = (static_cast<uint32_t>(GetRValue(*choice)) << 16) | (static_cast<uint32_t>(GetGValue(*choice)) << 8) | GetBValue(*choice); if (index == 5) check(gradient,true); if (index == 6) check(border_color,true); start({Operation::preview,{},draft,selected,snapshot,{}}); }
    } else if (id == save_skin && !selected.empty()) start({Operation::mutate,{{"operation","update"},{"id",selected},{"design",draft}},draft,selected,snapshot,{}});
    else if (id == apply_skin) start({Operation::apply,{},draft,selected,snapshot,{}});
    else if (id == preview_change) start({Operation::preview,{},draft,selected,snapshot,{}});
    else if (id == remove_photo) { draft["photo"] = nullptr; start({Operation::preview,{},draft,selected,snapshot,{}}); }
    else if (id == photo) {
      wchar_t path[32768]{}; OPENFILENAMEW dialog{}; dialog.lStructSize = sizeof(dialog); dialog.hwndOwner = window; dialog.lpstrFilter = L"照片（PNG、JPEG）\0*.png;*.jpg;*.jpeg\0\0"; dialog.lpstrFile = path; dialog.nMaxFile = 32768; dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
      const auto choice = services.pick_photo ? services.pick_photo(window) : GetOpenFileNameW(&dialog) ? std::optional<std::filesystem::path>(path) : std::nullopt;
      if (choice) start({Operation::photo,{},draft,selected,snapshot,*choice});
    }
  }
  void controls() {
    int top = 0;
    const auto add = [&](const wchar_t* type,const wchar_t* title,DWORD style,int id,int size = 30) { row(type,title,style,id,top,size); top += size + 10; };
    add(L"STATIC",L"命名设计与已应用的键盘外观分别保存。修改后可预览，再选择保存或应用。",SS_LEFT,-1,60);
    add(L"COMBOBOX",L"命名皮肤",CBS_DROPDOWNLIST | WS_VSCROLL | WS_TABSTOP,library,180); top -= 150;
    add(L"STATIC",L"皮肤名称",SS_LEFT,-1,22);
    add(L"EDIT",L"新皮肤",ES_AUTOHSCROLL | WS_TABSTOP,name); SendMessageW(control(name),EM_SETLIMITTEXT,128,0);
    for (const auto& item : {std::pair<int,const wchar_t*>{create_skin,L"保存为新设计"},{rename_skin,L"重命名"},{delete_skin,L"删除命名皮肤"}}) add(L"BUTTON",item.second,BS_PUSHBUTTON | WS_TABSTOP,item.first);
    add(L"STATIC",L"键盘皮肤预览",SS_OWNERDRAW,preview,170);
    for (const auto& item : {std::pair<int,const wchar_t*>{undo,L"撤销设计"},{redo,L"重做设计"},{optimize_contrast,L"优化对比度"},{reset_design,L"重置设计"}}) add(L"BUTTON",item.second,BS_PUSHBUTTON | WS_TABSTOP,item.first);
    for (const auto& item : {std::pair<int,const wchar_t*>{templates,L"皮肤模板"},{backgrounds,L"背景预设"}}) {
      add(L"STATIC",item.second,SS_LEFT,-1,22); add(L"COMBOBOX",item.second,CBS_DROPDOWNLIST | WS_VSCROLL | WS_TABSTOP,item.first,180); top -= 150;
      add(L"BUTTON",L"载入所选预设",BS_PUSHBUTTON | WS_TABSTOP,item.first == templates ? use_template : use_background);
    }
    add(L"STATIC",L"AI 皮肤风格",SS_LEFT,-1,22); add(L"EDIT",L"",ES_AUTOHSCROLL | WS_TABSTOP,ai_prompt); SendMessageW(control(ai_prompt),EM_SETLIMITTEXT,1000,0);
    add(L"BUTTON",L"生成 AI 皮肤",BS_PUSHBUTTON | WS_TABSTOP,ai_generate); add(L"BUTTON",L"取消生成",BS_PUSHBUTTON | WS_TABSTOP,ai_cancel);
    add(L"COMBOBOX",L"生成的方案",CBS_DROPDOWNLIST | WS_VSCROLL | WS_TABSTOP,ai_proposals,180); top -= 150;
    add(L"BUTTON",L"载入所选 AI 方案",BS_PUSHBUTTON | WS_TABSTOP,ai_use);
    for (size_t index = 0; index < colors.size(); ++index) add(L"BUTTON",colors[index].title,BS_PUSHBUTTON | WS_TABSTOP,first_color + static_cast<int>(index));
    for (size_t index = 0; index < 7; ++index) { add(L"STATIC",numbers[index].title,SS_LEFT,-1,22); add(L"EDIT",L"",ES_AUTOHSCROLL | WS_TABSTOP,first_number + static_cast<int>(index)); }
    const auto combo = [&](int id,const wchar_t* title,const std::vector<const wchar_t*>& choices) { add(L"STATIC",title,SS_LEFT,-1,22); add(L"COMBOBOX",title,CBS_DROPDOWNLIST | WS_VSCROLL | WS_TABSTOP,id,180); top -= 150; for (const auto* value : choices) SendMessageW(control(id),CB_ADDSTRING,0,reinterpret_cast<LPARAM>(value)); };
    combo(shape,L"按键形状",{L"圆角",L"胶囊",L"票券",L"卵石"}); combo(material,L"按键材质",{L"平面",L"浮起",L"玻璃",L"纸张"}); combo(pattern,L"背景图案",{L"无",L"圆点",L"网格",L"斜线"});
    for (const auto& item : {std::pair<int,const wchar_t*>{monospaced,L"等宽字体"},{gradient,L"启用渐变"},{horizontal,L"横向渐变"},{border_color,L"使用自定义边框颜色"}}) add(L"BUTTON",item.second,BS_AUTOCHECKBOX | WS_TABSTOP,item.first);
    for (const auto& item : {std::pair<int,const wchar_t*>{photo,L"选择照片（最多 512 KB）"},{remove_photo,L"移除照片"},{preview_change,L"预览修改"},{save_skin,L"保存到皮肤库"},{apply_skin,L"应用到键盘"},{refresh,L"重新读取"}}) add(L"BUTTON",item.second,BS_PUSHBUTTON | WS_TABSTOP,item.first);
    add(L"STATIC",L"",SS_LEFT,notice,100); layout(); enable();
  }
  static LRESULT CALLBACK proc(HWND hwnd,UINT message,WPARAM wparam,LPARAM lparam) {
    auto* self = reinterpret_cast<State*>(GetWindowLongPtrW(hwnd,GWLP_USERDATA));
    if (message == WM_NCCREATE) { self = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams); self->window = hwnd; SetWindowLongPtrW(hwnd,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(self)); }
    if (!self) return DefWindowProcW(hwnd,message,wparam,lparam);
    try {
      switch (message) {
      case WM_CREATE: self->controls(); return 0;
      case WM_SIZE: self->layout(); return 0;
      case WM_COMMAND:
        if (HIWORD(wparam) == EN_SETFOCUS || HIWORD(wparam) == CBN_SETFOCUS || HIWORD(wparam) == BN_SETFOCUS) SendMessageW(GetParent(hwnd),message,wparam,lparam);
        else self->command(wparam); return 0;
      case WM_DRAWITEM: { const auto& draw = *reinterpret_cast<DRAWITEMSTRUCT*>(lparam); if (draw.CtlID == preview) { keyboard_skin::preview(draw.hDC,draw.rcItem,self->look,self->font); return TRUE; } break; }
      case WM_MOUSEWHEEL: return SendMessageW(GetParent(hwnd),message,wparam,lparam);
      case completed: self->accept(); return 0;
      case ai_progress: self->text_notice(L"正在生成皮肤，已完成 " + std::to_wstring(wparam) + L" 个方案…"); return 0;
      case WM_DESTROY: self->cancel_generation(); if (self->worker.joinable()) self->worker.join(); return 0;
      case WM_NCDESTROY: self->window = nullptr; SetWindowLongPtrW(hwnd,GWLP_USERDATA,0); break;
      }
    } catch (...) { self->text_notice(L"无法处理皮肤操作，请重新读取。"); }
    return DefWindowProcW(hwnd,message,wparam,lparam);
  }
};
KeyboardSkinPage::KeyboardSkinPage(std::function<std::filesystem::path()> directory, Services services) : state_(std::make_unique<State>()) { state_->directory = std::move(directory); state_->services = std::move(services); }
KeyboardSkinPage::~KeyboardSkinPage() { if (state_->window) DestroyWindow(state_->window); if (state_->worker.joinable()) state_->worker.join(); }
HWND KeyboardSkinPage::handle() const { return state_->window; }
bool KeyboardSkinPage::create(HWND parent,HINSTANCE instance,HFONT font,UINT dpi) {
  if (state_->window || state_->worker.joinable()) return false;
  state_->instance = instance; state_->font = font; state_->dpi = dpi;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = page_class; type.hCursor = LoadCursorW(nullptr,IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_WINDOW);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  if (!CreateWindowExW(WS_EX_CONTROLPARENT,page_class,L"键盘皮肤制作",WS_CHILD | WS_CLIPCHILDREN,0,0,1,1,parent,nullptr,instance,state_.get())) return false;
  state_->start({State::Operation::read,{},Json(),"",std::nullopt,{}}); return true;
}
}
