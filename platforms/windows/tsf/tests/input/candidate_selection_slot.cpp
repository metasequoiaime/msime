#include "../../../common/SecondThirdCandidatePolicy.h"
#include <cstddef>
#include <optional>
#include <stdexcept>
#include <utility>

using msime::windows::candidate_selection_slot;
using msime::windows::tip_candidate_selection_index;

namespace {
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}
} // namespace

int main() {
  require(candidate_selection_slot(0xBA, ';') == std::optional<std::size_t>(1) &&
              candidate_selection_slot(0xDE, '\'') == std::optional<std::size_t>(2) &&
              candidate_selection_slot('1', '1') == std::optional<std::size_t>(0) &&
              candidate_selection_slot('2', '2') == std::optional<std::size_t>(1) &&
              candidate_selection_slot('3', '3') == std::optional<std::size_t>(2) &&
              candidate_selection_slot('8', '8') == std::optional<std::size_t>(7) &&
              candidate_selection_slot('9', '9') == std::optional<std::size_t>(8) &&
              candidate_selection_slot(0x61, '1') == std::optional<std::size_t>(0) &&
              candidate_selection_slot(0x62, '2') == std::optional<std::size_t>(1) &&
              candidate_selection_slot(0x63, '3') == std::optional<std::size_t>(2) &&
              candidate_selection_slot(0x68, '8') == std::optional<std::size_t>(7) &&
              candidate_selection_slot(0x69, '9') == std::optional<std::size_t>(8) &&
              !candidate_selection_slot(0xBA, ':') && !candidate_selection_slot(0xDE, '"') &&
              !candidate_selection_slot(0xBA, 0x00FC) && !candidate_selection_slot(0xC0, '\'') &&
              !candidate_selection_slot('0', '0') && !candidate_selection_slot(0x60, '0'),
          "TIP candidate key mapped to the wrong page slot");

  // presenter 持有宿主会话的真实一页：按键的位置选，超出一页（8 个槽位）的不选。
  constexpr std::size_t page = 8;
  require(tip_candidate_selection_index(0xBA, ';', true, page) == std::optional<std::size_t>(1),
          "';' did not pick the second row of the host session page");
  require(tip_candidate_selection_index(0xDE, '\'', true, page) == std::optional<std::size_t>(2),
          "'\'' did not pick the third row of the host session page");
  require(tip_candidate_selection_index('3', '3', true, page) == std::optional<std::size_t>(2),
          "Digit 3 did not pick the third row of the host session page");
  require(!tip_candidate_selection_index('9', '9', true, page), "Digit 9 picked a row past the page");
  require(!tip_candidate_selection_index(0xBA, ':', true, page), "Shift+';' picked a row");

  // 最小镜像（没有宿主会话时只有键入的原文这一项，UI-less 时是 Server 那一页的文字）：一律选第一项，上屏的候选来自 Server 的选词回复。按真实位置选会让一项的镜像越界，TIP 就不再读 Server 已经选好的回复。
  const std::pair<unsigned, unsigned> keys[] = {{0xBA, ';'}, {0xDE, '\''}, {'2', '2'}, {'3', '3'}, {'9', '9'}, {0x63, '3'}};
  for (const auto &[code, text] : keys)
    require(tip_candidate_selection_index(code, text, false, page) == std::optional<std::size_t>(0),
            "A selection key on the minimal mirror did not pick its only row");
}
