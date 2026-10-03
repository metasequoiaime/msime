#pragma once
#include <cstddef>
#include <string>

namespace Global
{
// The digits and operators of the V mode: crates/engine/src/local/expression.rs SPELLING_SYMBOLS. The Server reads them from View.spelling_symbols, but the TIP classifies a key before the Server answers, so it keeps this copy; scripts/test-windows-expression-symbols-parity.py keeps the two equal.
inline constexpr wchar_t ExpressionSpellingSymbols[] = L"0123456789+-*/.()%^";

// 某个本地模式的符号表里有没有这个字符：V 模式传 ExpressionSpellingSymbols，网址模式传 UrlSpellingSymbols。
inline bool IsSpellingSymbol(const wchar_t *symbols, wchar_t wch)
{
    return wch != L'\0' &&
           std::char_traits<wchar_t>::find(symbols, std::char_traits<wchar_t>::length(symbols), wch) != nullptr;
}

// A V composition: the keystroke buffer starts with the V that opened it, and the Server reports the mode on. With the mode off, Shift+V is an ordinary capital and its digits keep selecting.
inline bool IsExpressionModeComposition(const wchar_t *buffer, size_t length, bool expressionMode)
{
    return expressionMode && buffer && length > 0 && buffer[0] == L'V';
}

enum class ExpressionKey
{
    Unclaimed,
    Input,
    SelectByNumber,
};

// V 组字或网址组字里不带 Ctrl、Alt 的按键，symbols 是该模式的符号表。表里的字符是输入，包括 Shift 打出的符号（V 模式 Shift+8 的 `*`，网址模式 Shift+2 的 `@`、Shift+3 的 `#`）和在别处翻页的 `-` 等键；打出其他字符的数字键（US 布局 V 模式里 Shift+1 的 `!`，或数字行要按 Shift 的布局上的裸数字键）按槽位选词。Server 的 edit_kind 和 digit_selects_candidate（src/input/EditPolicy.h）对同一个键做同样的判断，两边对它是不是选词不会分歧。网址模式里字母照常走组字，不在表里的符号（`"` `<` `>` `\` `{` `}` `|` 和反引号）照常走标点，Engine 先上屏网址再写标点。
inline ExpressionKey ClassifyModeKey(const wchar_t *symbols, unsigned code, wchar_t wch)
{
    if (IsSpellingSymbol(symbols, wch))
    {
        return ExpressionKey::Input;
    }
    if (code >= L'1' && code <= L'9')
    {
        return ExpressionKey::SelectByNumber;
    }
    return ExpressionKey::Unclaimed;
}

// 网址模式当作输入的数字和符号：crates/engine/src/local/url.rs SPELLING_SYMBOLS。Server 从 View.spelling_symbols 读取，TIP 在 Server 回答之前就要给按键分类，所以保留一份拷贝，由 scripts/test-windows-expression-symbols-parity.py 核对两份相同。
inline constexpr wchar_t UrlSpellingSymbols[] = L"0123456789-._~:/?#[]@!$&'()*+,;=%^";

// 网址触发词和紧跟其后进入网址模式的按键：crates/engine/src/local/url.rs TRIGGERS。触发词要与组字原文完全相同，大写不算。
struct UrlTrigger
{
    const wchar_t *word;
    const wchar_t *keys;
};

inline constexpr UrlTrigger UrlTriggers[] = {{L"www", L"."}, {L"http", L":"}, {L"https", L":"}, {L"ftp", L".:"}};

// 这段文字是否恰好是某个触发词，且 wch 是它的触发键。
inline bool IsUrlTriggerAndKey(const wchar_t *text, size_t length, wchar_t wch)
{
    for (const UrlTrigger &trigger : UrlTriggers)
    {
        if (std::char_traits<wchar_t>::length(trigger.word) == length &&
            std::char_traits<wchar_t>::compare(text, trigger.word, length) == 0 &&
            IsSpellingSymbol(trigger.keys, wch))
        {
            return true;
        }
    }
    return false;
}

// 这个键是否在组字中打开网址模式：方案检测网址（scheme::DetectsUrls）、不在 Engine 的专用英文模式、光标在末尾、键击缓冲恰好是触发词、按键是它的触发键。Engine 只在同样的条件下把触发键列进 spelling_symbols，所以 Server 也把它当作输入（EditPolicy.h 的 edit_kind），两边对它是输入还是翻页、标点的判断一致。五笔在 `http` 后的 `s` 上就已进入网址模式，但 TIP 把五笔记成全拼分不出来，要等触发键 `:` 才进入。
inline bool OpensUrlMode(const wchar_t *buffer, size_t length, size_t caret, wchar_t wch, bool detectsUrls,
                         bool dedicatedEnglish)
{
    if (!detectsUrls || dedicatedEnglish || !buffer || length == 0 || caret != length)
    {
        return false;
    }
    return IsUrlTriggerAndKey(buffer, length, wch);
}

// 删掉一个字符之后是否还在网址模式。网址模式由 OpensUrlMode 放进触发键时进入，之后不再从缓冲前缀推断：光标处的编辑会让缓冲开头不再是触发词加触发键，而 Engine 仍在网址模式。与 Engine 相同，只有删掉的恰好是触发键（剩下的正好是触发词，删掉的是它的触发键）或缓冲删空时才退回普通组字。
inline bool UrlModeAfterDeletion(bool urlMode, const wchar_t *remaining, size_t length, wchar_t removed)
{
    return urlMode && length > 0 && remaining && !IsUrlTriggerAndKey(remaining, length, removed);
}

// Whether this key opens the "/" or "@" mode: on an empty composition, with Chinese punctuation in force (with ASCII punctuation the key is the literal mark the user chose, and the Engine opens nothing), and only for a mode the Server reports on. The key then starts the composition, and the Engine takes it as the mode's first character.
inline bool OpensLocalMode(wchar_t wch, bool composing, bool chinesePunctuation, bool commandMode, bool mentionMode)
{
    if (composing || !chinesePunctuation)
    {
        return false;
    }
    return (wch == L'/' && commandMode) || (wch == L'@' && mentionMode);
}

// The Server's LocalModeTriggersChanged payload: three '0'/'1' flags, V then "/" then "@". Anything else turns all three off, which routes every key as it was routed before the modes existed.
struct LocalModeTriggers
{
    bool expression = false;
    bool command = false;
    bool mention = false;
};

inline LocalModeTriggers ParseLocalModeTriggers(const wchar_t *payload, size_t capacity)
{
    LocalModeTriggers triggers;
    if (!payload || capacity < 4 || payload[3] != L'\0')
    {
        return triggers;
    }
    for (size_t index = 0; index < 3; ++index)
    {
        if (payload[index] != L'0' && payload[index] != L'1')
        {
            return triggers;
        }
    }
    triggers.expression = payload[0] == L'1';
    triggers.command = payload[1] == L'1';
    triggers.mention = payload[2] == L'1';
    return triggers;
}
} // namespace Global
