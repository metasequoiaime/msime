#include "InputQueue.h"
#include "TerminalDeactivationPolicy.h"
#include "CandidateWheel.h"

namespace msime::windows {
std::optional<PendingReply>
InputState::select_candidate(const FocusLease &lease, uint64_t expected_session,
                             uint64_t generation, size_t index) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->select_candidate(lease, expected_session, generation,
                                         index)
               : std::nullopt;
}
std::optional<PendingReply> InputState::toggle_character_set(
    const FocusLease &lease, const FanyImeNamedpipeData &packet, bool enabled,
    const std::function<bool(bool)> &persist) {
  check_thread();
  auto *owner = session(lease.transport);
  if (!owner)
    return std::nullopt;
  return owner->toggle_character_set(lease, packet, enabled, persist);
}
std::optional<nlohmann::json>
InputState::candidate_action(const FocusLease &lease, uint64_t session,
                             uint64_t generation, size_t index,
                             CandidateAction action, uint8_t position) {
  check_thread();
  auto *owner = this->session(lease.transport);
  return owner ? owner->candidate_action(lease, session, generation, index,
                                         action, position)
               : std::nullopt;
}
bool InputState::ui_delivered(const FocusLease &lease, uint64_t generation) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner && owner->confirm_ui(lease, generation);
}
namespace {
thread_local const InputQueue *active_queue = nullptr;
struct WorkerScope {
  explicit WorkerScope(const InputQueue *queue) { active_queue = queue; }
  ~WorkerScope() { active_queue = nullptr; }
};
} // namespace
InputState::InputState(FocusGate &gate, size_t clients, std::string options)
    : gate_(gate), router_(gate, clients), options_(std::move(options)) {
  if (options_.empty() || options_.size() > 16384)
    throw std::invalid_argument("Invalid input queue configuration");
  const auto document = nlohmann::json::parse(options_);
  navigation_ = preference_navigation(
      document.value("preferences", nlohmann::json::object()));
  word_character_ = preference_word_character(
      document.value("preferences", nlohmann::json::object()));
  tsf_preedit_style_ = preference_tsf_preedit_style(
      document.value("preferences", nlohmann::json::object()));
  character_set_shortcut_enabled_ =
      document.value("preferences", nlohmann::json::object())
          .value("keybindings", nlohmann::json::object())
          .value("toggle_character_set_ctrl_shift_f", true);
}
InputState::~InputState() { shutdown(); }
void InputState::shutdown() noexcept {
  // Sessions must release their thread-local Rust handles on this same worker.
  // Shutdown is fail-closed even if shared Engine cleanup reports an error.
  for (auto &[id, client] : clients_) {
    (void)id;
    try {
      cleanup(router_.disconnected(client.ticket));
    } catch (...) {
      gate_.invalidate(client.ticket);
    }
  }
  clients_.clear();
}
void InputState::check_thread() const {
  if (std::this_thread::get_id() != thread_)
    throw std::logic_error("Wrong input state thread");
}
FocusedSession *InputState::session(const PipeTicket &ticket) {
  auto found = clients_.find(ticket.client);
  return found != clients_.end() && same_ticket(found->second.ticket, ticket)
             ? found->second.session.get()
             : nullptr;
}
size_t InputState::quiesce_dictionaries() {
  check_thread();
  quiesced_ = true;
  size_t released = 0;
  for (auto &[id, client] : clients_) {
    (void)id;
    if (!client.session)
      continue;
    // Drop the session, which releases both the Engine's open dictionary
    // files and the shared access lock they were taken under. Its destructor
    // performs the same teardown a disconnect would.
    client.session.reset();
    ++released;
  }
  // Nothing owns a focus any more, so no stale lease can be acknowledged.
  gate_.invalidate_all();
  return released;
}
size_t InputState::resume_dictionaries() {
  check_thread();
  quiesced_ = false;
  size_t rebuilt = 0;
  for (auto &[id, client] : clients_) {
    if (client.session)
      continue;
    try {
      client.session = std::make_unique<FocusedSession>(gate_, id, options_);
      ++rebuilt;
    } catch (...) {
      // A client whose session cannot be rebuilt is left without one. It
      // behaves exactly as an unknown client until it reconnects, rather than
      // taking the Server down over one failed rebuild.
    }
  }
  return rebuilt;
}
bool InputState::deactivate_terminal(uint64_t client, uint64_t token) {
  check_thread();
  if (!client || !token)
    return false;
  auto found = clients_.find(client);
  // The token is an authenticated identity, not a best-effort hint. An
  // unknown client (or a client whose session was already quiesced) must not
  // receive an OK: a delayed Aux request could otherwise acknowledge a newer
  // activation that reused the same client id.
  if (!terminal_deactivation_state_available(
          found != clients_.end(),
          found != clients_.end() && found->second.session != nullptr))
    return false;
  return found->second.session->cancel_focus_token(token);
}
bool InputState::passthrough_key(uint64_t client, uint64_t token, uint32_t key_class) {
  check_thread();
  if (!client || !token || quiesced_)
    return false;
  const auto found = clients_.find(client);
  if (found == clients_.end() || !found->second.session)
    return false;
  return found->second.session->passthrough_key(token, key_class);
}
void InputState::cleanup(const FocusRoute &route) {
  if (route.cleanup)
    if (auto *owner = session(route.cleanup->transport))
      owner->cancel(*route.cleanup);
}
FocusRoute InputState::connected(const PipeTicket &ticket) {
  check_thread();
  auto result = router_.connected(ticket);
  if (!result.accepted)
    return result;
  cleanup(result);
  auto found = clients_.find(ticket.client);
  if (found != clients_.end())
    found->second.ticket = ticket;
  else {
    try {
      clients_.emplace(ticket.client,
                       Client{ticket, std::make_unique<FocusedSession>(
                                          gate_, ticket.client, options_)});
    } catch (...) {
      router_.disconnected(ticket);
      throw;
    }
  }
  return result;
}
FocusRoute InputState::disconnected(const PipeTicket &ticket) {
  check_thread();
  auto result = router_.disconnected(ticket);
  if (result.accepted) {
    cleanup(result);
    clients_.erase(ticket.client);
  }
  return result;
}
FocusRoute InputState::dispatch(const PipeTicket &ticket,
                                const FanyImeNamedpipeData &packet) {
  check_thread();
  auto result = router_.dispatch(ticket, packet);
  cleanup(result);
  if (result.activation) {
    auto *owner = session(ticket);
    if (!owner || !owner->prepare(result.activation->pending)) {
      cleanup(router_.failed(result.activation->pending));
      return {};
    }
  }
  return result;
}
bool InputState::confirmed(const FocusLease &lease) {
  check_thread();
  if (!router_.confirmed(lease))
    return false;
  if (preferences_) {
    auto *owner = session(lease.transport);
    return owner && owner->queue_preferences(lease, preferences_->serialized());
  }
  return true;
}
bool InputState::reset_cache() {
  check_thread();
  bool all_reset = true;
  for (auto &[id, client] : clients_) {
    (void)id;
    if (!client.session)
      continue;
    if (!client.session->reset_cache())
      all_reset = false;
  }
  return all_reset;
}
FocusRoute InputState::failed(const FocusLease &lease) {
  check_thread();
  auto result = router_.failed(lease);
  cleanup(result);
  return result;
}
std::optional<PendingReply>
InputState::key(const FocusLease &lease, const FanyImeNamedpipeData &packet,
                ReplyPath path, bool uiless,
                std::optional<std::string> local_text) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->key(lease, packet, path, uiless, std::move(local_text))
               : std::nullopt;
}
std::optional<PendingReply>
InputState::navigate(const FocusLease &lease,
                     const FanyImeNamedpipeData &packet,
                     const NavigationBindings &bindings) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->navigate(lease, packet, bindings) : std::nullopt;
}
std::optional<PendingReply> InputState::edit(const FocusLease &lease,
                                             const FanyImeNamedpipeData &packet,
                                             TsfPreeditStyle style) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->edit(lease, packet, style) : std::nullopt;
}
std::optional<PendingReply>
InputState::basic_key(const FocusLease &lease,
                      const FanyImeNamedpipeData &packet, TsfPreeditStyle style,
                      std::optional<std::string> local_text) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->basic_key(lease, packet, style, std::move(local_text))
               : std::nullopt;
}
bool InputState::synchronize_input_mode(const FocusLease &lease,
                                        const FanyImeNamedpipeData &packet) {
  check_thread();
  if (!valid_main_frame(packet, lease.transport.client) ||
      (packet.event_type != FanyImePipeEventType::IMESwitch &&
       packet.event_type != FanyImePipeEventType::PuncSwitch &&
       packet.event_type != FanyImePipeEventType::DoubleSingleByteSwitch &&
       packet.event_type != FanyImePipeEventType::StatusSnapshot &&
       packet.event_type != FanyImePipeEventType::FocusRestored))
    throw std::invalid_argument("Invalid input mode notification");
  auto *owner = session(lease.transport);
  if (!owner)
    return false;
  // Full/half-width is a TSF presentation mode. The Engine does not own this
  // compartment, but the event must still be accepted so ModeMailbox can
  // publish the authoritative host notification to the toolbar.
  if (packet.event_type == FanyImePipeEventType::DoubleSingleByteSwitch)
    return true;
  if (packet.event_type == FanyImePipeEventType::PuncSwitch)
    return owner->set_chinese_punctuation(lease, packet.keycode != 0);
  if (!owner->set_input_enabled(lease, packet.keycode != 0))
    return false;
  if (packet.event_type == FanyImePipeEventType::StatusSnapshot ||
      packet.event_type == FanyImePipeEventType::FocusRestored)
    return owner->set_chinese_punctuation(lease, packet.pinyin_length != 0);
  return true;
}
bool InputState::balance_paired_punctuation(
    const FocusLease &lease, const FanyImeNamedpipeData &packet) {
  check_thread();
  if (!valid_main_frame(packet, lease.transport.client) ||
      packet.event_type !=
          FanyImePipeEventType::PairedPunctuationAutoClosed)
    throw std::invalid_argument("Invalid paired punctuation notification");
  auto *owner = session(lease.transport);
  return owner && owner->balance_paired_punctuation(
                      lease, static_cast<uint8_t>(packet.keycode));
}
bool InputState::delivered(const FocusLease &lease, uint64_t request) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner && owner->confirm(lease, request);
}
std::optional<nlohmann::json>
InputState::dedicated_english(const FocusLease &lease, bool exit) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->dedicated_english(lease, exit) : std::nullopt;
}
std::optional<nlohmann::json>
InputState::set_dedicated_english(const FocusLease &lease, bool enabled) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->set_dedicated_english(lease, enabled) : std::nullopt;
}
bool InputState::cancel_composition(const FocusLease &lease) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner && owner->cancel_composition(lease);
}
HideCandidateDisposition InputState::hide_candidate(const FocusLease &lease) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->hide_candidate(lease)
               : HideCandidateDisposition::Rejected;
}
std::optional<nlohmann::json>
InputState::rerank_settled(const FocusLease &lease) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->rerank_settled(lease) : std::nullopt;
}
std::optional<nlohmann::json>
InputState::apply_cloud_response(const FocusLease &lease,
                                 const std::string &query,
                                 const std::string &body) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->apply_cloud_response(lease, query, body) : std::nullopt;
}
std::optional<nlohmann::json>
InputState::apply_ai_candidates(const FocusLease &lease,
                                const std::string &query,
                                const std::string &candidates) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->apply_ai_candidates(lease, query, candidates)
               : std::nullopt;
}
std::optional<std::string>
InputState::translation_query(const FocusLease &lease) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->translation_query(lease) : std::nullopt;
}
std::optional<std::pair<FocusLease, std::string>>
InputState::current_translation_request() {
  check_thread();
  const auto lease = gate_.active();
  if (!lease)
    return std::nullopt;
  auto query = translation_query(*lease);
  return query ? std::optional<std::pair<FocusLease, std::string>>(
                     std::in_place, *lease, std::move(*query))
               : std::nullopt;
}
std::optional<nlohmann::json>
InputState::apply_translations(const FocusLease &lease, uint64_t generation,
                               const std::string &translations) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->apply_translations(lease, generation, translations)
               : std::nullopt;
}
std::optional<nlohmann::json>
InputState::update_preferences(const FocusLease &lease,
                               const std::string &snapshot) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->update_preferences(lease, snapshot) : std::nullopt;
}
std::optional<nlohmann::json>
InputState::page_candidate(const FocusLease &lease, uint64_t session,
                           uint64_t generation, bool previous, unsigned steps,
                           bool from_wheel) {
  check_thread();
  if (!candidate_page_allowed(from_wheel, navigation_.mouse_wheel))
    return std::nullopt;
  auto *owner = this->session(lease.transport);
  return owner ? owner->page_candidate(lease, session, generation, previous,
                                       steps)
               : std::nullopt;
}
void InputState::publish_preferences(const PreferenceSnapshot &snapshot) {
  check_thread();
  if (preferences_ && (snapshot.revision() < preferences_->revision() ||
                       (snapshot.revision() == preferences_->revision() &&
                        snapshot.serialized() != preferences_->serialized())))
    throw std::invalid_argument("Stale or conflicting published preferences");
  const auto document =
      nlohmann::json::parse(snapshot.serialized()).at("preferences");
  const auto navigation = preference_navigation(document);
  const auto word = preference_word_character(document);
  const auto style = preference_tsf_preedit_style(document);
  const bool character_set_shortcut =
      document.value("keybindings", nlohmann::json::object())
          .value("toggle_character_set_ctrl_shift_f", true);
  preferences_ = snapshot;
  navigation_ = navigation;
  word_character_ = word;
  tsf_preedit_style_ = style;
  character_set_shortcut_enabled_ = character_set_shortcut;
  for (auto &[id, client] : clients_) {
    (void)id;
    if (client.session)
      client.session->queue_current_preferences(snapshot.serialized());
  }
}
bool InputState::queue_preferences(const FocusLease &lease,
                                   const std::string &snapshot) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner && owner->queue_preferences(lease, snapshot);
}

