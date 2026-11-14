#include "../../src/clipboard/CloudClipboardRoute.h"
#include "../../src/clipboard/CloudClipboardLoginBinding.h"
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
  const Json local_login{{"user_id", "synthetic-local"}, {"session_id", "synthetic-local-login"}};
  const Json provider_login{{"user_id", "synthetic-provider"}, {"session_id", "synthetic-provider-login"}};
  bool read_local = false;
  auto local = [&] { read_local = true; return local_login; };
  auto provider = [&](const std::string &socket) {
    assert(socket == "/synthetic/independent.sock");
    return provider_login;
  };
  assert(msime::linux_host::cloud_clipboard_login_binding(
      "/synthetic/independent.sock", provider, local) == provider_login);
  assert(!read_local);
  assert(msime::linux_host::cloud_clipboard_login_binding({}, provider, local) == local_login);
  assert(read_local);
  for (const auto &invalid : {Json(), Json::object(), Json{{"ok", false}},
       Json{{"user_id", "synthetic-provider"}, {"session_id", ""}},
       Json{{"user_id", "synthetic-provider"}, {"session_id", 1}},
       Json{{"user_id", "synthetic-provider"}, {"session_id", "synthetic"}, {"token", "synthetic"}}}) {
    read_local = false;
    bool rejected = false;
    try {
      msime::linux_host::cloud_clipboard_login_binding(
          "/synthetic/independent.sock", [&](const std::string &) { return invalid; }, local);
    } catch (const std::exception &) { rejected = true; }
    assert(rejected);
    assert(!read_local);
  }

  Fixture add;
  const auto added = msime::linux_host::cloud_clipboard_request(
      "/synthetic/state", {}, "/synthetic/auto.sock",
      Json{{"operation", "add"}, {"text", "合成上传"}},
      [&](const std::string &path) {
        assert(path == "/synthetic/state");
        add.calls.push_back("source");
        return Json{{"source", "relay"}};
      },
      [&](const Json &request) {
        add.calls.push_back("relay");
        add.relay_request = request;
        return Json{{"accepted", true}};
      },
      [&](const std::string &, const Json &) {
        assert(false && "relay route must not use provider");
        return Json::object();
      });
  assert(added.at("accepted") == true);
  assert((add.calls == std::vector<std::string>{"source", "relay"}));
  assert(add.relay_request.at("action") == Json({{"operation", "add"}, {"text", "合成上传"}}));

  Fixture explicit_provider;
  explicit_provider.source_fails = true;
  assert(!explicit_provider.list("/synthetic/explicit.sock").at("enabled").get<bool>());
  assert((explicit_provider.calls == std::vector<std::string>{"/synthetic/explicit.sock"}));

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
  assert((discovered.calls == std::vector<std::string>{"source"}));

  Fixture legacy;
  legacy.source_fails = true;
  assert(legacy.list({}, "/synthetic/auto.sock", {}).at("enabled") == false);
  assert((legacy.calls == std::vector<std::string>{"/synthetic/auto.sock"}));

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
