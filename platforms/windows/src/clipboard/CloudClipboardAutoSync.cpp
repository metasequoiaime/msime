#include "CloudClipboardAutoSync.h"

#include "ClipboardHistory.h"
#include "ClipboardPrivacyPolicy.h"
#include "CloudClipboardReceivePolicy.h"
#include "CloudClipboardWriter.h"
#include "../../../common/HostApiString.h"
#include "msime_client.h"

#include <windows.h>
#include <nlohmann/json.hpp>

#include <algorithm>
#include <chrono>
#include <optional>
#include <utility>

namespace msime::windows {
namespace {
using Json = nlohmann::json;

std::string wide_to_utf8(const std::wstring &text) {
  if (text.empty()) return {};
  const int size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(),
                                       static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
  if (size <= 0) return {};
  std::string result(size, '\0');
  if (!WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(),
                           static_cast<int>(text.size()), result.data(), size, nullptr, nullptr))
    return {};
  return result;
}

std::wstring utf8_to_wide(const std::string &text) {
  if (text.empty()) return {};
  const int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                                       static_cast<int>(text.size()), nullptr, 0);
  if (size <= 0) return {};
  std::wstring result(size, L'\0');
  if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                           static_cast<int>(text.size()), result.data(), size))
    return {};
  return result;
}

struct ClipboardSample {
  DWORD sequence;
  std::optional<std::string> text;
  bool has_content;
};

ClipboardSample read_clipboard_text() {
  const auto sequence = GetClipboardSequenceNumber();
  if (!OpenClipboard(nullptr)) return {sequence, std::nullopt, true};
  struct CloseClipboardScope { ~CloseClipboardScope() { CloseClipboard(); } } close;
  const auto current_sequence = GetClipboardSequenceNumber();
  const bool has_content = CountClipboardFormats() != 0;
  for (const auto *name : {clipboard_exclude_monitor_format, clipboard_viewer_ignore_format}) {
    if (IsClipboardFormatAvailable(RegisterClipboardFormatW(name)))
      return {current_sequence, std::nullopt, has_content};
  }
  for (const auto *name : {clipboard_history_permission_format, clipboard_cloud_permission_format}) {
    const auto format = RegisterClipboardFormatW(name);
    if (!IsClipboardFormatAvailable(format)) continue;
    const auto data = GetClipboardData(format);
    const void *bytes = data ? GlobalLock(data) : nullptr;
    const auto permission = clipboard_permission_from_data(bytes, bytes ? GlobalSize(data) : 0);
    if (bytes) GlobalUnlock(data);
    if (permission == ClipboardPermission::denied)
      return {current_sequence, std::nullopt, has_content};
  }
  if (!IsClipboardFormatAvailable(CF_UNICODETEXT))
    return {current_sequence, has_content ? std::nullopt : std::optional<std::string>(""), has_content};
  const auto data = GetClipboardData(CF_UNICODETEXT);
  const auto *text = data ? static_cast<const wchar_t *>(GlobalLock(data)) : nullptr;
  if (!text) return {current_sequence, std::nullopt, true};
  const auto value = bounded_clipboard_text(text, GlobalSize(data));
  GlobalUnlock(data);
  return {current_sequence, wide_to_utf8(value), true};
}

bool write_clipboard_text(const std::string &text, DWORD expected_sequence) {
  return write_cloud_clipboard_text(utf8_to_wide(text), expected_sequence);
}

Json parse_response(char *raw) {
  auto owned = msime::host_api::own_string(raw);
  if (!owned) return Json::object();
  try {
    const auto envelope = Json::parse(owned.get());
    return envelope.value("ok", false) ? envelope.value("value", Json::object()) : Json::object();
  } catch (...) {
    return Json::object();
  }
}
}

CloudClipboardAutoSync::CloudClipboardAutoSync(std::filesystem::path directory)
    : directory_(std::move(directory)) {
  // 所有成员完成初始化后才能启动线程。
  worker_ = std::thread([this] { run(); });
}

CloudClipboardAutoSync::~CloudClipboardAutoSync() {
  {
    std::lock_guard lock(mutex_);
    stop_ = true;
  }
  wake_.notify_one();
  if (worker_.joinable()) worker_.join();
}

bool CloudClipboardAutoSync::stopped() {
  std::lock_guard lock(mutex_);
  return stop_;
}

