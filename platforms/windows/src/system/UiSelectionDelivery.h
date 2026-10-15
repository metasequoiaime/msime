#pragma once
#include "FocusGate.h"
#include "MainTransport.h"
#include "ReplyCodec.h"

namespace msime::windows {
enum class UiDeliveryResult { Stale, Sent, WriteFailed };
// External I/O thread only, after queue-owned selection preparation. A Sent
// result still requires queue-owned confirmation; it is not proof TSF applied
// the text. Never retry uncertain writes or rerun the Engine selection.
inline UiDeliveryResult deliver_ui_selection(MainTransport &transport,
                                             FocusGate &gate,
                                             const FocusLease &lease,
                                             const UiSelectionFrames &frames) {
  bool attempted = false, sent = false;
  try {
    gate.with_active(lease, [&] {
      if (!transport.current(lease.transport))
        return;
      attempted = true;
      if (frames.before_trigger) {
        const auto result = transport.send(
            lease.transport, FanyImePipeRole::ToTsf,
            std::vector<uint8_t>(frames.before_trigger->begin(),
                                 frames.before_trigger->end()));
        if (result != KeyEventSendResult::Sent) return;
      }
      for (const auto &frame : frames.worker_prefix)
        if (transport.send(lease.transport, FanyImePipeRole::ToTsfWorkerThread, frame) != KeyEventSendResult::Sent)
          return;
      sent = transport.send(lease.transport, FanyImePipeRole::ToTsfWorkerThread,
                            frames.worker) == KeyEventSendResult::Sent;
    });
  } catch (...) {
    attempted = true;
  }
  if (!attempted)
    return UiDeliveryResult::Stale;
  if (sent)
    return UiDeliveryResult::Sent;
  gate.invalidate(lease.transport);
  transport.close(lease.transport);
  return UiDeliveryResult::WriteFailed;
}
} // namespace msime::windows
