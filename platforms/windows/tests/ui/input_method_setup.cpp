#include "../../settings/InputMethodSetup.h"

#include <cassert>

int main() {
  using msime::settings::input_method::InputMethodState;
  using msime::settings::input_method::input_method_repair_attempts;
  using msime::settings::input_method::input_method_repair_delay_ms;
  using msime::settings::input_method::input_method_repair_succeeded;

  assert(input_method_repair_attempts == 3);
  assert(input_method_repair_delay_ms == 250);
  assert(input_method_repair_succeeded(InputMethodState::ready));
  assert(input_method_repair_succeeded(InputMethodState::not_default));
  assert(input_method_repair_succeeded(InputMethodState::unknown));
  assert(!input_method_repair_succeeded(InputMethodState::missing));
}
