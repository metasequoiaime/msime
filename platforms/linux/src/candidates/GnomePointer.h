#pragma once

#include "ShellCandidateMenu.h"

#include <optional>
#include <string>
#include <vector>

namespace msime::linux_host {

struct ShellCandidateRect {
  int x = 0;
  int y = 0;
  int width = 0;
  int height = 0;
};

// 从 GNOME Shell 扩展导出的 org.msime.Pointer 读候选框矩形（x/y 左上角，width/height 大小）。
// 扩展未启用、总线不可用、调用超时或候选框不可见（宽高非正）都返回空，调用方据此改在辅助
// 区域显示快捷键提示，而不是弹出一个压在候选框下面或位置错误的菜单。
std::optional<ShellCandidateRect> shell_candidate_rect();

// 供契约测试注入假服务的总线名与超时；生产调用上面的无参版本。
std::optional<ShellCandidateRect> shell_candidate_rect(const char *bus_name, int timeout_ms);

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
