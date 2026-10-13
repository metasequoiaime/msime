#include "FocusedSession.h"
#include <limits>

using namespace msime::windows;
namespace {
void require(bool value) {
  if (!value)
    throw std::runtime_error("UI selection state regression");
}
template <class Action> void rejected(Action action) {
  bool failed = false;
  try {
    action();
  } catch (const std::logic_error &) {
    failed = true;
  }
  require(failed);
}
} // namespace
void ui_selection_tests(const std::string &options, bool dictionary) {
  FocusGate gate;
  FocusedSession focused(gate, 42, options);
  auto change = gate.begin({42, {1, 2, 3}}, 77);
  require(change && focused.prepare(change->pending));
  const auto lease = change->pending;
  require(gate.acknowledge(lease, [] { return true; }));
  uint64_t request = 1;
  auto send = [&](uint32_t key, uint16_t text,
                  ReplyPath path = ReplyPath::Composition) {
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = request++;
    packet.keycode = key;
    packet.wch = text;
    packet.modifiers_down = text == 'U' ? 1 : 0;
    const auto reply = focused.key(lease, packet, path);
    require(reply.has_value() && !reply->ui_selection);
    const auto view = focused.view();
    if (!view.at("candidates").empty()) {
      const auto id = view.at("candidates").at(0).at("id");
      require(!focused.select_candidate(lease, id.at("session"),
                                        id.at("generation"), id.at("index"), 0));
      require(focused.view() == view);
    }
    require(focused.confirm(lease, packet.request_id));
  };
  uint64_t previous_receipt = 0;
  for (size_t round = 0; round < 2; ++round) {
    for (char c : std::string("U4e2d"))
      send(c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c, c);
    const auto view = focused.view();
    const auto id = view.at("candidates").at(0).at("id");
    const auto session = id.at("session").get<uint64_t>();
    const auto generation = id.at("generation").get<uint64_t>();
    const auto index = id.at("index").get<size_t>();
    auto stale_lease = lease;
    ++stale_lease.epoch;
    require(!focused.select_candidate(stale_lease, session, generation, index, 0));
    require(!focused.select_candidate(lease, session + 1, generation, index, 0));
    require(!focused.select_candidate(lease, session, generation + 1, index, 0));
    require(!focused.select_candidate(lease, session, generation,
                                      std::numeric_limits<size_t>::max(), 0));
    // 没有打开释义页时，带着释义页编号的点击来自一张已经不在的页。
    require(!focused.select_candidate(lease, session, generation, index, 1));
    require(focused.view() == view);
    auto pending = focused.select_candidate(lease, session, generation, index, 0);
    require(pending && pending->ui_selection && !pending->encoded);
    require(pending->source.request_id == 0 &&
            pending->source.transition.at("commit") == "中");
    require(pending->ui_selection->worker ==
            ui_complete_selection("中")->worker);
    const auto selected = focused.view();
    rejected([&] { send('A', 'a'); });
    require(!focused.select_candidate(lease, session, generation, index, 0));
    rejected([&] { focused.confirm(lease, 0); });
    rejected([&] { focused.confirm_ui(lease, previous_receipt); });
    require(focused.view() == selected &&
            focused.pending(lease)->ui_selection.has_value());
    previous_receipt = selected.at("generation").get<uint64_t>();
    require(focused.confirm_ui(lease, previous_receipt));
    require(!focused.pending(lease));
  }
  for (size_t round = 0; dictionary && round < 2; ++round) {
    for (char c : std::string("nihao"))
      send(c - 'a' + 'A', c);
    std::optional<nlohmann::json> id;
    for (size_t page = 0; page < 100; ++page) {
      const auto view = focused.view();
      for (const auto &candidate : view.at("candidates"))
        if (candidate.at("text") == "你")
          id = candidate.at("id");
      if (id)
        break;
      if (view.at("page").get<size_t>() + 1 >=
          view.at("page_count").get<size_t>())
        break;
      send(0x22, 0, ReplyPath::NextPage);
    }
    require(id.has_value());
    const auto partial = focused.select_candidate(
        lease, id->at("session"), id->at("generation"), id->at("index"), 0);
    require(partial && partial->ui_selection->before_trigger &&
            partial->next_prefix == "你");
    rejected([&] { focused.cancel_composition(lease); });
    require(focused.confirm_ui(
        lease, partial->source.transition.at("view").at("generation")));
    if (round == 0) {
      auto stale = lease;
      ++stale.epoch;
      const auto before = focused.view();
      require(!focused.cancel_composition(stale) && focused.view() == before);
      require(focused.cancel_composition(lease));
      require(gate.with_active(lease, [] {}) &&
              focused.view().at("editing_text") == "" &&
              focused.view().at("candidates").empty());
      require(!focused.select_candidate(lease, id->at("session"),
                                        id->at("generation"), id->at("index"), 0));
      for (char c : std::string("U4e2d"))
        send(c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c, c);
      const auto fresh = focused.view().at("candidates").at(0).at("id");
      const auto selected = focused.select_candidate(
          lease, fresh.at("session"), fresh.at("generation"), fresh.at("index"), 0);
      require(selected && selected->ui_selection->worker ==
                              ui_complete_selection("中")->worker);
      require(focused.confirm_ui(
          lease, selected->source.transition.at("view").at("generation")));
      continue;
    }
    const auto next = focused.view().at("candidates").at(0).at("id");
    const auto final = focused.select_candidate(
        lease, next.at("session"), next.at("generation"), next.at("index"), 0);
    require(final && !final->ui_selection->before_trigger &&
            final->next_prefix.empty());
    require(final->ui_selection->worker ==
            ui_complete_selection("你好")->worker);
    require(gate.deactivate(lease));
    require(!focused.confirm_ui(
        lease, final->source.transition.at("view").at("generation")));
  }
}
