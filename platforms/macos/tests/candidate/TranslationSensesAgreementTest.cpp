#include "../../../../shared/input/GlossSenses.h"
#include "../../src/candidate/CandidateGlossSenses.h"
#include "../../../windows/src/candidate/CandidateTranslationPolicy.h"
#include "../../../linux/src/candidates/CandidateTranslationPolicy.h"

#include <cassert>
#include <string>
#include <utility>
#include <vector>

// The gloss-sense rule, pinned case by case, and every host's name for it asked the same questions.
//
// Splitting a candidate's gloss into its senses is host-independent - the dictionary decides what a separator is - so it lives once in shared/input/GlossSenses.h. Windows, Linux and macOS each keep their own name for it as a forwarder; checking those here catches a host that stops forwarding and grows its own copy again, which would look like a dictionary difference rather than a bug.
//
// This host is the one that can compile all three headers - every one of them is plain C++ with no platform header - so the test lives here.
int main()
{
    using Senses = std::vector<std::string>;
    const std::vector<std::pair<std::string, Senses>> fixtures = {
        // The ordinary shapes: one sense, several ASCII-separated, several full-width separated.
        {"hello", {"hello"}},
        {"hello; hi; greetings", {"hello", "hi", "greetings"}},
        {"你好；嗨", {"你好", "嗨"}},
        {"mixed; 混合；both", {"mixed", "混合", "both"}},
        // Whitespace around separators, and a trailing one.
        {"  padded  ;  sides  ", {"padded", "sides"}},
        {"trailing;", {"trailing"}},
        {";leading", {"leading"}},
        {"\tleft\r\n; right ", {"left", "right"}},
        // Nothing but separators and spaces: no sense survives.
        {";;;", {}},
        {"  ", {}},
        {"", {}},
        {"；；", {}},
        {" ; \xEF\xBC\x9B", {}},
        // Repeated separators leave no empty sense between them.
        {"a;;b", {"a", "b"}},
        {"a；；b", {"a", "b"}},
        // A byte that shares the full-width separator's lead byte but is a different character, so a split that matched on one byte would cut it in half.
        {"， comma", {"， comma"}},
        // Newlines are the host's column separator, not a sense separator: they must survive.
        {"first line\nsecond line", {"first line\nsecond line"}},
        {"line one; two\nline two", {"line one", "two\nline two"}},
    };

    for (const auto &[gloss, expected] : fixtures)
    {
        const auto shared = msime::input::gloss_senses(gloss);
        assert(shared == expected);
        assert(msime::mac::candidate_gloss_senses(gloss) == expected);
        assert(msime::windows::translation_senses(gloss) == expected);
        assert(msime::linux_host::split_translation_gloss(gloss) == expected);
    }

    const auto reserved = msime::input::gloss_senses("a;b;c;d;e");
    assert(reserved.capacity() == 5);

    assert(msime::windows::first_translation_sense("hello; hi") == "hello");
    assert(msime::windows::first_translation_sense(" ; ").empty());
    return 0;
}
