#include "../../src/clipboard/CloudClipboardRoute.h"
#include <cassert>
#include <iostream>
#include <vector>

using Json = nlohmann::json;

struct Fixture {
  std::vector<std::string> calls;
  Json selected{{"source", "relay"}};
  bool source_fails = false;
  bool relay_fails = false;
  Json relay_request;
  Json list(std::string configured = {}, std::string discovered = "/synthetic/auto.sock",
            std::string directory = "/synthetic/state") {
    return msime::linux_host::cloud_clipboard_list(directory, configured, discovered,
        [&](const std::string &path) {
          assert(path == directory);
          calls.push_back("source");
          if (source_fails) throw std::runtime_error("synthetic source failure");
          return selected;
        },
        [&](const Json &request) {
          calls.push_back("relay");
          relay_request = request;
          if (relay_fails) throw std::runtime_error("synthetic relay failure");
          return Json{{"enabled", true}, {"items", Json::array({Json{{"text", "合成中继条目"}}})}};
        },
        [&](const std::string &socket, const Json &action) {
          assert(action == Json({{"operation", "list"}, {"search", ""}}));
          calls.push_back(socket);
          return Json{{"enabled", false}, {"items", Json::array()}};
        });
  }
};

int main() {
  Fixture explicit_provider;
  explicit_provider.source_fails = true;
  assert(!explicit_provider.list("/synthetic/explicit.sock").at("enabled").get<bool>());
  assert(explicit_provider.calls == std::vector<std::string>{"/synthetic/explicit.sock"});

  Fixture relay;
  assert(relay.list().at("items").at(0).at("text") == "合成中继条目");
  assert(relay.calls == std::vector<std::string>({"source", "relay"}));
  assert(relay.relay_request == Json({{"preferences_directory", "/synthetic/state"},
      {"directory", "/synthetic/state"}, {"action", {{"operation", "list"}, {"search", ""}}}}));

  Fixture no_socket;
  assert(no_socket.list({}, {}).at("enabled") == true);
  assert(no_socket.calls == std::vector<std::string>({"source", "relay"}));

  Fixture discovered;
  discovered.selected = Json{{"source", "account"}};
  assert(discovered.list().at("enabled") == false);
  assert(discovered.calls == std::vector<std::string>({"source", "/synthetic/auto.sock"}));
  discovered.calls.clear();
  assert(discovered.list({}, {}).empty());
  assert(discovered.calls == std::vector<std::string>{"source"});

  Fixture legacy;
  legacy.source_fails = true;
  assert(legacy.list({}, "/synthetic/auto.sock", {}).at("enabled") == false);
  assert(legacy.calls == std::vector<std::string>{"/synthetic/auto.sock"});

  for (int failure = 0; failure < 4; ++failure) {
    Fixture rejected;
    rejected.source_fails = failure == 0;
    rejected.relay_fails = failure == 1;
    if (failure == 2) rejected.selected = Json{{"source", "unknown"}};
    if (failure == 3) rejected.selected = Json::object();
    bool failed = false;
    try { rejected.list(); } catch (const std::exception &) { failed = true; }
    assert(failed);
    assert(rejected.calls == (failure == 1 ? std::vector<std::string>{"source", "relay"}
                                           : std::vector<std::string>{"source"}));
  }
  std::cout << "云剪贴板选路回归通过\n";
}
