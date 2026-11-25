#include "../src/candidates/CandidateActionPolicy.h"

#include <cassert>
#include <string>

int main() {
  using msime::linux_host::candidate_dictionary_removal_available;
  using msime::linux_host::candidate_removal_available;
  using msime::linux_host::candidate_removal_slot;
  assert(!candidate_removal_available(""));
  assert(!candidate_removal_available("a"));
  assert(!candidate_removal_available("字"));
  assert(!candidate_removal_available("😀"));
  assert(candidate_removal_available("ab"));
  assert(candidate_removal_available("词语"));
  assert(candidate_removal_available("😀a"));
  assert(!candidate_removal_available("\xc0\x80"));
  assert(!candidate_removal_available("\xed\xa0\x80"));
  assert(!candidate_removal_available("ab\xff"));
  assert(!candidate_removal_available("词\xe4\xb"));
  assert(candidate_dictionary_removal_available(0, 0, "词语"));
  assert(candidate_dictionary_removal_available(0, 1, "词语"));
  assert(candidate_dictionary_removal_available(0, 4, "词语"));
  assert(candidate_dictionary_removal_available(0, 4, "a"));
  assert(!candidate_dictionary_removal_available(0, 0, "词"));
  assert(!candidate_dictionary_removal_available(0, 4, "a\xff"));
  assert(!candidate_dictionary_removal_available(3, 0, "词语"));
  assert(!candidate_dictionary_removal_available(4, 0, "词语"));
  // 粤拼、注音、越南文、藏文和笔画的候选同样不是用户词库条目。
  assert(!candidate_dictionary_removal_available(5, 0, "你好"));
  assert(!candidate_dictionary_removal_available(6, 0, "你好"));
  assert(!candidate_dictionary_removal_available(7, 0, "xin"));
  assert(!candidate_dictionary_removal_available(8, 0, "བཀྲ་ཤིས"));
  assert(!candidate_dictionary_removal_available(9, 0, "一二"));
  assert(candidate_dictionary_removal_available(2, 0, "词语"));
  using msime::linux_host::candidate_dictionary_actions_available;
  assert(candidate_dictionary_actions_available(0, 0));
  assert(candidate_dictionary_actions_available(1, 4));
  assert(!candidate_dictionary_actions_available(0, 2));
  for (std::uint64_t scheme : {3, 4, 5, 6, 7, 8, 9, 255, 256})
    assert(!candidate_dictionary_actions_available(scheme, 0));
  assert(!candidate_dictionary_removal_available(0, 2, "词语"));
  assert(candidate_removal_slot('1', 0) == 0);
  assert(candidate_removal_slot('8', 0) == 7);
  assert(candidate_removal_slot('&', 2) == 0);
  assert(candidate_removal_slot('x', 9) == 7);
  assert(!candidate_removal_slot('&', 0));
  // Fcitx5 的候选菜单与状态区操作没有实现这组维护快捷键，文案不能提示用户去按。
  assert(std::string(msime::linux_host::candidate_pin_label) == "置顶");
  assert(msime::linux_host::candidate_fix_label(1) == "固定到第 1 位");
  assert(msime::linux_host::candidate_fix_label(5) == "固定到第 5 位");
  assert(std::string(msime::linux_host::candidate_clear_fix_label) == "取消固定");
  assert(std::string(msime::linux_host::candidate_remove_label) == "删除");
  // 只有 iBus 宿主实现了这组快捷键，它的属性菜单和右键菜单才把键写在动作名上。
  assert(std::string(msime::linux_host::candidate_pin_label_with_shortcut) == "置顶 (Ctrl+↑)");
  assert(msime::linux_host::candidate_fix_label_with_shortcut(1) ==
         "固定到第 1 位 (Ctrl+Shift+1)");
  assert(msime::linux_host::candidate_fix_label_with_shortcut(5) ==
         "固定到第 5 位 (Ctrl+Shift+5)");
  assert(std::string(msime::linux_host::candidate_clear_fix_label_with_shortcut) == "取消固定 (Ctrl+0)");
  assert(std::string(msime::linux_host::candidate_remove_label_with_shortcut) == "删除 (Ctrl+Del)");
  // 右键菜单弹不出来时的提示列出全部四组按键。
  const std::string hint = msime::linux_host::candidate_maintenance_shortcut_hint;
  for (const char *chord : {"Ctrl+↑", "Ctrl+1–5", "Ctrl+0", "Ctrl+Del"})
    assert(hint.find(chord) != std::string::npos);
}
