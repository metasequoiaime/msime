#include "CandidateActionAvailability.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("candidate action availability failed at line " +
                             std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)
// Any scheme whose candidates the user dictionary may edit; the policy only distinguishes the ones whose candidates it may not.
constexpr unsigned quanpin = 0;
} // namespace

int main() {
  try {
    // The three sources whose candidates live in the word tables these actions
    // edit. Everything else would produce a menu item the Engine then refuses.
    REQUIRE(candidate_actions_available(quanpin, candidate_source_database));
    REQUIRE(candidate_actions_available(quanpin, candidate_source_user_database));
    REQUIRE(candidate_actions_available(quanpin, candidate_source_english_dictionary));

    // Cloud and AI suggestions are projections, not dictionary rows.
    REQUIRE(!candidate_actions_available(quanpin, candidate_source_cloud_suggestion));
    REQUIRE(!candidate_actions_available(quanpin, candidate_source_ai_suggestion));

    // Everything past the named sources - quick phrases, Emoji, kaomoji,
    // generated and fallback rows - is excluded as well. The rule is an
    // allowlist, so a source the Engine adds is refused until someone decides
    // it belongs, rather than being offered by default.
    for (unsigned source = candidate_source_english_dictionary + 1; source < 32; ++source)
      REQUIRE(!candidate_actions_available(quanpin, source));

    // Japanese is display-only whatever the source: the Host API refuses
    // dictionary maintenance for that scheme, so every one of these would fail.
    // This is the guard HarmonyOS had written against the scheme name while its
    // view held the numeric id, where it never fired at all.
    for (unsigned source = 0; source < 32; ++source)
      REQUIRE(!candidate_actions_available(candidate_scheme_japanese, source));

    // Korean's Hanja list is display-only too: the Engine pins, fixes and removes none of its rows.
    for (unsigned source = 0; source < 32; ++source)
      REQUIRE(!candidate_actions_available(candidate_scheme_korean, source));

    // Cantonese, Zhuyin and Stroke read their own language dictionaries, which the Engine never writes.
    for (unsigned source = 0; source < 32; ++source) {
      REQUIRE(!candidate_actions_available(candidate_scheme_cantonese, source));
      REQUIRE(!candidate_actions_available(candidate_scheme_zhuyin, source));
      REQUIRE(!candidate_actions_available(candidate_scheme_stroke, source));
    }
    static_assert(candidate_scheme_stroke == 8, "the Engine's SchemeType::Stroke");

    // Other schemes are not refused by accident of numbering.
    REQUIRE(candidate_actions_available(1, candidate_source_database));
    REQUIRE(candidate_actions_available(2, candidate_source_database));
    REQUIRE(candidate_actions_available(9, candidate_source_database));

    std::cout << "Windows candidate action availability checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
