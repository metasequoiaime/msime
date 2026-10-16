#pragma once
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <nlohmann/json.hpp>
#include <stdexcept>
#include <string>
#include <cstring>
#include <deque>
#include <poll.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <thread>
#include <unistd.h>

// Synthetic local provider: deliberately returns a final result after cancel.
class VoiceProviderFixture {
public:
  std::atomic<unsigned> started{0}, cancelled{0}, finished{0}, stop_requests{0};
  std::atomic<bool> release_final{false}, release_partial{false};
  explicit VoiceProviderFixture(const std::string &path) : path_(path) {
    sockaddr_un address{};
    address.sun_family = AF_UNIX;
    if (path.size() >= sizeof(address.sun_path))
      throw std::runtime_error("Synthetic voice socket path too long");
    std::memcpy(address.sun_path, path.c_str(), path.size() + 1);
    listener_ = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listener_ < 0 || bind(listener_, reinterpret_cast<sockaddr *>(&address), sizeof(address)) ||
        listen(listener_, 4)) {
      if (listener_ >= 0) close(listener_);
      throw std::runtime_error("Cannot start synthetic voice provider");
    }
    worker_ = std::thread([this] { run(); });
  }
  ~VoiceProviderFixture() {
    stopped_ = true;
    worker_.join();
    close(listener_);
    unlink(path_.c_str());
  }
private:
  struct Pending {
    int socket;
    uint64_t generation;
  };
  std::string path_;
  int listener_ = -1;
  std::atomic<bool> stopped_{false};
  std::thread worker_;
  void run() {
    std::deque<Pending> pending;
    while (!stopped_) {
      if (!pending.empty() && release_partial.exchange(false)) {
        const auto response = nlohmann::json{{"ok", true}, {"type", "partial"}, {"text", "测试😀"},
                                              {"generation", pending.front().generation}}.dump() + "\n";
        send(pending.front().socket, response.data(), response.size(), MSG_NOSIGNAL);
      }

      if (!pending.empty() && release_final.exchange(false)) {
        const auto response = nlohmann::json{{"ok", true}, {"type", "final"}, {"text", "synthetic voice"},
                                              {"generation", pending.front().generation}}.dump() + "\n";
        send(pending.front().socket, response.data(), response.size(), MSG_NOSIGNAL);
        close(pending.front().socket);
        pending.pop_front();
        ++finished;
      }
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
      if (value.is_object() && value.value("kind", "") == "voice" && pending.size() < 4) {
        pending.push_back({client, value.at("query").value("generation", uint64_t{0})});
        ++started;
      } else {
        if (value.is_object() && value.value("kind", "") == "voice_cancel" &&
            std::any_of(pending.begin(), pending.end(), [&](const auto &request) {
              return request.generation == value.at("query").value("generation", uint64_t{0});
            }))
          ++cancelled;
        if (value.is_object() && value.value("kind", "") == "voice_stop" &&
            std::any_of(pending.begin(), pending.end(), [&](const auto &request) {
              return request.generation == value.at("query").value("generation", uint64_t{0});
            }))
          ++stop_requests;
        close(client);
      }
    }
    for (const auto &request : pending)
      close(request.socket);
  }
};
