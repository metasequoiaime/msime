#pragma once

#include <cstdint>

// A key this tip does not consume is inserted by the host itself, so half-width digits, the symbols outside the punctuation table and English-mode letters never reach a commit path and were never counted. This rule decides which of those keys count; it stays pure so it can be tested without a TSF host.
//
// The count is a key-time prediction, not an edit confirmation: a read-only document or an application shortcut still counts. That approximation is the reference's product decision; the filter only drops keys that certainly are not a character the user typed.
inline bool ShouldCountPassthroughChar(wchar_t wch, bool keyboardDisabled, bool ctrlDown, bool altDown, bool winDown)
{
    // Shift stays allowed on purpose: it is what makes uppercase letters and the shifted symbol row their own characters. Functional and dead keys produce no printable character, so the range check keeps Enter, Tab and Backspace out; 0x7F is DEL. A lone surrogate cannot be classified and would be rejected by the Server's UTF-8 conversion anyway.
    return !keyboardDisabled && !ctrlDown && !altDown && !winDown && wch >= 0x20 && wch != 0x7F &&
           !(wch >= 0xD800 && wch <= 0xDFFF);
}

// 设置应用的表情/符号、手写、剪贴板和语音面板用 SendInput 注入文字时，在 dwExtraInfo 里带上这个标记（crates/host-windows/src/send_input_marker.rs，两处数值必须一致）；Server 语音输入的 SendInput 上屏（VoiceInputSession.cpp）也带它。注入方已按来源记过这段文字，tip 再把直通的字符记一次就重复了，所以带这个标记的按键不进直通统计；按键怎么处理不受影响。数值与 MetasequoiaIME.h 里 0x4D53505x 一族的自注入标记相邻但不在 IsSelfGeneratedSendInputExtraInfo 里。
constexpr std::uintptr_t PanelTextSendInputExtraInfo = 0x4D535053u;

inline bool IsPanelTextSendInput(std::uintptr_t extraInfo)
{
    return extraInfo == PanelTextSendInputExtraInfo;
}
