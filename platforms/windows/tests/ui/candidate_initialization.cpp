#include "../../src/candidate/CandidatePresentation.h"
#include <cassert>

int main() {
  using namespace msime::windows;
  const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
  PendingReply reply{};
  reply.source.client_id = 42;
  reply.source.activation_epoch = 1;
  reply.source.request_id = 7;
  // Built in named pieces rather than one nested initializer: the nesting is
  // four deep, and a brace miscounted in the middle of it does not read as an
  // error anywhere near where it was written.
  const nlohmann::json candidates = nlohmann::json::array(
      {{{"id", {{"session", 2}, {"generation", 3}, {"index", 0}}},
        {"text", "cloud"},
        {"source", 2},
        {"highlighted", true}},
       {{"id", {{"session", 2}, {"generation", 3}, {"index", 1}}},
        {"text", "local"},
        {"source", 0},
        {"highlighted", false}}});
  const nlohmann::json view = {{"session", 2},
                               {"generation", 3},
                               {"focused", true},
                               {"editing_text", "synthetic"},
                               {"preedit", "synthetic"},
                               {"scheme", 0},
                               {"candidates", candidates}};
  reply.source.transition = {{"view", view}};
  FanyImeNamedpipeData packet{};
  packet.client_id = 42;
  packet.request_id = 7;
  packet.event_type = FanyImePipeEventType::KeyEvent;
  packet.point[0] = 12;
  packet.point[1] = 34;
  const auto visible = candidate_presentation(lease, reply, packet);
  assert(visible.visible && visible.preedit == "synthetic");
  assert(visible.session == 2 && visible.generation == 3);
  assert(visible.x == 12 && visible.y == 34);
  // 按键送达的投影也放插入点（#10792）。这个视图没有 preedit_caret，退回 preedit_display_caret：preedit 和 editing_text 相同、没有 caret_position，落在末尾。
  assert(visible.preedit_caret == visible.preedit.size());
  assert(visible.candidates.size() == 2 &&
         !visible.candidates[0].actions_available &&
         visible.candidates[1].actions_available && !visible.traditional_output);
  assert(!visible.game_host);
  // 视图带引擎的 preedit_caret 时按它放，不再按 caret_position 换算。
  reply.source.transition["view"]["preedit_caret"] = 3;
  reply.source.transition["view"]["caret_position"] = 9;
  const auto engine_caret = candidate_presentation(lease, reply, packet);
  assert(engine_caret.visible && engine_caret.preedit_caret == 3);
  reply.source.transition = {{"view", view}};
  // 游戏会话的标记来自包元数据，不改变可见性和坐标。
  packet.modifiers_down = PipeMetadata::GameHost;
  const auto game = candidate_presentation(lease, reply, packet);
  assert(game.game_host && game.visible && game.x == 12 && game.y == 34);
  packet.modifiers_down = FanyImePipeFlags::UiLess;
  const auto hidden = candidate_presentation(lease, reply, packet);
  assert(!hidden.visible && hidden.preedit.empty());
  assert(hidden.preedit_caret == std::string::npos);
  assert(!hidden.game_host);
  // UiLess 仍然由宿主自己画，带上 GameHost 也照样隐藏。
  packet.modifiers_down = FanyImePipeFlags::UiLess | PipeMetadata::GameHost;
  const auto hidden_game = candidate_presentation(lease, reply, packet);
  assert(!hidden_game.visible && hidden_game.preedit.empty() &&
         hidden_game.game_host);
}
