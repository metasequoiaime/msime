#include "SessionController.h"
#include "input/CandidateUiActionPolicy.h"
#include "CandidateRenderSync.h"
#include "ReplyCodec.h"
#include "UiSelectionDelivery.h"
#include <thread>

namespace msime::windows {
namespace {
thread_local const SessionController *active_controller = nullptr;
// 外部工作线程等事务锁，直到 deadline：锁被按键、候选点击或 250 毫秒一次的英文模式读取占着时不把请求丢掉。每 5 毫秒重试一次；Server 停止时立即放弃。
std::unique_lock<std::mutex> wait_transaction(std::mutex &transactions,
                                              const std::atomic<bool> &stopping,
                                              std::chrono::steady_clock::time_point deadline) {
  std::unique_lock transaction(transactions, std::try_to_lock);
  while (!transaction.owns_lock() && !stopping.load() &&
         std::chrono::steady_clock::now() < deadline) {
    std::this_thread::sleep_for(std::chrono::milliseconds(5));
    (void)transaction.try_lock();
  }
  return transaction;
}
}
SessionController::SessionController(
    MainTransport &transport, RegistrationInbox &inbox, size_t clients,
    size_t input_capacity, std::string options, SessionPump::KeyHandler key,
    SessionPump::EventHandler event, std::function<bool()> healthy,
    std::function<void()> stop_service, std::chrono::milliseconds interval,
    std::string preferences_directory, SessionPump::Presentation presentation,
    PreferenceMonitor::Published published)
    : inbox_(inbox), transport_(transport), healthy_(std::move(healthy)),
      stop_service_(std::move(stop_service)), interval_(interval),
      presentation_(std::move(presentation)), event_(std::move(event)),
      input_(focus_, clients, input_capacity, std::move(options)),
      cloud_([this](CloudCandidateWorker::Result result) {
        try {
          (void)input_.submit(
              [this, result = std::move(result)](InputState &state) mutable {
                if (stopping_ || !transport_.current(result.lease.transport))
                  return;
                auto view = state.apply_cloud_response(
                    result.lease, result.query, result.body);
                if (view) {
                  candidates_.online(result.lease, *view);
                  if (auto query = state.translation_query(result.lease))
                    (void)translations_.submit(result.lease, std::move(*query));
                }
              });
        } catch (...) {
          // Optional provider delivery must never stop the input queue.
        }
      }),
      ai_([this](AiCandidateWorker::Result result) {
        try {
          (void)input_.submit(
              [this, result = std::move(result)](InputState &state) mutable {
                if (stopping_ || !transport_.current(result.lease.transport))
                  return;
                // The worker returns plain strings; the Engine wants the JSON
                // array its apply entry point documents.
                nlohmann::json candidates = nlohmann::json::array();
                for (auto &text : result.candidates)
                  candidates.push_back(std::move(text));
                auto view = state.apply_ai_candidates(
                    result.lease, result.query, candidates.dump());
                if (view) {
                  candidates_.online(result.lease, *view);
                  // AI insertion changes the visible candidate generation just
                  // like cloud insertion. Re-query translations for the new
                  // page immediately so the AI row and its neighbors do not
                  // remain unannotated until the next key event.
                  if (auto query = state.translation_query(result.lease))
                    (void)translations_.submit(result.lease, std::move(*query));
                }
              });
        } catch (...) {
          // Optional provider delivery must never stop the input queue.
        }
      }),
      translations_([this](TranslationWorker::Result result) {
        try {
          (void)input_.submit(
              [this, result = std::move(result)](InputState &state) mutable {
                if (stopping_ || !transport_.current(result.lease.transport))
                  return;
                // 只带读音和拆解、没有释义的结果（TranslationWorker 交的是空列表）不交给会话：空列表会把会话里已有的释义清掉。
                std::optional<nlohmann::json> view;
                if (result.translations != "[]")
                  view = state.apply_translations(
                      result.lease, result.generation, result.translations);
                // 读音和逐词拆解只用于显示，不进会话，随释义一起交给候选窗；没有新释义可交时单独交。
                if (view)
                  candidates_.translations(result.lease, *view,
                                           std::move(result.readings));
                else if (!result.readings.empty())
                  candidates_.readings(result.lease, std::move(result.readings));
              });
        } catch (...) {
          // Optional provider delivery must never stop the input queue.
        }
      }),
      workers_(
          transport, input_, focus_, clients, std::move(key),
          [this](const FocusRoute &route, const FanyImeNamedpipeData &packet) {
            if (route.route) {
              candidates_.event(*route.route, packet);
              modes_.event(*route.route, packet);
            }
            return event_(route, packet);
          },
          {[this](const FocusLease &lease, const PendingReply &reply,
                  const FanyImeNamedpipeData &packet) {
             candidates_.delivered(lease, reply, packet);
             if (presentation_.delivered)
               presentation_.delivered(lease, reply, packet);
           },
           [this](const PipeTicket &ticket) {
             candidates_.disconnected(ticket);
             modes_.disconnected(ticket);
             if (presentation_.disconnected)
               presentation_.disconnected(ticket);
           },
           [this](const FocusLease &lease, const PendingReply &reply) {
             // Every delivered reply restarts the settle clock, so the pass
             // only runs when the user has actually stopped, and never between
             // the keystrokes of ordinary typing — which is what keeps the
             // larger model off the keystroke path, where it measures p95
             // 153ms against a 16ms frame.
             settled_pending_ = lease;
             settled_since_ = std::chrono::steady_clock::now();
             if (reply.online_query) {
               (void)cloud_.submit(lease, *reply.online_query);
               // The same query carries the resolved AI config and the
               // ai_eligible flag; the AI worker decides for itself whether it
               // applies, so an ineligible query costs nothing here.
               (void)ai_.submit(lease, *reply.online_query);
             }
           },
           [this](const FocusLease &lease, const PendingReply &reply) {
           if (reply.translation_query)
               (void)translations_.submit(lease, *reply.translation_query);
           },
           [this](const FocusLease &lease, const FanyImeNamedpipeData &packet) {
             wait_candidate_render_for_key(lease, packet);
           }},
          transactions_) {
  if (!event_ || !healthy_ || !stop_service_ || interval.count() < 1 ||
      interval.count() > 1000)
    throw std::invalid_argument("Invalid session supervision configuration");
  if (!preferences_directory.empty())
    preferences_ = std::make_unique<PreferenceMonitor>(
        input_, std::move(preferences_directory), std::chrono::milliseconds(250),
        [this, published = std::move(published)](
            const PreferenceSnapshot &snapshot) mutable {
          if (published)
            published(snapshot);
          if (stopping_)
            return;
          // A settings publication may change translation enablement, target,
          // or provider credentials while a candidate page is already shown.
          // Re-read the now-applied query on the input thread so the worker
          // requests the new signature immediately instead of waiting for the
          // next key event.
          (void)input_.submit([this](InputState &state) {
            if (stopping_)
              return;
            // Provider credentials, endpoint, target language, and the
            // enablement flag are all part of the translation policy. Drop
            // both positive and negative results before asking for the new
            // query, even when the candidate page remains eligible.
            translations_.clear_cache();
            // 读音开关、释义开关或目标语言变了：旧的读音和拆解作废，等这次重新翻译带回新的。
            candidates_.clear_readings();
            if (auto request = state.current_translation_request())
              (void)translations_.submit(request->first,
                                          std::move(request->second));
          });
        });
  control_ = std::thread(&SessionController::run, this);
}
SessionController::~SessionController() { stop(); }
SelectionRequestResult
SessionController::request_selection(const FocusLease &lease, uint64_t session,
                                     uint64_t generation, size_t index) {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error("Selection cannot reenter controller callbacks");
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock())
    return SelectionRequestResult::Busy;
  const auto fail = [&] {
    focus_.invalidate(lease.transport);
    transport_.close(lease.transport);
    failure_ = ControllerFailure::Control;
    request_stop();
    return SelectionRequestResult::Failed;
  };
  try {
    const auto shown = candidate_view();
    if (!shown || !shown->visible || shown->session != session ||
        shown->generation != generation || shown->lease.epoch != lease.epoch ||
        shown->lease.token != lease.token ||
        !same_ticket(shown->lease.transport, lease.transport))
      return SelectionRequestResult::Rejected;
    if (should_wait_for_candidate_render(0, shown->render_serial, false,
                                         shown->visible))
      (void)candidates_.wait_rendered(
          lease, shown->render_serial,
          std::chrono::milliseconds(candidate_render_wait_max_ms));
    // Re-read after the receipt; the page may have changed while painting.
    const auto painted = candidate_view();
    if (!painted || painted->session != session ||
        painted->generation != generation ||
        painted->render_serial < shown->render_serial)
      return SelectionRequestResult::Rejected;
    bool found = false;
    for (const auto &candidate : painted->candidates)
      if (candidate.index == index && candidate.session == session &&
          candidate.generation == generation)
        found = true;
    if (!found)
      return SelectionRequestResult::Rejected;
    std::optional<PendingReply> pending;
    auto prepared = input_.submit([&](InputState &state) {
      if (!stopping_ && transport_.current(lease.transport))
        pending = state.select_candidate(lease, session, generation, index);
    });
    if (!prepared || prepared->get() != InputTaskStatus::Completed)
      return fail();
    if (!pending)
      return SelectionRequestResult::Rejected;
    if (!pending->ui_selection || stopping_ ||
        deliver_ui_selection(transport_, focus_, lease,
                             *pending->ui_selection) != UiDeliveryResult::Sent)
      return fail();
    bool delivered = false;
    auto confirmed = input_.submit([&](InputState &state) {
      if (stopping_)
        return;
      delivered = state.ui_delivered(
          lease, pending->source.transition.at("view").at("generation"));
      if (delivered) {
        const bool active = focus_.with_active(lease, [&] {
          if (!transport_.current(lease.transport)) {
            delivered = false;
            return;
          }
          // UI has no key packet. Preserve the last confirmed screen anchor;
          // id zero and ui_selection distinguish this callback from key input.
          FanyImeNamedpipeData packet{};
          packet.client_id = lease.transport.client;
          packet.event_type = FanyImePipeEventType::KeyEvent;
          packet.point[0] = painted->x;
          packet.point[1] = painted->y;
          // 点选也要保住游戏会话标记，否则点选之后的快照会按普通宿主处理。
          packet.modifiers_down = painted->game_host ? PipeMetadata::GameHost : 0;
          candidates_.delivered(lease, *pending, packet);
          if (presentation_.delivered)
            presentation_.delivered(lease, *pending, packet);
        });
        delivered = delivered && active;
      }
    });
    if (!confirmed || confirmed->get() != InputTaskStatus::Completed ||
        !delivered)
      return fail();
    return SelectionRequestResult::Sent;
  } catch (...) {
    return fail();
  }
}
CandidateActionRequestResult SessionController::request_candidate_action(
    const FocusLease &lease, uint64_t session, uint64_t generation,
    size_t index, CandidateAction action, uint8_t position) {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error(
        "Candidate action cannot reenter controller callbacks");
  if (action == CandidateAction::Select ||
      (action == CandidateAction::FixPosition &&
       (position < 1 || position > 5)) ||
      (action != CandidateAction::FixPosition && position != 0))
    return CandidateActionRequestResult::Rejected;
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock())
    return CandidateActionRequestResult::Busy;
  const auto fail = [&] {
    focus_.invalidate(lease.transport);
    transport_.close(lease.transport);
    failure_ = ControllerFailure::Control;
    request_stop();
    return CandidateActionRequestResult::Failed;
  };
  try {
    const auto shown = candidate_view();
    if (!shown || !shown->visible || shown->session != session ||
        shown->generation != generation || shown->lease.epoch != lease.epoch ||
        shown->lease.token != lease.token ||
        !same_ticket(shown->lease.transport, lease.transport))
      return CandidateActionRequestResult::Rejected;
    if (should_wait_for_candidate_render(0, shown->render_serial, false,
                                         shown->visible))
      (void)candidates_.wait_rendered(
          lease, shown->render_serial,
          std::chrono::milliseconds(candidate_render_wait_max_ms));
    // Re-read after the receipt. A cloud/translation refresh can replace the
    // visible page while the native menu is painting; never apply an action
    // against a candidate list that was not the one most recently rendered.
    const auto painted = candidate_view();
    if (!painted || !painted->visible || painted->session != session ||
        painted->generation != generation ||
        painted->lease.epoch != lease.epoch ||
        painted->lease.token != lease.token ||
        !same_ticket(painted->lease.transport, lease.transport))
      return CandidateActionRequestResult::Rejected;
    if (!candidate_ui_action_matches(painted->candidates, session, generation,
                                     index))
      return CandidateActionRequestResult::Rejected;
    std::optional<nlohmann::json> transition;
    auto prepared = input_.submit([&](InputState &state) {
      if (!stopping_ && transport_.current(lease.transport))
        transition = state.candidate_action(lease, session, generation, index,
                                            action, position);
      if (transition) {
        candidates_.action(lease, transition->at("view"));
        // Candidate pin/remove/fix commands create a fresh Engine generation
        // while keeping the composition visible. Re-submit the translation
        // query for that replacement view just as a key transition does.
        if (auto query = state.translation_query(lease))
          (void)translations_.submit(lease, std::move(*query));
      }
    });
    if (!prepared || prepared->get() != InputTaskStatus::Completed)
      return fail();
    return transition ? CandidateActionRequestResult::Sent
                      : CandidateActionRequestResult::Rejected;
  } catch (...) {
    return fail();
  }
}
CandidatePageRequestResult
SessionController::request_page(const CandidatePage &page) {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error("Candidate paging cannot reenter controller callbacks");
  if (!page.lease.epoch || !page.session || !page.generation ||
      page.steps == 0 || page.steps > 9 || stopping_)
    return CandidatePageRequestResult::Rejected;
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock())
    return CandidatePageRequestResult::Busy;
  const auto fail = [&] {
    focus_.invalidate(page.lease.transport);
    transport_.close(page.lease.transport);
    failure_ = ControllerFailure::Control;
    request_stop();
    return CandidatePageRequestResult::Failed;
  };
  try {
    const auto shown = candidate_view();
    if (!shown || !shown->visible || shown->session != page.session ||
        shown->generation != page.generation ||
        shown->lease.epoch != page.lease.epoch ||
        shown->lease.token != page.lease.token ||
        !same_ticket(shown->lease.transport, page.lease.transport))
      return CandidatePageRequestResult::Rejected;
    if (should_wait_for_candidate_render(0, shown->render_serial, false,
                                         shown->visible))
      (void)candidates_.wait_rendered(
          page.lease, shown->render_serial,
          std::chrono::milliseconds(candidate_render_wait_max_ms));
    // A mouse-wheel/page-button request can outlive the frame that opened the
    // menu. Re-read after the render receipt so a replacement page cannot be
    // mistaken for the still-visible one.
    const auto painted = candidate_view();
    if (!painted || !painted->visible || painted->session != page.session ||
        painted->generation != page.generation ||
        painted->lease.epoch != page.lease.epoch ||
        painted->lease.token != page.lease.token ||
        !same_ticket(painted->lease.transport, page.lease.transport))
      return CandidatePageRequestResult::Rejected;
    std::optional<nlohmann::json> transition;
    auto prepared = input_.submit([&](InputState &state) {
      if (!stopping_ && transport_.current(page.lease.transport))
        transition = state.page_candidate(page.lease, page.session,
                                          page.generation, page.previous,
                                          page.steps, page.from_wheel);
      if (transition) {
        candidates_.action(page.lease, *transition);
        // Paging advances the Engine generation and replaces the visible
        // candidate set. Request translations for the new page immediately;
        // waiting for another key event would leave the page unannotated.
        if (auto query = state.translation_query(page.lease))
          (void)translations_.submit(page.lease, std::move(*query));
      }
    });
    if (!prepared || prepared->get() != InputTaskStatus::Completed)
      return fail();
    return transition ? CandidatePageRequestResult::Sent
                      : CandidatePageRequestResult::Rejected;
  } catch (...) {
    return fail();
  }
}
VoiceCompositionResult SessionController::send_voice_composition(
    const FocusLease &lease, uint32_t message, std::wstring_view text,
    wchar_t generation) {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error("Voice composition cannot reenter controller callbacks");
  const auto frames = voice_composition_bytes(message, text, generation);
  if (!frames)
    return VoiceCompositionResult::Rejected;
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock())
    return VoiceCompositionResult::Busy;
  bool attempted = false;
  bool sent = false;
  try {
    focus_.with_active(lease, [&] {
      if (!transport_.current(lease.transport) || stopping_)
        return;
      attempted = true;
      sent = true;
      for (const auto &frame : *frames) {
        if (transport_.send(lease.transport,
                            FanyImePipeRole::ToTsfWorkerThread, frame) !=
            KeyEventSendResult::Sent) {
          sent = false;
          break;
        }
      }
    });
  } catch (...) {
    attempted = true;
  }
  if (sent)
    return VoiceCompositionResult::Sent;
  if (!attempted)
    return VoiceCompositionResult::Rejected;
  focus_.invalidate(lease.transport);
  transport_.close(lease.transport);
  failure_ = ControllerFailure::Control;
  request_stop();
  return VoiceCompositionResult::Failed;
}
std::optional<CandidatePresentation> SessionController::candidate_view() {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error(
        "Candidate read cannot reenter controller callbacks");
  if (stopping_ || !input_.stats().accepting)
    return std::nullopt;
  auto value = candidates_.snapshot(focus_, false, [&](const FocusLease &lease) {
    return transport_.try_current(lease.transport);
  });
  if (stopping_ || !input_.stats().accepting)
    return std::nullopt;
  return value;
}
void SessionController::wait_candidate_render_for_key(
    const FocusLease &lease, const FanyImeNamedpipeData &packet) {
  if (packet.event_type != FanyImePipeEventType::KeyEvent ||
      !candidate_render_key(packet.keycode))
    return;
  const auto shown = candidate_view();
  if (!shown || !shown->visible ||
      !same_ticket(shown->lease.transport, lease.transport) ||
      shown->lease.epoch != lease.epoch || shown->lease.token != lease.token)
    return;
  (void)candidates_.wait_rendered(
      lease, shown->render_serial,
      std::chrono::milliseconds(candidate_render_wait_max_ms));
}
bool SessionController::mode_active() {
  if (stopping_)
    return false;
  return modes_.active();
}
std::optional<ModePresentation> SessionController::mode_view() {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error("Mode read cannot reenter controller callbacks");
  if (stopping_ || !input_.stats().accepting)
    return std::nullopt;
  auto value = modes_.snapshot(focus_, transport_);
  if (stopping_ || !input_.stats().accepting)
    return std::nullopt;
  return value;
}
std::optional<bool>
SessionController::dedicated_english_state(const FocusLease &lease) {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error("Mode state read cannot reenter controller callbacks");
  if (stopping_) return std::nullopt;
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock()) return std::nullopt;
  std::optional<bool> result;
  try {
    auto read = input_.submit([&](InputState &state) {
      if (stopping_ || !transport_.current(lease.transport)) return;
      if (auto view = state.dedicated_english(lease, false))
        result = view->at("dedicated_english").get<bool>();
    });
    if (!read || read->get() != InputTaskStatus::Completed || stopping_)
      return std::nullopt;
    return result;
  } catch (...) {
    return std::nullopt;
  }
}

