// The polish presets and slot resolution for the iOS app, straight from the shared header the desktop hosts use, so the prompt text exists once in the repository.
#include <cstdlib>
#include <cstring>

#include "../../../../shared/voice/PolishPrompt.h"

namespace {
std::string text(const char *value) { return value ? std::string(value) : std::string(); }
} // namespace

// The prompt sent for a `voice_input` configuration. The caller frees the result with free().
extern "C" char *msime_ios_voice_polish_prompt(const char *id, const char *custom_1, const char *custom_2,
                                                 const char *custom_3) {
  msime::windows::PolishPromptSlots slots;
  slots.id = text(id);
  slots.custom_1 = text(custom_1);
  slots.custom_2 = text(custom_2);
  slots.custom_3 = text(custom_3);
  const auto prompt = msime::windows::polish_prompt_for(slots);
  return strdup(prompt.c_str());
}
