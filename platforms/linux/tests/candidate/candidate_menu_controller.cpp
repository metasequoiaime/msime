// 候选右键菜单的收起与 Esc（CandidateMenuController）：GNOME 扩展的两条 D-Bus 调用换成假实现。扩展的菜单点外面或候选框收起时自己关掉、不通知宿主，宿主手里的 token 会残留；这时按一次 Esc 必须照常取消组字（consume_escape 返回 false），不能被当成「菜单还开着」吞掉。
#include "candidates/CandidateMenuController.h"

#include <cassert>
#include <string>
#include <vector>

using msime::linux_host::CandidateMenuController;
using msime::linux_host::ShellMenuEntry;

namespace {

struct FakeShell {
  bool available = true;
  std::vector<std::string> shown_tokens;
  std::vector<std::string> identities;
  std::vector<std::vector<ShellMenuEntry>> shown_entries;
  std::vector<std::string> hidden_tokens;
};

CandidateMenuController::ShellCalls calls(FakeShell &shell) {
  return {[&shell](const std::string &token, const std::string &identity,
                   const std::vector<ShellMenuEntry> &entries) {
            if (!shell.available)
              return false;
            shell.shown_tokens.push_back(token);
            shell.identities.push_back(identity);
            shell.shown_entries.push_back(entries);
            return true;
          },
          [&shell](const std::string &token) { shell.hidden_tokens.push_back(token); }};
}

}  // namespace

int main() {
  const auto items = msime::linux_host::candidate_menu_items(true);
  const CandidateMenuController::Target target{42, 7, 3};

  // 推给扩展：条目带完整动作名，identity 是 session/generation/index。
  {
    FakeShell shell;
    CandidateMenuController menu(calls(shell));
    assert(menu.show_in_shell(items, target));
    assert(shell.shown_tokens.size() == 1);
    assert(shell.identities[0] == "42/7/3");
    assert(shell.shown_entries[0][0].action_name == "CandidatePin/42/7/3");

    // 扩展已经自己关了菜单，token 还留在宿主这边：一次 Esc 就取消组字，并顺手按 token 收起。
    assert(!menu.consume_escape());
    assert((shell.hidden_tokens == std::vector<std::string>{shell.shown_tokens[0]}));
    // token 已清掉：之后的 Esc 和会话出口都不再发收起。
    assert(!menu.consume_escape());
    menu.release();
    assert(shell.hidden_tokens.size() == 1);
  }

  // 新一轮菜单换新 token；推送前收起上一轮，迟到的收起由扩展按 token 比对。
  {
    FakeShell shell;
    CandidateMenuController menu(calls(shell));
    assert(menu.show_in_shell(items, target));
    assert(menu.show_in_shell(items, target));
    assert(shell.shown_tokens.size() == 2 && shell.shown_tokens[0] != shell.shown_tokens[1]);
    assert((shell.hidden_tokens == std::vector<std::string>{shell.shown_tokens[0]}));
    menu.release();
    assert(shell.hidden_tokens.size() == 2 && shell.hidden_tokens[1] == shell.shown_tokens[1]);
  }

  // 扩展没启用：show_in_shell 返回 false（调用方改提示快捷键），没有 token 要收。
  {
    FakeShell shell;
    shell.available = false;
    CandidateMenuController menu(calls(shell));
    assert(!menu.show_in_shell(items, target));
    assert(!menu.consume_escape());
    menu.release();
    assert(shell.hidden_tokens.empty());
  }

  // 析构时收起还挂着的扩展菜单。
  {
    FakeShell shell;
    {
      CandidateMenuController menu(calls(shell));
      assert(menu.show_in_shell(items, target));
    }
    assert(shell.hidden_tokens.size() == 1);
  }

#ifndef MSIME_LINUX_FLOATING_PANELS
  // 没有悬浮面板的构建画不出自绘浮层，调用方改提示快捷键。
  {
    FakeShell shell;
    CandidateMenuController menu(calls(shell));
    assert(!menu.show_overlay(items, target, {}, {}, [](const std::string &) {}));
  }
#endif
}
