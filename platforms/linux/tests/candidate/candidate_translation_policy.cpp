#include "../src/candidates/CandidateTranslationPolicy.h"

#include <cassert>

int main() {
  using msime::linux_host::split_translation_gloss;
  assert((split_translation_gloss("apple; fruit") ==
          std::vector<std::string>{"apple", "fruit"}));
  assert((split_translation_gloss("苹果；家伙") ==
          std::vector<std::string>{"苹果", "家伙"}));
  assert((split_translation_gloss(" first ; ; second ; ") ==
          std::vector<std::string>{"first", "second"}));
  assert((split_translation_gloss("cloud gloss") ==
          std::vector<std::string>{"cloud gloss"}));
  assert(split_translation_gloss(" ; \xEF\xBC\x9B").empty());

  // A non-English offline dictionary answers first and the user's own
  // translator then outranks it: its gloss replaces the dictionary's in place,
  // a text only it answered is appended, an empty answer changes nothing, and
  // the dictionary keeps what the provider left.
  using msime::linux_host::prefer_online_glosses;
  using Glosses = std::vector<std::pair<std::string, std::string>>;
  Glosses glosses{{"你好", "bonjour"}, {"世界", "monde"}};
  prefer_online_glosses(glosses, Glosses{{"你好", "salut"},
                                         {"再见", "au revoir"},
                                         {"世界", ""}});
  assert((glosses == Glosses{{"你好", "salut"},
                             {"世界", "monde"},
                             {"再见", "au revoir"}}));
  Glosses none;
  prefer_online_glosses(none, Glosses{});
  assert(none.empty());

  // Only the command mode's one-sentence query is /fy's; a gloss request, or the sentence the menu action translates outside that mode, keeps the gloss rules.
  using msime::linux_host::command_translation_query;
  assert(command_translation_query("command", true));
  assert(!command_translation_query("command", false));
  assert(!command_translation_query("none", true));
  assert(!command_translation_query("mention", true));

  using msime::linux_host::should_retry_translation_after_provider;
  assert(should_retry_translation_after_provider(true, false, false));
  assert(should_retry_translation_after_provider(true, true, false));
  assert(!should_retry_translation_after_provider(true, true, true));
  assert(!should_retry_translation_after_provider(false, false, false));
}
