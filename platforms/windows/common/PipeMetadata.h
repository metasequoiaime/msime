#pragma once

#include "../../../shared/contracts/windows_ipc.h"

namespace msime::windows::PipeMetadata {
// Set by the TSF while its original candidate list is active. This metadata
// is distinct from keyboard modifiers: VK_RETURN has different semantics for
// an original candidate list and an incremental/raw composition.
inline constexpr std::uint32_t CandidateActive = 0x40000000u;
// Set by the TSF on a key-down that is the auto-repeat of a held key (bit 30 of its lParam). The Server still handles the key as it always did; only the typing effect's combo leaves it uncounted.
inline constexpr std::uint32_t AutoRepeat = 0x20000000u;
// TSF 在强制使用我们候选窗的游戏宿主上设这一位，表示这次会话是游戏会话、宿主给的锚点可能不可信。只有协商到 FanyImeProtocol::GameHostCandidate 之后才会设。
// 只能设在 Key/Show/Move/Hide 包上：src/input/ModeMailbox.h 对 StatusSnapshot/FocusRestored 用的是 `modifiers_down != 0` 判全角，src/input/MainFrame.h 要求这两类包的 modifiers_down <= 1，带上这一位会把全角状态读错，或者整帧被判为非法。
inline constexpr std::uint32_t GameHost = 0x10000000u;

inline constexpr std::uint32_t key_modifiers(std::uint32_t value) {
  return value &
         ~(FanyImePipeFlags::UiLess | CandidateActive | AutoRepeat | GameHost);
}
} // namespace msime::windows::PipeMetadata
