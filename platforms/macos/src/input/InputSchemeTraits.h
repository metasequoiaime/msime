#pragma once

// Scheme behaviour this host decides from a view's `scheme` number. The view publishes `chinese_text`, `script_conversion` and `candidate_list_open` itself, and those are read from the view; everything here is either a host-only trait or an Engine trait the view does not carry. An unknown scheme number answers false everywhere, the way host-api reads `SchemeType::from_u8`.
namespace msime::mac::scheme
{
// The Engine's `SchemeType` ordinals (crates/engine/src/types.rs), as they appear in a view's `scheme`.
constexpr int Quanpin = 0;
constexpr int Shuangpin = 1;
constexpr int Wubi = 2;
constexpr int Japanese = 3;
constexpr int Korean = 4;
constexpr int Cantonese = 5;
constexpr int Zhuyin = 6;
constexpr int Vietnamese = 7;
constexpr int Stroke = 8;

// ---- Host-only traits ----

// The letter the Engine receives takes its case from Shift alone, so Caps Lock does not change it (Dubeolsik binds jamo by case; see KoreanKeyLetter).
constexpr bool FoldsLetterCase(int scheme) { return scheme == Korean; }

// A Caps Lock uppercase letter that would start a composition is not handed back to the application: the scheme composes it (Korean folds it, Vietnamese keeps it uppercase).
constexpr bool CapsLockBypassExempt(int scheme) { return scheme == Korean || scheme == Vietnamese; }

// Letters build the written text directly (a Hangul syllable, a Vietnamese word) rather than a reading converted through candidates, so there is no word to take a character from.
constexpr bool LetterComposition(int scheme) { return scheme == Korean || scheme == Vietnamese; }

// Candidates appear only in a list the user opens with MSIME_OPEN_CANDIDATE_LIST (the Korean Hanja list, the Zhuyin list).
constexpr bool OpensCandidateList(int scheme) { return scheme == Korean || scheme == Zhuyin; }

// The composition is always drawn inline whatever the preedit display preference says: until a list is opened there is no candidate window to show it in, and hidden it would be text the user cannot see being written.
constexpr bool AlwaysInlinePreedit(int scheme) { return LetterComposition(scheme) || OpensCandidateList(scheme); }

// ---- Engine traits the view does not publish; each mirrors the `SchemeType` predicate of the same name ----

// `commits_on_blur`: leaving the composition (focus loss, a scheme or mode switch, a navigation key handed to the application) writes it out instead of discarding it.
constexpr bool CommitsOnBlur(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese; }

// `locks_caret`: the caret stays at the end of the composition, so there are no segments for Ctrl+Backspace and Ctrl+Left/Right to edit.
constexpr bool LocksCaret(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese; }

// `uses_chinese_punctuation`: punctuation goes through the Chinese table. Korean and Vietnamese write half-width ASCII marks whatever the Chinese punctuation switches say.
constexpr bool UsesChinesePunctuation(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese ||
           scheme == Zhuyin || scheme == Stroke;
}

// `host_smart_punctuation`: the reversible smart punctuation gestures (space-to-ASCII, repeat-to-Chinese) may run.
constexpr bool HostSmartPunctuation(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese || scheme == Stroke;
}

// `widens_full_width`: commits and direct characters are widened when the full-width switch is on. host-api's `complete_transition` applies the same rule to Engine commits.
constexpr bool WidensFullWidth(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese ||
           scheme == Zhuyin || scheme == Stroke;
}

// `shows_glosses`: candidates may carry translation glosses.
constexpr bool ShowsGlosses(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Korean; }
} // namespace msime::mac::scheme