InputQueue::InputQueue(FocusGate &gate, size_t clients, size_t capacity,
                       std::string options)
    : capacity_(capacity) {
  if (!capacity || capacity > 4096)
    throw std::invalid_argument("Invalid input queue capacity");
  worker_ = std::thread(&InputQueue::run, this, std::ref(gate), clients,
                        std::move(options));
  std::unique_lock lock(mutex_);
  ready_.wait(lock, [&] { return started_; });
  if (startup_failed_) {
    lock.unlock();
    worker_.join();
    throw std::runtime_error("Input queue initialization failed");
  }
}
InputQueue::~InputQueue() { stop(); }
std::optional<std::future<InputTaskStatus>> InputQueue::submit(Task task) {
  if (!task)
    return std::nullopt;
  Job job{std::move(task), {}, std::chrono::steady_clock::now()};
  auto completion = job.completion.get_future();
  {
    std::lock_guard lock(mutex_);
    if (stopping_ || jobs_.size() >= capacity_)
      return std::nullopt;
    jobs_.push_back(std::move(job));
  }
  ready_.notify_one();
  return completion;
}
void InputQueue::request_stop() {
  {
    std::lock_guard lock(mutex_);
    stopping_ = true;
    stats_.accepting = false;
  }
  ready_.notify_one();
}
void InputQueue::stop() {
  // Test before the join lock: a task must not deadlock with external stop().
  if (active_queue == this)
    throw std::logic_error("Input worker cannot join itself");
  std::lock_guard lock(stop_mutex_);
  request_stop();
  if (worker_.joinable())
    worker_.join();
}
InputQueueStats InputQueue::stats() const {
  std::lock_guard lock(mutex_);
  auto result = stats_;
  result.queued = jobs_.size();
  return result;
}
bool InputQueue::on_worker_thread() const noexcept {
  return active_queue == this;
}
std::chrono::milliseconds InputQueue::current_task_wait() const noexcept {
  return on_worker_thread() ? active_task_wait_ : std::chrono::milliseconds(0);
}
void InputQueue::run(FocusGate &gate, size_t clients, std::string options) {
  WorkerScope scope(this);
  try {
    InputState state(gate, clients, std::move(options));
    {
      std::lock_guard lock(mutex_);
      stats_.accepting = true;
      started_ = true;
    }
    ready_.notify_all();
    for (;;) {
      Job job;
      std::deque<Job> cancelled;
      {
        std::unique_lock lock(mutex_);
        ready_.wait(lock, [&] { return stopping_ || !jobs_.empty(); });
        if (stopping_) {
          cancelled.swap(jobs_);
          stats_.cancelled += cancelled.size();
        } else {
          job = std::move(jobs_.front());
          jobs_.pop_front();
          stats_.active = true;
        }
      }
      if (!job.task) {
        for (auto &pending : cancelled)
          pending.completion.set_value(InputTaskStatus::Cancelled);
        break;
      }
      auto status = InputTaskStatus::Completed;
      active_task_wait_ = std::chrono::duration_cast<std::chrono::milliseconds>(
          std::chrono::steady_clock::now() - job.enqueued_at);
      try {
        job.task(state);
      } catch (...) {
        // No raw exception text/input escapes the queue. Stop before any later
        // task can act on partially advanced Engine or routing state.
        status = InputTaskStatus::Failed;
        state.shutdown();
        request_stop();
      }
      {
        std::lock_guard lock(mutex_);
        stats_.active = false;
        if (status == InputTaskStatus::Completed)
          ++stats_.completed;
        else
          ++stats_.failed;
      }
      job.completion.set_value(status);
      active_task_wait_ = std::chrono::milliseconds(0);
    }
  } catch (...) {
    std::deque<Job> cancelled;
    {
      std::lock_guard lock(mutex_);
      startup_failed_ = !started_;
      started_ = true;
      stopping_ = true;
      stats_.accepting = false;
      stats_.active = false;
      cancelled.swap(jobs_);
      stats_.cancelled += cancelled.size();
    }
    for (auto &pending : cancelled)
      pending.completion.set_value(InputTaskStatus::Cancelled);
    ready_.notify_all();
  }
}
std::optional<PendingReply> InputState::configured_key(
    const FocusLease &lease, const FanyImeNamedpipeData &packet,
    TsfPreeditStyle style, const NavigationBindings &bindings,
    std::optional<std::string> local_text, WordCharacterBinding word_binding) {
  check_thread();
  auto *owner = session(lease.transport);
  return owner ? owner->configured_key(lease, packet, style, bindings,
                                       std::move(local_text), word_binding)
               : std::nullopt;
}
} // namespace msime::windows