std::string CloudClipboardAutoSync::current_binding() {
  const auto directory = directory_.u8string();
  if (directory.empty()) return {};
  const auto source = parse_response(msime_client_cloud_clipboard_source(
      reinterpret_cast<const uint8_t *>(directory.data()), directory.size()));
  if (!source.is_object() || !source.contains("revision")) return {};
  const auto selected = source.value("source", std::string{});
  Json binding{{"source", selected}, {"revision", source.at("revision")}};
  if (selected == "account") {
    const auto body = Json{{"directory", directory}, {"action", {{"operation", "binding"}}}}.dump();
    const auto login = parse_response(msime_client_account_cloud_clipboard(
        reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    if (!login.is_object() || login.value("user_id", std::string{}).empty() ||
        login.value("session_id", std::string{}).empty()) return {};
    binding["login"] = login;
  } else if (selected != "relay") {
    return {};
  }
  return binding.dump();
}

bool CloudClipboardAutoSync::request(const std::string &action, const std::string &binding,
                                     std::string *result) {
  try {
    if (stopped() || binding.empty() || current_binding() != binding) return false;
    const auto selected = Json::parse(binding);
    const auto directory = directory_.u8string();
    auto action_json = Json::parse(action);
    Json response;
    if (selected.at("source") == "relay") {
      const auto body = Json{{"preferences_directory", directory}, {"directory", directory},
                             {"expected_revision", selected.at("revision")}, {"action", action_json}}.dump();
      response = parse_response(msime_client_relay_cloud_clipboard(
          reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    } else {
      action_json["expected_login"] = selected.at("login");
      const auto body = Json{{"directory", directory}, {"preferences_directory", directory},
                             {"action", action_json}}.dump();
      response = parse_response(msime_client_account_cloud_clipboard(
          reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    }
    if (response.empty() || stopped() || current_binding() != binding) return false;
    if (result) *result = response.dump();
    return true;
  } catch (...) {
    return false;
  }
}

void CloudClipboardAutoSync::poll() {
  try {
    const auto before = read_clipboard_text();
    const bool locally_changed = initialized_ && observed_sequence_ != before.sequence;
    observed_sequence_ = before.sequence;
    initialized_ = true;
    const auto captured = current_binding();
    const bool same_binding = !captured.empty() && captured == binding_;
    if (captured != binding_) {
      binding_ = captured;
      policy_.begin(before.sequence, before.has_content);
    }
    if (captured.empty()) return;
    std::string page;
    if (!request(R"({"operation":"list","search":""})", captured, &page)) return;
    const auto value = Json::parse(page);
    if (!value.value("enabled", false) ||
        value.value("e2e", Json::object()).value("status", std::string{}) == "pending") return;
    const auto items = value.value("items", Json::array());
    if (!items.is_array()) return;
    std::optional<std::string> revision;
    std::string text;
    if (!items.empty()) {
      if (!items.at(0).is_object()) return;
      revision = clipboard_remote_revision(items.at(0).value("updated_at", std::string{}));
      text = items.at(0).value("text", std::string{});
      if (!revision || text.empty() || text.size() > 12000) return;
    }
    const auto current = read_clipboard_text();
    observed_sequence_ = current.sequence;
    if (!current.text || stopped() || current_binding() != captured) return;
    const bool receive = policy_.receive(revision, current.sequence);
    if (same_binding && (locally_changed || current.sequence != before.sequence)) {
      // 当前复制只尝试一次，失败不保留正文，不转发给之后切换的账号或中继。
      if (!current.text->empty() && *current.text != text && current.text->size() <= 12000)
        request(Json{{"operation", "add"}, {"text", *current.text}}.dump(), captured, nullptr);
      return;
    }
    if (!receive || !revision) return;
    if (*current.text == text) {
      policy_.received(*revision, current.sequence);
      return;
    }
    if (write_clipboard_text(text, current.sequence)) {
      observed_sequence_ = GetClipboardSequenceNumber();
      policy_.received(*revision, observed_sequence_);
    }
  } catch (...) {
    // 读取、权限和网络失败均不影响输入线程。
  }
}

void CloudClipboardAutoSync::run() {
  std::unique_lock lock(mutex_);
  while (!stop_) {
    lock.unlock();
    poll();
    lock.lock();
    wake_.wait_for(lock, std::chrono::seconds(3), [this] { return stop_; });
  }
}

} // namespace msime::windows
