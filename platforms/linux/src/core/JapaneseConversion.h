#pragma once

#include "../../../../shared/input/JapaneseConversion.h"

namespace msime::linux_host {

// 日语空格「変換」的状态机放在 shared/input/JapaneseConversion.h，Linux 的两个前端和 Windows Server 共用；这里保留原来的名字，调用方不必改。
using JapaneseConversion = msime::input::JapaneseConversion;
using msime::input::kCandidateSourceFallback;

} // namespace msime::linux_host
