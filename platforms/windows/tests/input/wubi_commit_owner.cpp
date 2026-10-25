#include "ipc/ReplyComposer.h"
#include "../../tsf/WubiCommitOwner.h"
#include "../core/TestHostOptions.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <initializer_list>

using namespace msime::windows;
using msime::tsf::WubiCommitOwner;
namespace {
std::string phase;
void require(bool value, const char *message) {
  if (!value) { std::fprintf(stderr, "FAIL: %s (%s)\n", message, phase.c_str()); std::exit(EXIT_FAILURE); }
}
void scenario(const std::filesystem::path &fixture, bool unique, bool early,
              TsfPreeditStyle style, bool uiless, bool traditional, bool capital) {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-wubi-owner-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directory(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  auto options = test_host_options(root);
  options["phrase_preedit"] = true;
  options["preferences"]["scheme"] = "wubi";
  options["preferences"]["default_ime_mode"] = "chinese";
  options["preferences"]["wubi_mixed_pinyin"] = false;
  options["preferences"]["wubi_auto_commit_unique"] = unique;
  options["preferences"]["traditional_chinese_output"] = traditional;
  for (const char *directory : {"resources", "dictionaries"})
    std::filesystem::copy_file(fixture, root / directory / "msime-wubi.db");
  ServerSession server(42, options.dump());
  server.activate(1);
  msime::tsf::EngineSessionAdapter tip;
  std::string raw, error;
  require(tip.create(options.dump(), &error) && tip.focus(true, &raw, &error), "TIP session created");
  ReplyComposer composer(42, 1);
  WubiCommitOwner owner;
  uint64_t request = 0;
  std::string document, remainder;
  unsigned commits = 0;
  const auto apply = [&](const msime::tsf::EngineResult &result) {
    return msime::tsf::ApplyHostCharacterResult(result, [&](const std::string &text) {
      document += text; ++commits; return true;
    }, [&] { remainder.clear(); return true; }, [&] { remainder = result.view.editing_text; return true; });
  };
  const std::string keys = capital ? "abcdX" : (unique ? "abcdabcd" : "abcdx");
  for (unsigned char letter : keys) {
    phase = std::to_string(unique) + ":" + std::to_string(early) + ":" + std::to_string(uiless) + ":" + std::to_string(traditional) + ":" + std::to_string(capital) + ":" + std::to_string(letter);
    require(!owner.active(), "the previous worker releases FIFO before the next key is sent");
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = ++request;
    packet.keycode = letter >= 'a' ? letter - 'a' + 'A' : letter;
    packet.wch = letter;
    const bool shift = letter == 'X';
    packet.modifiers_down = (uiless ? FanyImePipeFlags::UiLess : 0) | (shift ? 1 : 0);
    // 和生产派发一样，在可能收到 worker 之前先注册现有 FIFO token。
    owner.begin(request, 100);
    const auto reply = composer.basic_key(server, packet, 1, style);
    require(reply.has_value(), "Server handles the same physical key");
    const bool worker = reply->worker.has_value();
    if (early && worker)
      require(owner.worker(17) == WubiCommitOwner::Worker::Early, "early worker cannot acquire an edit session");
    require(tip.character(letter, shift, &raw, &error), "TIP handles the physical key");
    msime::tsf::EngineResult result;
    require(msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error), "TIP transition parsed");
    require(result.view.editing_text == server.view().at("editing_text"), "sessions keep the same remaining code");
    const unsigned before = commits;
    if (!owner.observe(result)) {
      require(apply(result) == msime::tsf::CharacterResultStatus::Applied, "ordinary TIP transition applied");
    }
    require(owner.finish_local() == result.has_commit, "production owner decision waits only for a commit");
    if (result.has_commit) {
      require(commits == before && owner.current(request), "local auto commit leaves document and FIFO owned");
      require(worker && reply->worker->at(0) == FanyImeWorkerReplyType::CommitCandidateAndContinue,
              "Server retains the authoritative worker");
      if (early) require(owner.take_early_worker() == 17, "early worker is posted after the local edit");
      require(owner.worker(17) == WubiCommitOwner::Worker::Ready, "one worker edit is admitted");
      require(owner.worker(17) == WubiCommitOwner::Worker::Invalid, "duplicate edit is rejected");
      require(tip.view(&raw, &error) && msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error),
              "worker restores the live view instead of the unused legacy key buffer");
      require(apply(msime::tsf::ServerWubiCommit(std::move(result), *reply->committed_text)) ==
              msime::tsf::CharacterResultStatus::Applied, "Server final text is applied once");
    } else require(!worker && !reply->committed_text, "ordinary code input never commits");
    owner.clear();
    composer.confirm_delivery(42, 1, request);
  }
  const std::string word = traditional ? "合成國" : "合成国";
  const std::string expected = unique ? word + word : (capital ? word + "X" : "");
  require(document == expected && commits == (unique ? 2u : (capital ? 1u : 0u)), "one physical code writes one candidate once");
  require(remainder == ((!unique && !capital) ? "x" : ""), "top commit retains the next code");
  require(server.view().value("phrase_prefix", "") == ((!unique && !capital) ? "合成国" : ""),
          "phrase preedit holds the topped word without an unsolicited document write");
}
void lifetime() {
  WubiCommitOwner owner;
  msime::tsf::EngineResult commit;
  commit.handled = commit.has_commit = true;
  owner.begin(1, 100);
  require(!owner.expired(2099) && owner.expired(2100), "missing worker has a bounded deadline");
  require(owner.worker(1) == WubiCommitOwner::Worker::Early, "worker can precede the local result");
  require(owner.observe(commit) && owner.finish_local(), "local commit retains ownership");
  owner.clear();
  require(!owner.current(1) && !owner.has_early_worker(), "focus or disconnect discards old ownership and payload");
  owner.begin(2, 200);
  require(!owner.editing(1), "a stale edit cannot write into the replacement token");
  owner.clear();
  auto final = msime::tsf::ServerWubiCommit(commit, "脚本合成改写!");
  std::string document;
  require(msime::tsf::ApplyHostCharacterResult(final, [&](const std::string &text) {
    document += text; return true;
  }, [] { return true; }, [] { return true; }) == msime::tsf::CharacterResultStatus::Applied &&
      document == "脚本合成改写!", "worker preserves rewritten text instead of the local candidate");
}
}
int main(int argc, char **argv) {
  require(argc == 2, "synthetic fixture path supplied");
  lifetime();
  for (bool unique : {true, false})
    for (bool early : {true, false})
      for (auto style : {TsfPreeditStyle::Local, TsfPreeditStyle::Pinyin, TsfPreeditStyle::Empty})
        for (bool uiless : {false, true})
          for (bool traditional : {false, true}) {
            scenario(argv[1], unique, early, style, uiless, traditional, false);
            if (!unique) scenario(argv[1], unique, early, style, uiless, traditional, true);
          }
  std::puts("Wubi commit owner: 72 dual-session scenarios and lifetime boundaries passed");
}
