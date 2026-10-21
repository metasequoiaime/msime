#include "../../src/core/PhrasePreedit.h"

#include <cassert>
#include <string>

using msime::linux_host::compose_phrase_preedit;
using msime::linux_host::utf8_scalar_count;
using msime::linux_host::view_has_composition;

int main()
{
    // A Ctrl+Backspace that empties the reading of a half-chosen phrase leaves only the chosen piece, as the reference's `keep_creating_word_after_empty_raw` does. That is still a composition, so the IBus segment-key gate and both front ends' key routing keep sending Backspace, Ctrl+Backspace, Enter and Escape to the session instead of the application.
    {
        assert(view_has_composition("", false, "海滩"));
        assert(view_has_composition("paobu", false, ""));
        assert(view_has_composition("", true, ""));
        assert(!view_has_composition("", false, ""));
    }

    // Nothing held: the composition is exactly the reading, and both cursors are the offset the
    // runtime gave. This is every keystroke before a candidate is picked, so it has to cost
    // nothing and change nothing.
    {
        const auto composed = compose_phrase_preedit("", "nihao", 3);
        assert(composed.text == "nihao");
        assert(composed.caret_bytes == 3);
        assert(composed.caret_scalars == 3);
    }

    // A piece held: it leads the reading, and the caret moves past it. The two front ends want the
    // same position counted differently - fcitx5 in bytes, IBus in scalars - and 海滩 is three
    // bytes per character, so a host that used the wrong one would put the cursor inside a
    // character.
    {
        const auto composed = compose_phrase_preedit("海滩", "paobu", 2);
        assert(composed.text == "海滩paobu");
        assert(composed.caret_bytes == 8);
        assert(composed.caret_scalars == 4);
    }

    // 已选的一段和读音分成两个子句：分界和总长按两种单位给出，IBus 用标量画单、双下划线，Fcitx5 用字节画下划线和下划线加高亮。只有读音或只剩已选段时不分。
    {
        const auto composed = compose_phrase_preedit("海滩", "paobu", 2);
        assert(msime::linux_host::phrase_preedit_has_clauses(composed));
        assert(composed.prefix_bytes == 6 && composed.prefix_scalars == 2 && composed.scalars == 7);
        assert(composed.text.substr(composed.prefix_bytes) == "paobu");
        assert(!msime::linux_host::phrase_preedit_has_clauses(compose_phrase_preedit("", "nihao", 3)));
        const auto held = compose_phrase_preedit("海滩", "", 0);
        assert(!msime::linux_host::phrase_preedit_has_clauses(held) && held.scalars == 2);
    }

    // The caret at either end of the reading.
    {
        const auto start = compose_phrase_preedit("海滩", "paobu", 0);
        assert(start.caret_bytes == 6 && start.caret_scalars == 2);
        const auto end = compose_phrase_preedit("海滩", "paobu", 5);
        assert(end.caret_bytes == 11 && end.caret_scalars == 7);
    }

    // A caret past the end of the reading is clamped rather than handed on: the toolkits take it
    // as an offset into the string and would read past the end of it.
    {
        const auto composed = compose_phrase_preedit("海滩", "pa", 99);
        assert(composed.caret_bytes == 8);
        assert(composed.caret_scalars == 4);
    }

    // The reading emptied while a piece is held - the state the runtime does not produce, because
    // it commits what is held instead - still composes to something drawable rather than a cursor
    // outside the string.
    {
        const auto composed = compose_phrase_preedit("海滩", "", 0);
        assert(composed.text == "海滩");
        assert(composed.caret_bytes == 6 && composed.caret_scalars == 2);
    }

    // A Japanese composition shows the kana, and only while the caret is where typing leaves it.
    {
        using msime::linux_host::composition_shows_reading;
        assert(composition_shows_reading("にほん", 5, 5));
        // A caret moved into the letters keeps the letters: the offset is into the romaji.
        assert(!composition_shows_reading("にほん", 2, 5));
        // Every other scheme carries no reading at all.
        assert(!composition_shows_reading("", 5, 5));
        // A caret past the end is still the end.
        assert(composition_shows_reading("にほん", 9, 5));
    }

    // The scalar count itself, including the shapes that break a naive one: a four-byte character
    // is one, and a stray continuation byte cannot make the count exceed the length.
    assert(utf8_scalar_count("") == 0);
    assert(utf8_scalar_count("abc") == 3);
    assert(utf8_scalar_count("海滩") == 2);
    assert(utf8_scalar_count("😀") == 1);
    assert(utf8_scalar_count("a\x80\x80") == 1);

    return 0;
}
