#include "SessionPump.h"
#include "InternalEventFlags.h"
#include "ipc_negotiation.h"
#include "PipeMetadata.h"
#include "preference_transition.h"
#include <cstring>
#include <string>

namespace msime::windows {
namespace {
// 被拒按键的诊断行：只有标识、校验名和计数，见 RequestRejected。
std::string rejected_key_line(uint64_t client, uint64_t request, const RequestRejected &error) {
  std::string line = "Request rejected: kind=key client=" + std::to_string(client) +
                     " request=" + std::to_string(request) + " check=" + error.check();
  if (!error.detail().empty())
    line += " " + error.detail();
  return line + "; client connection reset";
}
} // namespace
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
      std::unique_lock transaction(*transactions_);
      if (!valid_main_frame(*packet, ticket.client))
        return PumpResult::DispatchFailed;
      if (packet->event_type == FanyImePipeEventType::KeyEvent &&
          (packet->modifiers_down & PipeMetadata::GroupedPanelKey) &&
          (!(ticket.capabilities & FanyImeProtocol::GroupedPanelKeys) ||
           PipeMetadata::key_modifiers(packet->modifiers_down) != 0 ||
           packet->wch < 'a' || packet->wch > 'z' ||
           packet->keycode != static_cast<std::uint32_t>(packet->wch - 'a' + 'A')))
        return PumpResult::DispatchFailed;
      FocusRoute route;
      uint64_t current_preference_revision = 0;
      if (!enqueue([&, ticket, packet = *packet](InputState &state) {
            if (transport_.current(ticket)) {
              route = state.dispatch(ticket, packet);
              current_preference_revision = state.preference_revision();
            }
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
        const auto send_ready = [&] {
          attempted = true;
          // 新焦点先接收修订要求，再接收可输入的焦点回执，重连不继承旧缓存分类。
          if (ticket.capabilities & FanyImeProtocol::OrderedPreferences) {
            const auto frame = FanyImePreferencePipe::encode({current_preference_revision, 0, 0, 0});
            std::vector<uint8_t> revision_bytes(sizeof(*frame));
            std::memcpy(revision_bytes.data(), &*frame, sizeof(*frame));
            if (transport_.send(ticket, FanyImePipeRole::ToTsfWorkerThread, revision_bytes) != KeyEventSendResult::Sent)
              return false;
          }
          return transport_.send(ticket, FanyImePipeRole::ToTsfWorkerThread, *bytes) == KeyEventSendResult::Sent;
        };
        if (route.activation) sent = focus_.acknowledge(*route.route, send_ready);
        else focus_.with_active(*route.route, [&] { sent = send_ready(); });
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
                if (disposition == HideCandidateDisposition::PredictionKept)
                  delivered_packet.modifiers_down |= internal_prediction_hide;
              }
              if (route.route &&
                  packet.event_type == FanyImePipeEventType::ShowCandidateWnd &&
                  (packet.modifiers_down & FanyImePipeFlags::UiLess) == 0)
                state.show_candidate(*route.route);
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
      std::optional<RequestRejected> rejected;
      if (!enqueue(
              [&, lease = *route.route, packet = *packet](InputState &state) {
                if (!transport_.current(ticket))
                  return;
                // 只接住边界校验的拒绝：它在改动 Engine 之前抛出，队列状态完好。其他异常照旧由输入队列停机处理。
                try {
                  reply = key_(state, lease, packet);
                } catch (const RequestRejected &error) {
                  rejected = error;
                }
              }))
        return PumpResult::QueueUnavailable;
      if (rejected) {
        if (presentation_.rejected)
          presentation_.rejected(rejected_key_line(ticket.client, packet->request_id, *rejected));
        return PumpResult::RequestRejected;
      }
      if (!focus_.with_active(*route.route, [] {}))
        continue;
      if (!reply || reply->source.client_id != ticket.client ||
          reply->source.activation_epoch != route.route->epoch ||
          reply->source.request_id != packet->request_id ||
          (reply->encoded &&
           reply->encoded->packet.request_id != packet->request_id))
        return PumpResult::DispatchFailed;
      std::uint64_t preference_revision = 0;
      if (reply->preference_change.valid()) {
        // 文件锁等待只占本管道线程，停止、断连和焦点失效均能中止等待。
        transaction.unlock();
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
        while (reply->preference_change.wait_for(std::chrono::milliseconds(10)) != std::future_status::ready) {
          if (!input_.stats().accepting || !transport_.current(ticket)) return PumpResult::Disconnected;
          if (!focus_.with_active(*route.route, [] {})) break;
          if (std::chrono::steady_clock::now() >= deadline) return PumpResult::DispatchFailed;
        }
        transaction.lock();
        if (!focus_.with_active(*route.route, [] {})) continue;
        const auto saved = reply->preference_change.get();
        if (saved && !enqueue([&, value = *saved](InputState &state) {
              preference_revision = state.publish_preferences_at_least(value).revision();
            })) return PumpResult::QueueUnavailable;
      }
      if ((ticket.capabilities & FanyImeProtocol::OrderedPreferences) &&
          FanyImeProtocol::IsJapaneseToggleShortcut(packet->keycode, packet->modifiers_down)) {
        const auto frame = FanyImePreferencePipe::encode({preference_revision,
            packet->request_id, route.route->token, route.route->epoch});
        if (!frame) return PumpResult::DispatchFailed;
        std::vector<std::uint8_t> bytes(sizeof(*frame));
        std::memcpy(bytes.data(), &*frame, sizeof(*frame));
        bool sent = false;
        const bool eligible = focus_.with_active(*route.route, [&] {
          sent = transport_.send(ticket, FanyImePipeRole::ToTsfWorkerThread, bytes) == KeyEventSendResult::Sent;
        });
        if (!eligible) continue;
        if (!sent) return PumpResult::WriteFailed;
      }
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
