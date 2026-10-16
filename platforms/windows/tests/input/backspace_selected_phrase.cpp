#include "ipc/ReplyComposer.h"
#include "../../tsf/EngineSessionAdapter.h"
#include "../../tsf/HostCandidateContinuation.h"
#include "../../tsf/HostRawCommit.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <filesystem>

using namespace msime::windows;
using nlohmann::json;
namespace {
const char *phase = "initialization";
std::filesystem::path fixture;
void require(bool value, const char *message) {
  if (!value) { std::fprintf(stderr, "FAIL: %s\n", message); std::exit(EXIT_FAILURE); }
}
enum class Action { Backspace, Keyboard, Segment, SegmentRestore, SegmentMark, Cancel, Raw, Finish, Multiple, OldDefault };
void scenario(bool initial, bool change, Action action = Action::Backspace) {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-backspace-phrase-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root);
  struct Cleanup { std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  char *defaults = msime_client_default_preferences();
  require(defaults != nullptr, "shared preference defaults available");
  auto preferences = json::parse(defaults).at("value");
  msime_client_string_free(defaults);
  preferences["scheme"] = "quanpin";
  preferences["default_ime_mode"] = "chinese";
  preferences["learning"] = false;
  preferences["candidate_page_size"] = 9;
  preferences["backspace_reverts_selection"] = initial;
  if (action == Action::OldDefault) preferences.erase("backspace_reverts_selection");
  json options{{"api_version", 1}, {"preferences", preferences}};
  for (const char *name : {"resources", "user_data", "cache", "dictionaries"}) {
    std::filesystem::create_directories(root / name); options[name] = (root / name).u8string();
  }
  for (const char *name : {"resources", "dictionaries"})
    std::filesystem::copy_file(fixture, root / name / "msime-pinyin.db");
  phase = "Server creation";
  ServerSession server(42, options.dump()); server.activate(1);
  msime::tsf::EngineSessionAdapter tip;
  std::string raw, error;
  require(tip.create(options.dump(), &error) && tip.focus(true, &raw, &error), "TIP session created");
  ReplyComposer composer(42, 1);
  uint64_t request = 0;
  const auto same = [&] {
    require(tip.view(&raw, &error), "TIP view available");
    const auto view = json::parse(raw).at("value");
    require(view.at("editing_text") == server.view().at("editing_text") &&
            view.value("phrase_prefix", "") == server.view().value("phrase_prefix", ""),
            "TIP and Server retain the same spelling and selected prefix");
  };
  for (unsigned char letter : std::string(action == Action::Multiple ? "woshihao" : "woshi")) {
    FanyImeNamedpipeData packet{}; packet.client_id = 42; packet.request_id = ++request;
    packet.keycode = letter - 'a' + 'A'; packet.wch = letter;
    require(composer.basic_key(server, packet, 1, TsfPreeditStyle::Local).has_value(), "Server types");
    composer.confirm_delivery(42, 1, request);
    require(tip.character(letter, false, &raw, &error), "TIP types");
    same();
  }
  const auto select = [&](const std::string &target, bool keyboard) {
    const auto before = server.view();
    size_t index = 0, slot = 0;
    bool found = false;
    for (const auto &candidate : before.at("candidates")) {
      if (candidate.at("text") == target) { index = candidate.at("id").at("index"); found = true; break; }
      ++slot;
    }
    require(found, "synthetic partial candidate exists");
    if (keyboard) {
      // 两个进程分别选择同一槽位，候选身份与代次各自由本地会话提供。
      require(tip.view(&raw, &error), "TIP selection view");
      const auto view = json::parse(raw).at("value");
      const auto &candidate = view.at("candidates").at(slot);
      require(candidate.at("text") == target &&
              tip.select(view.at("generation"), candidate.at("id").at("index"), &raw, &error), "TIP keyboard selection");
      FanyImeNamedpipeData packet{}; packet.client_id = 42; packet.request_id = ++request;
      packet.keycode = static_cast<uint32_t>('1' + slot); packet.wch = static_cast<wchar_t>('1' + slot);
      const auto selected = composer.basic_key(server, packet, 1, TsfPreeditStyle::Local);
      require(selected && !selected->committed_text, "Server keyboard selection keeps prefix");
      composer.confirm_delivery(42, 1, request);
    } else {
      const auto selected = composer.select_candidate(server, before.at("session"), before.at("generation"), index, 0, true);
      require(selected && !selected->committed_text && selected->ui_selection, "Server holds partial mouse selection");
      // 按实际 worker 帧重新组装，让 TIP 通过继续协议选择自己的候选。
      FanyImeCandidateContinuationPipe::Assembler assembler;
      std::optional<FanyImeCandidateContinuationPipe::Completed> completed;
      const auto accept = [&](const std::vector<uint8_t> &bytes) {
        require(bytes.size() == 404 && bytes[0] == FanyImeWorkerReplyType::CandidateContinuation, "continuation wire frame");
        std::array<uint16_t, FanyImePipeLimits::CandidateTextCapacity> data{};
        for (size_t n = 0; n < data.size(); ++n) data[n] = static_cast<uint16_t>(bytes[4 + n * 2] | (bytes[5 + n * 2] << 8));
        completed = assembler.accept(data.data(), data.size(), 12, 34);
      };
      for (const auto &frame : selected->ui_selection->worker_prefix) accept(frame);
      accept(selected->ui_selection->worker);
      require(completed && msime::tsf::ContinueHostCandidate(tip, completed->text), "TIP continues its own partial mouse selection");
      require(!msime::tsf::ContinueHostCandidate(tip, completed->text), "duplicate continuation cannot select twice");
      composer.confirm_ui_delivery(42, 1, server.view().at("generation"));
    }
    require(server.view().value("phrase_prefix", "") != "", "partial selection stays in Runtime");
    same();
  };
  phase = "partial selection";
  select("我", action == Action::Keyboard);
  if (action == Action::Multiple) select("是", false);
  const auto command = [&](uint32_t code, uint32_t modifiers, uint32_t hostCommand) {
    FanyImeNamedpipeData packet{}; packet.client_id = 42; packet.request_id = ++request;
    packet.keycode = code; packet.modifiers_down = modifiers;
    auto reply = composer.basic_key(server, packet, 1, TsfPreeditStyle::Local);
    require(reply && (!reply->encoded || static_cast<bool>(*reply->encoded)), "Server command produces valid reply");
    require(tip.command(hostCommand, &raw, &error), "TIP command succeeds");
    const auto result = json::parse(raw).at("value");
    require(result.at("commit").is_null() ? !reply->committed_text
            : reply->committed_text == std::optional<std::string>(result.at("commit").get<std::string>()),
            "both processes observe exactly the same commit");
    composer.confirm_delivery(42, 1, request);
    same();
    return result;
  };
  phase = "post-selection command";
  if (action == Action::Segment || action == Action::SegmentRestore || action == Action::SegmentMark) {
    auto result = command(0x08, 2, MSIME_BACKSPACE_SEGMENT);
    require(result.at("commit").is_null() && result.at("view").at("editing_text") == "" &&
            result.at("view").value("phrase_prefix", "") == "我", "segment deletion keeps prefix-only composition");
    if (action == Action::SegmentMark) {
      FanyImeNamedpipeData packet{}; packet.client_id = 42; packet.request_id = ++request;
      packet.keycode = 0xBF; packet.wch = '/';
      const auto reply = composer.configured_key(server, packet, 1, TsfPreeditStyle::Local, preference_navigation(preferences));
      require(reply && (!reply->encoded || static_cast<bool>(*reply->encoded)), "mark after prefix-only state has a valid Server projection");
      require(tip.character('/', false, &raw, &error), "TIP mark after prefix-only composition");
      const auto edited = json::parse(raw).at("value");
      require(!edited.at("commit").is_null() && edited.at("commit").get<std::string>().rfind("我", 0) == 0 &&
              reply->committed_text == std::optional<std::string>(edited.at("commit").get<std::string>()),
              "following mark commits held prefix exactly once in both processes");
      composer.confirm_delivery(42, 1, request); same(); return;
    }
    if (action == Action::SegmentRestore) {
      result = command(0x08, 0, MSIME_BACKSPACE);
      require(initial ? (result.at("commit").is_null() && result.at("view").at("editing_text") == "wo")
                      : result.at("commit") == "我", "ordinary Backspace on a held-only phrase follows the preference");
      return;
    }
    result = command(0x08, 2, MSIME_BACKSPACE_SEGMENT);
    require(result.at("commit").is_null() && result.at("view").at("editing_text") == "" &&
            result.at("view").value("phrase_prefix", "").empty(), "second segment deletion drops the selected segment without committing");
    return;
  }
  if (action == Action::Cancel) {
    server.cancel_composition(1); composer.cancel();
    require(tip.command(MSIME_CANCEL, &raw, &error), "TIP cancellation");
    same();
    require(server.view().at("editing_text") == "" && server.view().value("phrase_prefix", "").empty(), "cancel discards spelling and selected prefix");
    return;
  }
  if (action == Action::Raw) {
    std::string inserted;
    int cleanups = 0;
    require(msime::tsf::CommitHostRaw(tip, [&](const std::string &text) { inserted += text; return true; },
                [&] { ++cleanups; }, &error) == msime::tsf::RawCommitStatus::Completed && inserted == "我shi" && cleanups == 1,
            "raw commit writes held prefix and spelling once");
    FanyImeNamedpipeData packet{}; packet.client_id = 42; packet.request_id = ++request; packet.keycode = 0x0D;
    const auto &reply = composer.dispatch(server, packet, 1, ReplyPath::LocalCommit, false, inserted);
    require(!reply.encoded && reply.committed_text == std::optional<std::string>(inserted), "Server observes local raw commit without another insertion");
    composer.confirm_delivery(42, 1, request); same(); return;
  }
  if (action == Action::Finish) {
    auto view = server.view();
    size_t index = 0;
    bool found = false;
    for (const auto &candidate : view.at("candidates")) if (candidate.at("text") == "是") { index = candidate.at("id").at("index"); found = true; break; }
    require(found, "final candidate exists");
    require(tip.view(&raw, &error), "TIP final selection view");
    const auto tipView = json::parse(raw).at("value");
    bool selectedTip = false;
    for (const auto &candidate : tipView.at("candidates")) if (candidate.at("text") == "是") {
      require(tip.select(tipView.at("generation"), candidate.at("id").at("index"), &raw, &error), "TIP final candidate selection");
      require(json::parse(raw).at("value").at("commit") == "我是", "TIP final commit includes prefix exactly once");
      selectedTip = true; break;
    }
    require(selectedTip, "TIP final candidate exists");
    const auto selected = composer.select_candidate(server, view.at("session"), view.at("generation"), index, 0, true);
    require(selected && selected->committed_text == std::optional<std::string>("我是"), "final mouse selection includes prefix exactly once");
    composer.confirm_ui_delivery(42, 1, server.view().at("generation")); same(); return;
  }
  phase = "live preferences";
  if (change) {
    preferences["backspace_reverts_selection"] = !initial;
    const json snapshot{{"format_version", 1}, {"revision", 1}, {"preferences", preferences}};
    server.update_preferences(1, snapshot.dump());
    require(tip.update_preferences(snapshot.dump(), &raw, &error), "live TIP preference update");
  }
  phase = "backspace";
  const bool multiple = action == Action::Multiple;
  const auto expectedSteps = multiple ? std::vector<std::string>{"ha", "h", initial != change ? "shi" : ""}
                                      : std::vector<std::string>{"sh", "s", initial != change ? "wo" : ""};
  for (const auto &expected : expectedSteps) {
    FanyImeNamedpipeData packet{}; packet.client_id = 42; packet.request_id = ++request; packet.keycode = 0x08;
    const auto reply = composer.basic_key(server, packet, 1, TsfPreeditStyle::Local);
    require(reply.has_value() && (!reply->encoded || static_cast<bool>(*reply->encoded)), "Server backspace produces valid reply");
    require(tip.command(MSIME_BACKSPACE, &raw, &error), "TIP backspace");
    const auto edited = json::parse(raw).at("value");
    require(edited.at("view").at("editing_text") == expected, "backspace edits remaining spelling or restores once");
    if (std::string(expected).empty()) {
      require(edited.at("commit") == (multiple ? "我是" : "我") &&
              reply->committed_text == std::optional<std::string>(multiple ? "我是" : "我"),
              "final deletion commits selected prefix exactly once in both processes");
    } else require(edited.at("commit").is_null() && !reply->committed_text, "editing or restoring does not commit");
    composer.confirm_delivery(42, 1, request);
    same();
  }
}
} // namespace
int main(int argc, char **argv) {
  require(argc == 2, "synthetic dictionary fixture is required");
  fixture = std::filesystem::path(argv[1]);
  try {
    for (bool initial : {false, true}) {
      for (bool change : {false, true}) scenario(initial, change);
      for (auto action : {Action::Keyboard, Action::Segment, Action::SegmentRestore, Action::SegmentMark, Action::Cancel, Action::Raw, Action::Finish, Action::Multiple})
        scenario(initial, false, action);
    }
    scenario(true, false, Action::OldDefault);
  } catch (const std::exception &error) {
    std::fprintf(stderr, "FAIL at %s: %s\n", phase, error.what());
    return EXIT_FAILURE;
  }
  std::puts("backspace selected phrase: passed");
}
