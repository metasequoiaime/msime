#include "../src/overlay/WaveOverlayModel.h"
#include "../src/overlay/WaveOverlayIbusSurface.h"
#include "../src/overlay/WaveOverlaySurfaceFactory.h"
#include "../src/voice/VoiceAction.h"
#include <cassert>
#include <cstdlib>

namespace {

using msime::linux_host::WaveOverlayLayout;
using msime::linux_host::WaveOverlayModel;

void check_layouts() {
  using msime::linux_host::wave_overlay_layout;
  using msime::linux_host::wave_overlay_size;
  // The macOS precedence: actions over a label over a transcript, and the four sizes.
  assert(wave_overlay_layout(true, true, true) == WaveOverlayLayout::Action);
  assert(wave_overlay_layout(false, true, true) == WaveOverlayLayout::Processing);
  assert(wave_overlay_layout(false, false, true) == WaveOverlayLayout::Transcript);
  assert(wave_overlay_layout(false, false, false) == WaveOverlayLayout::Compact);
  assert(wave_overlay_size(WaveOverlayLayout::Compact).width == 78 && wave_overlay_size(WaveOverlayLayout::Compact).height == 32);
  assert(wave_overlay_size(WaveOverlayLayout::Processing).width == 112 && wave_overlay_size(WaveOverlayLayout::Processing).height == 40);
  assert(wave_overlay_size(WaveOverlayLayout::Action).width == 142 && wave_overlay_size(WaveOverlayLayout::Action).height == 40);
  assert(wave_overlay_size(WaveOverlayLayout::Transcript).width == 420 && wave_overlay_size(WaveOverlayLayout::Transcript).height == 112);
  assert(msime::linux_host::wave_overlay_corner_radius(40, true, false, false) == 20);
  assert(msime::linux_host::wave_overlay_corner_radius(40, false, true, false) == 20);
  assert(msime::linux_host::wave_overlay_corner_radius(112, false, false, true) == 16);
  assert(msime::linux_host::wave_overlay_corner_radius(32, false, false, false) == 10);
}

void check_presentation_rules() {
  WaveOverlayModel model;
  model.listening = true;
  // A plain hold shows the bare compact wave: no actions, no label.
  assert(!model.actions_visible() && !model.label_visible() && model.animates());
  assert(model.layout() == WaveOverlayLayout::Compact);
  model.set_transcript("你好");
  assert(model.layout() == WaveOverlayLayout::Transcript);
  // Space-locking it brings the actions, which win over the transcript.
  model.locked = true;
  assert(model.actions_visible() && model.layout() == WaveOverlayLayout::Action && model.animates());
  // Recognition keeps the actions and puts the macOS label in place of the wave.
  model.locked = false;
  model.listening = false;
  model.compact_status = WaveOverlayModel::CompactStatus::Recognizing;
  assert(model.actions_visible() && model.label_visible() && !model.animates());
  assert(model.label() == "识别中...");
  model.compact_status = WaveOverlayModel::CompactStatus::Processing;
  assert(model.label() == "处理中...");
  // A failure shows only its message: no actions, no transcript, no wave.
  model.reset();
  model.failure = true;
  model.status = "未识别到文字，请重新录音";
  model.set_transcript("残留");
  assert(!model.actions_visible() && model.label_visible() && !model.has_transcript() && !model.animates());
  assert(model.label() == model.status);

  // Hit testing follows the actions: only the two circles 18 px in from each end of the pill take the pointer.
  WaveOverlayModel locked;
  locked.listening = true;
  locked.locked = true;
  WaveOverlayModel::Action action = WaveOverlayModel::Action::Confirm;
  assert(msime::linux_host::wave_overlay_hit_action(locked, 18, 20, 142, 40, action) &&
         action == WaveOverlayModel::Action::Cancel);
  assert(msime::linux_host::wave_overlay_hit_action(locked, 124, 20, 142, 40, action) &&
         action == WaveOverlayModel::Action::Confirm);
  assert(!msime::linux_host::wave_overlay_hit_action(locked, 71, 20, 142, 40, action));
  locked.locked = false;
  assert(!msime::linux_host::wave_overlay_hit_action(locked, 18, 20, 142, 40, action));
}

void check_wave_motion() {
  WaveOverlayModel model;
  model.listening = true;
  // Silence keeps every bar a dot.
  model.set_input_level(0.0f);
  for (int frame = 0; frame < 120; ++frame) model.advance(1000.0 + frame * 0.016);
  for (float level : model.levels) assert(level < 0.06f);
  // Speech raises the bars, peaked toward the centre, and keeps them within 0...1.
  model.set_input_level(1.0f);
  float edge = 0.0f, centre = 0.0f;
  for (int frame = 0; frame < 240; ++frame) {
    model.advance(2000.0 + frame * 0.016);
    for (float level : model.levels) assert(level >= 0.0f && level <= 1.0f);
    edge += model.levels.front() + model.levels.back();
    centre += model.levels[5] + model.levels[6];
  }
  assert(centre > edge);
  bool raised = false;
  for (float level : model.levels) raised = raised || level >= 0.06f;
  assert(raised);
  // Input beyond the range is clamped, and the bars settle back to dots once listening stops.
  model.set_input_level(5.0f);
  assert(model.input_level == 1.0f);
  model.set_input_level(-1.0f);
  assert(model.input_level == 0.0f);
  model.listening = false;
  model.set_input_level(1.0f);
  for (int frame = 0; frame < 240; ++frame) model.advance(3000.0 + frame * 0.016);
  for (float level : model.levels) assert(level < 0.06f);
  // The motion is deterministic for the same clock.
  WaveOverlayModel first, second;
  first.listening = second.listening = true;
  first.set_input_level(0.7f);
  second.set_input_level(0.7f);
  first.advance(12.5);
  second.advance(12.5);
  assert(first.levels == second.levels);

  // The wave's geometry: the compact pill spreads twelve bars across its width, the action pill keeps them between the circles.
  const auto compact = msime::linux_host::wave_overlay_wave(model.levels, 78, 32, false, false);
  assert(compact.center_y == 16 && compact.bars.front().x > 0 && compact.bars.back().x < 78);
  const auto action = msime::linux_host::wave_overlay_wave(model.levels, 142, 40, true, false);
  assert(action.bars.front().x > 30 && action.bars.back().x < 112);
  const auto transcript = msime::linux_host::wave_overlay_wave(model.levels, 420, 112, false, true);
  assert(transcript.center_y == 18);
}

void check_transcript_window() {
  WaveOverlayModel model;
  model.set_transcript(std::string(200, 'a'));
  assert(model.transcript.size() == 200 && !model.transcript_truncated);
  model.set_transcript(std::string(3000, 'a'));
  assert(model.transcript.size() == msime::linux_host::kWaveOverlayTranscriptWindow && model.transcript_truncated);
  std::string han;
  for (int i = 0; i < 2100; ++i) han += "你";
  model.set_transcript(han);
  assert(model.transcript.size() == msime::linux_host::kWaveOverlayTranscriptWindow * 3);
  model.set_transcript("ok\xf0\x28\x8c\x28\xe0\x80\xaf\xe5\xb0\xbe");
  assert(model.transcript == "ok((尾" && !model.transcript_truncated);
  model.set_transcript("\xed\xa0\x80\xf4\x90\x80\x80valid");
  assert(model.transcript == "valid");

  // The newest text that fits: here a line holds four bytes.
  const std::string text = "abcdefghij";
  std::vector<std::size_t> cuts;
  for (std::size_t offset = 0; offset <= text.size(); ++offset) cuts.push_back(offset);
  const auto lines = [](const std::string &candidate) { return (candidate.size() + 3) / 4; };
  assert(msime::linux_host::wave_overlay_visible_transcript(text, cuts, false, 3, lines) == text);
  const auto newest = msime::linux_host::wave_overlay_visible_transcript(text, cuts, false, 2, lines);
  assert(newest.rfind("…", 0) == 0 && lines(newest) <= 2);
  assert(newest.size() >= 3 && newest.compare(newest.size() - 3, 3, "hij") == 0);
  // Dropped text always shows as "…", even when the rest fits.
  assert(msime::linux_host::wave_overlay_visible_transcript(text, cuts, true, 5, lines) == "…" + text);
}

void check_feedback_text() {
  WaveOverlayModel model;
  model.status = msime::linux_host::kVoiceStatusRecording;
  model.listening = true;
  model.set_input_level(1.0f);
  const auto feedback = msime::linux_host::wave_overlay_feedback_text(model);
  assert(feedback.find("正在录音…") == 0);
  assert(feedback.find("麦克风 [▰▰▰▰▰▰▰▰▰▰]") != std::string::npos);
  model.set_input_level(0.3f);
  assert(msime::linux_host::wave_overlay_feedback_text(model).find("[▰▰▰▱▱▱▱▱▱▱]") != std::string::npos);
  model.transcript = "第一行\n第二行";
  const auto sanitized = msime::linux_host::wave_overlay_feedback_text(model);
  assert(sanitized.find("\n第一行 第二行") != std::string::npos);
  model.set_transcript(std::string(200, 'x'));
  const auto tail = msime::linux_host::wave_overlay_feedback_text(model);
  assert(tail.find("\n…" + std::string(160, 'x')) != std::string::npos);
  model.locked = true;
  const auto locked = msime::linux_host::wave_overlay_feedback_text(model);
  assert(locked.find(msime::linux_host::kVoiceStatusLockedHint) == 0);
  assert(locked.find("麦克风 [") == std::string::npos);
  model.reset();
  model.failure = true;
  model.status = "结束录音失败，本次语音已取消，请检查语音服务后重试";
  assert(msime::linux_host::wave_overlay_feedback_text(model) == model.status);
}

}  // namespace

