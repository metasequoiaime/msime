#include "HandwritingWindow.h"
#include "ResourcePackWorker.h"
#include "../../src/input/NativeTextInput.h"
#include "../../common/StateDirectory.h"
#include "../../compat/WindowsDpi.h"
#include "../../tsf/IPC/PassthroughStatistics.h"
#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include "../../../../shared/contracts/msime_edition.h"

#include <windowsx.h>
#include <commdlg.h>
#include <nlohmann/json.hpp>
#include <algorithm>
#include <array>
#include <condition_variable>
#include <cstdio>
#include <cwchar>
#include <deque>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <optional>
#include <string>
#include <thread>
#include <utility>
#include <vector>

namespace msime::windows::win7 {
namespace {
using Json = nlohmann::json;
using Stroke = std::vector<std::pair<float, float>>;
constexpr UINT completed_message = WM_APP + 72;
constexpr UINT model_message = WM_APP + 73;
constexpr wchar_t host_class[] = L"MSIME.Windows7Handwriting" MSIME_EDITION_NAME_SUFFIX;
constexpr wchar_t canvas_class[] = L"MSIME.Windows7Handwriting.Canvas" MSIME_EDITION_NAME_SUFFIX;
// 与共享 HandwritingRecognitionRequest 一致；超限时保留可撤销笔划，不增长缓存。
constexpr size_t max_strokes = 64;
constexpr size_t max_points = 4096;
constexpr size_t max_candidates = 12;

std::wstring wide(std::string_view value) {
  if (value.empty()) return {};
  const int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), nullptr, 0);
  if (!size) return {};
  std::wstring result(static_cast<size_t>(size), L'\0');
  MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), result.data(), size);
  return result;
}

std::filesystem::path environment_path(const wchar_t* name) {
  const DWORD required = GetEnvironmentVariableW(name, nullptr, 0);
  if (!required || required > 32768) return {};
  std::wstring value(required, L'\0');
  const DWORD copied = GetEnvironmentVariableW(name, value.data(), required);
  if (!copied || copied >= required) return {};
  value.resize(copied);
  return std::filesystem::path(value);
}

Json call(const Json& request) {
  const auto bytes = request.dump();
  auto answer = msime::host_api::own_string(msime_client_windows_handwriting(reinterpret_cast<const uint8_t*>(bytes.data()), bytes.size()));
  if (!answer) return Json();
  // 解析失败也回收跨 ABI 字符串；不把手写内容写入日志。
  return Json::parse(answer.get(), nullptr, false);
}

Json bounded_document(const std::filesystem::path& path) {
  if (!path.is_absolute()) return Json();
  std::ifstream stream(path, std::ios::binary);
  if (!stream) return Json();
  std::string content(1024 * 1024 + 1, '\0');
  stream.read(content.data(), static_cast<std::streamsize>(content.size()));
  content.resize(static_cast<size_t>(stream.gcount()));
  if (content.size() > 1024 * 1024) return Json();
  return Json::parse(content, nullptr, false);
}

std::vector<std::filesystem::path> select_model_files(HWND parent) {
  std::vector<wchar_t> buffer(65536, L'\0');
  OPENFILENAMEW dialog{sizeof(OPENFILENAMEW)};
  dialog.hwndOwner = parent;
  dialog.lpstrTitle = L"选择手写模型和许可证文件（可多选）";
  dialog.lpstrFilter = L"手写模型与许可证\0*.model;*.txt\0所有文件\0*.*\0\0";
  dialog.lpstrFile = buffer.data();
  dialog.nMaxFile = static_cast<DWORD>(buffer.size());
  dialog.Flags = OFN_EXPLORER | OFN_ALLOWMULTISELECT | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST |
      OFN_NOCHANGEDIR | OFN_DONTADDTORECENT | OFN_HIDEREADONLY;
  if (!GetOpenFileNameW(&dialog)) return {};
  const auto first = std::filesystem::path(buffer.data());
  const auto* next = buffer.data() + std::wcslen(buffer.data()) + 1;
  if (!*next) return {first};
  std::vector<std::filesystem::path> files;
  while (*next) {
    files.push_back(first / next);
    next += std::wcslen(next) + 1;
  }
  return files;
}

