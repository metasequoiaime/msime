#include "../../src/voice/VoiceCaptureSelection.h"
#include <cassert>

int main() {
  using namespace msime::windows;
  const auto defaults = voice_capture_selection(nlohmann::json::object());
  assert(defaults.supported() && defaults.device_id.empty());
  // 后端名不分大小写，与设置页的「麦克风」下拉一致。
  for (const auto *backend : {"", "auto", "windows", "Windows", "AUTO"}) {
    const auto selected = voice_capture_selection({{"capture_backend", backend},
                                                  {"capture_device", "wasapi:0061"}});
    assert(selected.supported() && selected.device_id == "wasapi:0061");
  }
  // 别的平台写下的后端连同它的设备一起当作没选：用系统默认设备录音，而不是拒绝录音。
  for (const auto *backend : {"pulse", "pipewire", "alsa", "macos", "unknown"}) {
    const auto selected = voice_capture_selection({{"capture_backend", backend},
                                                  {"capture_device", "alsa_input.synthetic"}});
    assert(selected.supported() && selected.backend.empty() && selected.device_id.empty());
  }
  assert(windows_capture_backend("WINDOWS") && !windows_capture_backend("pulse"));
  // Do not reinterpret an old ordinal as a new endpoint or silently clear it.
  // The Engine rejects this id before opening a device.
  assert(voice_capture_selection({{"capture_device", "0"}}).device_id == "0");
  assert(!voice_capture_selection({{"capture_device", "wasapi\n0061"}}).supported());
  assert(!voice_capture_selection({{"capture_device", std::string(513, 'x')}}).supported());
}
