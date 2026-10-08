#include "MainFrame.h"
#include "PipeMetadata.h"
#include "InternalEventFlags.h"
#include <stdexcept>

using namespace msime::windows;
// GameHost 是包元数据，不是按键修饰键：key_modifiers 必须剥掉它，并且它不能和其他元数据位、Server 内部标志重叠。
static_assert(PipeMetadata::key_modifiers(PipeMetadata::GameHost) == 0);
static_assert((PipeMetadata::GameHost &
               (FanyImePipeFlags::UiLess | PipeMetadata::CandidateActive |
                PipeMetadata::AutoRepeat)) == 0);
static_assert((PipeMetadata::GameHost &
               (internal_late_event | internal_continuation_hide)) == 0);
static_assert(PipeMetadata::key_modifiers(PipeMetadata::GameHost | 7u) == 7u);
void require(bool value) {
  if (!value)
    throw std::runtime_error("Main frame validation failed");
}
int main() {
  using namespace FanyImePipeEventType;
  FanyImeNamedpipeData packet{};
  packet.client_id = 42;
  packet.request_id = 1;
  for (auto event : {KeyEvent, HideCandidateWnd, ShowCandidateWnd,
                     MoveCandidateWnd, IMESwitch, PuncSwitch,
                     DoubleSingleByteSwitch, ClientHello, ClientActivated,
                     ClientDeactivated, ClientSuspended, StatusSnapshot,
                     FocusRestored}) {
    packet.event_type = event;
    require(valid_main_frame(packet, 42));
    require(!valid_main_frame(packet, 43));
    require(!valid_main_frame(packet, 0));
  }
  for (auto event : {LangbarRightClick, 5u, 6u, 17u, UINT32_MAX}) {
    packet.event_type = event;
    require(!valid_main_frame(packet, 42));
  }
  packet.event_type = KeyEvent;
  packet.request_id = 0;
  require(!valid_main_frame(packet, 42));
  packet.request_id = FANY_IME_NO_REQUEST_ID;
  require(!valid_main_frame(packet, 42));
  packet.event_type = ClientActivated;
  require(valid_main_frame(packet, 42));
  packet.request_id = 0;
  require(!valid_main_frame(packet, 42));
  packet.request_id = 1;
  packet.modifiers_down = FanyImePipeFlags::UiLess;
  require(valid_main_frame(packet, 42));
  for (auto event :
       {KeyEvent, ShowCandidateWnd, MoveCandidateWnd, HideCandidateWnd}) {
    packet.event_type = event;
    packet.modifiers_down = PipeMetadata::GameHost;
    require(valid_main_frame(packet, 42));
  }
  packet.event_type = ClientActivated;
  packet.modifiers_down = 0;
  for (int length : {-1, 128, INT32_MAX}) {
    packet.pinyin_length = length;
    require(!valid_main_frame(packet, 42));
  }
  packet.pinyin_length = 127;
  require(valid_main_frame(packet, 42));
  packet.pinyin_string[127] = u'x';
  require(!valid_main_frame(packet, 42));
  packet.pinyin_length = 0;
  require(!valid_main_frame(packet, 42));
  packet.pinyin_string[127] = 0;
  packet.pinyin_string[0] = u'x';
  require(!valid_main_frame(packet, 42));
  packet.pinyin_string[0] = 0;
  for (auto event : {StatusSnapshot, FocusRestored}) {
    packet.event_type = event;
    packet.pinyin_length = 1;
    packet.keycode = packet.modifiers_down = 1;
    require(valid_main_frame(packet, 42));
    packet.keycode = 2;
    require(!valid_main_frame(packet, 42));
    packet.keycode = 1;
    packet.modifiers_down = 2;
    require(!valid_main_frame(packet, 42));
    // 这两类包的 modifiers_down 是全角状态，带上 GameHost 就是非法帧，所以 TSF 只在 Key/Show/Move/Hide 包上设它。
    packet.modifiers_down = 1 | PipeMetadata::GameHost;
    require(!valid_main_frame(packet, 42));
    packet.modifiers_down = 1;
    packet.pinyin_length = 2;
    require(!valid_main_frame(packet, 42));
  }
  packet = {};
  packet.client_id = 42;
  packet.request_id = 1;
  packet.event_type = PairedPunctuationAutoClosed;
  packet.keycode = '<';
  require(valid_main_frame(packet, 42));
  require(!valid_main_frame(packet, 43));
  require(!valid_main_frame(packet, 0));
  // Only the book title nests, so any other opening is a malformed notification.
  for (uint32_t keycode : {0u, uint32_t('('), uint32_t('>'), uint32_t(u'《')}) {
    packet.keycode = keycode;
    require(!valid_main_frame(packet, 42));
  }
}
