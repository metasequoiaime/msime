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

// 没有组字时 Engine 不收的 Shift+字母（五笔的 Shift+A，Shift+字母总开关关掉时拼音的大写字母）：两份会话都答 handled=false，Server 没有提交；TIP 已经把这个键当输入吃下，所以按 KeyHandler 的顺序把它当作这个字符上屏，不再答「未处理」让回放队列重放 8 次后丢掉、重连（#9696 的 server.log）。之后的字母照常组字。
void declined_capital_scenario(const std::filesystem::path &fixture, const char *scheme_name,
                               bool shift_letter_entry, const std::string &input,
                               const std::string &expected_passed, const std::string &remainder,
                               const std::string &local_mode = "none") {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-declined-capital-" + std::to_string(
          std::chrono::steady_clock::now().time_since_epoch().count()));
  Cleanup cleanup{root};
  auto options = test_host_options(root);
  auto &preferences = options["preferences"];
  preferences["scheme"] = scheme_name;
  preferences["default_ime_mode"] = "chinese";
  preferences["wubi_mixed_pinyin"] = false;
  // 出厂的各模式开关（LocalModePreferences::default），只换总开关。
  preferences["local_modes"] = {
      {"unicode", true}, {"date_time", true}, {"quick_phrase", true}, {"emoji", true},
      {"kaomoji", true}, {"super_jianpin", true}, {"temporary_english", true},
      {"temporary_japanese", true}, {"expression", false}, {"command", false}, {"mention", false},
      {"mention_places", false}, {"shift_letter_entry", shift_letter_entry}};
  for (const char *directory : {"resources", "dictionaries"}) {
    std::filesystem::copy_file(fixture, root / directory / "msime-pinyin.db");
    std::filesystem::copy_file(fixture, root / directory / "msime-wubi.db");
  }
  ServerSession server(42, options.dump());
  server.activate(1);
  ReplyComposer composer(42, 1);
  msime::tsf::EngineSessionAdapter tip;
  std::string raw, error, passed;
  require(tip.create(options.dump(), &error) && tip.focus(true, &raw, &error),
          "TIP session created");
  msime::tsf::WubiCommitOwner owner;
  uint64_t request = 0;
  for (unsigned char letter : input) {
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = ++request;
    const bool shift = letter >= 'A' && letter <= 'Z';
    packet.keycode = shift ? letter : letter - 'a' + 'A';
    packet.wch = letter;
    packet.modifiers_down = shift ? 1u : 0u;
    owner.begin(request, 100);
    const auto reply = composer.basic_key(server, packet, 1, TsfPreeditStyle::Local);
    require(reply.has_value(), "Server answers the character");
    require(tip.character(letter, shift, &raw, &error), "TIP accepts the character");
    msime::tsf::EngineResult result;
    require(tip.parse_result(raw, &result, &error), "TIP character result parsed");
    // _ApplyHostCompositionResult 的顺序：五笔提交归属先看答复，再按被拒的空闲字符改写，最后照常应用。
    require(!owner.observe(result), "no Server commit worker is awaited for these keys");
    msime::tsf::EngineResult declined;
    const bool declines = msime::tsf::DeclinedIdleCharacterCommit(result, static_cast<char>(letter), &declined);
    if (declines)
      require(!reply->committed_text && !reply->worker, "the Server declines the same character");
    const auto status = msime::tsf::ApplyHostCharacterResult(declines ? declined : result,
        [&](const std::string &text) { passed += text; return true; },
        [] { return true; }, [] { return true; });
    if (status != msime::tsf::CharacterResultStatus::Applied)
      std::fprintf(stderr, "scheme=%s shift_letter_entry=%d input=%s key=%c handled=%d status=%d\n",
                   scheme_name, shift_letter_entry, input.c_str(), letter, result.handled,
                   static_cast<int>(status));
    // 答「未处理」时编辑会话返回 S_FALSE，回放队列重放同一个键，Engine 每次都答同样的 handled=false。
    require(status == msime::tsf::CharacterResultStatus::Applied,
            "the TIP finishes every key it ate instead of replaying it");
    require(!owner.finish_local(), "the key completes without a Server worker");
    owner.clear();
    composer.confirm_delivery(42, 1, request);
    require(tip.view(&raw, &error), "TIP view available");
    const auto view = nlohmann::json::parse(raw).at("value");
    require(view.at("editing_text") == server.view().at("editing_text"),
            "TIP and Server retain the same composition");
  }
  if (passed != expected_passed || server.view().at("editing_text") != remainder)
    std::fprintf(stderr, "scheme=%s shift_letter_entry=%d input=%s passed=%s remainder=%s\n", scheme_name,
                 shift_letter_entry, input.c_str(), passed.c_str(),
                 server.view().at("editing_text").get<std::string>().c_str());
  require(passed == expected_passed, "the declined capital reaches the document exactly once");
  require(server.view().at("editing_text") == remainder, "the keys after it compose as usual");
  require(server.view().value("local_mode", "none") == local_mode, "the expected local mode");
}

} // namespace

int main(int argc, char **argv) {
  require(argc == 2, "synthetic Wubi fixture required");
  try {
    fallback_scenario(argv[1]);
    // 五笔：Engine 不收的空闲 Shift+字母（A，以及 #9719 之后仍按拼音拼写查的 E、M、J 和 V）上屏为这个大写字母，后面的五笔码照常组字；Shift+T 仍进入日期时间模式。连续大写里第二枚起不进模式（#7616），同样上屏。
    declined_capital_scenario(argv[1], "wubi", true, "A", "A", "");
    declined_capital_scenario(argv[1], "wubi", true, "Aw", "A", "w");
    for (const char *capital : {"E", "M", "J", "V"})
      declined_capital_scenario(argv[1], "wubi", true, capital, capital, "");
    declined_capital_scenario(argv[1], "wubi", true, "T", "", "T", "date_time");
    declined_capital_scenario(argv[1], "wubi", true, "AT", "AT", "");
    // 拼音关掉「Shift+字母进入快捷模式」总开关：没有组字时的大写字母不进模式、不开始组字，上屏为大写字母；开着时 Shift+A 开始组字，组字中的大写字母是原文词段（#8815），两种都不变。
    declined_capital_scenario(argv[1], "quanpin", false, "A", "A", "");
    declined_capital_scenario(argv[1], "quanpin", false, "Tn", "T", "n");
    declined_capital_scenario(argv[1], "quanpin", false, "nA", "", "nA");
    declined_capital_scenario(argv[1], "quanpin", true, "A", "", "A");
    declined_capital_scenario(argv[1], "quanpin", true, "nA", "", "nA");
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
