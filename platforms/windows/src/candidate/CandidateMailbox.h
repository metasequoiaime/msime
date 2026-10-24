#pragma once
#include "CandidatePresentation.h"
#include "ipc/InternalEventFlags.h"
#include <chrono>
#include <condition_variable>
#include <functional>

namespace msime::windows {
// Single latest value, not an unbounded per-key UI event queue. Publish only
// from the input queue's confirmed-delivery callback under the focus gate.
class CandidateMailbox final {
public:
  void rendered(const FocusLease &lease, uint64_t render_serial) {
    std::lock_guard lock(mutex_);
    if (!latest_ || stopped_ || latest_->lease.epoch != lease.epoch ||
        latest_->lease.token != lease.token ||
        !same_ticket(latest_->lease.transport, lease.transport) ||
        render_serial < latest_->render_serial)
      return;
    rendered_generation_ = render_serial;
    rendered_lease_ = lease;
    rendered_ready_.notify_all();
  }
  bool wait_rendered(const FocusLease &lease, uint64_t generation,
                     std::chrono::milliseconds timeout) {
    std::unique_lock lock(mutex_);
    return rendered_ready_.wait_for(lock, timeout, [&] {
      return stopped_ ||
             (rendered_generation_ >= generation && rendered_lease_ &&
              rendered_lease_->epoch == lease.epoch &&
              rendered_lease_->token == lease.token &&
              same_ticket(rendered_lease_->transport, lease.transport));
    }) && !stopped_;
  }
  void delivered(const FocusLease &lease, const PendingReply &reply,
                 const FanyImeNamedpipeData &packet) {
    auto value = candidate_presentation(lease, reply, packet);
    std::lock_guard lock(mutex_);
    if (!stopped_) {
      attach_candidate_readings(value.candidates, readings_);
      value.render_serial = ++render_serial_;
      // 同一租约里游戏会话标记只置位不清除：某个按键包漏了 GameHost 位时，不能把已经认出的游戏会话打回普通宿主。
      const bool same_lease = latest_ && latest_->lease.epoch == lease.epoch &&
                              latest_->lease.token == lease.token &&
                              same_ticket(latest_->lease.transport, lease.transport);
      if (same_lease && latest_->game_host)
        value.game_host = true;
      // 只有带 GameHost 位的按键包 TIP 才写坐标（Ipc.cpp 给游戏会话补上锚点），其余按键包的 point 是线上结构的默认值 (100,100)。同一租约里沿用 Show/Move 报过的位置：否则在随后的 Show/Move 到达之前候选窗刷新一次，卡片就跳到屏幕左上的 (100,100)；宿主报的是 INVALID_Y（还没有文字范围、卡片应当藏着）时，也会被这个默认值顶掉而露出来。
      if (same_lease && (packet.modifiers_down & PipeMetadata::GameHost) == 0) {
        value.x = latest_->x;
        value.y = latest_->y;
      }
      // 按键包不带坐标，沿用同一租约里 Show/Move 报过的锚点。
      if (same_lease && latest_->caret_anchor_known) {
        value.caret_anchor_known = true;
        value.caret_anchor_x = latest_->caret_anchor_x;
        value.caret_anchor_y = latest_->caret_anchor_y;
      }
      latest_ = std::move(value);
      suppressed_ = false;
      pending_hide_.reset();
    }
  }
  // Replace Engine-owned candidate data after an asynchronous cloud result.
  // Coordinates and the selected prefix belong to the existing presentation.
  void online(const FocusLease &lease, const nlohmann::json &view) {
    refresh_view(lease, view, true);
  }
  // Translation application keeps the same Engine generation; only the
  // candidate metadata changes.
  // readings 是同一次翻译算出的读音，替换上一次的；之后同样文字的候选（下一个键、翻页回来）也挂得上，直到下一次翻译或偏好变化。
  void translations(const FocusLease &lease, const nlohmann::json &view,
                    CandidateReadings readings = {}) {
    auto keyed = keyed_readings(std::move(readings));
    {
      std::lock_guard lock(mutex_);
      readings_ = std::move(keyed);
    }
    refresh_view(lease, view, false);
  }
  // 这一页没有新释义可交给会话、只有读音：换上新的读音，重新挂到眼下的候选上，并发布一个新快照让候选窗重画。
  void readings(const FocusLease &lease, CandidateReadings readings) {
    auto keyed = keyed_readings(std::move(readings));
    std::lock_guard lock(mutex_);
    readings_ = std::move(keyed);
    if (stopped_ || !latest_ || latest_->lease.epoch != lease.epoch ||
        latest_->lease.token != lease.token ||
        !same_ticket(latest_->lease.transport, lease.transport))
      return;
    attach_candidate_readings(latest_->candidates, readings_);
    latest_->render_serial = ++render_serial_;
  }
  // 偏好变了（读音开关、释义开关、目标语言）：旧的读音作废，眼下的候选上也摘掉，等重新翻译。
  void clear_readings() {
    std::lock_guard lock(mutex_);
    readings_.clear();
    if (stopped_ || !latest_)
      return;
    const bool shown = std::any_of(
        latest_->candidates.begin(), latest_->candidates.end(),
        [](const PresentationCandidate &candidate) {
          return !candidate.pronunciation.empty();
        });
    if (!shown)
      return;
    attach_candidate_readings(latest_->candidates, readings_);
    latest_->render_serial = ++render_serial_;
  }
  // Candidate menu actions advance Engine's generation without sending text
  // through TSF. Publish the resulting view immediately on the input queue.
  void action(const FocusLease &lease, const nlohmann::json &view) {
    refresh_view(lease, view, true);
  }
  // 卷轴展开（CandidateExpandPanel.h）：候选窗点开「∨」时 open 为 true，只认这个租约；收起、候选窗隐藏、断开时清掉，连同手里的完整候选列表。界面线程调用，只拿这把锁。
  void expansion(const FocusLease &lease, bool open) {
    std::lock_guard lock(mutex_);
    clear_expansion_locked();
    if (open && !stopped_)
      expansion_lease_ = lease;
  }
  // 要不要为这个租约再取一次完整候选列表：展开着、正显示着这个租约的候选，而手里的列表不是眼下这一代的（接着打字、翻页、云候选到达都会换代；只补上释义的同一代不用再取）。
  bool expansion_due(const FocusLease &lease) {
    std::lock_guard lock(mutex_);
    apply_pending_hide_locked();
    return !stopped_ && expansion_lease_ && same_lease(*expansion_lease_, lease) && latest_ &&
           same_lease(latest_->lease, lease) && latest_->visible && !suppressed_ &&
           !latest_->candidates.empty() && latest_->translation_page == 0 &&
           !expanded_current_locked(*latest_);
  }
  // 输入队列上取到的完整候选列表（expanded_candidates_from_snapshot），属于 session 的 generation 那一代。只在展开还开着、而且正是眼下这一页的代次时留下，并发布一个新快照让候选窗重画；繁体输出时文字换成繁体，和这一页画的一样。繁简转换要调用共享层，在锁外做。
  void expanded(const FocusLease &lease, uint64_t session, uint64_t generation,
                std::vector<ExpandedCandidate> items) {
    bool traditional = false;
    {
      std::lock_guard lock(mutex_);
      if (!expansion_matches_locked(lease, session, generation))
        return;
      traditional = latest_->traditional_output;
    }
    if (traditional)
      for (auto &item : items)
        item.text = candidate_output(item.text, true, item.source);
    auto shared = std::make_shared<const std::vector<ExpandedCandidate>>(std::move(items));
    std::lock_guard lock(mutex_);
    // 换算期间切换了简繁：这份文字已经不对，等下一次取。
    if (!expansion_matches_locked(lease, session, generation) ||
        latest_->traditional_output != traditional)
      return;
    expanded_ = std::move(shared);
    expanded_session_ = session;
    expanded_generation_ = generation;
    expanded_traditional_ = traditional;
    latest_->render_serial = ++render_serial_;
  }

private:
  static bool same_lease(const FocusLease &a, const FocusLease &b) {
    return a.epoch == b.epoch && a.token == b.token && same_ticket(a.transport, b.transport);
  }
  bool expansion_matches_locked(const FocusLease &lease, uint64_t session,
                                uint64_t generation) const {
    return !stopped_ && expansion_lease_ && same_lease(*expansion_lease_, lease) && latest_ &&
           same_lease(latest_->lease, lease) && latest_->session == session &&
           latest_->generation == generation && latest_->visible;
  }
  void clear_expansion_locked() {
    expansion_lease_.reset();
    expanded_.reset();
    expanded_session_ = 0;
    expanded_generation_ = 0;
    expanded_traditional_ = false;
  }
  // 手里的完整列表是不是这一页的：同一会话、同一代，文字按同一种简繁换算过。切换简繁（Ctrl+Shift+F）只换投影、不换代次，不比简繁的话网格会停在旧的字形上，点哪个都因文字对不上被拒绝。
  bool expanded_current_locked(const CandidatePresentation &value) const {
    return expanded_ && expanded_session_ == value.session &&
           expanded_generation_ == value.generation &&
           expanded_traditional_ == value.traditional_output;
  }
  // 繁体输出时候选显示的是繁体，所以每条读音按繁体文字再存一份。在锁外做：繁简转换要调用共享层。
  static CandidateReadings keyed_readings(CandidateReadings readings) {
    CandidateReadings keyed;
    keyed.reserve(readings.size() * 2);
    for (auto &[text, reading] : readings) {
      auto traditional = simplified_to_traditional(text, true);
      if (traditional != text)
        keyed.emplace(std::move(traditional), reading);
      keyed.emplace(text, std::move(reading));
    }
    return keyed;
  }
  void refresh_view(const FocusLease &lease, const nlohmann::json &view,
                    bool require_new_generation) {
    std::lock_guard lock(mutex_);
    apply_pending_hide_locked();
    try {
      if (stopped_ || !latest_ || latest_->lease.epoch != lease.epoch ||
          latest_->lease.token != lease.token ||
          !same_ticket(latest_->lease.transport, lease.transport))
        return;
      // 画着的是 Ctrl+Enter 的释义页：它在 Server 里一直开着，直到下一个按键把它关掉（ReplyComposer::translation_page_key），那个按键的投递会带回 Engine 的列表。这里按 Engine 的 view 重建会把释义页换成底下的列表，而数字键、空格上屏的仍是看不见的义项。打开释义页的那个按键自己就会带出一次释义查询，结果回来时正好落在这里，所以不能借这个机会关掉释义页，只能不盖掉它。
      if (latest_->translation_page != 0)
        return;
      if (view.at("session").get<uint64_t>() != latest_->session ||
          (require_new_generation &&
           view.at("generation").get<uint64_t>() <= latest_->generation))
        return;
      const auto text = view.at("preedit").get<std::string>();
      if (latest_->preedit.size() < text.size() ||
          latest_->preedit.compare(latest_->preedit.size() - text.size(),
                                   text.size(), text) != 0)
        return;
      const auto prefix =
          latest_->preedit.substr(0, latest_->preedit.size() - text.size());
      // 视图来自 Engine，不带包元数据；游戏会话标记和坐标一样属于原有的展示。
      const bool game_host = latest_->game_host;
      // Tab 预选的释义列在 ReplyComposer 里活到下一个按键，释义或云候选在这之间送到时下划线不能丢；高亮候选已经没有那一列时就不画了，上屏时 ReplyComposer 也按这条规则判断。
      const int armed_gloss_column = latest_->armed_gloss_column;
      // Show/Move 报过的组字锚点同样属于原有的展示；释义或云候选一到就把它丢掉的话，输入模式提示会找不到光标。
      const bool caret_anchor_known = latest_->caret_anchor_known;
      const int caret_anchor_x = latest_->caret_anchor_x;
      const int caret_anchor_y = latest_->caret_anchor_y;
      latest_ =
          candidate_presentation_from_view(lease, view, latest_->x, latest_->y,
                                           prefix, latest_->traditional_output);
      latest_->game_host = game_host;
      latest_->caret_anchor_known = caret_anchor_known;
      latest_->caret_anchor_x = caret_anchor_x;
      latest_->caret_anchor_y = caret_anchor_y;
      latest_->armed_gloss_column =
          candidate_armed_gloss_column(latest_->candidates, armed_gloss_column);
      attach_candidate_readings(latest_->candidates, readings_);
      latest_->render_serial = ++render_serial_;
      pending_hide_.reset();
    } catch (...) {
      // Provider data is optional; malformed/stale projections are ignored.
    }
  }

public:
  // Input queue under the active focus gate, like delivered(). Hide follows
  // successful Engine cancellation; show/move never manufacture composition.
  void event(const FocusLease &lease, const FanyImeNamedpipeData &packet) {
    std::lock_guard lock(mutex_);
    apply_pending_hide_locked();
    if (stopped_ || !latest_ || packet.client_id != lease.transport.client ||
        latest_->lease.epoch != lease.epoch ||
        latest_->lease.token != lease.token ||
        !same_ticket(latest_->lease.transport, lease.transport))
      return;
    // 切到英文的通知结束组字，和上屏、取消一样算正常收起，候选窗可以播退场；别的隐藏（组字还开着时被宿主或焦点变化收掉）立即消失，见 CandidatePresentation::composition_ended。
    bool english_mode = false;
    switch (packet.event_type) {
    case FanyImePipeEventType::ClientActivated:
      if (packet.keycode != 0)
        suppressed_ = true;
      break;
    case FanyImePipeEventType::IMESwitch:
    case FanyImePipeEventType::StatusSnapshot:
    case FanyImePipeEventType::FocusRestored:
      // Input mode has already been synchronized on the input queue. An
      // English notification cancels composition; re-enabling must not revive
      // the old projection. A mode request alone is not a notification.
      if (packet.keycode != 0)
        break;
      // 只有 IMESwitch 是用户切到英文；StatusSnapshot 也在线程焦点变化和重连时发，FocusRestored 本身就是焦点转移，它们带走的候选立即消失，和 macOS 的 focus_out 一样。
      english_mode = packet.event_type == FanyImePipeEventType::IMESwitch;
      [[fallthrough]];
    case FanyImePipeEventType::HideCandidateWnd:
      if ((packet.modifiers_down & internal_continuation_hide) != 0) {
        pending_hide_.reset();
        break;
      }
      // Only a hide delivered behind a congested input queue describes state
      // already superseded by a later key. Normal commits and focus changes
      // must disappear immediately, preserving the typing snap.
      if ((packet.modifiers_down & internal_late_event) != 0) {
        if (!pending_hide_)
          pending_hide_ = std::chrono::steady_clock::now() + hide_grace;
      } else {
        pending_hide_.reset();
        suppressed_ = true;
        // 上屏、取消之后跟来的这个隐藏只是确认组字已经结束，保留投递时记下的原因。
        if (latest_->visible)
          latest_->composition_ended = english_mode;
        latest_->visible = false;
        latest_->preedit.clear();
        latest_->candidates.clear();
        clear_expansion_locked();
      }
      break;
    case FanyImePipeEventType::ShowCandidateWnd:
      pending_hide_.reset();
      suppressed_ = (packet.modifiers_down & FanyImePipeFlags::UiLess) != 0;
      [[fallthrough]];
    case FanyImePipeEventType::MoveCandidateWnd:
      pending_hide_.reset();
      // The host may take over candidate rendering without another key or
      // Show event. A move can suppress display, never revive hidden content.
      if ((packet.modifiers_down & FanyImePipeFlags::UiLess) != 0)
        suppressed_ = true;
      // 按键包漏了 GameHost 位时，随后的 Show/Move 能补上；同样只置位不清除。
      if ((packet.modifiers_down & PipeMetadata::GameHost) != 0)
        latest_->game_host = true;
      latest_->x = packet.point[0];
      latest_->y = packet.point[1];
      if (packet.point[1] != invalid_candidate_anchor_y) {
        latest_->caret_anchor_known = true;
        latest_->caret_anchor_x = packet.point[0];
        latest_->caret_anchor_y = packet.point[1];
      }
      break;
    }
  }
  void disconnected(const PipeTicket &ticket) {
    std::lock_guard lock(mutex_);
    if (latest_ && same_ticket(latest_->lease.transport, ticket)) {
      latest_.reset();
      pending_hide_.reset();
      rendered_lease_.reset();
      rendered_generation_ = 0;
    }
    if (expansion_lease_ && same_ticket(expansion_lease_->transport, ticket))
      clear_expansion_locked();
  }
  void stop() {
    std::lock_guard lock(mutex_);
    stopped_ = true;
    latest_.reset();
    clear_expansion_locked();
    pending_hide_.reset();
    rendered_lease_.reset();
    rendered_generation_ = 0;
    rendered_ready_.notify_all();
  }
  // External consumer only, never while holding the gate. No UI callbacks run
  // under either lock. The returned copy is valid at read time, not a grant to
  // perform a later candidate action without checking its identity again.
  // The optional identity validator runs under the gate and must not reenter;
  // non-waiting consumers must supply a non-waiting validator as well.
  std::optional<CandidatePresentation>
  snapshot(FocusGate &gate, bool wait = true,
           const std::function<bool(const FocusLease &)> &current = {}) {
    std::optional<FocusLease> lease;
    {
      std::unique_lock lock(mutex_, std::defer_lock);
      if (wait)
        lock.lock();
      else if (!lock.try_lock())
        return std::nullopt;
      apply_pending_hide_locked();
      if (latest_)
        lease = latest_->lease;
    }
    std::optional<CandidatePresentation> result;
    if (lease) {
      auto read = [&] {
        // Always gate -> mailbox, matching the producer's lock order. Fetch
        // the newest generation here, not the value observed before the gate.
        std::unique_lock lock(mutex_, std::defer_lock);
        if (wait)
          lock.lock();
        else if (!lock.try_lock())
          return;
        apply_pending_hide_locked();
        if (latest_ && latest_->lease.epoch == lease->epoch &&
            latest_->lease.token == lease->token &&
            same_ticket(latest_->lease.transport, lease->transport))
          result = latest_;
        // 完整候选列表只跟着取它的那一代、那种简繁走：换代或切换简繁之后、重新取到之前，这一页没有网格。
        if (result) {
          result->expansion_open =
              expansion_lease_ && same_lease(*expansion_lease_, result->lease);
          if (result->expansion_open && expanded_current_locked(*result))
            result->expanded = expanded_;
        }
        if (result && suppressed_) {
          result->visible = false;
          result->preedit.clear();
          result->candidates.clear();
        }
        lock.unlock();
        // Keep identity validation in the same active-focus scope. The
        // validator must also avoid waiting on independent handshake I/O.
        if (result && current && !current(result->lease))
          result.reset();
      };
      if (wait)
        gate.with_active(*lease, read);
      else
        gate.try_with_active(*lease, read);
    }
    return result;
  }

private:
  static constexpr auto hide_grace = std::chrono::milliseconds(24);

  void apply_pending_hide_locked() {
    if (!pending_hide_ || std::chrono::steady_clock::now() < *pending_hide_)
      return;
    pending_hide_.reset();
    if (!latest_)
      return;
    suppressed_ = true;
    latest_->visible = false;
    latest_->preedit.clear();
    latest_->candidates.clear();
    clear_expansion_locked();
  }

  std::mutex mutex_;
  bool stopped_ = false;
  bool suppressed_ = false;
  std::optional<std::chrono::steady_clock::time_point> pending_hide_;
  std::optional<CandidatePresentation> latest_;
  // 最近一次翻译算出的读音，按候选文字存，见 translations()。
  CandidateReadings readings_;
  // 卷轴展开开着时的租约，以及为它取到的完整候选列表和那一代的身份，见 expansion()。
  std::optional<FocusLease> expansion_lease_;
  std::shared_ptr<const std::vector<ExpandedCandidate>> expanded_;
  uint64_t expanded_session_ = 0, expanded_generation_ = 0;
  bool expanded_traditional_ = false;
  std::optional<FocusLease> rendered_lease_;
  uint64_t rendered_generation_ = 0;
  uint64_t render_serial_ = 0;
  std::condition_variable rendered_ready_;
};
} // namespace msime::windows