std::wstring megabytes(uint64_t bytes) {
  std::array<char, 48> text{};
  std::snprintf(text.data(), text.size(), "%.1f MB", static_cast<double>(bytes) / (1024.0 * 1024.0));
  return wide(text.data());
}

const wchar_t* model_error(const Json& response) {
  const auto error = response.is_object() ? response.value("error", "") : "";
  if (error == "local_model_cancelled") return L"已取消；再次下载可继续未完成的部分。";
  if (error == "resource_pack_busy") return L"手写模型正在其他任务中安装，请稍后刷新。";
  if (error.find("local_model_import_missing") == 0) return L"文件不完整：请同时选择手写模型和许可证文件。";
  if (error.find("local_model_import_unreadable") == 0) return L"无法读取所选文件，请检查文件权限后重试。";
  if (error.find("local_model_checksum_mismatch") == 0 || error.find("local_model_size_mismatch") == 0)
    return L"模型校验失败，未发布这些文件；请重新下载或选择正确文件。";
  if (error.find("local_model_network") == 0 || error.find("local_model_http_status") == 0)
    return L"模型下载失败，请检查网络后重试，也可离线导入。";
  return L"模型操作未能完成，请检查空间、目录权限后重试。";
}

struct Context { std::filesystem::path directory; Json options; };
Context context() {
  auto root = environment_path(L"MSIME_CLIENT_STATE_DIR");
  const bool injected = root.is_absolute();
  if (!root.is_absolute()) root = resolve_state_directory();
  auto options = environment_path(L"MSIME_CLIENT_HOST_OPTIONS");
  if (!options.is_absolute()) options = root / L"runtime-options.json";
  const auto document = bounded_document(options);
  if (document.is_object()) {
    const auto preferences = document.find("preferences_directory");
    if (!injected && preferences != document.end() && preferences->is_string()) {
      const auto directory = std::filesystem::u8path(preferences->get<std::string>());
      if (directory.is_absolute()) root = directory;
    }
  }
  return {root, document};
}

// 只在工作线程读取固定安装位置；已下载模型必须先由共享资源包接口确认发布状态。
std::optional<std::string> model_path() {
  const auto host = context();
  const auto& root = host.directory;
  const auto& document = host.options;
  auto model = environment_path(L"MSIME_HANDWRITING_MODEL");
  if (document.is_object()) {
    const auto configured = document.find("handwriting_model");
    if (configured != document.end() && configured->is_string() && !configured->get_ref<const std::string&>().empty())
      model = std::filesystem::u8path(configured->get<std::string>());
  }
  std::error_code error;
  if (!model.empty())
    return model.is_absolute() && std::filesystem::is_regular_file(model, error) ? std::optional<std::string>(model.u8string()) : std::nullopt;
  std::array<wchar_t, 32768> executable{};
  const DWORD length = GetModuleFileNameW(nullptr, executable.data(), static_cast<DWORD>(executable.size()));
  if (length && length < executable.size()) {
    model = std::filesystem::path(executable.data()).parent_path() / L"handwriting/handwriting-zh_CN.model";
    if (std::filesystem::is_regular_file(model, error)) return model.u8string();
  }
  if (!root.is_absolute()) return std::nullopt;
  const auto bytes = Json{{"state_root", root.u8string()}}.dump();
  auto response = msime::host_api::own_string(msime_client_resource_packs(reinterpret_cast<const uint8_t*>(bytes.data()), bytes.size()));
  if (!response) return std::nullopt;
  const auto packs = Json::parse(response.get(), nullptr, false);
  if (!packs.is_object() || !packs.value("ok", false) || !packs.contains("value") || !packs["value"].is_array()) return std::nullopt;
  for (const auto& pack : packs["value"]) {
    if (!pack.is_object() || pack.value("id", "") != "handwriting" || pack.value("state", "") != "installed") continue;
    model = root / L"resource-packs/handwriting/handwriting-zh_CN.model";
    if (std::filesystem::is_regular_file(model, error)) return model.u8string();
  }
  return std::nullopt;
}

