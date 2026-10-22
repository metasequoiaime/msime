#pragma once

#include <string>
#include <string_view>

namespace msime::mac {

// 候选徽标的字形选择，纯逻辑，不依赖窗口和会话。
//
// 改动前的 macOS 把 AI 联想候选的来源徽标写死成 🤖，固定排位候选只用蓝色文字、不加角标。
// 这里按 Windows v0.11.1 的 `server/src/window/candidate_view_model.h` 补齐两项：AI 徽标可
// 在 🤖/🐳 之间选，固定排位徽标可开关并选 📎/📌/· 三种字形。返回的字符串带前导空格，
// 直接拼到候选文字后面。

// AI 联想候选的来源徽标。robot 是出厂默认，未知样式同样回退到 robot，手改配置写错不至于
// 让 AI 候选彻底失去标识。
inline std::string candidate_ai_badge(std::string_view style) {
    if (style == "whale") return " \xF0\x9F\x90\xB3"; // 🐳
    return " \xF0\x9F\xA4\x96";                       // 🤖
}

// 固定排位候选的徽标。`show` 为假或样式未知时返回空串，不加角标；白名单与配置层同宽。
inline std::string candidate_fixed_badge(bool show, std::string_view style) {
    if (!show) return {};
    if (style == "pushpin") return " \xF0\x9F\x93\x8C";  // 📌
    if (style == "dot") return " \xC2\xB7";              // ·
    if (style == "paperclip") return " \xF0\x9F\x93\x8E"; // 📎
    return {};
}

} // namespace msime::mac
