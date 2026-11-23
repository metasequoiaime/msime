// GNOME Shell 候选框矩形与右键菜单 D-Bus 契约测试：验证助手能读扩展给出的候选框矩形，
// 候选框不可见（全 0）或扩展不在时返回空；并验证菜单条目能以 a(ssuy) 推给扩展、
// 收起 token 能原样回传，缺服务时两条菜单调用都快速失败。
#include <chrono>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <optional>
#include <string>
#include <thread>
#include <tuple>
#include <vector>

#include <gio/gio.h>

#include "GnomePointer.h"
#include "ShellCandidateMenu.h"

using msime::linux_host::ShellCandidateRect;
using msime::linux_host::ShellMenuEntry;

namespace {
constexpr const char kBusName[] = "org.msime.Pointer.Test";
constexpr const char kObjectPath[] = "/org/msime/Pointer";
constexpr const char kInterfaceXml[] =
    "<node><interface name='org.msime.Pointer'>"
    "<method name='GetCandidateRect'>"
    "<arg type='i' direction='out' name='x'/>"
    "<arg type='i' direction='out' name='y'/>"
    "<arg type='i' direction='out' name='width'/>"
    "<arg type='i' direction='out' name='height'/>"
    "</method>"
    "<method name='ShowCandidateMenu'>"
    "<arg type='s' direction='in' name='token'/>"
    "<arg type='s' direction='in' name='identity'/>"
    "<arg type='a(ssuy)' direction='in' name='items'/>"
    "</method>"
    "<method name='HideCandidateMenu'>"
    "<arg type='s' direction='in' name='token'/>"
    "</method>"
    "</interface></node>";

ShellCandidateRect served{123, 456, 300, 40};
bool name_acquired = false;

using ReceivedItem = std::tuple<std::string, std::string, std::uint32_t, std::uint8_t>;
struct ReceivedMenu {
  std::string token;
  std::string identity;
  std::vector<ReceivedItem> items;
};
ReceivedMenu shown;
std::string hidden_token;

void on_method_call(GDBusConnection *, const gchar *, const gchar *, const gchar *,
                    const gchar *method, GVariant *parameters,
                    GDBusMethodInvocation *invocation, gpointer) {
  if (g_strcmp0(method, "GetCandidateRect") == 0) {
    g_dbus_method_invocation_return_value(invocation, g_variant_new("(iiii)", served.x, served.y,
                                                                   served.width, served.height));
    return;
  }
  if (g_strcmp0(method, "ShowCandidateMenu") == 0) {
    ReceivedMenu menu;
    const gchar *token = nullptr;
    const gchar *identity = nullptr;
    GVariantIter *items = nullptr;
    g_variant_get(parameters, "(&s&sa(ssuy))", &token, &identity, &items);
    menu.token = token;
    menu.identity = identity;
    const gchar *action = nullptr;
    const gchar *label = nullptr;
    std::uint32_t flags = 0;
    std::uint8_t position = 0;
    while (g_variant_iter_loop(items, "(&s&suy)", &action, &label, &flags, &position))
      menu.items.emplace_back(action, label, flags, position);
    g_variant_iter_free(items);
    shown = std::move(menu);
    g_dbus_method_invocation_return_value(invocation, g_variant_new("()"));
    return;
  }
  if (g_strcmp0(method, "HideCandidateMenu") == 0) {
    const gchar *token = nullptr;
    g_variant_get(parameters, "(&s)", &token);
    hidden_token = token;
    g_dbus_method_invocation_return_value(invocation, g_variant_new("()"));
    return;
  }
  g_dbus_method_invocation_return_error(invocation, G_DBUS_ERROR, G_DBUS_ERROR_UNKNOWN_METHOD,
                                        "未知方法");
}

const GDBusInterfaceVTable kMethods = {on_method_call, nullptr, nullptr, {nullptr}};

void on_bus_acquired(GDBusConnection *connection, const gchar *, gpointer) {
  GError *error = nullptr;
  GDBusNodeInfo *node = g_dbus_node_info_new_for_xml(kInterfaceXml, &error);
  if (!node) {
    if (error) g_error_free(error);
    return;
  }
  g_dbus_connection_register_object(connection, kObjectPath, node->interfaces[0], &kMethods,
                                    nullptr, nullptr, &error);
  if (error) g_error_free(error);
  g_dbus_node_info_unref(node);
}

void on_name_acquired(GDBusConnection *, const gchar *, gpointer) { name_acquired = true; }

void pump_ms(int milliseconds) {
  for (int i = 0; i < milliseconds / 10; ++i) {
    while (g_main_context_iteration(nullptr, FALSE)) {
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(10));
  }
}
}  // namespace