struct Result { uint64_t generation; Json response; };
struct Job { uint64_t generation; Json request; };

class Recognizer {
public:
  explicit Recognizer(HWND host) : host_(host), thread_([this] { run(); }) {}
  ~Recognizer() {
    {
      std::lock_guard<std::mutex> guard(mutex_);
      stopping_ = true;
      pending_.reset();
    }
    wake_.notify_one();
    thread_.join();
  }
  void submit(Job job) {
    {
      std::lock_guard<std::mutex> guard(mutex_);
      pending_ = std::move(job);
    }
    wake_.notify_one();
  }
  bool record(std::wstring_view text) {
    const int size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
    if (size <= 0 || size > 4096) return false;
    std::string utf8(static_cast<size_t>(size), '\0');
    WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), utf8.data(), size, nullptr, nullptr);
    SYSTEMTIME now{};
    GetLocalTime(&now);
    std::array<char, 16> day{};
    std::snprintf(day.data(), day.size(), "%04u-%02u-%02u", now.wYear, now.wMonth, now.wDay);
    {
      std::lock_guard<std::mutex> guard(mutex_);
      if (records_.size() >= 64 || stopping_) return false;
      records_.push_back({{"operation", "record"}, {"text", std::move(utf8)}, {"source", "handwriting"},
          {"day", day.data()}, {"hour", now.wHour}});
    }
    wake_.notify_one();
    return true;
  }
  std::optional<Result> take() {
    std::lock_guard<std::mutex> guard(mutex_);
    return std::exchange(result_, std::nullopt);
  }
private:
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> guard(mutex_);
      wake_.wait(guard, [this] { return stopping_ || pending_.has_value() || !records_.empty(); });
      if (!records_.empty()) {
        auto record = std::move(records_.front());
        records_.pop_front();
        guard.unlock();
        try {
          const auto host = context();
          if (host.directory.is_absolute()) {
            const auto bytes = Json{{"directory", host.directory.u8string()}, {"action", std::move(record)}}.dump();
            const auto answer = msime::host_api::own_string(msime_client_typing_statistics(reinterpret_cast<const uint8_t*>(bytes.data()), bytes.size()));
            (void)answer;
          }
        } catch (...) {
          // 统计失败不重复上屏，也不把候选正文写入日志。
        }
        continue;
      }
      if (stopping_) return;
      auto job = std::move(*pending_);
      pending_.reset();
      guard.unlock();
      Json answer;
      try {
        if (job.request.value("operation", "") == "recognize") {
          if (const auto model = model_path()) job.request["model"] = *model;
        }
        answer = call(job.request);
      } catch (...) {
        answer = Json();
      }
      guard.lock();
      if (stopping_) return;
      result_ = Result{job.generation, std::move(answer)};
      // 不传递堆指针；窗口析构先 join，防止消息发送给复用后的 HWND。
      PostMessageW(host_, completed_message, 0, 0);
    }
  }
  HWND host_;
  std::mutex mutex_;
  std::condition_variable wake_;
  bool stopping_ = false;
  std::optional<Job> pending_;
  std::optional<Result> result_;
  std::deque<Json> records_;
  std::thread thread_;
};
} // namespace

