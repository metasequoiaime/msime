// 方向键不能把高亮停在候选窗不画的云占位席位上（#10665）。席位在引擎列表的第二格，候选窗跳过它；高亮落上去时候选窗一个高亮行也没有，`candidate_presentation` 在 Server 的输入队列上抛出，整个 Server 随之停下，用户那边是候选框消失、之后打不出中文。
// 先用合成的列表钉住 leave_hidden_seat 的走法，再在真实的 Server 会话（ReplyComposer）和 TIP 宿主会话上按下、上、横排的右、左，核对候选窗的投影成立、两份会话的高亮停在同一格。词库是合成的夹具，云候选打开、不联网：席位一直留着，和云端连不上的用户一样。
#include "ReplyComposer.h"
#include "CandidatePresentation.h"
#include "NavigationPolicy.h"
#include "../../common/CandidateSources.h"
#include "../../tsf/HostCandidateNavigation.h"
#include "../../tsf/Global/CandidateNavigationPolicy.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <vector>

using namespace msime::windows;
using nlohmann::json;
namespace {
void require(bool value, const char *message) {
  if (!value) {
    std::fprintf(stderr, "FAIL: %s\n", message);
    std::exit(EXIT_FAILURE);
  }
}

// 合成的引擎列表：每行看不看得见，高亮在第几行。step 按引擎的规矩走（到头就停）。
struct List {
  std::vector<bool> visible;
  std::size_t highlighted = 0;
  int steps = 0;
  bool fail = false;
  std::optional<HighlightedSeat> seat() const {
    if (visible.empty()) return std::nullopt;
    return HighlightedSeat{highlighted, visible[highlighted]};
  }
  bool step(bool forward) {
    ++steps;
    if (fail) return false;
    if (forward && highlighted + 1 < visible.size()) ++highlighted;
    if (!forward && highlighted > 0) --highlighted;
    return true;
  }
  bool leave(std::optional<std::size_t> before) {
    return leave_hidden_seat(before, visible.size(), [&] { return seat(); },
                             [&](bool forward) { return step(forward); });
  }
};

void policy_tests() {
  // 从首选按下：落在第二格的席位上，接着往下走到第三格。
  List down{{true, false, true, true}, 1};
  require(down.leave(0) && down.highlighted == 2 && down.steps == 1, "down skips the hidden seat");
  // 从第三格按上：落在席位上，接着往上回到首选。
  List up{{true, false, true, true}, 1};
  require(up.leave(2) && up.highlighted == 0 && up.steps == 1, "up skips the hidden seat backwards");
  // 席位是最后一行：往下走不动，掉头回到首选。
  List last{{true, false}, 1};
  require(last.leave(0) && last.highlighted == 0 && last.steps == 2, "a trailing seat turns the highlight back");
  // 整页只剩席位：两头都走不动，不来回试。
  List only{{false}, 0};
  require(only.leave(std::nullopt) && only.highlighted == 0 && only.steps == 2, "a page of only hidden seats stops");
  // 高亮本来就在看得见的行上：什么都不发。
  List visible{{true, false, true}, 2};
  require(visible.leave(0) && visible.steps == 0, "a visible highlight is left alone");
  // 没有候选：什么都不发。
  List empty{{}, 0};
  require(empty.leave(std::nullopt) && empty.steps == 0, "no candidates, no steps");
  // 命令失败如实报告。
  List failing{{true, false, true}, 1};
  failing.fail = true;
  require(!failing.leave(0), "a failed step is reported");
}

std::optional<HighlightedSeat> server_seat(const json &view) {
  for (const auto &candidate : view.at("candidates"))
    if (candidate.at("highlighted").get<bool>())
      return HighlightedSeat{candidate.at("id").at("index").get<std::size_t>(),
                             candidate_source_is_visible(candidate.value("source", 0u))};
  return std::nullopt;
}

std::optional<HighlightedSeat> tip_seat(msime::tsf::EngineSessionAdapter &tip) {
  std::string raw, error;
  require(tip.view(&raw, &error), "TIP view readable");
  return msime::tsf::HostHighlightedSeat(raw, nullptr);
}

void session_tests(const char *fixture) {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-cloud-seat-navigation-" +
                     std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() {
      std::error_code error;
      std::filesystem::remove_all(root, error);
    }
  } cleanup{root};
  json options{{"api_version", 1},
               {"preferences",
                {{"scheme", "quanpin"},
                 {"default_ime_mode", "chinese"},
                 {"learning", false},
                 {"cloud_candidates", true},
                 {"candidate_layout", "horizontal"},
                 {"candidate_page_size", 8},
                 {"navigation",
                  {{"minus_equal", true},
                   {"comma_period", true},
                   {"brackets", false},
                   {"tab", true},
                   {"page_up_down", true},
                   {"arrows", true},
                   {"left_right", true}}},
                 {"chinese_punctuation", true}}}};
  for (const char *name : {"resources", "user_data", "cache", "dictionaries"}) {
    std::filesystem::create_directories(root / name);
    options[name] = (root / name).u8string();
  }
  for (const char *name : {"resources", "dictionaries"})
    std::filesystem::copy_file(fixture, root / name / "msime-pinyin.db");

