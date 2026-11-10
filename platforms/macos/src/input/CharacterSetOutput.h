#pragma once

#include "InputSchemeTraits.h"

namespace msime::mac::character_set {

// 粤拼是原生繁体方案，简体输出使用独立偏好；其他方案继续使用全局繁体输出偏好。
constexpr bool follows_cantonese(int scheme) {
    return scheme == scheme::Cantonese;
}

constexpr bool checked(int scheme, bool traditional_output, bool cantonese_simplified_output) {
    return follows_cantonese(scheme) ? cantonese_simplified_output : traditional_output;
}

constexpr const char *label(int scheme) {
    return follows_cantonese(scheme) ? "简体输出" : "繁体输出";
}

} // namespace msime::mac::character_set