struct HandwritingWindow::State {
  State(SubmitCandidate sink, SelectModelFiles select) : submit(std::move(sink)), select_files(std::move(select)) {}
  HWND host = nullptr;
  HWND canvas = nullptr;
  HWND status = nullptr;
  HWND undo = nullptr;
  HWND clear = nullptr;
  HWND model_status = nullptr;
  HWND model_progress = nullptr;
  HWND model_download = nullptr;
  HWND model_import = nullptr;
  HWND model_remove = nullptr;
  HWND model_cancel = nullptr;
  HWND model_refresh = nullptr;
  std::array<HWND, max_candidates> buttons{};
  std::vector<Stroke> strokes;
  std::vector<std::wstring> candidates;
  uint64_t generation = 0;
  bool drawing = false;
  UINT dpi = 96;
  HFONT font = nullptr;
  SubmitCandidate submit;
  SelectModelFiles select_files;
  bool model_known = false;
  std::string model_state;
  msimeui::NativeTextInputTarget target = msimeui::NativeTextInputTarget::Capture();
  std::unique_ptr<Recognizer> recognizer;
  std::unique_ptr<ResourcePackWorker> models;

  ~State() {
    models.reset();
    recognizer.reset();
    if (host && IsWindow(host)) DestroyWindow(host);
    if (font) DeleteObject(font);
  }
  int px(int value) const { return MulDiv(value, static_cast<int>(dpi), 96); }
  void notice(const wchar_t* message) { SetWindowTextW(status, message); }
  void model_action(ResourcePackAction action, std::vector<std::filesystem::path> files = {}) {
    if (models->start(action, "handwriting", std::move(files))) model_complete();
  }
  void import_model() {
    auto files = select_files ? select_files(host) : select_model_files(host);
    if (files.empty()) return;
    if (files.size() > 16) { SetWindowTextW(model_progress, L"一次最多选择 16 个文件。请只选择手写模型及其许可证。"); return; }
    model_action(ResourcePackAction::import_files, std::move(files));
  }
  void model_complete() {
    if (!models) return;
    const auto update = models->take();
    if (update.result) {
      const auto& catalog = update.result->catalog;
      model_known = false;
      if (catalog.is_object() && catalog.value("ok", false) && catalog.contains("value") && catalog["value"].is_array()) {
        for (const auto& pack : catalog["value"]) {
          if (!pack.is_object() || pack.value("id", "") != "handwriting") continue;
          model_state = pack.value("state", "");
          model_known = model_state == "installed" || model_state == "outdated" || model_state == "missing";
          const auto title = std::wstring(L"手写模型包：") + (model_state == "installed" ? L"已安装" :
              model_state == "outdated" ? L"需要更新" : L"未安装") + L"（" + megabytes(pack.value<uint64_t>("size", 0)) + L"）";
          SetWindowTextW(model_status, title.c_str());
          SetWindowTextW(model_download, model_state == "outdated" ? L"更新模型" : L"下载模型");
          break;
        }
      }
      if (!model_known) SetWindowTextW(model_status, L"手写模型包：状态不可用");
      const auto& response = update.result->response;
      const bool ok = response.is_object() && response.value("ok", false);
      const auto action = update.result->action;
      SetWindowTextW(model_progress, !ok ? model_error(response) : action == ResourcePackAction::list ?
          L"系统识别器可直接使用；模型包用于系统识别无结果时的回退。" : action == ResourcePackAction::remove ?
          L"已移除下载的模型包；系统识别器和随包模型仍可使用。" : L"手写模型已校验并安装，可以继续书写。");
      // 模型变化后重识别当前笔划，旧的识别结果由画布代次拒绝。
      if (ok && action != ResourcePackAction::list && !drawing) recognize();
    }
    const bool transfer = update.action == ResourcePackAction::download || update.action == ResourcePackAction::import_files;
    EnableWindow(model_download, !update.busy && model_known && model_state != "installed");
    EnableWindow(model_import, !update.busy && model_known);
    EnableWindow(model_remove, !update.busy && model_known && model_state != "missing");
    EnableWindow(model_refresh, !update.busy);
    EnableWindow(model_cancel, update.busy && transfer && !update.cancelling);
    if (!update.busy) return;
    if (update.cancelling) SetWindowTextW(model_progress, L"正在取消模型安装…");
    else if (!update.progress.phase.empty()) {
      const auto& progress = update.progress;
      const auto text = std::wstring(progress.phase == "import" ? L"正在导入模型：" : progress.phase == "download" ?
          L"正在下载模型：" : progress.phase == "done" ? L"正在完成模型安装：" : L"正在校验模型：") +
          megabytes(progress.done) + L" / " + megabytes(progress.total);
      SetWindowTextW(model_progress, text.c_str());
    } else SetWindowTextW(model_progress, update.action == ResourcePackAction::list ? L"正在查询模型状态…" :
        update.action == ResourcePackAction::remove ? L"正在移除模型包…" : L"正在准备模型安装…");
  }
  void reset_candidates() {
    candidates.clear();
    for (HWND button : buttons) { ShowWindow(button, SW_HIDE); SetWindowTextW(button, L""); }
  }
  void recognize() {
    ++generation;
    reset_candidates();
    InvalidateRect(canvas, nullptr, FALSE);
    EnableWindow(undo, !strokes.empty());
    EnableWindow(clear, !strokes.empty());
    if (strokes.empty()) { notice(L"在方框内书写，松开鼠标后自动识别。"); return; }
    Json ink = Json::array();
    for (const auto& stroke : strokes) {
      Json points = Json::array();
      for (auto [x, y] : stroke) points.push_back({{"x", x}, {"y", y}});
      ink.push_back({{"points", std::move(points)}});
    }
    notice(L"正在识别…");
    Json request{{"operation", "recognize"}, {"request", {{"strokes", std::move(ink)}}}};
    recognizer->submit({generation, std::move(request)});
  }
  void complete() {
    auto result = recognizer->take();
    if (!result || result->generation != generation || drawing) return;
    const auto& response = result->response;
    if (!response.is_object() || !response.value("ok", false)) {
      const auto error = response.is_object() ? response.value("error", "") : "";
      notice(error == "no_chinese_recognizer" ? L"未安装中文手写识别器，请添加系统中文手写组件或安装手写模型。" :
          error == "handwriting_not_offered" ? L"此版本不提供手写输入。" : L"手写识别未能完成，请重试。");
      return;
    }
    const auto& value = response.at("value");
    if (value.contains("system_recognizer")) {
      notice(value.value("system_recognizer", false) ? L"在方框内书写，松开鼠标后自动识别。" : L"未检测到系统中文手写识别器；可下载或离线导入手写模型。");
      return;
    }
    if (!value.contains("candidates") || !value.at("candidates").is_array()) { notice(L"手写识别未能完成，请重试。"); return; }
    reset_candidates();
    for (const auto& candidate : value.at("candidates")) {
      if (!candidate.is_string() || candidates.size() == max_candidates) break;
      auto text = wide(candidate.get_ref<const std::string&>());
      if (text.empty()) continue;
      HWND button = buttons[candidates.size()];
      candidates.push_back(std::move(text));
      SetWindowTextW(button, candidates.back().c_str());
      ShowWindow(button, SW_SHOWNOACTIVATE);
    }
    notice(candidates.empty() ? L"未识别到内容，可以继续书写或撤销后重写。" : L"点击候选输入到原来的编辑器。");
  }
  void insert(size_t index) {
    if (index >= candidates.size()) return;
    const bool sent = submit ? submit(candidates[index]) : target.Insert(candidates[index], host, PanelTextSendInputExtraInfo);
    if (!sent) { notice(L"未能输入：请保持原来的编辑器在前台后重试。"); return; }
    const bool recorded = recognizer->record(candidates[index]);
    strokes.clear();
    recognize();
    if (!recorded) notice(L"已输入；统计队列繁忙，本次未记录。");
  }
  void point(LPARAM coordinates) {
    if (!drawing || strokes.empty() || strokes.back().size() >= max_points) return;
    RECT bounds{};
    GetClientRect(canvas, &bounds);
    const float width = static_cast<float>(std::max<LONG>(1, bounds.right));
    const float height = static_cast<float>(std::max<LONG>(1, bounds.bottom));
    const std::pair<float, float> value{
        std::clamp(static_cast<float>(GET_X_LPARAM(coordinates)) * 420.0f / width, 0.0f, 420.0f),
        std::clamp(static_cast<float>(GET_Y_LPARAM(coordinates)) * 420.0f / height, 0.0f, 420.0f)};
    if (strokes.back().empty() || strokes.back().back() != value) strokes.back().push_back(value);
    InvalidateRect(canvas, nullptr, FALSE);
  }
  void paint() {
    PAINTSTRUCT paint{};
    HDC dc = BeginPaint(canvas, &paint);
    RECT bounds{};
    GetClientRect(canvas, &bounds);
    FillRect(dc, &bounds, GetSysColorBrush(COLOR_WINDOW));
    SetBkMode(dc, TRANSPARENT);
    HPEN guide = CreatePen(PS_DOT, 1, GetSysColor(COLOR_GRAYTEXT));
    HGDIOBJ old = SelectObject(dc, guide);
    MoveToEx(dc, bounds.right / 2, 0, nullptr); LineTo(dc, bounds.right / 2, bounds.bottom);
    MoveToEx(dc, 0, bounds.bottom / 2, nullptr); LineTo(dc, bounds.right, bounds.bottom / 2);
    SelectObject(dc, old); DeleteObject(guide);
    HPEN pen = CreatePen(PS_SOLID, std::max(1, px(3)), GetSysColor(COLOR_WINDOWTEXT));
    old = SelectObject(dc, pen);
    for (const auto& stroke : strokes) {
      bool first = true;
      for (const auto& [x, y] : stroke) {
        const int left = static_cast<int>(x * static_cast<float>(bounds.right) / 420.0f);
        const int top = static_cast<int>(y * static_cast<float>(bounds.bottom) / 420.0f);
        if (first) MoveToEx(dc, left, top, nullptr); else LineTo(dc, left, top);
        first = false;
      }
    }
    SelectObject(dc, old); DeleteObject(pen);
    FrameRect(dc, &bounds, GetSysColorBrush(COLOR_WINDOWTEXT));
    EndPaint(canvas, &paint);
  }
  static LRESULT CALLBACK canvas_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* state = reinterpret_cast<State*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
      state = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams);
      SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(state));
    }
    if (!state) return DefWindowProcW(window, message, wparam, lparam);
    switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_LBUTTONDOWN:
      if (state->strokes.size() == max_strokes) { state->notice(L"笔划已达上限，请先撤销或清空。"); return 0; }
      ++state->generation;
      state->reset_candidates();
      state->drawing = true;
      state->strokes.emplace_back();
      SetCapture(window);
      state->point(lparam);
      return 0;
    case WM_MOUSEMOVE:
      if (state->drawing) state->point(lparam);
      return 0;
    case WM_LBUTTONUP:
      if (!state->drawing) return 0;
      state->point(lparam);
      state->drawing = false;
      ReleaseCapture();
      if (state->strokes.back().size() < 2) state->strokes.pop_back();
      state->recognize();
      return 0;
    case WM_CAPTURECHANGED:
      if (state->drawing) {
        state->drawing = false;
        state->strokes.pop_back();
        state->recognize();
      }
      return 0;
    case WM_PAINT: state->paint(); return 0;
    case WM_ERASEBKGND: return 1;
    default: return DefWindowProcW(window, message, wparam, lparam);
    }
  }
  static LRESULT CALLBACK host_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* state = reinterpret_cast<State*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
      state = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams);
      state->host = window;
      SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(state));
    }
    if (!state) return DefWindowProcW(window, message, wparam, lparam);
    switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_COMMAND: {
      const int control = LOWORD(wparam);
      if (HIWORD(wparam) != BN_CLICKED) break;
      if (control >= handwriting_candidate && control < handwriting_candidate + static_cast<int>(max_candidates)) state->insert(static_cast<size_t>(control - handwriting_candidate));
      else if (control == handwriting_undo && !state->strokes.empty()) { state->strokes.pop_back(); state->recognize(); }
      else if (control == handwriting_clear) { state->strokes.clear(); state->recognize(); }
      else if (control == handwriting_model_download && IsWindowEnabled(state->model_download)) state->model_action(ResourcePackAction::download);
      else if (control == handwriting_model_import && IsWindowEnabled(state->model_import)) state->import_model();
      else if (control == handwriting_model_remove && IsWindowEnabled(state->model_remove)) state->model_action(ResourcePackAction::remove);
      else if (control == handwriting_model_refresh && IsWindowEnabled(state->model_refresh)) state->model_action(ResourcePackAction::list);
      else if (control == handwriting_model_cancel && IsWindowEnabled(state->model_cancel)) { state->models->cancel(); state->model_complete(); }
      return 0;
    }
    case completed_message: state->complete(); return 0;
    case model_message: state->model_complete(); return 0;
    case WM_CLOSE:
      state->models.reset();
      state->recognizer.reset();
      DestroyWindow(window);
      return 0;
    case WM_DESTROY:
      state->host = nullptr;
      PostQuitMessage(0);
      return 0;
    default: break;
    }
    return DefWindowProcW(window, message, wparam, lparam);
  }
};

