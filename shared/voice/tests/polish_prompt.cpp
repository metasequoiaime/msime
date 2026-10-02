#include "../PolishPrompt.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Polish prompt test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
} // namespace
int main() {
  try {
    // Each filled custom slot sends its own text.
    PolishPromptSlots slots;
    slots.custom_1 = "slot one";
    slots.custom_2 = "slot two";
    slots.custom_3 = "slot three";
    slots.id = "custom_1";
    require(polish_prompt_for(slots) == "slot one");
    slots.id = "custom_2";
    require(polish_prompt_for(slots) == "slot two");
    slots.id = "custom_3";
    require(polish_prompt_for(slots) == "slot three");
    // "custom" is not a slot id; it resolves like any unknown id.
    PolishPromptSlots unset;
    const std::string cleanup = polish_prompt_for(unset);
    slots.id = "custom";
    require(polish_prompt_for(slots) == cleanup);

    // An empty custom slot falls back to the cleanup preset, which is also what an unset configuration gets, and which preset that is has to be pinned: slot two returned the faithful one once, so a slot the user never filled in had the model proof-read where every other empty slot had it condense.
    PolishPromptSlots empty_slot;
    for (const char *id : {"custom_1", "custom_2", "custom_3"}) {
      empty_slot.id = id;
      require(polish_prompt_for(empty_slot) == cleanup);
    }

    PolishPromptSlots preset;
    preset.id = "faithful";
    const auto faithful = polish_prompt_for(preset);
    require(faithful.find("<asr_text>") != std::string::npos);

    // Each preset resolves to its own multi-rule text, and they differ.
    PolishPromptSlots plain;
    std::string previous;
    for (const char *id : {"cleanup", "faithful", "zh2en", "casual"}) {
      plain.id = id;
      const auto prompt = polish_prompt_for(plain);
      require(prompt.size() > 100);
      require(prompt != previous);
      previous = prompt;
    }
    // An unknown id falls back to the default preset rather than an empty
    // prompt, which would send the model no instruction at all.
    plain.id = "nonsense";
    require(!polish_prompt_for(plain).empty());
    plain.id.clear();
    require(!polish_prompt_for(plain).empty());

    std::cout << "Polish prompt: the selected slot decides\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Polish prompt test failed with an unknown error\n";
    return 1;
  }
}
