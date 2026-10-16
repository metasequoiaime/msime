#include "ipc/ReplyComposer.h"
#include "../../tsf/HostCandidateContinuation.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <filesystem>

using namespace msime::windows;
namespace {
void require(bool value, const char *message) {
  if (!value) { std::fprintf(stderr, "FAIL: %s\n", message); std::exit(EXIT_FAILURE); }
}
std::string assemble(const UiSelectionFrames &frames) {
  FanyImeCandidateContinuationPipe::Assembler assembler;
  std::optional<FanyImeCandidateContinuationPipe::Completed> completed;
  const auto accept = [&](const std::vector<uint8_t> &bytes) {
    require(bytes.size() == 404 && bytes[0] == FanyImeWorkerReplyType::CandidateContinuation,
            "fixed worker ABI and opcode");
    std::array<uint16_t, FanyImePipeLimits::CandidateTextCapacity> data{};
    for (size_t n = 0; n < data.size(); ++n)
      data[n] = static_cast<uint16_t>(bytes[4 + n * 2] | (bytes[5 + n * 2] << 8));
    completed = assembler.accept(data.data(), data.size(), 12, 34);
  };
  for (const auto &frame : frames.worker_prefix) {
    accept(frame);
    require(!completed, "prefix cannot apply a partial operation");
  }
  accept(frames.worker);
  require(completed && completed->focus == 12 && completed->epoch == 34, "complete payload keeps owner");
  return completed->text;
}
// 只模拟宿主边界，验证两进程不同的候选顺序不能让选择落在另一项上。
struct Host {
  nlohmann::json current, selected;
  uint64_t selectedGeneration = 0;
  size_t selectedIndex = 999;
  bool selectOk = true;
  bool valid() const { return true; }
  bool view(std::string *out, std::string *) { *out = current.dump(); return true; }
  bool select(uint64_t generation, size_t index, std::string *out, std::string *) {
    selectedGeneration = generation; selectedIndex = index; *out = selected.dump(); return selectOk;
  }
};
void host_guards(const std::string &payload, const nlohmann::json &before, const nlohmann::json &after) {
  Host baseline{{{"ok", true}, {"value", before}},
                {{"ok", true}, {"value", {{"handled", true}, {"commit", nullptr}, {"view", after}}}}};
  auto &view = baseline.current["value"];
  auto &candidates = view["candidates"];
  std::reverse(candidates.begin(), candidates.end());
  view["session"] = 987;
  view["generation"] = 876;
  size_t targetIndex = 0;
  for (auto &candidate : candidates) {
    candidate["id"]["session"] = 987;
    candidate["id"]["generation"] = 876;
    candidate["id"]["index"] = targetIndex++;
  }
  auto host = baseline;
  require(msime::tsf::ContinueHostCandidate(host, payload) && host.selectedGeneration == 876 &&
          host.selectedIndex == candidates.size() - 1, "use local candidate identity after reordering");
  host = baseline; host.current["value"]["editing_text"] = "changed";
  require(!msime::tsf::ContinueHostCandidate(host, payload) && host.selectedIndex == 999, "stale before rejected without selection");
  host = baseline; host.current["value"]["candidates"] = nlohmann::json::array();
  require(!msime::tsf::ContinueHostCandidate(host, payload), "missing candidate rejected");
  host = baseline; host.current["value"]["candidates"].push_back(candidates.back());
  require(!msime::tsf::ContinueHostCandidate(host, payload), "ambiguous candidate rejected");
  host = baseline; host.selectOk = false;
  require(!msime::tsf::ContinueHostCandidate(host, payload), "failed host selection rejected");
  host = baseline; host.selected["value"]["commit"] = "unexpected";
  require(!msime::tsf::ContinueHostCandidate(host, payload), "unexpected commit rejected");
  host = baseline; host.selected["value"]["view"]["local_mode"] = "none";
  require(!msime::tsf::ContinueHostCandidate(host, payload), "unexpected after rejected");
  host = baseline; host.selected["value"]["handled"] = false;
  require(!msime::tsf::ContinueHostCandidate(host, payload), "unhandled selection rejected");
  require(!msime::tsf::ContinueHostCandidate(host, "{") &&
          !msime::tsf::ContinueHostCandidate(host, "{\"version\":2}"), "invalid payload rejected");
}
} // namespace
int main() {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-candidate-continuation-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root);
  struct Cleanup { std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  nlohmann::json options{{"api_version", 1}, {"preferences", {{"scheme", "quanpin"},
      {"candidate_page_size", 9}, {"learning", false}, {"chinese_punctuation", true}}}};
  for (const char *name : {"resources", "user_data", "cache", "dictionaries"}) {
    const auto directory = root / name;
    std::filesystem::create_directories(directory);
    options[name] = directory.u8string();
  }
  ServerSession server(42, options.dump()); server.activate(1);
  msime::tsf::EngineSessionAdapter tip;
  std::string raw, error;
  require(tip.create(options.dump(), &error) && tip.focus(true, &raw, &error), "independent TIP session created");
  ReplyComposer composer(42, 1);
  uint64_t request = 0;
  const auto same_state = [&] {
    require(tip.view(&raw, &error), "TIP view available");
    const auto serverState = candidate_continuation_state(server.view());
    const auto tipState = candidate_continuation_state(nlohmann::json::parse(raw).at("value"));
    require(serverState == tipState, "two engines agree");
  };
  const auto type = [&](const std::string &text) {
    for (unsigned char key : text) {
      FanyImeNamedpipeData packet{}; packet.client_id = 42; packet.request_id = ++request;
      packet.keycode = key >= 'a' && key <= 'z' ? key - 'a' + 'A' : key;
      if (key == '.') packet.keycode = 0xBE;
      if (key == '/') packet.keycode = 0xBF;
      packet.wch = key;
      server.key(packet, 1);
      require(tip.character(key, false, &raw, &error), "TIP accepts continuation key");
      same_state();
    }
  };
  const auto select = [&](const std::string &target, bool guards) {
    const auto before = server.view();
    for (const auto &candidate : before.at("candidates")) {
      if (candidate.at("text") != target) continue;
      const auto &id = candidate.at("id");
      require(!composer.select_candidate(server, id.at("session").get<uint64_t>() + 1,
                  id.at("generation"), id.at("index"), 0, true), "foreign session rejected");
      require(!composer.select_candidate(server, id.at("session"),
                  id.at("generation").get<uint64_t>() + 1, id.at("index"), 0, true), "stale generation rejected");
      const auto selected = composer.select_candidate(server, id.at("session"), id.at("generation"), id.at("index"), 0, true);
      require(selected && selected->ui_selection && !selected->ui_selection->before_trigger,
              "successful continuation is not an id-zero rejection");
      require(!selected->ui_selection->worker_prefix.empty(), "state payload spans frames");
      const auto payload = assemble(*selected->ui_selection);
      if (guards) host_guards(payload, before, server.view());
      require(msime::tsf::ContinueHostCandidate(tip, payload), "TIP applies own candidate selection");
      require(!msime::tsf::ContinueHostCandidate(tip, payload), "duplicate request cannot select again");
      same_state();
      require(!composer.select_candidate(server, id.at("session"), id.at("generation"), id.at("index"), 0, true),
              "pending delivery blocks another selection");
      composer.confirm_ui_delivery(42, 1, server.view().at("generation"));
      return;
    }
    require(false, "completion candidate offered");
  };
  type("ht"); select("https://", true);
  require(server.view().at("local_mode") == "url", "URL mode survives selection");
  type("sample7"); select("https://sample7.com", false);
  type("/path9.x");
  require(server.view().at("editing_text") == "https://sample7.com/path9.x", "digits punctuation and path preserved");
  server.cancel_composition(1); require(tip.command(MSIME_CANCEL, &raw, &error), "reset TIP"); composer.cancel();
  type("ht"); select("https://", false);
  const std::string longHost(220, 'a');
  type(longHost); select("https://" + longHost + ".com", false);
  type("/end2");
  // 未协商能力的旧组合保留旧回复，不能把新 opcode 发给旧 DLL。
  server.cancel_composition(1); require(tip.command(MSIME_CANCEL, &raw, &error), "reset TIP again"); composer.cancel();
  type("ht");
  const auto view = server.view(); const auto id = view.at("candidates").at(0).at("id");
  const auto legacy = composer.select_candidate(server, id.at("session"), id.at("generation"), id.at("index"), 0);
  require(legacy && legacy->ui_selection && legacy->ui_selection->before_trigger &&
          legacy->ui_selection->worker_prefix.empty(), "legacy peer keeps existing wire path");
  std::puts("candidate continuation: passed");
}
