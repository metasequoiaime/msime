#include "Telemetry.h"
#include <curl/curl.h>
#include <nlohmann/json.hpp>
#include <filesystem>
#include <array>
#include <fstream>
#include <mutex>
#include <optional>
#include <random>
#include <sstream>
#include <cstdlib>
#ifdef _WIN32
#include <vector>
#include <windows.h>
#endif

namespace msime::telemetry {
namespace {
constexpr size_t max_queue_bytes = 1u << 20;
std::mutex lock;
std::filesystem::path file() {
#ifdef _WIN32
  // Read the wide value through Win32: getenv is deprecated under MSVC /WX and would pass the path through the ANSI code page, and MinGW's msvcrt import library has no _wdupenv_s.
  std::vector<wchar_t> base(32768);
  const DWORD length = GetEnvironmentVariableW(L"LOCALAPPDATA", base.data(), static_cast<DWORD>(base.size()));
  std::filesystem::path root = length && length < base.size() ? std::filesystem::path(std::wstring(base.data(), length)) : std::filesystem::temp_directory_path();
  return root / "MSIME" / "telemetry.json";
#else
  const char *base = std::getenv("XDG_STATE_HOME");
  if (!base) { const char *home = std::getenv("HOME"); base = home ? home : "/tmp"; }
  return std::filesystem::path(base) / (std::getenv("XDG_STATE_HOME") ? "msime/telemetry.json" : ".local/state/msime/telemetry.json");
#endif
}
std::string id() {
  // start() and the terminate handler can race here, so each thread draws from its own device.
  thread_local std::random_device random; std::ostringstream out; out << std::hex << random() << random(); return out.str();
}
// Hand-edited or foreign queue files can hold anything; keep only well-formed events.
bool queued(const nlohmann::json &event) {
  return event.is_object() && event.contains("id") && event["id"].is_string();
}
std::optional<std::string> read_queue(std::ifstream &input) {
  std::array<char, 8192> buffer{};
  std::string payload;
  while (input) {
    input.read(buffer.data(), static_cast<std::streamsize>(buffer.size()));
    const auto count = input.gcount();
    if (count <= 0) continue;
    const auto bytes = static_cast<size_t>(count);
    if (payload.size() > max_queue_bytes - bytes) return std::nullopt;
    payload.append(buffer.data(), bytes);
  }
  return input.eof() ? std::optional<std::string>(std::move(payload)) : std::nullopt;
}

bool safe_storage_path(const std::filesystem::path &path) {
  std::error_code error;
  auto current = path;
  while (true) {
    const auto status = std::filesystem::symlink_status(current, error);
    if (!error) return !std::filesystem::is_symlink(status);
    if (error != std::errc::no_such_file_or_directory) return false;
    error.clear();
    const auto parent = current.parent_path();
    if (parent == current) return true;
    current = parent;
  }
}

bool write_queue(const std::filesystem::path &path, const nlohmann::json &value) {
  if (!safe_storage_path(path)) return false;
  std::error_code error;
  std::filesystem::create_directories(path.parent_path(), error);
  if (error || !safe_storage_path(path.parent_path())) return false;
  const auto temporary = path.parent_path() /
                         (".telemetry-" + id());
  if (!safe_storage_path(temporary)) return false;
  {
    std::ofstream out(temporary, std::ios::binary | std::ios::trunc);
    if (!out) return false;
    out << value.dump();
    out.flush();
    if (!out) {
      std::filesystem::remove(temporary, error);
      return false;
    }
  }
  std::filesystem::rename(temporary, path, error);
  if (error) {
    std::filesystem::remove(temporary, error);
    return false;
  }
  return true;
}

void append(nlohmann::json event) {
  std::lock_guard guard(lock); auto path = file();
  nlohmann::json all = nlohmann::json::array();
  if (safe_storage_path(path)) {
    std::ifstream in(path);
    if (in) { try { if (const auto payload = read_queue(in)) all = nlohmann::json::parse(*payload); } catch (...) {} }
  }
  if (!all.is_array())
    all = nlohmann::json::array();
  nlohmann::json kept = nlohmann::json::array();
  for (auto &queuedEvent : all)
    if (queued(queuedEvent))
      kept.push_back(std::move(queuedEvent));
  all = std::move(kept);
  all.push_back(std::move(event));
  while (all.size() > 64)
    all.erase(all.begin());
  write_queue(path, all);
}
bool send(const nlohmann::json &event) {
  CURL *handle = curl_easy_init(); if (!handle) return false; std::string body = event.dump();
  struct curl_slist *headers = nullptr; headers = curl_slist_append(headers, "Content-Type: application/json");
  curl_easy_setopt(handle, CURLOPT_URL, "https://api.msime.app/v1/telemetry/events"); curl_easy_setopt(handle, CURLOPT_POST, 1L);
  curl_easy_setopt(handle, CURLOPT_POSTFIELDS, body.c_str()); curl_easy_setopt(handle, CURLOPT_TIMEOUT_MS, 8000L);
  curl_easy_setopt(handle, CURLOPT_CONNECTTIMEOUT_MS, 3000L); curl_easy_setopt(handle, CURLOPT_FOLLOWLOCATION, 0L);
  curl_easy_setopt(handle, CURLOPT_HTTPHEADER, headers); long status = 0; curl_easy_perform(handle); curl_easy_getinfo(handle, CURLINFO_RESPONSE_CODE, &status);
  curl_slist_free_all(headers); curl_easy_cleanup(handle); return status >= 200 && status < 300;
}
void remove(const std::string &eventID) {
  std::lock_guard guard(lock); auto path = file(); nlohmann::json all = nlohmann::json::array();
  if (!safe_storage_path(path)) return;
  std::ifstream in(path); if (in) { try { const auto payload = read_queue(in); if (!payload) return; all = nlohmann::json::parse(*payload); } catch (...) { return; } }
  if (!all.is_array()) return;
  nlohmann::json kept = nlohmann::json::array(); for (const auto &event : all) if (queued(event) && event["id"].get<std::string>() != eventID) kept.push_back(event);
  write_queue(path, kept);
}
}
// Telemetry must never take the host down: crash() runs inside the terminate handler.
void start(const std::string &platform, const std::string &version) {
  try {
    nlohmann::json event{{"id", id()}, {"kind", "download"}, {"platform", platform}, {"version", version}}; append(event); if (send(event)) remove(event["id"]);
  } catch (...) {}
}
void crash(const std::string &platform, const std::string &version, const std::string &message) {
  try {
    nlohmann::json event{{"id", id()}, {"kind", "crash"}, {"platform", platform}, {"version", version}, {"message", message.substr(0, 2048)}}; append(event); if (send(event)) remove(event["id"]);
  } catch (...) {}
}
}
