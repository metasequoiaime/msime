#pragma once

#include "WaveOverlayModel.h"

#include <cmath>
#include <cstddef>
#include <cstdint>
#include <string>

namespace msime::linux_host {

// The voice status as one or two lines of text, for the hosts' auxiliary-text fallback when no overlay can be drawn (GNOME Wayland has no layer-shell). IBus and Fcitx5 both show exactly this, so the fallback reads the same under either framework: the status (or the locked hint, or the failure message), a ten-cell microphone meter while recording, and on the next line the newest 160 characters of the transcript behind "…" when older text was left out.
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
    // The transcript is valid UTF-8 (set_transcript keeps it so), so skipping lead bytes lands on character boundaries.
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
