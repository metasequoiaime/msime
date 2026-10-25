#include "ReplyComposer.h"
#include "PipeMetadata.h"
#include "../../tsf/EngineSessionAdapter.h"
#include "../core/TestHostOptions.h"
#include <chrono>
#include <filesystem>
#include <iostream>
#include <stdexcept>

using namespace msime::windows;
using Json = nlohmann::json;
namespace {
void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}
Json comparable(Json view) {
  view.erase("session");
  view.erase("generation");
  for (auto &candidate : view["candidates"]) candidate.erase("id");
  return {{"editing_text", view.at("editing_text")},
          {"preedit", view.at("preedit")},
          {"phrase_prefix", view.value("phrase_prefix", std::string{})},
          {"key_grid", view.at("key_grid")},
          {"candidates", view.at("candidates")}};
}
void test(const std::filesystem::path &dictionary) {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-eighteen-key-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  auto options = test_host_options(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  std::filesystem::copy_file(dictionary, root / "dictionaries" / "msime-pinyin.db");
  options["preferences"]["default_ime_mode"] = "chinese";
  ServerSession server(42, options.dump());
  server.activate(1);
  msime::tsf::EngineSessionAdapter tip;
  std::string raw, error;
  require(tip.create(options.dump(), &error) && tip.focus(true, &raw, &error), "TIP session setup failed");
  ReplyComposer composer(42, 1);
  const auto bindings = preference_navigation(Json::object());
  uint64_t request = 0;
  const auto same = [&] {
    require(tip.view(&raw, &error), "TIP view failed");
    require(comparable(server.view()) == comparable(Json::parse(raw).at("value")), "TIP/Server composition diverged");
  };
  const auto press = [&](uint32_t code, uint32_t character, bool grouped) {
    FanyImeNamedpipeData packet{};
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.client_id = 42;
    packet.request_id = ++request;
    packet.keycode = code;
    packet.wch = static_cast<FanyImeWireChar>(character);
    packet.modifiers_down = grouped ? PipeMetadata::GroupedPanelKey : 0;
    if (code == 0x0d) packet.modifiers_down |= PipeMetadata::CandidateActive;
    const auto reply = composer.configured_key(server, packet, 1, TsfPreeditStyle::Pinyin, bindings);
    require(reply.has_value() && (!reply->encoded || static_cast<bool>(*reply->encoded)), "Server route rejected key");
    if (code == 0x08) require(tip.command(MSIME_BACKSPACE, &raw, &error), "TIP Backspace failed");
    else if (code == 0x0d) require(tip.command(MSIME_COMMIT_CANDIDATE, &raw, &error), "TIP Enter failed");
    else require(tip.panel_character(static_cast<uint8_t>(character), false, grouped, &raw, &error), "TIP character failed");
    const auto transition = Json::parse(raw).at("value");
    require(transition.at("commit") == reply->source.transition.at("commit"), "TIP/Server commits diverged");
    composer.confirm_delivery(42, 1, packet.request_id);
    same();
    return transition;
  };
  const auto group = [&](const char *text) {
    for (; *text; ++text) press(static_cast<uint32_t>(*text - 'a' + 'A'), static_cast<uint32_t>(*text), true);
  };
  group("bihai");
  bool nihao = false, bohao = false;
  const auto ambiguous = server.view();
  for (const auto &candidate : ambiguous.at("candidates")) {
    nihao |= candidate.at("text") == "你好";
    bohao |= candidate.at("text") == "波好";
  }
  require(nihao && bohao, "Ambiguous syllables were reduced to one letter");
  press(0x08, 0, false);
  require(server.view().at("editing_text") == "biha", "Backspace did not remove one group");
  group("i");
  const auto entered = press(0x0d, 0, false);
  require(entered.at("commit") == "你好", "Enter committed ambiguous raw codes");
  const auto physical = press('N', 'n', false);
  require(physical.at("view").at("key_grid") == "none" &&
          physical.at("view").at("editing_text") == "n", "Hardware did not resume exact letters");
  // 已有确定全拼时切入多义按键必须拒绝且不丢掉原组字。
  const auto before = server.view();
  FanyImeNamedpipeData rejected{};
  rejected.event_type = FanyImePipeEventType::KeyEvent;
  rejected.client_id = 42;
  rejected.request_id = ++request;
  rejected.keycode = 'B'; rejected.wch = 'b'; rejected.modifiers_down = PipeMetadata::GroupedPanelKey;
  bool refused = false;
  try { (void)composer.configured_key(server, rejected, 1, TsfPreeditStyle::Pinyin, bindings); }
  catch (const std::invalid_argument &) { refused = true; }
  require(refused && server.view() == before, "Grouped key changed an exact composition");
  require(!tip.panel_character('b', false, true, &raw, &error), "TIP accepted grouped key into exact composition");
  same();
  server.command(1, MSIME_CANCEL);
  require(tip.command(MSIME_CANCEL, &raw, &error), "TIP cancel failed");
  group("bihai");
  const auto switched = press('N', 'n', false);
  require(switched.at("commit") == "你好" && switched.at("view").at("editing_text") == "n" &&
          switched.at("view").at("key_grid") == "none", "Hardware transition lost its exact letter");
  server.command(1, MSIME_CANCEL);
  require(tip.command(MSIME_CANCEL, &raw, &error), "TIP cancel failed");
  group("z");
  const auto missing = server.view();
  require(missing.at("candidates").empty(), "Empty-candidate fixture unexpectedly matched");
  FanyImeNamedpipeData empty_enter{};
  empty_enter.event_type = FanyImePipeEventType::KeyEvent;
  empty_enter.client_id = 42;
  empty_enter.request_id = ++request;
  empty_enter.keycode = 0x0d;
  empty_enter.modifiers_down = PipeMetadata::CandidateActive;
  const auto held = composer.configured_key(server, empty_enter, 1, TsfPreeditStyle::Pinyin, bindings);
  require(held && held->source.transition.at("commit").is_null() && server.view() == missing,
          "Empty-candidate Enter committed raw group codes");
  composer.confirm_delivery(42, 1, empty_enter.request_id);
  same();
  const auto resumed = press('N', 'n', false);
  require(resumed.at("commit").is_null() && resumed.at("view").at("editing_text") == "n" &&
          resumed.at("view").at("key_grid") == "none", "Empty-candidate hardware transition committed raw groups");
}
}
int main(int argc, char **argv) {
  try {
    require(argc == 2, "Synthetic dictionary argument required");
    test(argv[1]);
    std::cout << "Windows eighteen-key dual-session checks passed\n";
    return 0;
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
