#include "../src/overlay/WaveOverlayModel.h"
#include "../src/overlay/WaveOverlayIbusSurface.h"
#include "../src/overlay/WaveOverlaySurfaceFactory.h"
#include "../src/voice/VoiceAction.h"
#include <cassert>
#include <cstdlib>

int main() {
  msime::linux_host::WaveOverlayModel model;
  model.set_input_level(2.0f);
  for (float level : model.levels) assert(level >= 0.0f && level <= 1.0f);
  model.set_input_level(-1.0f);
  for (float level : model.levels) assert(level == 0.0f);
  model.set_transcript(std::string(200, 'a'));
  assert(model.transcript.size() == 160);
  std::string han;
  for (int i = 0; i < 200; ++i) han += "你";
  model.set_transcript(han);
  assert(model.transcript.size() == 160 * 3);
  model.set_transcript("ok\xf0\x28\x8c\x28\xe0\x80\xaf\xe5\xb0\xbe");
  assert(model.transcript == "ok((尾");
  model.set_transcript("\xed\xa0\x80\xf4\x90\x80\x80valid");
  assert(model.transcript == "valid");
  model.status = "正在识别…";
  model.listening = true;
  model.set_input_level(1.0f);
  const auto feedback = msime::linux_host::wave_overlay_feedback_text(model);
  assert(feedback.find("正在识别…") == 0);
  assert(feedback.find("麦克风 [") != std::string::npos);
  model.transcript = "第一行\n第二行";
  const auto sanitized = msime::linux_host::wave_overlay_feedback_text(model);
  assert(sanitized.find("第一行 第二行") != std::string::npos);
  model.locked = true;
  const auto locked = msime::linux_host::wave_overlay_feedback_text(model);
  assert(locked.find("录音已锁定") == 0);
  assert(locked.find("麦克风 [") == std::string::npos);
  unsetenv("MSIME_WAVE_OVERLAY_BACKEND");
  unsetenv("DISPLAY");
  unsetenv("WAYLAND_DISPLAY");
  assert(msime::linux_host::create_wave_overlay_surface(nullptr));
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
  model.light_theme = true;
  model.palette = msime::linux_host::FloatingSurfaceColors{0x112233u, 0xEEEEEEu, 0x3584E4u, std::nullopt};
  model.reset();
  assert(model.light_theme);
  // The theme's palette survives a reset as the mode does: both belong to the preferences, not to one recording.
  assert(model.palette && model.palette->surface == 0x112233u && !model.palette->border);
}
