#pragma once

#include <string>
#include <vector>

namespace msime::mac {

// Vim 模式名单（共享偏好 `vim_mode_apps`）标识的比较：只折叠 ASCII 大小写，与偏好库
// `valid_app_input_mode_rules` 的 `to_ascii_lowercase` 去重、AppearancePreferences 的
// `InputModeRuleIdentifiersEqual` 同一口径。macOS 的标识是前台应用的 bundle id；
// ASCII 字母在 UTF-8 里是单字节，逐字节折叠与逐 UTF-16 码元折叠结果一致。
inline bool VimModeIdentifierEqual(const std::string &left, const std::string &right) {
    if (left.size() != right.size()) return false;
    for (std::size_t index = 0; index < left.size(); ++index) {
        char a = left[index], b = right[index];
        if (a >= 'A' && a <= 'Z') a = static_cast<char>(a - 'A' + 'a');
        if (b >= 'A' && b <= 'Z') b = static_cast<char>(b - 'A' + 'a');
        if (a != b) return false;
    }
    return true;
}

// 名单是否命中这个前台应用标识；空标识一律不命中。
inline bool VimModeListContains(const std::vector<std::string> &apps, const std::string &identifier) {
    if (identifier.empty()) return false;
    for (const auto &app : apps)
        if (VimModeIdentifierEqual(app, identifier)) return true;
    return false;
}

// 镜像 Windows 的 `VimMode::ShouldSwitchToEnglish`：交给应用的无修饰 Esc 是否搭车把水杉切到英文。
// Windows 的 `eaten` 与 `deferredKeysPending` 两个入参在这里没有对应物：Esc 会不会被引擎吃掉由
// `compositionActive` 代答——组字三项（`editing_text` / `candidates` / `phrase_prefix`）全空时
// MSIME_CANCEL 无事可取消，这次 Esc 才确定会原样回到应用；在飞按键由调用方到达判定之前的各早退
// 分支（展开候选网格、语音确认等）覆盖。Command 并入 `modifierHeld`，对应 Windows 检查的 Win 键。
inline bool VimModeEscapeSwitchesToEnglish(unsigned short keyCode, bool repeat, bool modifierHeld,
                                           bool compositionActive, bool listed, bool englishMode) {
    return keyCode == 53 && !repeat && !modifierHeld && !compositionActive && listed && !englishMode;
}

}  // namespace msime::mac
