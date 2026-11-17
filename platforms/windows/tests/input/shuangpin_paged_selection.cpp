// 双拼翻页之后的键盘选词：候选窗由 Server 的会话画，TIP 另有自己的宿主会话负责组字和上屏。鼠标滚轮和候选窗的翻页箭头只翻 Server 那一页（FocusedSession::page_candidate），宿主会话还停在第一页，同一个位置上是另一个词。TIP 必须照 Server 对这个键的回复在自己的会话里选同一个词（HostServerSelection.h），而不是按自己那一页的位置选——以前数字选到只消耗一段读音的候选时（回复是 NeedToCreateWord）正是按位置选的：Server 翻到第三页按 4 选「霓」，宿主会话选的是自己第一页的第 4 个「妮好」，整词直接上屏。用两份真实引擎会话和合成词库验证，不读用户或发布词库。
#include "ipc/ReplyComposer.h"
#include "../../tsf/EngineSessionAdapter.h"
#include "../../tsf/HostServerSelection.h"
#include "../../tsf/CandidateNumberDispatch.h"
#include "../../common/SecondThirdCandidatePolicy.h"
#include "../../tsf/IPC/ReplyWaitPolicy.h"
#include "../../../../shared/contracts/quick_phrase_cursor_pipe.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <filesystem>

