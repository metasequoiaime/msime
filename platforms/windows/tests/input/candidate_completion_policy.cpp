#include "input/CandidateCompletionPolicy.h"
#include <cassert>
#include <cstdint>
#include <initializer_list>

int main() {
  using msime::windows::candidate_finishes_composition;
  for (const auto source : std::initializer_list<std::uint8_t>{2, 4, 5, 6, 7, 8, 13})
    assert(candidate_finishes_composition(source));
  for (const auto source : std::initializer_list<std::uint8_t>{0, 1, 3, 9, 255})
    assert(!candidate_finishes_composition(source));
}
