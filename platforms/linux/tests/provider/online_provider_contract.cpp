#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <cerrno>
#include <chrono>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <memory>
#include <stdexcept>
#include <string>
#include <thread>

using Json = nlohmann::json;

namespace {
void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}
Json response(char *raw) {
  std::unique_ptr<char, decltype(&msime_client_string_free)> owned(
      raw, msime_client_string_free);
  require(raw != nullptr, "provider response missing");
  return Json::parse(raw);
}
} // namespace

int main() {
  // Use a unique private subdirectory so CTest and direct invocations work
  // even when the source tree is mounted read-only.
  const auto directory = std::filesystem::temp_directory_path() /
                         ("msime-provider-contract-" + std::to_string(getpid()) + "-" +
                          std::to_string(std::chrono::steady_clock::now()
                                             .time_since_epoch()
                                             .count()));
  std::error_code directory_error;
  std::filesystem::create_directory(directory, directory_error);
  require(!directory_error, "private provider directory creation failed");
  std::filesystem::permissions(
      directory, std::filesystem::perms::owner_all,
      std::filesystem::perm_options::replace, directory_error);
  require(!directory_error, "private provider directory permissions failed");
  const auto socket_file = directory / "provider.sock";
  const auto socket_path = socket_file.string();
  const int server = ::socket(AF_UNIX, SOCK_STREAM, 0);
  require(server >= 0, "socket failed");
  sockaddr_un address{};
  address.sun_family = AF_UNIX;
  require(socket_path.size() < sizeof(address.sun_path), "socket path too long");
  std::strncpy(address.sun_path, socket_path.c_str(), sizeof(address.sun_path) - 1);
  unlink(socket_path.c_str());
  require(bind(server, reinterpret_cast<sockaddr *>(&address), sizeof(address)) == 0,
          "bind failed");
  require(listen(server, 1) == 0, "listen failed");
  std::thread provider([server] {
    const int client = accept(server, nullptr, nullptr);
    require(client >= 0, "accept failed");
    // Read the whole request line, not one read's worth of it. A provider that
    // leaves part of the request unread and then closes hands the client
    // ECONNRESET instead of the reply it already queued - unix_release_sock sets
    // that on the peer whenever the closing socket still has data waiting - so
    // this fixture would fail about one run in five, depending on whether the
    // client's terminating newline arrived as its own segment. A real provider
    // reads a line; so does this one now.
    std::string received;
    while (received.find('\n') == std::string::npos) {
      char buffer[16384]{};
      const auto count = read(client, buffer, sizeof(buffer) - 1);
      require(count > 0, "query was not sent");
      received.append(buffer, static_cast<size_t>(count));
      require(received.size() <= 16384, "query exceeded the request bound");
    }
    require(received.find("version") != std::string::npos, "query was not sent");
    const std::string reply = "{\"candidates\":[{\"text\":\"candidate\",\"source\":0}]}\n";
    require(write(client, reply.data(), reply.size()) ==
                static_cast<ssize_t>(reply.size()),
            "reply failed");
    close(client);
    close(server);
  });
  const Json query = { {"scheme", 0}, {"generation", 1}, {"identity", "id"},
                       {"query_text", "nihao"}, {"cache_key", "key"},
                       {"pinyin_segments", {"ni", "hao"}},
                       {"cloud_eligible", true}, {"ai_eligible", false},
                       {"session_id", 1} };
  const auto encoded = query.dump();
  auto result = response(msime_client_online_provider_request(
      reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(),
      reinterpret_cast<const uint8_t *>(socket_path.data()), socket_path.size()));
  require(result.at("ok").get<bool>() && result.at("value").at("candidates").at(0).at("text") == "candidate",
          "provider candidate mismatch");
  provider.join();
  unlink(socket_path.c_str());
  std::filesystem::remove(directory, directory_error);
  require(!directory_error, "private provider directory cleanup failed");
  auto invalid = response(msime_client_online_provider_request(
      reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(),
      reinterpret_cast<const uint8_t *>("relative.sock"), 13));
  require(!invalid.at("ok").get<bool>(), "relative socket path was accepted");
  return 0;
}
