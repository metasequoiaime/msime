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

// 已读出的宿主视图同时用于文字和光标，绘制时无需再次查询会话。
template<class ToWide>
KeyCompositionState KeyCompositionStateFromView(const EngineView &view, ToWide toWide) {
    KeyCompositionState state;
    state.rawInput = toWide(view.editing_text);
    state.phraseActive = !view.phrase_prefix.empty();
    // 中文方案和网址等本地模式的原文为 ASCII；其他方案沿用按 UTF-16 长度钳制的位置。
    state.caret = (std::min)(view.caret, state.rawInput.size());
    state.unicodeMode = view.local_mode == "unicode";
    state.urlMode = view.local_mode == "url";
    return state;
}

template<class Host, class ToWide>
KeyCompositionState ReadKeyCompositionState(Host *host, KeyCompositionState legacy, ToWide toWide) {
    if (host && host->valid()) {
        EngineResult current;
        if (!HostView(*host, &current)) return {};
        return KeyCompositionStateFromView(current.view, toWide);
    }
    legacy.caret = (std::min)(legacy.caret, legacy.rawInput.size());
    return legacy;
}

// 回车原文观测与实际提交使用同一宿主会话；有效宿主读失败时不能改用另一份旧组字。
template<class Host, class ToWide>
std::wstring ReadLocalCommitObservation(Host *host, std::wstring legacy, ToWide toWide) {
    if (host && host->valid()) {
        EngineResult current;
        if (!HostView(*host, &current)) return {};
        return toWide(current.view.phrase_prefix + current.view.editing_text);
    }
    return legacy;
}
} // namespace msime::tsf