bool SessionController::exit_dedicated_english(const FocusLease &lease) {
  if (input_.on_worker_thread() || active_controller == this || stopping_.load())
    throw std::logic_error("Dedicated-English exit cannot reenter controller callbacks");
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock()) return false;
  bool exited = false;
  auto submitted = input_.submit([&](InputState &state) {
    if (stopping_ || !transport_.current(lease.transport)) return;
    exited = state.dedicated_english(lease, true).has_value();
  });
  if (!submitted || submitted->wait_for(std::chrono::seconds(2)) != std::future_status::ready)
    return false;
  return submitted->get() == InputTaskStatus::Completed && exited;
}
bool SessionController::set_dedicated_english(const FocusLease &lease,
                                              bool enabled) {
  if (input_.on_worker_thread() || active_controller == this || stopping_.load())
    throw std::logic_error("Dedicated-English switch cannot reenter controller callbacks");
  // 这是用户在托盘里点的开关，只发一次、没有重试，所以锁忙时等着而不是放弃；等锁和等输入队列共用 2 秒。
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
  const auto transaction = wait_transaction(*transactions_, stopping_, deadline);
  if (!transaction.owns_lock() || stopping_.load()) return false;
  // 等待可能超时，任务之后仍会在输入队列上运行，所以结果和租约放在任务自己持有的存储里，不引用这个栈帧。
  auto applied = std::make_shared<bool>(false);
  auto submitted = input_.submit([this, lease, enabled, applied](InputState &state) {
    if (stopping_ || !transport_.current(lease.transport)) return;
    if (auto view = state.set_dedicated_english(lease, enabled))
      *applied = view->at("dedicated_english").get<bool>() == enabled;
  });
  if (!submitted || submitted->wait_until(deadline) != std::future_status::ready)
    return false;
  return submitted->get() == InputTaskStatus::Completed && *applied;
}
bool SessionController::focus_current(const FocusLease &lease) {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error(
        "Focus validation cannot reenter controller callbacks");
  if (stopping_ || !input_.stats().accepting)
    return false;
  bool current = false;
  focus_.with_active(lease, [&] {
    current = !stopping_ && transport_.current(lease.transport);
  });
  return current && !stopping_;
}
bool SessionController::send_caps_lock(const FocusLease &lease, bool enabled) {
  if (input_.on_worker_thread() || active_controller == this || stopping_)
    return false;
  const auto frame = caps_lock_frame(enabled);
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock())
    return false;
  bool sent = false;
  try {
    focus_.with_active(lease, [&] {
      if (stopping_ || !transport_.current(lease.transport))
        return;
      sent = transport_.send(lease.transport, FanyImePipeRole::ToTsfWorkerThread,
                             frame) == KeyEventSendResult::Sent;
    });
  } catch (...) {
    return false;
  }
  return sent;
}
bool SessionController::reset_cache() {
  if (input_.on_worker_thread() || active_controller == this || stopping_)
    return false;
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock())
    return false;
  auto result = std::make_shared<std::atomic<bool>>(false);
  auto submitted = input_.submit([this, result](InputState &state) {
    if (stopping_)
      return;
    result->store(state.reset_cache(), std::memory_order_release);
  });
  if (!submitted || submitted->wait_for(std::chrono::milliseconds(100)) !=
                         std::future_status::ready)
    return false;
  return submitted->get() == InputTaskStatus::Completed &&
         result->load(std::memory_order_acquire);
}
bool SessionController::send_tsf_config(const TsfLocalConfig &config) {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error("Config push cannot reenter controller callbacks");
  if (stopping_)
    return false;
  const auto tickets = transport_.current_tickets();
  const auto frames = tsf_config_frames(config);
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock())
    return false;
  bool all_sent = true;
  for (const auto &ticket : tickets) {
    for (const auto &frame : frames) {
      bool sent = false;
      try {
        sent = !stopping_ &&
               transport_.send(ticket, FanyImePipeRole::ToTsfWorkerThread,
                               frame) == KeyEventSendResult::Sent;
      } catch (...) {
        sent = false;
      }
      if (!sent) {
        all_sent = false;
        break;
      }
    }
  }
  return all_sent;
}
namespace {
// Long enough for an import of a large personal dictionary, short enough that
// a maintenance process which dies mid-operation cannot leave input broken for
// more than this.
constexpr auto quiesce_budget = std::chrono::seconds(30);
} // namespace
bool SessionController::quiesce_dictionaries() {
  if (stopping_.load())
    return false;
  auto done = std::make_shared<std::atomic<bool>>(false);
  auto submitted = input_.submit([done](InputState &state) {
    state.quiesce_dictionaries();
    done->store(true);
  });
  if (!submitted)
    return false;
  // Tearing down sessions is local work, but it waits behind whatever the
  // queue is already doing, so this is more generous than the Aux deactivate.
  if (submitted->wait_for(std::chrono::seconds(2)) != std::future_status::ready)
    return false;
  if (submitted->get() != InputTaskStatus::Completed || !done->load())
    return false;
  quiesce_deadline_.store(
      (std::chrono::steady_clock::now() + quiesce_budget)
          .time_since_epoch()
          .count());
  return true;
}
bool SessionController::resume_dictionaries() {
  auto done = std::make_shared<std::atomic<bool>>(false);
  auto submitted = input_.submit([done](InputState &state) {
    state.resume_dictionaries();
    done->store(true);
  });
  // Clear the deadline first: a resume that fails to be admitted must not be
  // retried forever by the watchdog on every tick.
  quiesce_deadline_.store(0);
  if (!submitted)
    return false;
  if (submitted->wait_for(std::chrono::seconds(2)) != std::future_status::ready)
    return false;
  return submitted->get() == InputTaskStatus::Completed && done->load();
}
bool SessionController::deactivate_terminal(uint64_t client, uint64_t token) {
  if (!client || !token || stopping_.load())
    return false;
  auto result = std::make_shared<std::atomic<bool>>(false);
  auto submitted = input_.submit([client, token, result](InputState &state) {
    result->store(state.deactivate_terminal(client, token));
  });
  // A full or stopped queue is not a deactivation. Answering anyway would tell
  // the DLL a teardown happened when the request never even ran.
  if (!submitted)
    return false;
  // The DLL polls for at most 150 ms. Waiting the whole budget would leave no
  // room to write the reply, so this settles well inside it and reports
  // failure rather than answering late.
  if (submitted->wait_for(std::chrono::milliseconds(100)) !=
      std::future_status::ready)
    return false;
  return submitted->get() == InputTaskStatus::Completed && result->load();
}
void SessionController::passthrough_key(uint64_t client, uint64_t token, uint32_t key_class) {
  if (!client || !token || stopping_.load())
    return;
  // 返回的 future 来自 promise，丢掉它不会阻塞。
  (void)input_.submit([client, token, key_class](InputState &state) {
    (void)state.passthrough_key(client, token, key_class);
  });
}
ModeRequestResult SessionController::request_mode(const FocusLease &lease,
                                                  WorkerMode mode) {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error("Mode request cannot reenter controller callbacks");
  const auto bytes = worker_mode_bytes(mode);
  if (!bytes || stopping_)
    return ModeRequestResult::Rejected;
  std::unique_lock transaction(*transactions_, std::try_to_lock);
  if (!transaction.owns_lock())
    return ModeRequestResult::Rejected;
  bool attempted = false;
  bool sent = false;
  try {
    focus_.with_active(lease, [&] {
      if (stopping_ || !transport_.current(lease.transport))
        return;
      attempted = true;
      sent = transport_.send(lease.transport,
                             FanyImePipeRole::ToTsfWorkerThread, *bytes) ==
             KeyEventSendResult::Sent;
    });
  } catch (...) {
    // A throwing transport is also uncertain; never retry the mode command.
    attempted = true;
  }
  if (!attempted)
    return ModeRequestResult::Rejected;
  if (sent)
    return ModeRequestResult::Sent;
  focus_.invalidate(lease.transport);
  transport_.close(lease.transport);
  return ModeRequestResult::WriteFailed;
}
void SessionController::request_stop() {
  stopping_ = true;
  cloud_.request_stop();
  ai_.request_stop();
  translations_.request_stop();
  candidates_.stop();
  modes_.stop();
  inbox_.close();
}
void SessionController::stop() {
  if (input_.on_worker_thread() || active_controller == this)
    throw std::logic_error("Controller cannot join a dependent thread");
  std::lock_guard lock(stop_mutex_);
  request_stop();
  if (control_.joinable())
    control_.join();
}
/// How long a composition stands still before the settled model ranks it.
///
/// Shorter than the half second the cloud path waits, because this pass is
/// local and it changes what the user is reading rather than annotating it.
constexpr auto kSettledRerankDelay = std::chrono::milliseconds(150);

