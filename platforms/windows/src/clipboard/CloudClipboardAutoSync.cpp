#include "CloudClipboardAutoSync.h"

#include "ClipboardHistory.h"
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

std::optional<std::string> read_clipboard_text() {
  if (!OpenClipboard(nullptr)) return std::nullopt;
  struct CloseClipboardScope { ~CloseClipboardScope() { CloseClipboard(); } } close;
  if (!IsClipboardFormatAvailable(CF_UNICODETEXT)) return std::string{};
  const auto data = GetClipboardData(CF_UNICODETEXT);
  const auto *text = data ? static_cast<const wchar_t *>(GlobalLock(data)) : nullptr;
  if (!text) return std::nullopt;
  const auto value = bounded_clipboard_text(text, GlobalSize(data));
  GlobalUnlock(data);
  return wide_to_utf8(value);
}

bool write_clipboard_text(const std::string &text) {
  const auto wide = utf8_to_wide(text);
  if (wide.empty() || !OpenClipboard(nullptr)) return false;
  struct CloseClipboardScope { ~CloseClipboardScope() { CloseClipboard(); } } close;
  HGLOBAL data = GlobalAlloc(GMEM_MOVEABLE, (wide.size() + 1) * sizeof(wchar_t));
  if (!data) return false;
  auto *target = static_cast<wchar_t *>(GlobalLock(data));
  if (!target) { GlobalFree(data); return false; }
  std::copy(wide.begin(), wide.end(), target);
  target[wide.size()] = L'\0';
  GlobalUnlock(data);
  EmptyClipboard();
  if (!SetClipboardData(CF_UNICODETEXT, data)) { GlobalFree(data); return false; }
  return true;
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
    : directory_(std::move(directory)), worker_([this] { run(); }) {}

CloudClipboardAutoSync::~CloudClipboardAutoSync() {
  {
    std::lock_guard lock(mutex_);
    stop_ = true;
  }
  wake_.notify_one();
  if (worker_.joinable()) worker_.join();
}

void CloudClipboardAutoSync::observe_local_text(std::string text) {
  std::lock_guard lock(mutex_);
  if (receiver_owned_ && text == receiver_text_) {
    receiver_owned_ = false;
    local_text_ = std::move(text);
    return;
  }
  receiver_owned_ = false;
  local_text_ = text;
  pending_text_ = std::move(text);
  upload_pending_ = true;
  wake_.notify_one();
}

bool CloudClipboardAutoSync::request(const std::string &action, std::string *result) {
  const auto directory = directory_.u8string();
  if (directory.empty()) return false;
  try {
    const auto source_raw = msime_client_cloud_clipboard_source(
        reinterpret_cast<const uint8_t *>(directory.data()), directory.size());
    const auto source = parse_response(source_raw);
    if (!source.is_object()) return false;
    const auto selected = source.value("source", std::string{});
    const auto action_json = Json::parse(action);
    Json response;
    if (selected == "relay") {
      const auto body = Json{{"preferences_directory", directory}, {"directory", directory},
                             {"action", action_json}}.dump();
      response = parse_response(msime_client_relay_cloud_clipboard(
          reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    } else if (selected == "account") {
      const auto body = Json{{"directory", directory}, {"preferences_directory", directory},
                             {"action", action_json}}.dump();
      response = parse_response(msime_client_account_cloud_clipboard(
          reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    } else {
      return false;
    }
    if (response.empty()) return false;
    if (result) *result = response.dump();
    return true;
  } catch (...) {
    return false;
  }
}

void CloudClipboardAutoSync::upload(const std::string &text) {
  if (text.empty() || text.size() > 12000) return;
  std::string page;
  if (!request(R"({"operation":"list","search":""})", &page)) return;
  try {
    const auto value = Json::parse(page);
    if (!value.value("enabled", false) ||
        value.value("e2e", Json::object()).value("status", std::string{}) == "pending") return;
    request(Json{{"operation", "add"}, {"text", text}}.dump(), nullptr);
  } catch (...) {
  }
}

void CloudClipboardAutoSync::poll() {
  std::string page;
  if (!request(R"({"operation":"list","search":""})", &page)) return;
  try {
    const auto value = Json::parse(page);
    if (!value.value("enabled", false) ||
        value.value("e2e", Json::object()).value("status", std::string{}) == "pending") return;
    const auto items = value.value("items", Json::array());
    if (!items.is_array() || items.empty() || !items.at(0).is_object()) return;
    const auto item = items.at(0);
    const auto updated = item.value("updated_at", std::string{});
    const auto text = item.value("text", std::string{});
    if (updated.empty() || text.empty()) return;
    std::string current = read_clipboard_text().value_or(std::string{});
    {
      std::lock_guard lock(mutex_);
      if (updated <= latest_remote_updated_) return;
      if (!current.empty() && !receiver_owned_ && current != local_text_) {
        latest_remote_updated_ = updated;
        return;
      }
    }
    if (!write_clipboard_text(text)) return;
    const auto after = read_clipboard_text().value_or(std::string{});
    std::lock_guard lock(mutex_);
    if (after != text) {
      receiver_owned_ = false;
      local_text_ = after;
      return;
    }
    latest_remote_updated_ = updated;
    receiver_text_ = text;
    receiver_owned_ = true;
    local_text_ = text;
  } catch (...) {
  }
}

void CloudClipboardAutoSync::run() {
  std::unique_lock lock(mutex_);
  while (!stop_) {
    wake_.wait_for(lock, std::chrono::seconds(3), [this] { return stop_ || upload_pending_; });
    if (stop_) return;
    std::string upload_text;
    if (upload_pending_) {
      upload_pending_ = false;
      upload_text = pending_text_;
    }
    lock.unlock();
    if (!upload_text.empty()) upload(upload_text);
    poll();
    lock.lock();
  }
}

} // namespace msime::windows