using namespace msime::windows;
using msime::tsf::HostSelectionOutcome;
using msime::tsf::ServerSelectionReply;
using nlohmann::json;
namespace {
const char *phase = "initialization";
std::filesystem::path fixture;
void require(bool value, const char *message) {
  if (!value) { std::fprintf(stderr, "FAIL (%s): %s\n", phase, message); std::exit(EXIT_FAILURE); }
}
// 回复帧里的 UTF-16 文字换成 UTF-8，和 TIP 读回复时的转换一样。
std::string utf8(const FanyImeWireChar *text) {
  std::string out;
  for (size_t i = 0; i < FanyImePipeLimits::CandidateTextCapacity && text[i]; ++i) {
    uint32_t scalar = text[i];
    if (scalar >= 0xD800 && scalar <= 0xDBFF && i + 1 < FanyImePipeLimits::CandidateTextCapacity && text[i + 1]) scalar = 0x10000 + ((scalar - 0xD800) << 10) + (text[++i] - 0xDC00);
    if (scalar < 0x80) out += static_cast<char>(scalar);
    else if (scalar < 0x800) { out += static_cast<char>(0xC0 | (scalar >> 6)); out += static_cast<char>(0x80 | (scalar & 0x3F)); }
    else if (scalar < 0x10000) { out += static_cast<char>(0xE0 | (scalar >> 12)); out += static_cast<char>(0x80 | ((scalar >> 6) & 0x3F)); out += static_cast<char>(0x80 | (scalar & 0x3F)); }
    else { out += static_cast<char>(0xF0 | (scalar >> 18)); out += static_cast<char>(0x80 | ((scalar >> 12) & 0x3F)); out += static_cast<char>(0x80 | ((scalar >> 6) & 0x3F)); out += static_cast<char>(0x80 | (scalar & 0x3F)); }
  }
  return out;
}

void pure_rules() {
  phase = "reply parsing";
  using msime::tsf::ServerSelectedPiece;
  // 整段上屏的文字减掉宿主会话已经暂存的词段，才是这次选的候选。
  require(ServerSelectedPiece({FanyImeReplyType::Normal, "霓好"}, "") == "霓好", "a whole commit is the candidate itself");
  require(ServerSelectedPiece({FanyImeReplyType::Normal, "霓好"}, "霓") == "好", "the held piece is not part of the final candidate");
  // 只选一段：载荷是「剩下的原文\t已选的全部词段\t显示的预编辑」，取第二段。以前空格把整条载荷当作候选文字。
  require(ServerSelectedPiece({FanyImeReplyType::NeedToCreateWord, "hc\t霓\t霓hc"}, "") == "霓", "a partial selection names its piece");
  require(ServerSelectedPiece({FanyImeReplyType::NeedToCreateWord, "c\t霓好\t霓好c"}, "霓") == "好", "a second partial selection names only the new piece");
  // 暂存的词段没变：Server 这次什么都没选。
  require(ServerSelectedPiece({FanyImeReplyType::NeedToCreateWord, "hc\t霓\t霓hc"}, "霓").empty(), "an unchanged held piece selects nothing");
  require(ServerSelectedPiece({FanyImeReplyType::NeedToCreateWord, "hc"}, "").empty(), "a malformed partial reply selects nothing");
  require(ServerSelectedPiece({FanyImeReplyType::NavigationIgnored, ""}, "").empty() &&
          ServerSelectedPiece({FanyImeReplyType::OutofRange, ""}, "").empty(), "an ignored key selects nothing");
  require(msime::tsf::ServerReplyDecidesSelection(FanyImeReplyType::NavigationIgnored) &&
          msime::tsf::ServerReplyDecidesSelection(FanyImeReplyType::NeedToCreateWord) &&
          !msime::tsf::ServerReplyDecidesSelection(FanyImeReplyType::TransportUnavailable) &&
          !msime::tsf::ServerReplyDecidesSelection(FanyImeReplyType::CommitExactText),
          "only a selection reply decides for the host session");
}

enum class Paging { Wheel, Keyboard };

struct Sessions {
  ServerSession server;
  msime::tsf::EngineSessionAdapter tip;
  ReplyComposer composer{42, 1};
  uint64_t request = 0;
  std::string raw, error;
  explicit Sessions(const std::string &options) : server(42, options) {
    server.activate(1);
    require(tip.create(options, &error) && tip.focus(true, &raw, &error), "TIP session created");
  }
  msime::tsf::EngineView tip_view() {
    msime::tsf::EngineResult parsed;
    require(tip.view(&raw, &error) && msime::tsf::EngineSessionAdapter::parse_result(raw, &parsed, &error), "TIP view");
    return parsed.view;
  }
  PendingReply key(uint32_t keycode, wchar_t wch) {
    FanyImeNamedpipeData packet{}; packet.client_id = 42; packet.request_id = ++request;
    packet.keycode = keycode; packet.wch = wch;
    auto reply = composer.basic_key(server, packet, 1, TsfPreeditStyle::Local);
    require(reply.has_value(), "Server handles the key");
    composer.confirm_delivery(42, 1, request);
    return *reply;
  }
  void type(const std::string &letters) {
    for (unsigned char letter : letters) {
      key(letter - 'a' + 'A', letter);
      require(tip.character(letter, false, &raw, &error), "TIP types");
    }
    require(tip_view().editing_text == server.view().at("editing_text").get<std::string>(), "both sessions compose the same reading");
  }
  // 翻页：滚轮只翻 Server 那一页；键盘翻页由 Server 回导航回执，TIP 在自己的会话里执行同一个命令。
  void page(unsigned steps, Paging how) {
    const auto view = server.view();
    (void)server.page_candidate(1, view.at("session"), view.at("generation"), false, steps);
    if (how == Paging::Keyboard)
      for (unsigned step = 0; step < steps; ++step) require(tip.command(MSIME_NEXT_PAGE, &raw, &error), "TIP pages");
  }
  // TIP 收到这个键的回复后在自己的会话里跟着选，和 _SelectHighlightedHostCandidate 一样。
  HostSelectionOutcome follow(const PendingReply &reply, json *selected) {
    require(reply.encoded && static_cast<bool>(*reply.encoded), "Server sent a reply frame");
    const ServerSelectionReply server_reply{reply.encoded->packet.msg_type, utf8(reply.encoded->packet.candidate_string)};
    require(msime::tsf::ServerReplyDecidesSelection(server_reply.type), "a selection key gets a deciding reply");
    const auto view = tip_view();
    const auto piece = msime::tsf::ServerSelectedPiece(server_reply, view.phrase_prefix);
    if (piece.empty()) return HostSelectionOutcome::Refused;
    const auto outcome = msime::tsf::SelectHostCandidateByText(tip, view, piece, &raw, &error);
    if (outcome == HostSelectionOutcome::Selected) *selected = json::parse(raw).at("value");
    return outcome;
  }
  void same() {
    const auto view = tip_view();
    const auto server_view = server.view();
    require(view.editing_text == server_view.at("editing_text").get<std::string>() &&
            view.phrase_prefix == server_view.value("phrase_prefix", std::string{}),
            "TIP and Server keep the same reading and held piece");
  }
};

std::string make_options(const std::filesystem::path &root) {
  char *defaults = msime_client_default_preferences();
  require(defaults != nullptr, "shared preference defaults available");
  auto preferences = json::parse(defaults).at("value");
  msime_client_string_free(defaults);
  preferences["scheme"] = "shuangpin";
  preferences["default_ime_mode"] = "chinese";
  preferences["learning"] = false;
  preferences["candidate_page_size"] = 5;
  json options{{"api_version", 1}, {"preferences", preferences}};
  for (const char *name : {"resources", "user_data", "cache", "dictionaries"}) {
    std::filesystem::create_directories(root / name); options[name] = (root / name).u8string();
  }
  for (const char *name : {"resources", "dictionaries"})
    std::filesystem::copy_file(fixture, root / name / "msime-pinyin.db");
  return options.dump();
}

// 小鹤双拼 nihc = ni'hao。翻 `steps` 页后按 `digit`，Server 那一页上的第 `digit` 个是 `expected`。
void scenario(Paging how, unsigned steps, char digit, const char *expected, bool partial) {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-shuangpin-paging-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root);
  struct Cleanup { std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  Sessions s(make_options(root));
  s.type("nihc");
  s.page(steps, how);
  const auto shown = s.server.view().at("candidates");
  const size_t slot = static_cast<size_t>(digit - '1');
  require(slot < shown.size() && shown.at(slot).at("text") == expected, "the Server page shows the synthetic candidate at that slot");
  if (how == Paging::Wheel) {
    // 宿主会话还在第一页：同一个位置上不是用户看见的那个词，按位置选就会选错。
    const auto host = s.tip_view();
    require(slot < host.candidates.size() && host.candidates[slot].text != expected,
            "the host session's own page holds a different word at that slot");
  }
  phase = "digit selection";
  const auto chosen = s.key(static_cast<uint32_t>(digit), static_cast<wchar_t>(digit));
  json selected;
  require(s.follow(chosen, &selected) == HostSelectionOutcome::Selected, "TIP selects the Server's choice in its own session");
  if (!partial) {
    require(chosen.committed_text == std::optional<std::string>(expected) && selected.at("commit") == expected,
            "both sessions commit the word the candidate window showed");
    s.same();
    return;
  }
  require(selected.at("commit").is_null() && !chosen.committed_text, "a partial selection holds the piece in both sessions");
  require(s.tip_view().phrase_prefix == expected, "the host session holds the piece the user picked");
  s.same();
  phase = "finishing space";
  const auto finished = s.key(0x20, L' ');
  require(s.follow(finished, &selected) == HostSelectionOutcome::Selected, "TIP follows the finishing Space");
  const std::string whole = std::string(expected) + "好";
  require(finished.committed_text == std::optional<std::string>(whole) && selected.at("commit") == whole,
          "the held piece and the rest commit once, as shown");
  s.same();
}

// Server 最后一页只有一个候选，按 3 什么都不选；宿主会话第一页上第 3 个是整词，以前照样选中它上屏。
void ignored_past_last_page() {
  phase = "digit past the last page";
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-shuangpin-paging-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root);
  struct Cleanup { std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  Sessions s(make_options(root));
  s.type("nihc");
  s.page(3, Paging::Wheel);
  require(s.server.view().at("candidates").size() == 1, "the Server shows the one-candidate last page");
  require(s.tip_view().candidates.size() > 2, "the host session still shows a full first page");
  const auto chosen = s.key('3', L'3');
  json selected;
  require(s.follow(chosen, &selected) == HostSelectionOutcome::Refused && !chosen.committed_text,
          "a key the Server ignored selects nothing in the host session");
  s.same();
}

// 五项分页上按 9 必须消费无动作回执并完成按键，之后还可继续组字。
void ignored_past_configured_page() {
  phase = "digit nine past configured page";
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-shuangpin-nine-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root);
  struct Cleanup { std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  Sessions s(make_options(root));
  s.type("nihc");
  for (const auto key : {static_cast<unsigned>('9'), 0x69u}) {
    const auto before = s.tip_view();
    require(before.candidates.size() <= 5, "the host page has no ninth candidate");
    const auto slot = candidate_selection_slot(static_cast<unsigned>(key), '9');
    require(slot == std::optional<std::size_t>(8), "both digit rows request slot nine");
    const auto local = tip_candidate_selection_index(static_cast<unsigned>(key), '9', true, 5);
    require(!local, "the configured local page has no ninth slot");
    const auto reply = s.key(static_cast<unsigned>(key), L'9');
    require(reply.encoded && reply.encoded->packet.msg_type == FanyImeReplyType::NavigationIgnored,
            "the server acknowledges a no-op without committing text");
    bool consumed = false;
    const int result = msime::tsf::DispatchCandidateNumber(-1, true, true, 1,
        [](int) { throw std::runtime_error("An absent slot was selected"); return false; },
        [] { throw std::runtime_error("An absent slot was converted"); return 1; }, [&] {
          consumed = true;
          return msime::tsf::CompleteServerCandidateSelection(reply.encoded->packet.msg_type, 0,
              [] { throw std::runtime_error("An acknowledged no-op edited composition"); return 1; });
        });
    require(consumed && result == 0, "the key completes without dirtying the session or replaying");
    require(s.tip_view().editing_text == before.editing_text &&
            s.tip_view().phrase_prefix == before.phrase_prefix && !reply.committed_text,
            "an ignored ninth candidate keeps the composition and commits nothing");
    s.same();
    s.type("ni");
  }
}

// 数字键的回复没在软等待里到（#10826：CPU 吃紧时 Server 回包超过 100ms）。按键已经送到 Server，它照样选中并上屏了这个词；TIP 读到的是 SoftReplyMiss，必须按「读不到回复」退回本地这一页上按键位置的那一项（_HandleCandidateWorker），两边各上屏一次同一个词。以前软等待的结果帧是 Normal、文字为空，被当成 Server 的决定：宿主会话找不到空文字，选词返回 S_FALSE，延迟队列把整段读音连同数字键重放一遍，CPU 一直吃紧就一遍遍重来，最多八次后丢掉。
void reply_missed_selects_local_slot() {
  phase = "digit reply missed";
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-shuangpin-paging-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root);
  struct Cleanup { std::filesystem::path root;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
  } cleanup{root};
  Sessions s(make_options(root));
  s.type("nihc");
  const auto host_page = s.tip_view();
  require(host_page.candidates.size() > 2 && host_page.candidates[2].text == "泥好", "the host page shows the synthetic word at slot 3");
  const auto chosen = s.key('3', L'3');
  require(chosen.committed_text == std::optional<std::string>("泥好"), "the Server selects and commits the word on its own");
  const auto missed = msime::windows::tsf::SoftReplyMiss(s.request);
  require(missed.request_id == s.request, "the missed frame names the request it waited for");
  require(!msime::tsf::ServerReplyDecidesSelection(missed.msg_type), "a reply that never arrived is not a Server decision");
  require(!FanyImeQuickPhraseCursorPipe::SuffixUnits(missed.msg_type, false) &&
          missed.msg_type != FanyImeReplyType::CommitExactText && missed.msg_type != FanyImeReplyType::TransportUnavailable,
          "a missed reply is neither text to commit nor a broken transport");
  // 和 _SelectHighlightedHostCandidate 不带回复时一样：选本地高亮项（按键位置的那一项）的文字。
  const auto outcome = msime::tsf::SelectHostCandidateByText(s.tip, host_page, host_page.candidates[2].text, &s.raw, &s.error);
  require(outcome == HostSelectionOutcome::Selected, "the host session selects its own slot");
  require(json::parse(s.raw).at("value").at("commit") == "泥好", "the host session commits the same word once");
  s.same();
}
} // namespace

int main(int argc, char **argv) {
  require(argc == 2, "synthetic dictionary fixture is required");
  fixture = std::filesystem::path(argv[1]);
  try {
    pure_rules();
    // 滚轮翻两页到第三页，按 4 选单字「霓」：只消耗 ni，Server 回 NeedToCreateWord。
    scenario(Paging::Wheel, 2, '4', "霓", true);
    // 滚轮翻一页，按 2 选整词「铌好」：宿主会话第一页上没有它，要到完整列表里找。
    scenario(Paging::Wheel, 1, '2', "铌好", false);
    // 键盘翻页两边一起翻，同一页上照样能选。
    scenario(Paging::Keyboard, 2, '3', "拟", true);
    scenario(Paging::Keyboard, 1, '5', "鲵好", false);
    ignored_past_last_page();
    ignored_past_configured_page();
    reply_missed_selects_local_slot();
  } catch (const std::exception &error) {
    std::fprintf(stderr, "FAIL at %s: %s\n", phase, error.what());
    return EXIT_FAILURE;
  }
  std::puts("shuangpin paged selection: passed");
}
