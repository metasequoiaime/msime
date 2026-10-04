#include "../src/candidates/CandidateLocalModeLabels.h"

#include <cassert>
#include <cstring>

int main() {
  using msime::linux_host::candidate_local_mode_label;
  // Every non-default name the engine emits in `local_mode`.
  for (const char *mode :
       {"unicode", "date_time", "quick_phrase", "emoji", "kaomoji",
        "super_jianpin", "temporary_english", "temporary_japanese",
        "expression", "command", "mention", "url"}) {
    const char *label = candidate_local_mode_label(mode);
    assert(label != nullptr);
    assert(std::strlen(label) > 0);
  }
  assert(std::strcmp(candidate_local_mode_label("quick_phrase"), "短语") == 0);
  assert(std::strcmp(candidate_local_mode_label("super_jianpin"), "简拼") == 0);
  assert(std::strcmp(candidate_local_mode_label("temporary_english"), "EN") == 0);
  assert(std::strcmp(candidate_local_mode_label("temporary_japanese"), "日文") == 0);
  assert(std::strcmp(candidate_local_mode_label("expression"), "计算") == 0);
  assert(std::strcmp(candidate_local_mode_label("command"), "指令") == 0);
  assert(std::strcmp(candidate_local_mode_label("mention"), "@") == 0);
  assert(std::strcmp(candidate_local_mode_label("url"), "网址") == 0);
  assert(candidate_local_mode_label("none") == nullptr);
  // Names the Fcitx5 footer used to compare against; the Engine never emits them.
  assert(candidate_local_mode_label("phrase") == nullptr);
  assert(candidate_local_mode_label("abbreviation") == nullptr);
  assert(candidate_local_mode_label("english") == nullptr);
  assert(candidate_local_mode_label("japanese") == nullptr);
}
