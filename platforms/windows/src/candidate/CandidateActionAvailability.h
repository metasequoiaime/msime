#pragma once

namespace msime::windows {
// Which candidates the right-click actions - 置顶, 固定排位, 取消固定, 删除 -
// may be offered for.
//
// All four write to the user dictionary, so they only make sense for a
// candidate that came out of a dictionary in the first place. A cloud or AI
// suggestion is a projection the Engine will refuse to write; an Emoji, a
// kaomoji, a quick phrase or a generated fallback does not live in the word
// tables these actions edit. Offering them anyway produces a menu item that
// does nothing, which is worse than not offering it.
//
// The identifiers are the Engine's `CandidateSource` enum positions, from
// `core/word_item.h`. They are values on a wire rather than a type this side
// can include, so `scripts/test-candidate-sources.py` holds each name below to
// its position in that enum: inserting a source there would otherwise shift
// every later one and silently hand 删除 to cloud candidates.
inline constexpr unsigned candidate_source_database = 0;
inline constexpr unsigned candidate_source_user_database = 1;
inline constexpr unsigned candidate_source_cloud_suggestion = 2;
inline constexpr unsigned candidate_source_ai_suggestion = 3;
inline constexpr unsigned candidate_source_english_dictionary = 4;

// Japanese candidates are display-only whatever their source: the Host API
// refuses dictionary maintenance for that scheme, so every one of these items
// would fail. This is the guard HarmonyOS had written against the scheme *name*
// while its view held the Engine's numeric id, where it never fired at all.
inline constexpr unsigned candidate_scheme_japanese = 3;
// Korean's only candidates are the Hanja of the composing syllable, which the Engine keeps in table order and refuses to pin, fix or remove: they come from a table compiled into the Engine, not from the user's dictionary.
inline constexpr unsigned candidate_scheme_korean = 4;
// Cantonese and Zhuyin candidates come from their own language dictionaries, which the Engine reads and never writes, so it refuses to pin, fix or remove them too (`is_editable_source` in crates/engine/src/session/input.rs).
inline constexpr unsigned candidate_scheme_cantonese = 5;
inline constexpr unsigned candidate_scheme_zhuyin = 6;

inline bool candidate_actions_available(unsigned scheme, unsigned source) {
  if (scheme == candidate_scheme_japanese || scheme == candidate_scheme_korean ||
      scheme == candidate_scheme_cantonese || scheme == candidate_scheme_zhuyin)
    return false;
  return source == candidate_source_database ||
         source == candidate_source_user_database ||
         source == candidate_source_english_dictionary;
}
} // namespace msime::windows
