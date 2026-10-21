#include "../../src/core/CandidateBadgePolicy.h"

#include <cassert>
#include <string>

// 候选徽标的字形选择：AI 联想来源徽标的两档和固定排位徽标的开关与三档样式。
// 返回串带前导空格，未知样式按 Windows 的行为回退（AI → 🤖，固定位 → 不画）。
int main() {
    using msime::mac::candidate_ai_badge;
    using msime::mac::candidate_fixed_badge;

    // AI 联想：robot 是出厂默认，whale 是另一档；未知样式回退到 robot。
    assert(candidate_ai_badge("robot") == " \xF0\x9F\xA4\x96");
    assert(candidate_ai_badge("whale") == " \xF0\x9F\x90\xB3");
    assert(candidate_ai_badge("dolphin") == " \xF0\x9F\xA4\x96");
    assert(candidate_ai_badge("") == " \xF0\x9F\xA4\x96");

    // 固定排位：paperclip 是出厂默认，另有图钉和圆点。
    assert(candidate_fixed_badge(true, "paperclip") == " \xF0\x9F\x93\x8E");
    assert(candidate_fixed_badge(true, "pushpin") == " \xF0\x9F\x93\x8C");
    assert(candidate_fixed_badge(true, "dot") == " \xC2\xB7");

    // 关掉开关时不画，未知样式同样不画（与 Windows 的白名单同宽）。
    assert(candidate_fixed_badge(false, "paperclip").empty());
    assert(candidate_fixed_badge(true, "star").empty());
    assert(candidate_fixed_badge(true, "").empty());
    return 0;
}