void SessionController::run_settled_rerank() {
  if (!settled_pending_ || stopping_)
    return;
  if (std::chrono::steady_clock::now() - settled_since_ < kSettledRerankDelay)
    return;
  const auto lease = *settled_pending_;
  // Cleared before the work is queued: a composition is ranked once, and a
  // keystroke arriving meanwhile sets it again with a fresh clock.
  settled_pending_.reset();
  try {
    (void)input_.submit([this, lease](InputState &state) {
      if (stopping_ || !transport_.current(lease.transport))
        return;
      if (auto view = state.rerank_settled(lease))
        candidates_.online(lease, *view);
    });
  } catch (...) {
    // A reranking pass must never stop the input queue.
  }
}

void SessionController::run() {
  active_controller = this;
  try {
    while (!stopping_) {
      auto ticket = inbox_.take_for(interval_);
      if (stopping_ || inbox_.closed())
        break;
      if (!healthy_()) {
        failure_ = ControllerFailure::Service;
        break;
      }
      if (!input_.stats().accepting) {
        failure_ = ControllerFailure::InputQueue;
        break;
      }
      if (workers_.stats().stopping) {
        failure_ = ControllerFailure::SessionWorkers;
        break;
      }
      if (preferences_ && preferences_->failed()) {
        failure_ = ControllerFailure::Preferences;
        break;
      }
      run_settled_rerank();
      // A maintenance process that died between quiesce and resume would
      // otherwise leave every client without a session for good.
      if (const auto deadline = quiesce_deadline_.load();
          deadline != 0 &&
          std::chrono::steady_clock::now().time_since_epoch().count() >=
              deadline)
        (void)resume_dictionaries();
      if (ticket)
        workers_.submit(*ticket);
    }
  } catch (...) {
    failure_ = ControllerFailure::Control;
  }
  request_stop();
  if (preferences_)
    preferences_->request_stop();
  // Stop intake/listeners and cancel registry reads first. The queue remains
  // live while pumps submit their final session cleanup. Input/handshake
  // callbacks never join.
  try {
    stop_service_();
  } catch (...) {
    failure_ = ControllerFailure::Control;
  }
  cloud_.stop();
  ai_.stop();
  translations_.stop();
  workers_.stop();
  input_.stop();
  if (preferences_)
    preferences_->stop();
  active_controller = nullptr;
}
} // namespace msime::windows
