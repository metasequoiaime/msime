#pragma once
#include <atomic>
#include <chrono>
#include <cstring>
#include <mutex>
#include <nlohmann/json.hpp>
#include <poll.h>
#include <stdexcept>
#include <string>
#include <sys/socket.h>
#include <sys/un.h>
#include <thread>
#include <unistd.h>
#include <vector>

// Local synthetic translations; never contacts a network provider.
class TranslationProviderFixture {
public:
  // online_requests counts provider requests that may reach the network. The AI cache-only probe the hosts send on every input change (ai_cache_only, never leaves the provider) is counted apart in ai_cache_probes, so the debounce assertions keep meaning one network request per settled input.
  std::atomic<unsigned> requests{0}, english_greeting_requests{0}, online_requests{0},
      ai_cache_probes{0};
  std::atomic<bool> hold_responses{false}, return_online_candidate{false}, tag_responses{false},
      multi_sense{false}, empty_translations{false};
  // The ai_context of every online request, in arrival order.
  std::vector<std::string> online_ai_contexts() {
    std::lock_guard<std::mutex> lock(mutex_);
    return online_ai_contexts_;
  }
  // 每次翻译请求问到的候选文字，按到达顺序。
  std::vector<std::vector<std::string>> translation_requests() {
    std::lock_guard<std::mutex> lock(mutex_);
    return translation_requests_;
  }
  explicit TranslationProviderFixture(const std::string &path) : path_(path) {
    sockaddr_un address{};
    address.sun_family = AF_UNIX;
    if (path.size() >= sizeof(address.sun_path))
      throw std::runtime_error("Synthetic translation socket path too long");
    std::memcpy(address.sun_path, path.c_str(), path.size() + 1);
    listener_ = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listener_ < 0 || bind(listener_, reinterpret_cast<sockaddr *>(&address), sizeof(address)) ||
        listen(listener_, 4)) {
      if (listener_ >= 0) close(listener_);
      throw std::runtime_error("Cannot start synthetic translation provider");
    }
    worker_ = std::thread([this] { run(); });
  }
  ~TranslationProviderFixture() {
    stopped_ = true;
    worker_.join();
    close(listener_);
    unlink(path_.c_str());
  }
private:
  std::string path_;
  int listener_ = -1;
  std::atomic<bool> stopped_{false};
  std::thread worker_;
  std::mutex mutex_;
  std::vector<std::string> online_ai_contexts_;
  std::vector<std::vector<std::string>> translation_requests_;
  void run() {
    while (!stopped_) {
      pollfd ready{listener_, POLLIN, 0};
      if (poll(&ready, 1, 10) <= 0) continue;
      const int client = accept(listener_, nullptr, nullptr);
      if (client < 0) continue;
      timeval timeout{1, 0};
      setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
      std::string request;
      char byte;
      while (request.size() < 16384 && recv(client, &byte, 1, 0) == 1 && byte != '\n')
        request += byte;
      const auto value = nlohmann::json::parse(request, nullptr, false);
      if (value.is_object() && value.value("kind", "") == "translation" &&
          value.contains("query") && value["query"].contains("candidates")) {
        auto translations = nlohmann::json::array();
        const auto gloss = multi_sense
            ? std::string("first sense; second sense")
            : tag_responses
                  ? "synthetic gloss [" + std::to_string(requests.load() + 1) + "]"
                  : std::string("synthetic gloss");
        if (!empty_translations)
          for (const auto &text : value["query"]["candidates"])
            translations.push_back({{"text", text}, {"translation", gloss}});
        const auto response = nlohmann::json{{"translations", translations}}.dump() + "\n";
        if (value["query"].value("target_language", "") == "en")
          for (const auto &text : value["query"]["candidates"])
            if (text == "你好") ++english_greeting_requests;
        {
          std::lock_guard<std::mutex> lock(mutex_);
          auto &texts = translation_requests_.emplace_back();
          for (const auto &text : value["query"]["candidates"])
            texts.push_back(text.is_string() ? text.get<std::string>() : std::string{});
        }
        ++requests;
        while (hold_responses && !stopped_)
          std::this_thread::sleep_for(std::chrono::milliseconds(1));
        send(client, response.data(), response.size(), MSG_NOSIGNAL);
      } else if (value.is_object() && value.value("kind", "") == "online" &&
                 value.value("query", nlohmann::json::object()).is_object() &&
                 value["query"].value("ai_cache_only", false)) {
        ++ai_cache_probes;
        const std::string response = "{\"candidates\":[]}\n";
        send(client, response.data(), response.size(), MSG_NOSIGNAL);
      } else if (value.is_object() && value.value("kind", "") == "online") {
        {
          std::lock_guard<std::mutex> lock(mutex_);
          const auto &query = value.value("query", nlohmann::json::object());
          online_ai_contexts_.push_back(
              query.is_object() ? query.value("ai_context", std::string{}) : std::string{});
        }
        ++online_requests;
        const std::string response = return_online_candidate
            ? "{\"candidates\":[{\"text\":\"云端测试\",\"source\":0}]}\n"
            : "{\"candidates\":[]}\n";
        send(client, response.data(), response.size(), MSG_NOSIGNAL);
      }
      close(client);
    }
  }
};
