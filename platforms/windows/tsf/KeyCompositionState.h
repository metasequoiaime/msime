#pragma once
#include "HostComposition.h"
#include <algorithm>

namespace msime::tsf {
// 按键分类所用的组字快照；原文和光标使用 UTF-16，匹配 Windows 的 WCHAR。
struct KeyCompositionState {
    std::wstring rawInput;
    std::size_t caret = 0;
    bool unicodeMode = false;
    bool urlMode = false;
    bool phraseActive = false;
};

template<class Host, class ToWide>
KeyCompositionState ReadKeyCompositionState(Host *host, KeyCompositionState legacy, ToWide toWide) {
    if (host && host->valid()) {
        EngineResult current;
        if (!HostView(*host, &current)) return {};
        KeyCompositionState state;
        state.rawInput = toWide(current.view.editing_text);
        state.phraseActive = !current.view.phrase_prefix.empty();
        // 中文方案和网址等本地模式的原文为 ASCII；其他方案沿用按 UTF-16 长度钳制的位置。
        state.caret = (std::min)(current.view.caret, state.rawInput.size());
        state.unicodeMode = current.view.local_mode == "unicode";
        state.urlMode = current.view.local_mode == "url";
        return state;
    }
    legacy.caret = (std::min)(legacy.caret, legacy.rawInput.size());
    return legacy;
}
} // namespace msime::tsf
