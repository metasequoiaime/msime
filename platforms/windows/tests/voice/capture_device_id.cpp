#include "CaptureDeviceId.h"

#include <cassert>
#include <cstring>
#include <string>
#include <utility>

// Saved capture_device values are WASAPI endpoint ids in the encoding crates/host-api/src/voice_capture.rs lists for the settings page. A change on either side silently stops opening the microphone a person picked.
int main() {
  using namespace msime::windows;
  ma_device_info devices[2]{};
  devices[0].id.wasapi[0] = 'a';
  devices[1].id.wasapi[0] = 'b';
  std::memcpy(devices[0].name, "Synthetic microphone", sizeof("Synthetic microphone"));
  std::memcpy(devices[1].name, "Synthetic microphone", sizeof("Synthetic microphone"));
  const auto id = wasapi_capture_device_id(devices[0].id);
  assert(id == "wasapi:0061");
  assert(is_wasapi_capture_device_id(id));
  assert(select_capture_device(devices, 2, id) == &devices[0]);
  std::swap(devices[0], devices[1]);
  assert(select_capture_device(devices, 2, id) == &devices[1]);
  assert(!select_capture_device(devices, 1, id));
  assert(!select_capture_device(devices, 2, ""));
  assert(!select_capture_device(devices, 2, "0"));
  devices[0].id = devices[1].id;
  assert(!select_capture_device(devices, 2, id));

  // The same strings the Rust encoder's tests pin: "{0.0.1}" and a character outside the BMP as its two UTF-16 units.
  ma_device_id other{};
  const char16_t endpoint[] = u"{0.0.1}";
  std::memcpy(other.wasapi, endpoint, sizeof(endpoint));
  assert(wasapi_capture_device_id(other) == "wasapi:007b0030002e0030002e0031007d");
  other = {};
  other.wasapi[0] = static_cast<ma_wchar_win32>(0xd83c);
  other.wasapi[1] = static_cast<ma_wchar_win32>(0xdfa4);
  assert(wasapi_capture_device_id(other) == "wasapi:d83cdfa4");
  other = {};
  assert(wasapi_capture_device_id(other).empty());
  for (auto &unit : other.wasapi)
    unit = 'x';
  assert(wasapi_capture_device_id(other).empty());

  for (const auto *invalid : {"0", "wasapi:", "wasapi:001", "wasapi:gggg", "wasapi:00A1", "unknown:00",
                              "coreaudio:4275696c74496e", "alsa:68773a302c30"})
    assert(!is_wasapi_capture_device_id(invalid));
  assert(!is_wasapi_capture_device_id("wasapi:" + std::string(256, '1')));
  assert(is_wasapi_capture_device_id("wasapi:" + std::string(63 * 4, '1')));
  assert(!is_wasapi_capture_device_id("wasapi:" + std::string(64 * 4, '1')));
  return 0;
}