int main() {
  check_layouts();
  check_presentation_rules();
  check_wave_motion();
  check_transcript_window();
  check_feedback_text();
  unsetenv("MSIME_WAVE_OVERLAY_BACKEND");
  unsetenv("DISPLAY");
  unsetenv("WAYLAND_DISPLAY");
  const auto surface = msime::linux_host::create_wave_overlay_surface(nullptr);
  assert(surface);
  // The text fallback neither animates nor has a connection to watch.
  assert(!surface->animates() && surface->event_fd() == -1);
  assert(msime_voice_stream_inline_enabled(true, "doubao"));
  assert(msime_voice_stream_inline_enabled(true, "doubao", "tsf"));
  assert(msime_voice_stream_inline_enabled(true, "doubao", ""));
  assert(!msime_voice_stream_inline_enabled(true, "doubao", "sendinput"));
  assert(!msime_voice_stream_inline_enabled(true, "doubao", "ctrl_v"));
  assert(!msime_voice_stream_inline_enabled(true, "openai"));
  assert(msime_voice_stream_inline_enabled(true, "local"));
  assert(!msime_voice_stream_inline_enabled(true, "local", "sendinput"));
  assert(!msime_voice_stream_inline_enabled(false, "local"));
  assert(!msime_voice_stream_inline_enabled(false, "doubao"));
  assert(!msime_voice_overlay_light_theme("follow", "dark", false));
  assert(msime_voice_overlay_light_theme("follow", "light", true));
  assert(!msime_voice_overlay_light_theme("follow", "system", true));
  assert(msime_voice_overlay_light_theme("follow", "system", false));
  assert(msime_voice_overlay_light_theme("light", "dark", true));
  assert(!msime_voice_overlay_light_theme("dark", "light", false));
  WaveOverlayModel model;
  model.light_theme = true;
  model.palette = msime::linux_host::FloatingSurfaceColors{0x112233u, 0xEEEEEEu, 0x3584E4u, std::nullopt};
  model.failure = true;
  model.reset();
  assert(model.light_theme && !model.failure);
  // The theme's palette survives a reset as the mode does: both belong to the preferences, not to one recording.
  assert(model.palette && model.palette->surface == 0x112233u && !model.palette->border);
}
