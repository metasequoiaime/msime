// #11509：两份真实会话的原文与标点都跟随当前全半角状态；不读取用户词库。
#include "../../src/ipc/ReplyComposer.h"
#include "../../common/CharacterWidthText.h"
#include "../../tsf/HostRawCommit.h"
#include "../../tsf/HostSessionStart.h"
#include "../core/TestHostOptions.h"
#include <chrono>
#include <cstdio>
#include <stdexcept>

using namespace msime::windows;
using nlohmann::json;
namespace {
const char *phase = "initialization";
void require(bool value, const std::string &message) {
  if (!value) throw std::runtime_error(message);
}
json tip_result(const std::string &raw) { return json::parse(raw).at("value"); }

void legacy_prefix_case() {
  // 旧宿主保留的候选前缀已决定文字，不能跟着末尾原文再次转换宽度。
  ReplyComposer composer(42, 1, false);
  auto transition = [](uint64_t request, const char *editing, const char *commit) {
    return KeyResult{42, 1, request, true,
      {{"commit", commit}, {"view", {{"session", 1}, {"editing_text", editing},
        {"preedit", editing}, {"character_width", "Fullwidth"}}}}};
  };
  composer.stage(transition(1, "abc", "AI"), ReplyPath::Selection);
  composer.confirm_delivery(42, 1, 1);
  const auto &reply = composer.stage(transition(2, "", "ａｂｃ"), ReplyPath::LocalCommit, false, "AIabc");
  require(!reply.encoded && reply.committed_text == "AIａｂｃ", "legacy prefix keeps its original width");
  composer.confirm_delivery(42, 1, 2);
}

void session_cases(const std::string &options, bool japanese) {
  ServerSession server(42, options);
  server.activate(1);
  msime::tsf::EngineSessionAdapter tip;
  std::string raw, error;
  require(msime::tsf::create_host_session(tip, options, "", &error), "TIP create");
  require(tip.focus(true, &raw, &error), "TIP focus");
  // 中文方案的标点表里只有间隔号，`~`、`/` 交回宿主按全半角投影；日语方案走契约的日文标点表（#11839，
  // `shared/contracts/punctuation/policy.json` 的 `japanese`），`~` 出「〜」、`/` 出「・」，表里没有的反引号仍交回宿主。
  struct PunctuationCase {
    uint8_t key;
    const char *projected;  // 全角状态下宿主投影出的字面标点
    const char *chinese;    // 中文方案上屏的字，nullptr 表示交回宿主
    const char *japanese;   // 日语方案上屏的字，nullptr 表示交回宿主
  };
  const PunctuationCase punctuation[] = {
      {'~', "～", nullptr, "〜"},
      {'`', "｀", "·", nullptr},
      {'/', "／", nullptr, "・"},
  };
  for (bool fullwidth : {false, true, false}) {
    phase = "set width";
    server.set_character_width(1, fullwidth);
    require(tip.character_width(fullwidth, &raw, &error), "TIP width update");
    require(server.view().at("character_width") == (fullwidth ? "Fullwidth" : "Halfwidth"), "Server width view");
    for (const auto &symbol : punctuation) {
      FanyImeNamedpipeData packet{};
      packet.client_id = 42;
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.request_id = 2;
      packet.wch = symbol.key;
      packet.keycode = symbol.key == '/' ? 0xBF : 0xC0;
      packet.modifiers_down = symbol.key == '~' ? 1u : 0u;
      phase = "punctuation";
      auto committed = server.punctuation(packet, 1).transition.at("commit");
      const char *chosen = japanese ? symbol.japanese : symbol.chinese;
      require(tip.punctuation(symbol.key, &raw, &error), "TIP punctuation");
      if (chosen != nullptr) {
        const std::string expected(chosen);
        require(committed == expected, "Server punctuation width");
        require(tip_result(raw).at("commit") == expected, "TIP punctuation width");
      } else {
        // 无映射时共享会话让宿主输出字面标点，预取路径必须另行投影宽度。
        require(committed.is_null() && tip_result(raw).at("commit").is_null(), "unmapped punctuation stays with host");
        const std::string literal(1, static_cast<char>(symbol.key));
        const std::string expected = fullwidth ? symbol.projected : literal;
        require((fullwidth ? fullwidth_ascii_text(literal) : literal) == expected, "prefetched punctuation width");
      }
    }
    phase = "letters";
    for (unsigned char c : std::string("abc")) {
      FanyImeNamedpipeData packet{};
      packet.client_id = 42;
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.request_id = 3;
      packet.wch = c;
      packet.keycode = c - 'a' + 'A';
      (void)server.key(packet, 1);
      require(tip.character(c, false, &raw, &error), "TIP letters");
    }
    const std::string expected = fullwidth ? "ａｂｃ" : "abc";
    ReplyComposer composer(42, 1);
    FanyImeNamedpipeData enter{};
    enter.client_id = 42; enter.event_type = FanyImePipeEventType::KeyEvent;
    enter.request_id = 4; enter.keycode = 0x0D;
    phase = "local raw dispatch";
    bool rejected = false;
    try { (void)composer.dispatch(server, enter, 1, ReplyPath::LocalCommit, false, "abd"); }
    catch (const std::invalid_argument &) { rejected = true; }
    require(rejected && server.view().at("editing_text") == "abc", "wrong observation preserves composition");
    const auto &reply = composer.dispatch(server, enter, 1, ReplyPath::LocalCommit, false, "abc");
    require(reply.committed_text == expected && !reply.encoded, "Server raw observation accepts final width");
    composer.confirm_delivery(42, 1, 4);
    require(tip.command(MSIME_COMMIT_RAW, &raw, &error), "TIP raw command");
    require(tip_result(raw).at("commit") == expected, "TIP raw width");
    // 生产 Enter 入口优先提交日文读音；其中残留的拉丁字母仍要转全角。
    for (unsigned char c : std::string("abc"))
      require(tip.character(c, false, &raw, &error), "TIP Enter letters");
    std::string inserted;
    unsigned cleanups = 0;
    const auto status = msime::tsf::CommitHostRaw(tip,
        [&](const std::string &text) { inserted = text; return true; },
        [&] { ++cleanups; }, &error);
    require(status == msime::tsf::RawCommitStatus::Completed && cleanups == 1, "TIP Enter cleanup");
    require(inserted == (japanese ? (fullwidth ? "あｂｃ" : "あbc") : expected), "TIP Enter width");
  }
  // 词库维护后的重建从 TSF 当前模式恢复，不等下一次全半角切换。
  phase = "recreate";
  tip.destroy();
  require(msime::tsf::create_host_session(tip, options, "", &error, true), "TIP recreate fullwidth");
  require(tip.focus(true, &raw, &error), "TIP refocus");
  require(tip.character('q', false, &raw, &error), "TIP recreated letter");
  require(tip.command(MSIME_COMMIT_RAW, &raw, &error), "TIP recreated raw");
  require(tip_result(raw).at("commit") == "ｑ", "TIP recreated width");
}
}
int main() {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-fullwidth-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  try {
    require(fullwidth_ascii_text("A 1~·🌲") == "Ａ　１～·🌲", "ASCII projection preserves Unicode");
    require(!raw_commit_width_matches("abc", "ａｂｄ", true), "different text rejected");
    require(!raw_commit_width_matches("abc", "ａｂｃ", false), "halfwidth mode rejects width change");
    legacy_prefix_case();
    for (bool japanese : {false, true}) {
      auto options = test_host_options(root / (japanese ? "japanese" : "quanpin"));
      options["preferences"]["scheme"] = japanese ? "japanese" : "quanpin";
      session_cases(options.dump(), japanese);
      // 显式开启命令模式时，`/` 属于组字，原文确认仍遵守当前宽度。
      phase = "command slash";
      char *defaults = msime_client_default_preferences();
      require(defaults != nullptr, "shared preferences available");
      options["preferences"]["local_modes"] = json::parse(defaults).at("value").at("local_modes");
      msime_client_string_free(defaults);
      options["preferences"]["local_modes"]["command"] = true;
      msime::tsf::EngineSessionAdapter tip;
      std::string raw, error;
      require(msime::tsf::create_host_session(tip, options.dump(), "", &error, true), "command TIP create");
      require(tip.focus(true, &raw, &error), "command TIP focus");
      require(tip.character('/', false, &raw, &error), "command slash character");
      // 日文不进入命令模式，`/` 由契约日文表上屏；中文回车提交命令原文。
      if (japanese) {
        // 日语没有命令模式，`/` 走契约日文表直接上屏读点，不再交回宿主。
        require(tip_result(raw).at("handled").get<bool>() && tip_result(raw).at("commit") == "・",
                "Japanese slash commits its contracted mark");
      } else {
        require(tip.command(MSIME_COMMIT_RAW, &raw, &error), "command slash raw");
        require(tip_result(raw).at("commit") == "／", "command slash fullwidth");
      }
    }
  } catch (const std::exception &error) {
    std::fprintf(stderr, "fullwidth session (%s): %s\n", phase, error.what());
    return 1;
  }
  std::puts("fullwidth session: passed");
}
