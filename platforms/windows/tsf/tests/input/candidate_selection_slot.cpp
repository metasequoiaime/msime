#include "../../../common/SecondThirdCandidatePolicy.h"
#include <stdexcept>

using msime::windows::candidate_selection_slot;

int main() {
  if (candidate_selection_slot(0xBA, ';') != 1 ||
      candidate_selection_slot(0xDE, '\'') != 2 ||
      candidate_selection_slot('1', '1') != 0 ||
      candidate_selection_slot('2', '2') != 1 ||
      candidate_selection_slot('3', '3') != 2 ||
      candidate_selection_slot('8', '8') != 7 ||
      candidate_selection_slot('9', '9') != 8 ||
      candidate_selection_slot(0x61, '1') != 0 ||
      candidate_selection_slot(0x62, '2') != 1 ||
      candidate_selection_slot(0x63, '3') != 2 ||
      candidate_selection_slot(0x68, '8') != 7 ||
      candidate_selection_slot(0x69, '9') != 8 ||
      candidate_selection_slot(0xBA, ':') ||
      candidate_selection_slot(0xDE, '"') ||
      candidate_selection_slot(0xBA, 0x00FC) ||
      candidate_selection_slot(0xC0, '\'') ||
      candidate_selection_slot('0', '0') ||
      candidate_selection_slot(0x60, '0'))
    throw std::runtime_error("TIP candidate key selected the wrong page slot");
}
