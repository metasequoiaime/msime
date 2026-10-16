#pragma once
#include <nlohmann/json.hpp>
#include <algorithm>
#include <cctype>
#include <string>

namespace msime::windows {
// 存着的录音后端是不是这台主机的：空、`auto`、`windows` 都是 WASAPI。不分大小写，与设置页的「麦克风」下拉一致。
inline bool windows_capture_backend(std::string backend) {
  std::transform(backend.begin(), backend.end(), backend.begin(),
                 [](unsigned char ch) { return static_cast<char>(std::tolower(ch)); });
  return backend.empty() || backend == "auto" || backend == "windows";
}
struct VoiceCaptureSelection {
  std::string backend;
  std::string device_id;
  bool supported() const {
    const bool backend_ok = windows_capture_backend(backend);
    const bool device_ok = device_id.size() <= 512 &&
                           std::none_of(device_id.begin(), device_id.end(), [](unsigned char ch) {
                             return std::iscntrl(ch) != 0;
                           });
    return backend_ok && device_ok;
  }
};
// 别的平台写下的后端（从 macOS、Linux 拷来的配置里的 `macos`、`pulse`）这台主机打不开，连同它的设备一起当作没选，用系统默认设备录音，而不是拒绝录音；设置页也把它显示成系统默认设备。存储不改写。
inline VoiceCaptureSelection voice_capture_selection(const nlohmann::json &input) {
  auto backend = input.value("capture_backend", std::string{});
  if (!windows_capture_backend(backend))
    return {};
  return {std::move(backend), input.value("capture_device", std::string{})};
}
} // namespace msime::windows
