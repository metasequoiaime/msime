#pragma once
#include "EngineSessionAdapter.h"
#include <cstdint>

namespace msime::tsf {
// 宿主会话的 view 是不是在组字：暂存词段、预编辑、读音任一非空，或者列着组字候选。上屏后联想（View.prediction）也列着候选，但它没有组字，不能开 TSF 组字，也不能让空格、回车或失焦把它上屏。
inline bool HostViewComposes(const EngineView &view) {
    return !view.phrase_prefix.empty() || !view.preedit.empty() || !view.editing_text.empty() ||
           (!view.candidates.empty() && !view.prediction);
}

// 宿主会话停在上屏后联想、有可以显示的候选。
inline bool HostShowsPrediction(const EngineView &view) {
    return view.prediction && view.phrase_prefix.empty() && view.preedit.empty() && view.editing_text.empty() &&
           !view.candidates.empty();
}

// 联想显示着时按下一个键要怎么办。六个平台同一条规则：联想只是建议，只有点选候选才上屏；Esc 关掉联想，键本身吃掉；空格、回车、数字、标点和其他键先关掉联想，再照常处理（交给应用或开始组字）；单按的修饰键什么都不动。
enum class PredictionKeyAction {
    // 联想没显示，或者是单按的修饰键：照常处理。
    None,
    // Esc：关掉联想，键吃掉，应用收不到。
    Close,
    // 其他键：关掉联想，键照常处理。
    Dismiss,
};

// virtualKey 是 TSF 交来的虚拟键码，modifiers 是线上的修饰键位（Shift=1、Control=2、Alt=4）。带修饰键的 Esc 属于应用（比如 Ctrl+Esc 打开开始菜单），不当作关闭联想，但联想照样关掉。
inline PredictionKeyAction PredictionKey(bool visible, unsigned virtualKey, unsigned modifiers) {
    if (!visible) return PredictionKeyAction::None;
    switch (virtualKey) {
    case 0x10: // VK_SHIFT
    case 0x11: // VK_CONTROL
    case 0x12: // VK_MENU
    case 0x5B: // VK_LWIN
    case 0x5C: // VK_RWIN
    case 0xA0: // VK_LSHIFT
    case 0xA1: // VK_RSHIFT
    case 0xA2: // VK_LCONTROL
    case 0xA3: // VK_RCONTROL
    case 0xA4: // VK_LMENU
    case 0xA5: // VK_RMENU
        return PredictionKeyAction::None;
    default:
        break;
    }
    if (virtualKey == 0x1B && (modifiers & 0b111u) == 0) return PredictionKeyAction::Close;
    return PredictionKeyAction::Dismiss;
}

// 一次编辑会话结束后，光标处的联想该怎么办。
enum class PredictionRefresh {
    // 什么都不动：没有联想，也没有显示着的联想。
    None,
    // 在光标处显示这一代联想（新出来的，或者换了一代）。
    Show,
    // 显示着的就是这一代，不重新定位：光标没动，挪了的话 OnEndEdit 已经关掉它。
    Keep,
    // 关掉显示着的联想。
    Hide,
};

// `shownGeneration` 是正显示的联想代次，0 表示没有显示；`composing` 是 TSF 组字还开着；`showable` 为 false 时（UI-less 宿主自己画候选，或者 Server 连不上）不在光标处显示联想。
inline PredictionRefresh DecidePredictionRefresh(uint64_t shownGeneration, bool composing, bool showable,
                                                 const EngineView &host) {
    const bool shown = shownGeneration != 0;
    if (composing || !showable || !HostShowsPrediction(host))
        return shown ? PredictionRefresh::Hide : PredictionRefresh::None;
    if (shown && host.generation == shownGeneration) return PredictionRefresh::Keep;
    return PredictionRefresh::Show;
}
} // namespace msime::tsf
