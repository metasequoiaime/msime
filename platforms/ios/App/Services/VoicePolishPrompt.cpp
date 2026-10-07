// The polish presets and slot resolution for the iOS app, straight from the shared header the desktop hosts use, so the prompt text exists once in the repository.
#include <cstdlib>
#include <cstring>

#include "../../../../shared/voice/PolishPrompt.h"

namespace {
constexpr size_t kPolishPromptIdLimit = 256;
constexpr size_t kPolishPromptCustomLimit = 8 * 1024;

bool bounded_text(const char *value, size_t limit, std::string &out) {
  out.clear();
  if (!value) return true;
  const size_t length = strnlen(value, limit + 1);
  if (length > limit) return false;
  out.assign(value, length);
  return true;
}
} // namespace

// The prompt sent for a `voice_input` configuration. The caller frees the result with free().
extern "C" char *msime_ios_voice_polish_prompt(const char *id, const char *custom_1, const char *custom_2,
                                                 const char *custom_3) {
  msime::windows::PolishPromptSlots slots;
  if (!bounded_text(id, kPolishPromptIdLimit, slots.id)
      || !bounded_text(custom_1, kPolishPromptCustomLimit, slots.custom_1)
      || !bounded_text(custom_2, kPolishPromptCustomLimit, slots.custom_2)
      || !bounded_text(custom_3, kPolishPromptCustomLimit, slots.custom_3)) {
    return nullptr;
  }
  const auto prompt = msime::windows::polish_prompt_for(slots);
  return strdup(prompt.c_str());
}
