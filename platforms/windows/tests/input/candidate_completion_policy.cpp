#include "input/CandidateCompletionPolicy.h"
#include <cassert>
#include <initializer_list>

int main() {
  using msime::windows::candidate_finishes_composition;
  for (const auto source : {2u, 4u, 5u, 6u, 7u, 8u, 13u})
    assert(candidate_finishes_composition(source));
  for (const auto source : {0u, 1u, 3u, 9u, 255u})
    assert(!candidate_finishes_composition(source));
}
