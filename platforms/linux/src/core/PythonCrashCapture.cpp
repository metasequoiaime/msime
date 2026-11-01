#include "AuxiliaryCrashCapture.h"
#include "../../../common/CrashRecorder.h"

#include <cstring>
#include <memory>
#include <string>

// 只加载安装包内的这份桥；构建身份、同意观察和原生故障写入与其他辅助程序完全相同。
extern "C" void *msime_python_crash_start(const char *component, const char *preferences,
                                         const char *options) noexcept {
  try {
    if (!component)
      return nullptr;
    const std::string name(component);
    if (name != "online-provider" && name != "online-http" && name != "voice-provider" &&
        name != "voice-http" && name != "clipboard-monitor" && name != "clipboard-event" &&
        name != "setup" && name != "provider-session")
      return nullptr;
    const std::filesystem::path directory(preferences ? preferences : "");
    const std::filesystem::path runtime(options ? options : "");
    if ((!directory.empty() && !directory.is_absolute()) ||
        (!runtime.empty() && !runtime.is_absolute()))
      return nullptr;
    return new msime::telemetry::CaptureOnly([name, directory, runtime] {
      return msime::linux_host::auxiliary_crash_host(name, directory, runtime);
    });
  } catch (...) {
    return nullptr;
  }
}

extern "C" void msime_python_crash_stop(void *capture) noexcept {
  delete static_cast<msime::telemetry::CaptureOnly *>(capture);
}

// 只读就绪状态；设备回归据此等待实际观察器完成，不用固定睡眠猜测同意已生效。
extern "C" int msime_python_crash_is_armed() noexcept {
  return msime::crash::is_armed() ? 1 : 0;
}

extern "C" void msime_python_crash_exception(const char *frames) noexcept {
  try {
    if (!frames || std::strlen(frames) > 8192)
      return;
    // Python 只交入产品文件名、函数名和行号。再次拒绝路径分隔符、空格及任何非标识字符。
    for (const unsigned char *item = reinterpret_cast<const unsigned char *>(frames); *item; ++item) {
      if (!((*item >= 'a' && *item <= 'z') || (*item >= 'A' && *item <= 'Z') ||
            (*item >= '0' && *item <= '9') || std::strchr("_-.!:<>\n", *item)))
        return;
    }
    const auto summary = std::string("Python unhandled exception\n") + frames;
    msime::crash::record(summary.c_str());
  } catch (...) {
  }
}
