#include "CandidateTranslationPolicy.h"

#include <iostream>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("candidate translation merge failed at line " +
                             std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)
using Answered = std::vector<std::pair<std::string, std::string>>;
using Texts = std::vector<std::string>;
} // namespace

int main() {
  try {
    // The whole point of the rule: a candidate the packaged dictionary already
    // answered is not sent to a provider. The worker appends provider answers
    // to the same list, so asking again both spends a request and can leave one
    // candidate carrying two glosses.
    const Answered local{{"你好", "hello"}};
    REQUIRE(untranslated_texts(local, Texts{"你好", "世界"}) == Texts{"世界"});
    REQUIRE(untranslated_texts(local, Texts{"你好"}).empty());

    // With nothing answered locally - no packaged gloss for this page, which is
    // the ordinary case for Chinese words - every planned candidate is asked
    // about, in the plan's order.
    REQUIRE(untranslated_texts(Answered{}, Texts{"世界", "你好"}) ==
            (Texts{"世界", "你好"}));

    // A text planned twice is asked about once. The caller walks the result
    // against a provider batch and would otherwise send a duplicate.
    REQUIRE(untranslated_texts(Answered{}, Texts{"你好", "你好"}) ==
            Texts{"你好"});

    // An entry with no gloss is not an answer. Nobody has translated that
    // candidate, so the provider is still the only chance it has.
    //
    // This half cannot be reached from the worker's own producer today: the
    // shared candidate gloss request drops empty glosses before the worker sees
    // them. It is this function's contract rather than a second line of
    // defence, and it is asserted here because the function is what a future
    // caller reads - not because the assertion can go red from that producer.
    REQUIRE(untranslated_texts(Answered{{"你好", ""}}, Texts{"你好"}) ==
            Texts{"你好"});

    // An entry for some other text answers nothing here.
    REQUIRE(untranslated_texts(Answered{{"世界", "world"}}, Texts{"你好"}) ==
            Texts{"你好"});

    // Empty plan entries are dropped rather than asked about: an empty text is
    // not a candidate, and the provider request would be rejected anyway.
    REQUIRE(untranslated_texts(Answered{}, Texts{"", "你好", ""}) ==
            Texts{"你好"});

    // Nothing planned means nothing to ask, whatever is already answered.
    REQUIRE(untranslated_texts(local, Texts{}).empty());

    // A non-English offline dictionary fills only what the user's own
    // translator left: the online answer stays, the unanswered candidate gets
    // the dictionary's gloss, an online entry with an empty gloss takes the
    // dictionary's in place rather than leaving the text listed twice, and an
    // empty dictionary gloss adds nothing.
    Answered online{{"你好", "salut"}, {"再见", ""}};
    fill_offline_glosses(online, Answered{{"你好", "bonjour"},
                                          {"世界", "monde"},
                                          {"再见", "au revoir"},
                                          {"空", ""}});
    REQUIRE(online.capacity() >= 6);
    REQUIRE(online == (Answered{{"你好", "salut"},
                                {"再见", "au revoir"},
                                {"世界", "monde"}}));

    // With no online answer at all, the usual case, the dictionary is the page.
    Answered none;
    fill_offline_glosses(none, Answered{{"世界", "monde"}});
    REQUIRE(none == (Answered{{"世界", "monde"}}));

    // The senses split, which shares this header. Both separators, and the
    // first non-empty sense is what Ctrl+Enter commits.
    REQUIRE(first_translation_sense("hello; hi") == "hello");
    REQUIRE(first_translation_sense("；世界") == "世界");
    REQUIRE(first_translation_sense("   ").empty());

    // A `/fy` query is one English sentence into the query's own target, asked directly rather than through the candidate plan, which refuses a Chinese target.
    const auto command = command_translation_item(true, Texts{"hello world"}, "zh");
    REQUIRE(command && command->text == "hello world" &&
            command->source_language == "en" && command->target_language == "zh");
    // A candidate gloss query is never one, nor is a sentence query that does not hold exactly one non-empty text and a target.
    REQUIRE(!command_translation_item(false, Texts{"hello world"}, "zh"));
    REQUIRE(!command_translation_item(true, Texts{"hello", "world"}, "zh"));
    REQUIRE(!command_translation_item(true, Texts{}, "zh"));
    REQUIRE(!command_translation_item(true, Texts{""}, "zh"));
    REQUIRE(!command_translation_item(true, Texts{"hello"}, ""));

    std::cout << "candidate translation merge policy ok\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