  ServerSession server(42, options.dump());
  server.activate(1);
  ReplyComposer composer(42, 1);
  const auto bindings = preference_navigation(options.at("preferences"));
  msime::tsf::EngineSessionAdapter tip;
  std::string raw, error;
  require(tip.create(options.dump(), &error) && tip.focus(true, &raw, &error), "TIP host session created");
  PipeTicket ticket{};
  ticket.client = 42;
  const FocusLease lease{ticket, 1, 7};
  uint64_t request = 0;
  // 交给 Server，像 SessionPump 那样在送达后投影候选窗再确认；投影抛出就是 #10665 里让输入队列停下的那一下。
  const auto press = [&](uint32_t code, char16_t text) {
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = ++request;
    packet.keycode = code;
    packet.wch = text;
    auto reply = composer.configured_key(server, packet, 1, TsfPreeditStyle::Local, bindings);
    require(reply.has_value(), "Server answers the key");
    const auto presented = candidate_presentation(lease, *reply, packet);
    std::size_t highlighted = 0;
    for (const auto &candidate : presented.candidates) highlighted += candidate.highlighted;
    require(presented.candidates.empty() || highlighted == 1, "the candidate window shows exactly one highlight");
    composer.confirm_delivery(42, 1, packet.request_id);
    return reply;
  };
  for (const char letter : std::string("nihao")) {
    press(static_cast<uint32_t>(letter - 'a' + 'A'), static_cast<char16_t>(letter));
    require(tip.character(static_cast<uint8_t>(letter), false, &raw, &error), "TIP accepts the letter");
  }
  const auto view = server.view();
  require(view.at("candidates").size() > 3 &&
              view.at("candidates").at(1).value("source", 0u) == candidate_source_cloud_pending,
          "the cloud seat holds the second row while the cloud has not answered");
  require(server_seat(view)->index == 0 && tip_seat(tip)->index == 0, "both sessions start on the first row");

  // 方向键：Server 让过席位，TIP 照 Server 的回复走，同样让过去。
  const auto navigate = [&](uint32_t code, std::size_t expected, const char *message) {
    const auto reply = press(code, 0);
    require(reply->encoded && *reply->encoded, "navigation is answered with a reply packet");
    const auto command = tsf::CandidateNavigationCommand(reply->encoded->packet.msg_type);
    require(command != 0, "the reply names a navigation command");
    require(msime::tsf::NavigateHostCandidates(tip, command, &raw, &error), "TIP follows the reply");
    const auto server_highlight = server_seat(server.view());
    const auto tip_highlight = tip_seat(tip);
    require(server_highlight && server_highlight->visible && server_highlight->index == expected, message);
    require(tip_highlight && tip_highlight->visible && tip_highlight->index == expected,
            "the TIP host session stops on the same row as the Server");
  };
  navigate(0x28, 2, "down from the first row skips the hidden seat");
  navigate(0x28, 3, "down again moves one row");
  navigate(0x26, 2, "up moves one row");
  navigate(0x26, 0, "up past the hidden seat returns to the first row");

  // 横排的右、左在 runtime 里同样移动高亮（caret_edge_candidate_shortcut），走的是 Server 的编辑键路由。
  press(0x27, 0);
  auto highlight = server_seat(server.view());
  require(highlight && highlight->visible && highlight->index == 2, "right in a horizontal list skips the hidden seat");
  press(0x25, 0);
  highlight = server_seat(server.view());
  require(highlight && highlight->visible && highlight->index == 0, "left in a horizontal list skips it backwards");
}
} // namespace

int main(int argc, char **argv) {
  require(argc == 2, "synthetic dictionary fixture supplied");
  policy_tests();
  session_tests(argv[1]);
  std::puts("cloud seat navigation: ok");
  return 0;
}
