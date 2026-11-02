#pragma once
#include <algorithm>
#include <cstddef>
#include <optional>
#include <string_view>

namespace msime::tsf {
// TSF strings use UTF-16 code units. Preserve the Windows client's separator
// mapping while accepting the authoritative raw caret from either input path.
inline std::size_t MapPreeditCaret(std::wstring_view raw, std::size_t rawCaret,
                                  std::wstring_view preedit, std::size_t prefixLength) {
    rawCaret = (std::min)(rawCaret, raw.size());
    std::size_t lettersBeforeCaret = 0;
    for (std::size_t i = 0; i < rawCaret; ++i)
        if (raw[i] != L'\'') ++lettersBeforeCaret;
    std::size_t position = (std::min)(prefixLength, preedit.size());
    std::size_t seenLetters = 0;
    while (position < preedit.size() && seenLetters < lettersBeforeCaret) {
        if (preedit[position] != L'\'') ++seenLetters;
        ++position;
    }
    if (rawCaret > 0 && raw[rawCaret - 1] == L'\'')
        while (position < preedit.size() && preedit[position] == L'\'') ++position;
    return position;
}

// 画出来的预编辑（去掉前面的已选词段）就是引擎的 preedit 时，光标用引擎给的位置（View.preedit_caret，已换成 UTF-16）：只有引擎知道 preedit 的哪一段对应哪几个按键。上面按字母数换的办法只对全拼分词这种只加了隔音符的拼写成立，双拼「拼音分词」的 preedit 是展开的全拼，按字母数会把光标画进音节中间（#10632）。画的是别的文字（原始按键、日文假名、Server 送来的另一份预编辑）或视图里没有这一项时照旧按字母数换。
inline std::size_t MapPreeditCaret(std::wstring_view raw, std::size_t rawCaret,
                                  std::wstring_view preedit, std::size_t prefixLength,
                                  std::wstring_view enginePreedit,
                                  std::optional<std::size_t> enginePreeditCaret) {
    const std::size_t prefix = (std::min)(prefixLength, preedit.size());
    if (enginePreeditCaret && preedit.substr(prefix) == enginePreedit)
        return prefix + (std::min)(*enginePreeditCaret, enginePreedit.size());
    return MapPreeditCaret(raw, rawCaret, preedit, prefixLength);
}
} // namespace msime::tsf
