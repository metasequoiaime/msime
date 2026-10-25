#pragma once

#include <curl/curl.h>
#include <array>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

namespace msime::windows::compat {
// 仅负责传输；鉴权和豆包协议仍由共享 Rust 层产生与解析。
class Windows7WebSocket {
public:
  Windows7WebSocket() = default;
  Windows7WebSocket(const Windows7WebSocket&) = delete;
  Windows7WebSocket& operator=(const Windows7WebSocket&) = delete;
  ~Windows7WebSocket() {
    // 所有收发线程必须先退出，才能销毁 CURL 句柄。
    if (curl_) curl_easy_cleanup(curl_);
    if (headers_) curl_slist_free_all(headers_);
  }

  static bool available() {
    const auto* version = curl_version_info(CURLVERSION_NOW);
    if (!version || !version->protocols) return false;
    for (auto protocol = version->protocols; *protocol; ++protocol)
      if (std::string(*protocol) == "wss") return true;
    return false;
  }

  bool connect(const std::string& endpoint, const std::string& headers
#ifdef MSIME_WEBSOCKET_TESTING
               , const char* test_ca = nullptr
#endif
  ) {
    if (curl_ || endpoint.rfind("wss://", 0) != 0 || !available()) return false;
    static std::once_flag initialize;
    static CURLcode initialized = CURLE_FAILED_INIT;
    std::call_once(initialize, [] { initialized = curl_global_init(CURL_GLOBAL_DEFAULT); });
    if (initialized != CURLE_OK) return false;
    curl_ = curl_easy_init();
    if (!curl_) return false;
    std::size_t offset = 0;
    while (offset < headers.size()) {
      const auto end = headers.find("\r\n", offset);
      if (end == std::string::npos || end == offset) return false;
      const auto line = headers.substr(offset, end - offset);
      if (line.find_first_of("\r\n\0", 0, 3) != std::string::npos) return false;
      auto* list = curl_slist_append(headers_, line.c_str());
      if (!list) return false;
      headers_ = list;
      offset = end + 2;
    }
    if (curl_easy_setopt(curl_, CURLOPT_URL, endpoint.c_str()) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_HTTPHEADER, headers_) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_PROTOCOLS_STR, "wss") != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_FOLLOWLOCATION, 0L) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_CONNECT_ONLY, 2L) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_CONNECTTIMEOUT_MS, 10000L) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_TIMEOUT_MS, 10000L) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_NOSIGNAL, 1L) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_SSL_VERIFYPEER, 1L) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_SSL_VERIFYHOST, 2L) != CURLE_OK ||
        curl_easy_setopt(curl_, CURLOPT_HTTP_VERSION, CURL_HTTP_VERSION_1_1) != CURLE_OK)
      return false;
#ifdef MSIME_WEBSOCKET_TESTING
    if (test_ca && curl_easy_setopt(curl_, CURLOPT_CAINFO, test_ca) != CURLE_OK) return false;
#endif
#ifdef MSIME_WEBSOCKET_TESTING
    if (curl_easy_setopt(curl_, CURLOPT_ERRORBUFFER, error_) != CURLE_OK) return false;
#endif
    return curl_easy_perform(curl_) == CURLE_OK;
  }

#ifdef MSIME_WEBSOCKET_TESTING
  const char* test_error() const { return error_; }
#endif

  bool send(const std::vector<std::uint8_t>& packet) {
    if (!curl_ || packet.empty() || packet.size() > maximum_message) return false;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    std::size_t offset = 0;
    while (!closed_.load() && std::chrono::steady_clock::now() < deadline) {
      std::size_t sent = 0;
      CURLcode result;
      {
        std::lock_guard<std::mutex> lock(io_);
        result = curl_ws_send(curl_, packet.data() + offset, packet.size() - offset, &sent, 0, CURLWS_BINARY);
      }
      if (sent > packet.size() - offset) return false;
      offset += sent;
      if (result != CURLE_OK && result != CURLE_AGAIN) return false;
      if (offset == packet.size()) return true;
      // CURL 句柄不能被并发调用，等待必须在互斥锁外进行。
      std::this_thread::sleep_for(std::chrono::milliseconds(5));
    }
    return false;
  }

  bool receive(std::vector<std::uint8_t>& message) {
    message.clear();
    if (!curl_) return false;
    std::array<std::uint8_t, 8192> buffer{};
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(30);
    while (!closed_.load() && std::chrono::steady_clock::now() < deadline) {
      std::size_t received = 0;
      int flags = 0;
      curl_off_t remaining = 0;
      CURLcode result;
      {
        std::lock_guard<std::mutex> lock(io_);
        const curl_ws_frame* metadata = nullptr;
        result = curl_ws_recv(curl_, buffer.data(), buffer.size(), &received, &metadata);
        if (result == CURLE_OK) {
          if (!metadata) return false;
          // 下一次收发会使元数据失效，必须在持锁期间复制。
          flags = metadata->flags;
          remaining = metadata->bytesleft;
        }
      }
      if (result == CURLE_AGAIN) {
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
        continue;
      }
      if (result != CURLE_OK || (flags & CURLWS_CLOSE)) return false;
      if (flags & (CURLWS_PING | CURLWS_PONG)) continue;
      if (!(flags & CURLWS_BINARY) || remaining < 0 || received > maximum_message - message.size() ||
          static_cast<std::uint64_t>(remaining) > maximum_message - message.size() - received)
        return false;
      message.insert(message.end(), buffer.begin(), buffer.begin() + static_cast<std::ptrdiff_t>(received));
      if (remaining == 0 && !(flags & CURLWS_CONT)) return true;
    }
    return false;
  }

  void close() {
    if (closed_.exchange(true) || !curl_) return;
    std::lock_guard<std::mutex> lock(io_);
    // 尽力发送正常关闭；即使对端不响应，接收轮询也会在取消后退出。
    const std::uint8_t status[] = {0x03, 0xe8};
    std::size_t sent = 0;
    (void)curl_ws_send(curl_, status, sizeof(status), &sent, 0, CURLWS_CLOSE);
  }

private:
  static constexpr std::size_t maximum_message = 1024 * 1024;
#ifdef MSIME_WEBSOCKET_TESTING
  char error_[CURL_ERROR_SIZE]{};
#endif
  CURL* curl_ = nullptr;
  curl_slist* headers_ = nullptr;
  std::mutex io_;
  std::atomic<bool> closed_{false};
};
} // namespace msime::windows::compat
