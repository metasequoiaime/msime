#pragma once

#include "WaveOverlayModel.h"

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <string>

namespace msime::linux_host {

// 语音状态的一到两行文字，用于画不出浮层时宿主的辅助文本回退（GNOME Wayland 没有 layer-shell）。IBus 和 Fcitx5 显示的正是这段文字，两种框架下的回退因此一致：状态（或锁定提示，或失败消息），录音时附一个十格的麦克风音量条，下一行是转写文字最新的 160 个字符，省掉了更早的文字时前面带「…」。
inline std::string wave_overlay_feedback_text(const WaveOverlayModel &model) {
  std::string feedback;
  if (model.failure)
    feedback = model.status;
  else if (model.locked && model.listening)
    feedback = kVoiceStatusLockedHint;
  else if (!model.status.empty())
    feedback = model.status;
  else
    feedback = model.processing() ? model.label() : kVoiceStatusRecording;
  if (model.listening && !model.locked && !model.failure) {
    const auto lit = static_cast<std::size_t>(std::lround(model.input_level * 10.0f));
    feedback += "  麦克风 [";
    for (std::size_t index = 0; index < 10; ++index) feedback += index < lit ? "▰" : "▱";
    feedback += "]";
  }
  if (model.has_transcript()) {
    constexpr std::size_t kPreviewCharacters = 160;
    const auto &text = model.transcript;
    const auto lead = [](unsigned char byte) { return (byte & 0xc0) != 0x80; };
    std::size_t characters = 0;
    for (const char byte : text)
      if (lead(static_cast<unsigned char>(byte))) ++characters;
    // 转写文字是合法的 UTF-8（`set_transcript` 保证这一点），所以按首字节跳过总是落在字符边界上。
    std::size_t start = 0;
    for (std::size_t skip = characters > kPreviewCharacters ? characters - kPreviewCharacters : 0; skip > 0;) {
      ++start;
      while (start < text.size() && !lead(static_cast<unsigned char>(text[start]))) ++start;
      --skip;
    }
    std::string preview = text.substr(start);
    for (auto &character : preview)
      if (character == '\r' || character == '\n' || character == '\t') character = ' ';
    feedback += "\n";
    if (start > 0 || model.transcript_truncated) feedback += "…";
    feedback += preview;
  }
  return feedback;
}

}  // namespace msime::linux_host
