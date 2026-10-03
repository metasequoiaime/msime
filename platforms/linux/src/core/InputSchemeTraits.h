#pragma once

// Scheme behaviour the IBus and Fcitx5 hosts decide from a view's `scheme` number; InputSchemes.h reads it from views and preferences scheme ids. The view publishes `chinese_text`, `script_conversion` and `candidate_list_open` itself; everything in `scheme` is either a host-only trait or an Engine trait the view does not carry, and scripts/test-scheme-traits-parity.py checks the mirrored ones against the Engine. An unknown scheme number answers false everywhere, the way host-api reads `SchemeType::from_u8`.
namespace msime::linux_host::scheme {
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

// The letter the Engine receives takes its case from Shift alone, so Caps Lock does not change it (Dubeolsik binds jamo by case).
constexpr bool FoldsLetterCase(int scheme) { return scheme == Korean; }

// A Caps Lock uppercase letter that would start a composition is not handed back to the application: the scheme composes it (Korean folds it, Vietnamese keeps it uppercase).
constexpr bool CapsLockBypassExempt(int scheme) { return scheme == Korean || scheme == Vietnamese; }

// Letters build the written text directly (a Hangul syllable, a Vietnamese word) rather than a reading converted through candidates: any key the scheme does not spell with writes the composition out first, and there is no word to take a character from.
constexpr bool LetterComposition(int scheme) { return scheme == Korean || scheme == Vietnamese; }

// Candidates appear only in a list the user opens with MSIME_OPEN_CANDIDATE_LIST (the Korean Hanja list, the Zhuyin list).
constexpr bool OpensCandidateList(int scheme) { return scheme == Korean || scheme == Zhuyin; }

// The composition is always drawn inline whatever the preedit display preference says: until a list is opened there is no candidate window to show it in, and hidden it would be text the user cannot see being written.
constexpr bool AlwaysInlinePreedit(int scheme) { return LetterComposition(scheme) || OpensCandidateList(scheme); }

// ---- Engine traits the view does not publish; each mirrors the `SchemeType` predicate of the same name ----

// `is_chinese`: a Chinese scheme, the one a switch to a non-Chinese scheme remembers as `last_chinese_scheme`.
constexpr bool IsChinese(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese || scheme == Zhuyin || scheme == Stroke; }

// `script_conversion_applies`: the traditional-output conversion rewrites this scheme's text. Cantonese and Zhuyin are written in traditional characters already, Stroke candidates are taken as stored in stroke.db, and kana, Hangul and Vietnamese are not Chinese text.
constexpr bool ScriptConversionApplies(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi; }

// `learns_into_main_dictionary`: a candidate may be removed from, or pinned in, the user dictionary of the main Chinese lexicon.
constexpr bool LearnsIntoMainDictionary(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi; }

// `opens_local_modes`: Shift+letter and the `/` and `@` keys open local modes while nothing is composed. In every other scheme the symbols a view lists outside a local mode are the scheme's own spelling.
constexpr bool OpensLocalModes(int scheme) { return scheme == Quanpin || scheme == Shuangpin; }

// `commits_on_blur`: leaving the composition (focus loss, a scheme or mode switch, a navigation key handed to the application) writes it out instead of discarding it.
constexpr bool CommitsOnBlur(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese; }

// `locks_caret`: the caret stays at the end of the composition, so there are no segments for Ctrl+Backspace and Ctrl+Left/Right to edit.
constexpr bool LocksCaret(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese; }

// `uses_chinese_punctuation`: punctuation goes through the Chinese table. Korean and Vietnamese write half-width ASCII marks whatever the Chinese punctuation switches say.
constexpr bool UsesChinesePunctuation(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese || scheme == Zhuyin || scheme == Stroke; }

// `host_smart_punctuation`: the reversible smart punctuation gestures (space-to-ASCII, repeat-to-Chinese) and the paired-mark helpers may run.
constexpr bool HostSmartPunctuation(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese || scheme == Stroke; }

// `widens_full_width`: commits and direct characters are widened when the full-width switch is on.
constexpr bool WidensFullWidth(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese || scheme == Zhuyin || scheme == Stroke; }
} // namespace msime::linux_host::scheme