int main() {
  GError *error = nullptr;
  // CI 容器里没有会话总线；这条契约需要真实 bus 才能跑。
  GDBusConnection *bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
  if (!bus) {
    if (error) g_error_free(error);
    return 77;
  }
  g_object_unref(bus);

  guint owner = g_bus_own_name(G_BUS_TYPE_SESSION, kBusName, G_BUS_NAME_OWNER_FLAGS_NONE,
                               on_bus_acquired, on_name_acquired, nullptr, nullptr, nullptr);
  for (int i = 0; i < 200 && !name_acquired; ++i) pump_ms(10);
  if (!name_acquired) {
    g_bus_unown_name(owner);
    return 77;
  }

  // 同步助手会阻塞自己所在线程；假服务必须在另一条线程里继续泵事件才能应答。
  std::optional<ShellCandidateRect> got;
  std::thread caller([&] { got = msime::linux_host::shell_candidate_rect(kBusName, 2000); });
  pump_ms(300);
  caller.join();
  if (!got || got->x != 123 || got->y != 456 || got->width != 300 || got->height != 40) {
    g_bus_unown_name(owner);
    return 1;
  }

  // 候选框不可见：扩展返回全 0，助手应判为无有效矩形，不把 0 矩形交给定位函数。
  served = {0, 0, 0, 0};
  std::optional<ShellCandidateRect> invisible;
  std::thread invisible_caller(
      [&] { invisible = msime::linux_host::shell_candidate_rect(kBusName, 2000); });
  pump_ms(300);
  invisible_caller.join();
  if (invisible) {
    g_bus_unown_name(owner);
    return 3;
  }

  // 菜单推送：三项 fixture（置顶、固定第 2 位勾选、分隔线），扩展侧收到的字段应与
  // 传入逐项相等，action_name 保持完整的 session/generation/index 编码供原样回传。
  const std::vector<ShellMenuEntry> entries = {
      {"CandidatePin/42/7/3", "置顶 (Ctrl+↑)", 0, 0},
      {"CandidateFix2/42/7/3", "固定到第 2 位 (Ctrl+2)",
       msime::linux_host::kShellMenuFlagChecked, 2},
      {"", "", msime::linux_host::kShellMenuFlagSeparator, 0},
  };
  bool show_ok = false;
  std::thread show_caller([&] {
    show_ok = msime::linux_host::shell_show_candidate_menu(kBusName, "tok-1", "42/7/3", entries,
                                                           2000);
  });
  pump_ms(300);
  show_caller.join();
  if (!show_ok || shown.token != "tok-1" || shown.identity != "42/7/3" ||
      shown.items.size() != entries.size()) {
    g_bus_unown_name(owner);
    return 4;
  }
  if (std::get<0>(shown.items[0]) != "CandidatePin/42/7/3" ||
      std::get<1>(shown.items[0]) != "置顶 (Ctrl+↑)" || std::get<2>(shown.items[0]) != 0 ||
      std::get<3>(shown.items[0]) != 0) {
    g_bus_unown_name(owner);
    return 5;
  }
  if (std::get<0>(shown.items[1]) != "CandidateFix2/42/7/3" ||
      std::get<2>(shown.items[1]) != msime::linux_host::kShellMenuFlagChecked ||
      std::get<3>(shown.items[1]) != 2) {
    g_bus_unown_name(owner);
    return 6;
  }
  if (!std::get<0>(shown.items[2]).empty() ||
      std::get<2>(shown.items[2]) != msime::linux_host::kShellMenuFlagSeparator) {
    g_bus_unown_name(owner);
    return 7;
  }

  // 收起：token 原样回传，供扩展核对是否迟到的收起。
  bool hide_ok = false;
  std::thread hide_caller(
      [&] { hide_ok = msime::linux_host::shell_hide_candidate_menu(kBusName, "tok-1", 2000); });
  pump_ms(300);
  hide_caller.join();
  if (!hide_ok || hidden_token != "tok-1") {
    g_bus_unown_name(owner);
    return 8;
  }

  g_bus_unown_name(owner);
  pump_ms(100);  // 给总线一点时间释放名字。
  const auto missing = msime::linux_host::shell_candidate_rect(kBusName, 40);
  if (missing) return 2;
  bool show_missing = msime::linux_host::shell_show_candidate_menu(kBusName, "tok-1", "42/7/3",
                                                                   entries, 40);
  bool hide_missing = msime::linux_host::shell_hide_candidate_menu(kBusName, "tok-1", 40);
  if (show_missing || hide_missing) return 9;
  return 0;
}
