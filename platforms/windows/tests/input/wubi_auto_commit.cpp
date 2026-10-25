#include "ipc/ReplyComposer.h"
#include "../../tsf/HostCharacterResult.h"
#include "../core/TestHostOptions.h"
#include "../../tsf/WubiCommitOwner.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>

using namespace msime::windows;
namespace {
void require(bool value, const char *message) {
  if (!value) {
    std::fprintf(stderr, "FAIL: %s\n", message);
    std::exit(EXIT_FAILURE);
  }
}
struct Cleanup {
  std::filesystem::path root;
  ~Cleanup() {
    std::error_code error;
    std::filesystem::remove_all(root, error);
  }
};

void scenario(const std::filesystem::path &fixture, TsfPreeditStyle style,
              bool uiless, bool wubi98, bool unique, bool top,
              const std::string &input, const std::string &expected,
              const std::string &remainder, bool select = false,
              std::size_t expected_commits = 1,
              const std::string &expected_phrase = "",
              const std::string &expected_selection = "合成甲") {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-wubi-commit-" + std::to_string(
          std::chrono::steady_clock::now().time_since_epoch().count()));
  Cleanup cleanup{root};
  auto options = test_host_options(root);
  auto &preferences = options["preferences"];
  preferences["scheme"] = "wubi";
  preferences["default_ime_mode"] = "chinese";
  preferences["wubi_profile"] = wubi98 ? "wubi98" : "wubi86";
  preferences["wubi_mixed_pinyin"] = false;
  preferences["wubi_auto_commit_unique"] = unique;
  preferences["wubi_fifth_code_top_commit"] = top;
  // 两份会话读相同的合成表；资源名同时满足五笔按需资源检查。
  for (const char *directory : {"resources", "dictionaries"}) {
    std::filesystem::copy_file(fixture, root / directory / "msime-pinyin.db");
    std::filesystem::copy_file(fixture, root / directory / "msime-wubi.db");
  }
  ServerSession server(42, options.dump());
  server.activate(1);
  ReplyComposer composer(42, 1);
  msime::tsf::EngineSessionAdapter tip;
  std::string raw, error, inserted, counted;
  require(tip.create(options.dump(), &error) && tip.focus(true, &raw, &error),
          "TIP session created");
  std::size_t insertions = 0, observations = 0;
  uint64_t request = 0;
  msime::tsf::WubiCommitOwner owner;
  const auto check_views = [&] {
    require(tip.view(&raw, &error), "TIP view available");
    const auto view = nlohmann::json::parse(raw).at("value");
    require(view.at("editing_text") == server.view().at("editing_text") &&
            view.value("phrase_prefix", "") == server.view().value("phrase_prefix", ""),
            "TIP and Server retain the same remaining code");
  };
  for (unsigned char letter : input) {
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = ++request;
    const bool shift = letter >= 'A' && letter <= 'Z';
    packet.keycode = shift ? letter : letter - 'a' + 'A';
    packet.wch = letter;
    packet.modifiers_down = (shift ? 1u : 0u) |
        (uiless ? FanyImePipeFlags::UiLess : 0u);
    owner.begin(request, 100);
    const auto reply = composer.basic_key(server, packet, 1, style);
    require(reply.has_value(), "Server accepts the character");
    require(tip.character(letter, shift, &raw, &error), "TIP accepts the character");
    msime::tsf::EngineResult result;
    require(tip.parse_result(raw, &result, &error), "TIP character result parsed");
    const auto apply = [&](const msime::tsf::EngineResult &value) {
      return msime::tsf::ApplyHostCharacterResult(value,
          [&](const std::string &text) { inserted += text; ++insertions; return true; },
          [] { return true; }, [] { return true; });
    };
    const bool deferred = owner.observe(result);
    if (!deferred)
      require(apply(result) == msime::tsf::CharacterResultStatus::Applied,
              "ordinary TIP result application succeeds");
    require(owner.finish_local() == deferred, "local ownership matches the actual commit");
    // 保留原 PR 的 86/98 与偏好矩阵，最终写入改由 Server 的结果负责。
    if (deferred) {
      require(reply->worker && reply->committed_text &&
              reply->worker->at(0) == FanyImeWorkerReplyType::CommitCandidateAndContinue,
              "Server retains the final commit worker");
      require(owner.worker(1) == msime::tsf::WubiCommitOwner::Worker::Ready,
              "worker waits for the local character edit");
      require(tip.view(&raw, &error) && tip.parse_result(raw, &result, &error),
              "worker reads the live remaining composition");
      require(apply(msime::tsf::ServerWubiCommit(std::move(result), *reply->committed_text)) ==
              msime::tsf::CharacterResultStatus::Applied,
              "Server final text is inserted once");
    } else {
      require(!reply->worker && !reply->committed_text,
              "ordinary code has no commit worker or observation");
    }
    owner.clear();
    if (reply->committed_text) {
      counted += *reply->committed_text;
      ++observations;
    }
    if (reply->encoded) require(static_cast<bool>(*reply->encoded), "valid preedit reply");
    composer.confirm_delivery(42, 1, request);
    check_views();
  }
  if (inserted != expected || counted != expected)
    std::fprintf(stderr, "scenario input=%s style=%u uiless=%d wubi98=%d unique=%d top=%d inserted=%s counted=%s expected=%s\n",
        input.c_str(), static_cast<unsigned>(style), uiless, wubi98, unique, top,
        inserted.c_str(), counted.c_str(), expected.c_str());
  require(inserted == expected && counted == expected,
          "automatic commit text is inserted and counted exactly once");
  require(insertions == (expected.empty() ? 0u : expected_commits) && observations == insertions,
          "one insertion and one observation per completed code");
  require(server.view().at("editing_text") == remainder, "expected remaining code");
  require(server.view().value("phrase_prefix", "") == expected_phrase,
          "top commit keeps the selected phrase when another code remains");
  if (select) {
    // 关闭自动上屏之后，空格仍由正常候选选择回复写入，不走字符提交。
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.request_id = ++request;
    packet.keycode = 0x20;
    packet.wch = ' ';
    packet.modifiers_down = uiless ? FanyImePipeFlags::UiLess : 0u;
    const auto reply = composer.basic_key(server, packet, 1, style);
    require(reply && reply->committed_text == expected_selection && reply->encoded &&
            static_cast<bool>(*reply->encoded), "ordinary candidate selection still commits");
    composer.confirm_delivery(42, 1, request);
  }
}
void fallback_scenario(const std::filesystem::path &fixture) {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-wubi-fallback-" + std::to_string(
          std::chrono::steady_clock::now().time_since_epoch().count()));
  Cleanup cleanup{root};
  auto options = test_host_options(root);
  options["preferences"]["scheme"] = "wubi";
  options["preferences"]["default_ime_mode"] = "chinese";
  for (const char *directory : {"resources", "dictionaries"}) {
    std::filesystem::copy_file(fixture, root / directory / "msime-pinyin.db");
    std::filesystem::copy_file(fixture, root / directory / "msime-wubi.db");
  }
  ServerSession server(42, options.dump());
  server.activate(1);
  ReplyComposer composer(42, 1);
  uint64_t request = 0;
  std::size_t commits = 0;
  for (unsigned char letter : std::string("wqaa")) {
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = ++request;
    packet.keycode = letter - 'a' + 'A';
    packet.wch = letter;
    const auto reply = composer.basic_key(server, packet, 1, TsfPreeditStyle::Local);
    require(reply.has_value(), "legacy Server accepts the character");
    if (reply->committed_text) {
      require(reply->worker && *reply->committed_text == "合成甲" &&
              reply->worker->at(0) == FanyImeWorkerReplyType::CommitCandidateAndContinue,
              "legacy character path still sends the actual commit worker");
      ++commits;
    } else require(!reply->worker, "unfinished legacy code has no commit worker");
    composer.confirm_delivery(42, 1, request);
  }
  require(commits == 1 && server.view().at("editing_text") == "",
          "legacy path commits once and finishes its code");
}

} // namespace

