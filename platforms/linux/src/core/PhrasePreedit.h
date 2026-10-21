#pragma once

#include "../../../../shared/input/CompositionDisplay.h"

#include <cstddef>
#include <string>
#include <string_view>

namespace msime::linux_host {

// The composition as the user should see it while a phrase is being assembled.
//
// Picking a candidate that covers only part of the input leaves the Engine composing the rest and
// hands back the piece that was picked. With `phrase_preedit` requested, the runtime holds that
// piece in `view.phrase_prefix` instead of committing it, and the host draws it ahead of the
// reading - which is what the reference does, prepending `word_for_creating_word` and moving the
// caret past it. A host that asks for the piece and does not draw it shows nothing at all for text
// the user already chose, so the two go together (`scripts/test-phrase-preedit-hosts.py`).
//
// The caret is the reason this is a function rather than a `+`. The runtime's `caret_position` is a
// byte offset into the reading, which is ASCII; the prefix is not, and the two front ends count
// their cursors differently - fcitx5 takes a byte offset into the string it was given, while IBus
// takes a position in Unicode scalars. Both are produced here so neither has to convert, and so
// the conversion is tested once.
struct PhrasePreedit
{
    std::string text;
    std::size_t caret_bytes = 0;
    std::size_t caret_scalars = 0;
    // Where the held piece ends and the reading begins, and the whole length, in both units for the same reason as the caret. The hosts draw the two as separate clauses (see `phrase_preedit_has_clauses`).
    std::size_t prefix_bytes = 0;
    std::size_t prefix_scalars = 0;
    std::size_t scalars = 0;
};

// Scalars, not bytes: the count of UTF-8 lead bytes, which is what a well-formed string has one of
// per character. Malformed input cannot make this exceed the byte length.
inline std::size_t utf8_scalar_count(std::string_view text)
{
    std::size_t count = 0;
    for (unsigned char byte : text)
        if ((byte & 0xC0u) != 0x80u)
            ++count;
    return count;
}

// The composition shows the kana for Japanese; the rule and its reasoning live in
// shared/input/CompositionDisplay.h, because all three hosts ask the same question.
using msime::input::composition_shows_reading;

// Whether the view is composing. A held phrase piece counts on its own: a Ctrl+Backspace that empties the reading of a half-chosen phrase leaves the chosen piece in the composition with no reading and no candidates, as the reference's `keep_creating_word_after_empty_raw` does, and the next Backspace or Ctrl+Backspace edits it. A host that looked at the reading alone would hand Enter, Escape and punctuation to the application while the preedit still shows that piece. Pass `has_candidates = false` where a host's test is about the reading rather than the candidate page.
inline bool view_has_composition(std::string_view editing, bool has_candidates, std::string_view phrase_prefix)
{
    return !editing.empty() || has_candidates || !phrase_prefix.empty();
}

inline PhrasePreedit compose_phrase_preedit(std::string_view prefix, std::string_view editing,
                                            std::size_t caret)
{
    // A caret past the end of the reading is the Engine and the host disagreeing about a
    // composition that has already moved on; clamping keeps the cursor inside the string rather
    // than handing a toolkit an offset it will read past.
    if (caret > editing.size())
        caret = editing.size();
    PhrasePreedit composed;
    composed.text.reserve(prefix.size() + editing.size());
    composed.text.append(prefix);
    composed.text.append(editing);
    composed.caret_bytes = prefix.size() + caret;
    composed.caret_scalars = utf8_scalar_count(prefix) + utf8_scalar_count(editing.substr(0, caret));
    composed.prefix_bytes = prefix.size();
    composed.prefix_scalars = utf8_scalar_count(prefix);
    composed.scalars = composed.prefix_scalars + utf8_scalar_count(editing);
    return composed;
}

// 正在组词时（已选了一段、还有读音在输入）预编辑分成两个子句：已选的那一段画细下划线，还在输入的读音画粗下划线，对应 macOS 的细、粗两种下划线（shared/apple/TextClient.mm）。IBus 用单、双下划线；Fcitx5 没有粗下划线这种格式（`Bold` 在前端里是字形加粗），读音用下划线加高亮，和整句改字的焦点子句一样。只有其中一段时整段照旧一条细下划线。
inline bool phrase_preedit_has_clauses(const PhrasePreedit &composed)
{
    return composed.prefix_bytes > 0 && composed.prefix_bytes < composed.text.size();
}

} // namespace msime::linux_host
