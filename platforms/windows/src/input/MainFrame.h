#pragma once
#include "windows_ipc.h"

namespace msime::windows {
// Established Main stream only. Registration/negotiation remains separate.
// This validates shape and the pinned identity, NOT foreground ownership.
inline bool valid_main_frame(const FanyImeNamedpipeData &packet,
                             uint64_t client) {
  if (!client || packet.client_id != client || packet.pinyin_length < 0 ||
      packet.pinyin_length >= 128 || packet.pinyin_string[127] != 0 ||
      packet.pinyin_string[packet.pinyin_length] != 0)
    return false;
  using namespace FanyImePipeEventType;
  switch (packet.event_type) {
  case KeyEvent:
    return packet.request_id != 0 &&
           packet.request_id != FANY_IME_NO_REQUEST_ID;
  case ClientActivated:
    // This is a focus token, not a key correlation id; UINT64_MAX is valid.
    return packet.request_id != 0;
  case StatusSnapshot:
  case FocusRestored:
    return packet.keycode <= 1 && packet.modifiers_down <= 1 &&
           packet.pinyin_length <= 1;
  case PairedPunctuationAutoClosed:
    // '<' is the only paired punctuation with nesting state to pay back.
    return packet.keycode == '<';
  case FieldTraits:
    // 只有 TIP 会报的那几位；request_id 不用，和其他通知一样随意。
    return (packet.keycode & ~FanyImeFieldTraits::Reported) == 0;
  case HideCandidateWnd:
  case ShowCandidateWnd:
  case MoveCandidateWnd:
  case IMESwitch:
  case PuncSwitch:
  case DoubleSingleByteSwitch:
  case ClientHello: // A repeated hello is ignored by the lifecycle dispatcher.
  case ClientDeactivated:
  case ClientSuspended:
    return true;
  default: // In particular, Aux-only LangbarRightClick is not a Main event.
    return false;
  }
}
} // namespace msime::windows
