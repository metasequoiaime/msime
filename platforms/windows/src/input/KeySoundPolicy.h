#pragma once
#include "KeySoundClass.h"
#include "PipeMetadata.h"
#include "windows_ipc.h"
#include <cstdint>
#include <optional>

namespace msime::windows {
// Server 处理的键交给 msime_client_key_sound 的类别：1 空格、2 回车、3 退格、0 其他键。到达 Server 的键都是打字，例外是 TIP 为取消组字转发的单独修饰键和带 Ctrl、Alt 的快捷键，它们不出声。TIP 交给应用的键经 Aux 管道出声（passthrough_key_sound_class），共用同一张表。
inline std::optional<uint32_t> key_sound_class(const FanyImeNamedpipeData &packet) {
  if (packet.event_type != FanyImePipeEventType::KeyEvent)
    return std::nullopt;
  if (PipeMetadata::key_modifiers(packet.modifiers_down) & ~1u)
    return std::nullopt;
  return key_sound_class_for_virtual_key(packet.keycode);
}
} // namespace msime::windows
