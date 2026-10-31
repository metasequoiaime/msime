#pragma once

#include "../../../common/Telemetry.h"

namespace msime::linux_host {

// 独立工具与输入法使用同一版本的遥测目录；显式偏好目录只用于确实以它为操作对象的工具。
std::optional<telemetry::Host> auxiliary_crash_host(
    const std::string &component, const std::filesystem::path &preferences = {},
    const std::filesystem::path &options = {});

class AuxiliaryCrashCapture final : public telemetry::CaptureOnly {
public:
  explicit AuxiliaryCrashCapture(std::string component,
                                 std::filesystem::path preferences = {});
};

} // namespace msime::linux_host
