#include "SessionPump.h"
#include "InternalEventFlags.h"
#include "ipc_negotiation.h"

namespace msime::windows {
SessionPump::SessionPump(MainTransport &transport, InputQueue &input,
                         FocusGate &focus, KeyHandler key, EventHandler event,
                         Presentation presentation,
                         std::shared_ptr<std::mutex> transactions)
    : transport_(transport), input_(input), focus_(focus), key_(std::move(key)),
      event_(std::move(event)), presentation_(std::move(presentation)),
      transactions_(std::move(transactions)) {
  if (!key_ || !event_ || !transactions_)
    throw std::invalid_argument("Missing Windows dispatch handlers");
}
bool SessionPump::enqueue(InputQueue::Task task) {
  auto completion = input_.submit(std::move(task));
  return completion && completion->get() == InputTaskStatus::Completed;
}
void SessionPump::cleanup(const PipeTicket &ticket) noexcept {
  focus_.invalidate(ticket);
  transport_.close(ticket);
  try {
    if (enqueue([this, ticket](InputState &state) {
          state.disconnected(ticket);
          if (presentation_.disconnected)
            presentation_.disconnected(ticket);
        }))
      return;
  } catch (...) {
  }
  // No room for mandatory cleanup: stop the shared worker so it releases all
  // thread-local sessions. The owner must then stop other transport pumps too.
  input_.request_stop();
}
PumpResult SessionPump::run(const PipeTicket &ticket) {
  if (input_.on_worker_thread())
    return PumpResult::QueueUnavailable;
  struct Cleanup {
    SessionPump &pump;
    const PipeTicket &ticket;
    ~Cleanup() { pump.cleanup(ticket); }
  } cleanup_guard{*this, ticket};
  try {
    bool connected = false;
    if (!enqueue([&, ticket](InputState &state) {
          if (transport_.current(ticket))
            connected = state.connected(ticket).accepted;
        }))
      return PumpResult::QueueUnavailable;
    if (!connected)
      return PumpResult::Disconnected;
    bool activation_uiless = false;
    while (auto packet = transport_.read(ticket)) {
      // Never hold this while waiting for client input. Serialize the whole
      // prepare/send/confirm transaction with externally requested selections.
      std::lock_guard transaction(*transactions_);
      if (!valid_main_frame(*packet, ticket.client))
        return PumpResult::DispatchFailed;
      FocusRoute route;
      if (!enqueue([&, ticket, packet = *packet](InputState &state) {
            if (transport_.current(ticket))
              route = state.dispatch(ticket, packet);
          }))
        return PumpResult::QueueUnavailable;
      if (!route.accepted)
        continue;
      // Bound to this registered Main stream, not shared between clients.
      // Rejected/background notifications cannot alter its activation mode.
      if (packet->event_type == FanyImePipeEventType::ClientActivated)
        activation_uiless = packet->keycode != 0;
      else if (FanyImePipeEventType::IsRouteDeactivation(packet->event_type))
        activation_uiless = false;
      if (activation_uiless &&
          (packet->event_type == FanyImePipeEventType::KeyEvent ||
           packet->event_type == FanyImePipeEventType::ShowCandidateWnd ||
           packet->event_type == FanyImePipeEventType::MoveCandidateWnd ||
           packet->event_type == FanyImePipeEventType::HideCandidateWnd))
        packet->modifiers_down |= FanyImePipeFlags::UiLess;
      if (packet->event_type == FanyImePipeEventType::ClientHello)
        continue;
      // 字段属性记在这个客户端上，不经过焦点，也没有回复。管道按序投递，所以它先于 TIP 紧接着发的按键或状态快照生效。
      if (packet->event_type == FanyImePipeEventType::FieldTraits) {
        if (!enqueue([&, ticket, packet = *packet](InputState &state) {
              if (transport_.current(ticket))
                (void)state.set_field_traits(ticket, packet);
            }))
          return PumpResult::QueueUnavailable;
        continue;
      }
      if (route.route && route.fence) {
        const auto bytes =
            focus_ready_bytes(route.route->transport.client, route.route->epoch,
                              route.route->token);
        if (!bytes)
          return PumpResult::DispatchFailed;
        bool sent = false;
        bool attempted = false;
        if (route.activation) {
          sent = focus_.acknowledge(*route.route, [&] {
            attempted = true;
            return transport_.send(ticket, FanyImePipeRole::ToTsfWorkerThread,
                                   *bytes) == KeyEventSendResult::Sent;
          });
        } else {
          focus_.with_active(*route.route, [&] {
            attempted = true;
            sent = transport_.send(ticket, FanyImePipeRole::ToTsfWorkerThread,
                                   *bytes) == KeyEventSendResult::Sent;
          });
        }
        if (!attempted)
          continue; // Ordinary focus displacement is not a broken transport.
        if (!sent)
          return PumpResult::WriteFailed;
        bool confirmed = false;
        if (!enqueue([&, lease = *route.route](InputState &state) {
              confirmed = state.confirmed(lease);
            }))
          return PumpResult::QueueUnavailable;
        if (!confirmed)
          continue;
      }
      if (packet->event_type != FanyImePipeEventType::KeyEvent) {
        bool handled = false;
        bool eligible = false;
        if (!enqueue([&, route, packet = *packet](InputState &state) {
              if (!transport_.current(ticket))
                return;
              auto delivered_packet = packet;
              if (route.route &&
                  packet.event_type == FanyImePipeEventType::HideCandidateWnd) {
                const auto disposition = state.hide_candidate(*route.route);
                if (disposition == HideCandidateDisposition::Rejected)
                  return;
                if (disposition == HideCandidateDisposition::Suppressed)
                  delivered_packet.modifiers_down |= internal_continuation_hide;
              }
              if (route.route &&
                  (packet.event_type == FanyImePipeEventType::IMESwitch ||
                   packet.event_type == FanyImePipeEventType::PuncSwitch ||
                   packet.event_type ==
                       FanyImePipeEventType::DoubleSingleByteSwitch ||
                   packet.event_type == FanyImePipeEventType::StatusSnapshot ||
                   packet.event_type == FanyImePipeEventType::FocusRestored) &&
                  !state.synchronize_input_mode(*route.route, packet))
                return;
              if (route.route &&
                  packet.event_type ==
                      FanyImePipeEventType::PairedPunctuationAutoClosed &&
                  !state.balance_paired_punctuation(*route.route, packet))
                return;
              if (delivered_packet.event_type ==
                      FanyImePipeEventType::HideCandidateWnd &&
                  input_.current_task_wait() >= std::chrono::milliseconds(24))
                delivered_packet.modifiers_down |= internal_late_event;
              if (route.route)
                eligible = focus_.with_active(*route.route, [&] {
                  handled = event_(route, delivered_packet);
                });
              else {
                eligible = true;
                handled = event_(route, delivered_packet);
              }
            }))
          return PumpResult::QueueUnavailable;
        if (!eligible)
          continue;
        if (!handled)
          return PumpResult::DispatchFailed;
        continue;
      }
      if (!route.route)
        return PumpResult::DispatchFailed;
      if (presentation_.before_key &&
          packet->event_type == FanyImePipeEventType::KeyEvent)
        presentation_.before_key(*route.route, *packet);
      std::optional<PendingReply> reply;
      if (!enqueue(
              [&, lease = *route.route, packet = *packet](InputState &state) {
                if (transport_.current(ticket))
                  reply = key_(state, lease, packet);
              }))
        return PumpResult::QueueUnavailable;
      if (!focus_.with_active(*route.route, [] {}))
        continue;
      if (!reply || reply->source.client_id != ticket.client ||
          reply->source.activation_epoch != route.route->epoch ||
          reply->source.request_id != packet->request_id ||
          (reply->encoded &&
           reply->encoded->packet.request_id != packet->request_id))
        return PumpResult::DispatchFailed;
      if (reply->worker) {
        bool sent = false;
        const bool eligible = focus_.with_active(*route.route, [&] {
          sent = transport_.send(ticket,
                                 FanyImePipeRole::ToTsfWorkerThread,
                                 *reply->worker) == KeyEventSendResult::Sent;
        });
        if (!eligible)
          continue;
        if (!sent)
          return PumpResult::WriteFailed;
      }
      if (reply->encoded) {
        // 本地 TIP 的 UI-less 候选用本地身份；只回送导航意图，不能用 Server 页面替换它。
        const bool local_navigation = reply->host_navigation &&
            (ticket.capabilities & FanyImeProtocol::HostOnlineCandidates) &&
            (packet->modifiers_down & FanyImePipeFlags::UiLess) &&
            !(packet->modifiers_down & FanyImePipeFlags::ServerUiLessCandidates);
        const auto encoded = local_navigation ? navigation_reply(reply->source.request_id, *reply->host_navigation)
                                              : *reply->encoded;
        const auto bytes = wire_bytes(encoded);
        if (!bytes)
          return PumpResult::DispatchFailed;
        bool sent = false;
        const bool eligible = focus_.with_active(*route.route, [&] {
          sent = transport_.send(
                     ticket, FanyImePipeRole::ToTsf,
                     std::vector<uint8_t>(bytes->begin(), bytes->end())) ==
                 KeyEventSendResult::Sent;
        });
        if (!eligible)
          continue;
        if (!sent)
          return PumpResult::WriteFailed;
      }
      bool delivered = false;
      if (!enqueue([&, lease = *route.route,
                    request = reply->source.request_id](InputState &state) {
            delivered = state.delivered(lease, request);
            if (delivered) {
              if (presentation_.delivered)
                focus_.with_active(lease, [&] {
                  if (transport_.current(ticket))
                    presentation_.delivered(lease, *reply, *packet);
                });
              if (reply->online_query && presentation_.online)
                presentation_.online(lease, *reply);
              if (reply->translation_query && presentation_.translation)
                presentation_.translation(lease, *reply);
            }
          }))
        return PumpResult::QueueUnavailable;
      if (!delivered && focus_.with_active(*route.route, [] {}))
        return PumpResult::DispatchFailed;
    }
    return PumpResult::Disconnected;
  } catch (...) {
    // No input/path-bearing diagnostics and no retry after uncertain output.
    return PumpResult::DispatchFailed;
  }
}
} // namespace msime::windows