int main(int argc, char **argv) {
  require(argc == 2, "synthetic Wubi fixture required");
  try {
    fallback_scenario(argv[1]);
    for (auto style : {TsfPreeditStyle::Local, TsfPreeditStyle::Pinyin, TsfPreeditStyle::Empty})
      for (bool uiless : {false, true})
        for (bool wubi98 : {false, true}) {
          scenario(argv[1], style, uiless, wubi98, true, true, "wqaa", "合成甲", "");
          scenario(argv[1], style, uiless, wubi98, true, true, "wqaawqaa", "合成甲合成甲", "", false, 2);
          scenario(argv[1], style, uiless, wubi98, true, true, "wqabw", "", "w", false, 1, "合成乙");
          scenario(argv[1], style, uiless, wubi98, true, true, "wqabA", "合成乙A", "");
          scenario(argv[1], style, uiless, wubi98, false, true, "wqaaw", "", "w", false, 1, "合成甲");
          scenario(argv[1], style, uiless, wubi98, false, false, "wqaa", "", "wqaa", true);
          // #9696：没有组字时 Shift+T、Shift+U 在五笔下进入日期时间和 Unicode 模式，两边会话都把后面的字母留在模式里，不当五笔码组字，也不走四码自动上屏和顶字；空格上屏模式的首选。临时英文要英文词库，这份合成表没有，由 Engine 的回归覆盖。
          scenario(argv[1], style, uiless, wubi98, true, true, "Tdate", "", "Tdate");
          scenario(argv[1], style, uiless, wubi98, true, true, "Uabcd", "", "Uabcd", true, 1, "", "\xEA\xAF\x8D");
        }
  } catch (const std::exception &error) {
    std::fprintf(stderr, "FAIL: %s\n", error.what());
    return EXIT_FAILURE;
  }
  std::puts("Wubi auto commit: 86/98, preferences, Server final text and legacy continuation passed");
  return EXIT_SUCCESS;
}
