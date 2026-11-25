// GNOME Shell 扩展（msime-pointer）的 SessionBus 客户端：把候选右键菜单推给扩展渲染、按 token 收起。文件名沿用早先的指针查询方案。
#pragma once

#include "ShellCandidateMenu.h"

#include <string>
#include <vector>

namespace msime::linux_host {

// 把右键菜单条目推给 GNOME Shell 扩展（token 用于收起匹配，identity 仅展示层一致性）。
// 扩展未启用、总线不可用或超时返回 false，调用方不得阻塞输入。
bool shell_show_candidate_menu(const char *bus_name, const char *token, const char *identity,
                               const std::vector<ShellMenuEntry> &items, int timeout_ms);
// 收起指定 token 的菜单；迟到的收起由扩展按 token 比对，不会误关新菜单。
bool shell_hide_candidate_menu(const char *bus_name, const char *token, int timeout_ms);

// 生产版本：用随版本注入的总线名与短超时。
bool shell_show_candidate_menu(const char *token, const char *identity,
                               const std::vector<ShellMenuEntry> &items);
bool shell_hide_candidate_menu(const char *token);

}  // namespace msime::linux_host
