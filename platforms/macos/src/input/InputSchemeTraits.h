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
constexpr int Tibetan = 8;

// ---- Host-only traits ----

// The letter the Engine receives takes its case from Shift alone, so Caps Lock does not change it (Dubeolsik binds jamo by case; see KoreanKeyLetter).
constexpr bool FoldsLetterCase(int scheme) { return scheme == Korean; }

// 大写锁定下会开始组字的大写字母不交还给应用：方案自己组它（韩文折成小写，越南文保留大写，藏文的威利转写区分大小写，大写字母本身就是拼写）。
constexpr bool CapsLockBypassExempt(int scheme) { return scheme == Korean || scheme == Vietnamese || scheme == Tibetan; }

// 字母直接拼出要写的文字（一个韩文音节、一个越南文词、一串藏文音节），而不是经候选转换的读音，所以没有可以取字的词。
constexpr bool LetterComposition(int scheme) { return scheme == Korean || scheme == Vietnamese || scheme == Tibetan; }

// Candidates appear only in a list the user opens with MSIME_OPEN_CANDIDATE_LIST (the Korean Hanja list, the Zhuyin list).
constexpr bool OpensCandidateList(int scheme) { return scheme == Korean || scheme == Zhuyin; }

// The composition is always drawn inline whatever the preedit display preference says: until a list is opened there is no candidate window to show it in, and hidden it would be text the user cannot see being written.
constexpr bool AlwaysInlinePreedit(int scheme) { return LetterComposition(scheme) || OpensCandidateList(scheme); }

// ---- Engine traits the view does not publish; each mirrors the `SchemeType` predicate of the same name ----

// `commits_on_blur`: 离开组字（失去焦点、切换方案或模式、把导航键交给应用）时把组字写出去，而不是丢弃。
constexpr bool CommitsOnBlur(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese || scheme == Tibetan; }

// `locks_caret`: 光标固定在组字末尾，没有可供 Ctrl+Backspace 和 Ctrl+Left/Right 编辑的分段。
constexpr bool LocksCaret(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese || scheme == Tibetan; }

// `uses_chinese_punctuation`: 标点走中文标点表。韩文、越南文和藏文不论中文标点开关怎么设都写半角 ASCII 标点。
constexpr bool UsesChinesePunctuation(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese ||
           scheme == Zhuyin;
}

// `host_smart_punctuation`: the reversible smart punctuation gestures (space-to-ASCII, repeat-to-Chinese) may run.
constexpr bool HostSmartPunctuation(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese;
}

// `widens_full_width`: commits and direct characters are widened when the full-width switch is on. host-api's `complete_transition` applies the same rule to Engine commits.
constexpr bool WidensFullWidth(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese ||
           scheme == Zhuyin;
}

// `shows_glosses`: candidates may carry translation glosses.
constexpr bool ShowsGlosses(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Korean; }
} // namespace msime::mac::scheme
