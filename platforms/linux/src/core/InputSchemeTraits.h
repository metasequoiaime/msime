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
constexpr int Tibetan = 8;
constexpr int Stroke = 9;

// ---- Host-only traits ----

// The letter the Engine receives takes its case from Shift alone, so Caps Lock does not change it (Dubeolsik binds jamo by case).
constexpr bool FoldsLetterCase(int scheme) { return scheme == Korean; }

// Caps Lock 打开时本该开始组字的大写字母不交还给应用，而是由方案自己组字：韩文把它折回小写，越南文保留大写，藏文威利转写区分大小写，大写字母本身就是另一个字母。
constexpr bool CapsLockBypassExempt(int scheme) { return scheme == Korean || scheme == Vietnamese || scheme == Tibetan; }

// 字母直接组成要写出的文字（一个谚文音节、一个越南文单词、一串藏文音节），而不是经候选转换的读音：方案不拿来拼写的按键会先把组字写出去，也没有可以从中取字的词。
constexpr bool LetterComposition(int scheme) { return scheme == Korean || scheme == Vietnamese || scheme == Tibetan; }

// Candidates appear only in a list the user opens with MSIME_OPEN_CANDIDATE_LIST (the Korean Hanja list, the Zhuyin list).
constexpr bool OpensCandidateList(int scheme) { return scheme == Korean || scheme == Zhuyin; }

// The composition is always drawn inline whatever the preedit display preference says: until a list is opened there is no candidate window to show it in, and hidden it would be text the user cannot see being written.
constexpr bool AlwaysInlinePreedit(int scheme) { return LetterComposition(scheme) || OpensCandidateList(scheme); }

// 整句改字（MSIME_CONVERSION_LEFT / MSIME_CONVERSION_RIGHT）只在全拼和双拼里有：左右键交给引擎的改字命令，Ctrl+左右一个字母一个字母地编辑拼音。
constexpr bool EditsSentence(int scheme) { return scheme == Quanpin || scheme == Shuangpin; }

// ---- Engine traits the view does not publish; each mirrors the `SchemeType` predicate of the same name ----

// `is_chinese`: a Chinese scheme, the one a switch to a non-Chinese scheme remembers as `last_chinese_scheme`.
constexpr bool IsChinese(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese || scheme == Zhuyin || scheme == Stroke; }

// `script_conversion_applies`：繁体输出转换会改写这个方案的文字。粤拼和注音本来就写繁体字，笔画候选按 msime-stroke.db 里存的字形原样取用，假名、谚文、越南文和藏文都不是中文。
constexpr bool ScriptConversionApplies(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi; }

// `learns_into_main_dictionary`: a candidate may be removed from, or pinned in, the user dictionary of the main Chinese lexicon.
constexpr bool LearnsIntoMainDictionary(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi; }

// `opens_local_modes`: Shift+letter opens the local modes other than K (U, T, E, M, J, Y, R, V) while nothing is composed. K and the `/` and `@` keys follow `opens_table_modes`.
constexpr bool OpensLocalModes(int scheme) { return scheme == Quanpin || scheme == Shuangpin; }

// `opens_table_modes`: Shift+K and the `/` and `@` keys open the quick phrase, command and mention modes while nothing is composed. In every other scheme the symbols a view lists outside a local mode are the scheme's own spelling.
constexpr bool OpensTableModes(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi; }

// `commits_on_blur`: leaving the composition (focus loss, a scheme or mode switch, a navigation key handed to the application) writes it out instead of discarding it.
constexpr bool CommitsOnBlur(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese || scheme == Tibetan; }

// `locks_caret`: the caret stays at the end of the composition, so there are no segments for Ctrl+Backspace and Ctrl+Left/Right to edit.
constexpr bool LocksCaret(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese || scheme == Tibetan; }

// `uses_chinese_punctuation`：标点走中文标点表。韩文、越南文和藏文不论中文标点开关怎么设都写半角 ASCII 标点。
constexpr bool UsesChinesePunctuation(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese || scheme == Zhuyin || scheme == Stroke; }

// `host_smart_punctuation`: the reversible smart punctuation gestures (space-to-ASCII, repeat-to-Chinese) and the paired-mark helpers may run.
constexpr bool HostSmartPunctuation(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese || scheme == Stroke; }

// `widens_full_width`: commits and direct characters are widened when the full-width switch is on.
constexpr bool WidensFullWidth(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese || scheme == Zhuyin || scheme == Stroke; }
} // namespace msime::linux_host::scheme