HandwritingWindow::HandwritingWindow(SubmitCandidate submit, SelectModelFiles select)
    : state_(std::make_unique<State>(std::move(submit), std::move(select))) {}
HandwritingWindow::~HandwritingWindow() = default;
HWND HandwritingWindow::handle() const { return state_->host; }

bool HandwritingWindow::create(HINSTANCE instance) {
#if !MSIME_EDITION_HANDWRITING
  (void)instance;
  return false;
#else
  auto& state = *state_;
  state.dpi = compat::system_dpi();
  MONITORINFO monitor{sizeof(MONITORINFO)};
  if (!GetMonitorInfoW(MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST), &monitor)) {
    if (!SystemParametersInfoW(SPI_GETWORKAREA, 0, &monitor.rcWork, 0))
      monitor.rcWork = {0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)};
  }
  // 小屏幕上缩放整块面板，保证画布、候选与操作按钮都留在工作区内。
  const UINT fit = static_cast<UINT>(std::max<LONG>(48, std::min(
      (monitor.rcWork.right - monitor.rcWork.left) * 96 / 720,
      (monitor.rcWork.bottom - monitor.rcWork.top) * 96 / 660)));
  state.dpi = std::min(state.dpi, fit);
  WNDCLASSEXW type{sizeof(WNDCLASSEXW)};
  type.hInstance = instance;
  type.hCursor = LoadCursorW(nullptr, IDC_ARROW);
  type.hbrBackground = GetSysColorBrush(COLOR_BTNFACE);
  type.lpfnWndProc = State::host_proc;
  type.lpszClassName = host_class;
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  type.lpfnWndProc = State::canvas_proc;
  type.lpszClassName = canvas_class;
  type.hCursor = LoadCursorW(nullptr, IDC_CROSS);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  RECT frame{0, 0, state.px(700), state.px(600)};
  const DWORD style = WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
  const DWORD extended = WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
  AdjustWindowRectEx(&frame, style, FALSE, extended);
  const int left = monitor.rcWork.left + ((monitor.rcWork.right - monitor.rcWork.left) - (frame.right - frame.left)) / 2;
  const int top = monitor.rcWork.top + ((monitor.rcWork.bottom - monitor.rcWork.top) - (frame.bottom - frame.top)) / 2;
  state.host = CreateWindowExW(extended, host_class, L"手写输入", style, left, top,
      frame.right - frame.left, frame.bottom - frame.top, nullptr, nullptr, instance, &state);
  if (!state.host) return false;
  state.font = CreateFontW(-MulDiv(10, static_cast<int>(state.dpi), 72), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
      DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
  const auto control = [&](const wchar_t* kind, const wchar_t* title, DWORD flags, int id, int x, int y, int width, int height) {
    HWND child = CreateWindowExW(0, kind, title, WS_CHILD | WS_VISIBLE | flags, state.px(x), state.px(y), state.px(width), state.px(height),
        state.host, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), instance, nullptr);
    if (child) SendMessageW(child, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE);
    return child;
  };
  state.canvas = CreateWindowExW(0, canvas_class, L"手写区域", WS_CHILD | WS_VISIBLE, state.px(16), state.px(16), state.px(420), state.px(420),
      state.host, reinterpret_cast<HMENU>(static_cast<INT_PTR>(handwriting_canvas)), instance, &state);
  state.undo = control(L"BUTTON", L"撤销一笔", BS_PUSHBUTTON, handwriting_undo, 16, 444, 100, 28);
  state.clear = control(L"BUTTON", L"清空", BS_PUSHBUTTON, handwriting_clear, 128, 444, 100, 28);
  state.model_refresh = control(L"BUTTON", L"刷新模型", BS_PUSHBUTTON, handwriting_model_refresh, 240, 444, 100, 28);
  state.model_status = control(L"STATIC", L"正在查询手写模型包…", SS_LEFT, handwriting_model_status, 452, 392, 232, 44);
  state.model_download = control(L"BUTTON", L"下载模型", BS_PUSHBUTTON, handwriting_model_download, 452, 444, 110, 28);
  state.model_import = control(L"BUTTON", L"离线导入", BS_PUSHBUTTON, handwriting_model_import, 574, 444, 110, 28);
  state.model_remove = control(L"BUTTON", L"移除模型包", BS_PUSHBUTTON, handwriting_model_remove, 452, 480, 110, 28);
  state.model_cancel = control(L"BUTTON", L"取消安装", BS_PUSHBUTTON, handwriting_model_cancel, 574, 480, 110, 28);
  state.model_progress = control(L"STATIC", L"", SS_LEFT, handwriting_model_progress, 16, 516, 668, 28);
  state.status = control(L"STATIC", L"正在查询中文手写识别器…", SS_LEFT, handwriting_status, 16, 552, 668, 40);
  for (size_t i = 0; i < max_candidates; ++i) {
    state.buttons[i] = control(L"BUTTON", L"", BS_PUSHBUTTON, handwriting_candidate + static_cast<int>(i),
        452 + static_cast<int>(i % 2) * 118, 16 + static_cast<int>(i / 2) * 62, 112, 54);
    if (!state.buttons[i]) return false;
  }
  if (!state.canvas || !state.undo || !state.clear || !state.status || !state.model_status || !state.model_progress ||
      !state.model_download || !state.model_import || !state.model_remove || !state.model_cancel || !state.model_refresh) return false;
  state.reset_candidates();
  EnableWindow(state.undo, FALSE);
  EnableWindow(state.clear, FALSE);
  state.recognizer = std::make_unique<Recognizer>(state.host);
  state.models = std::make_unique<ResourcePackWorker>([] { return context().directory; },
      [window = state.host] { PostMessageW(window, model_message, 0, 0); });
  state.model_action(ResourcePackAction::list);
  state.recognizer->submit({state.generation, {{"operation", "status"}}});
  return true;
#endif
}

int HandwritingWindow::run(int show) {
  ShowWindow(state_->host, show == SW_HIDE ? SW_HIDE : SW_SHOWNOACTIVATE);
  MSG message{};
  BOOL result;
  while ((result = GetMessageW(&message, nullptr, 0, 0)) > 0) { TranslateMessage(&message); DispatchMessageW(&message); }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}

} // namespace msime::windows::win7
