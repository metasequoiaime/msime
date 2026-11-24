#include "GnomePointer.h"

#include <gio/gio.h>

namespace msime::linux_host {

namespace {
// 生产构建由 CMake 注入随版本的名字；契约测试不设宏，用无后缀的默认名加注入的总线名。
#ifndef MSIME_POINTER_BUS_NAME
#define MSIME_POINTER_BUS_NAME "org.msime.Pointer"
#endif
constexpr char kBusName[] = MSIME_POINTER_BUS_NAME;
constexpr char kObjectPath[] = "/org/msime/Pointer";
// 扩展用同一个版本化的名字声明接口（extension.js.in 的 `@MSIME_POINTER_BUS_NAME@`），几个版本的扩展才能在 Shell 的同一条连接、同一个对象路径上各自导出；这里固定成无后缀名，非 full 版本的调用会因接口不存在而全部失败。
constexpr char kInterface[] = MSIME_POINTER_BUS_NAME;
// 右键弹层的定位不能等太久：扩展没装时这次调用尽快失败，调用方改显示快捷键提示。
constexpr int kTimeoutMs = 40;
// 菜单要在 Shell 主线程建 PopupMenu 并接监听；40ms 太紧，超时但调用已执行会让引擎侧
// 拿不到 token。扩展没装时 DO_NOT_AUTO_START 仍会立刻失败，这里只是给忙时的 Shell 余量。
constexpr int kMenuTimeoutMs = 200;
}  // namespace

std::optional<ShellCandidateRect> shell_candidate_rect() {
  return shell_candidate_rect(kBusName, kTimeoutMs);
}

bool shell_show_candidate_menu(const char *token, const char *identity,
                               const std::vector<ShellMenuEntry> &items) {
  return shell_show_candidate_menu(kBusName, token, identity, items, kMenuTimeoutMs);
}

bool shell_hide_candidate_menu(const char *token) {
  return shell_hide_candidate_menu(kBusName, token, kMenuTimeoutMs);
}

std::optional<ShellCandidateRect> shell_candidate_rect(const char *bus_name, int timeout_ms) {
  GError *error = nullptr;
  // 不因查询而激活任何服务：没启用扩展时名字不存在，直接失败。
  GDBusProxy *proxy = g_dbus_proxy_new_for_bus_sync(
      G_BUS_TYPE_SESSION,
      static_cast<GDBusProxyFlags>(G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START |
                                   G_DBUS_PROXY_FLAGS_DO_NOT_LOAD_PROPERTIES),
      nullptr, bus_name, kObjectPath, kInterface, nullptr, &error);
  if (!proxy) {
    if (error) g_error_free(error);
    return std::nullopt;
  }
  GVariant *result = g_dbus_proxy_call_sync(proxy, "GetCandidateRect", nullptr,
                                            G_DBUS_CALL_FLAGS_NONE, timeout_ms, nullptr, &error);
  g_object_unref(proxy);
  if (!result) {
    if (error) g_error_free(error);
    return std::nullopt;
  }
  gint32 x = 0;
  gint32 y = 0;
  gint32 width = 0;
  gint32 height = 0;
  g_variant_get(result, "(iiii)", &x, &y, &width, &height);
  g_variant_unref(result);
  // 候选框不可见时扩展返回全 0；宽高非正一律当作无有效矩形，不让定位函数拿到退化矩形。
  if (width <= 0 || height <= 0) return std::nullopt;
  return ShellCandidateRect{x, y, width, height};
}

bool shell_show_candidate_menu(const char *bus_name, const char *token, const char *identity,
                               const std::vector<ShellMenuEntry> &items, int timeout_ms) {
  GError *error = nullptr;
  // 不因推送菜单而激活扩展：没启用时名字不存在，直接失败，快捷键仍可用。
  GDBusProxy *proxy = g_dbus_proxy_new_for_bus_sync(
      G_BUS_TYPE_SESSION,
      static_cast<GDBusProxyFlags>(G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START |
                                   G_DBUS_PROXY_FLAGS_DO_NOT_LOAD_PROPERTIES),
      nullptr, bus_name, kObjectPath, kInterface, nullptr, &error);
  if (!proxy) {
    if (error) g_error_free(error);
    return false;
  }

  GVariantBuilder builder;
  g_variant_builder_init(&builder, G_VARIANT_TYPE("a(ssuy)"));
  for (const auto &item : items)
    g_variant_builder_add(&builder, "(ssuy)", item.action_name.c_str(), item.label.c_str(),
                          item.flags, item.position);
  GVariant *arguments = g_variant_new("(ssa(ssuy))", token, identity, &builder);
  GVariant *result = g_dbus_proxy_call_sync(proxy, "ShowCandidateMenu", arguments,
                                            G_DBUS_CALL_FLAGS_NONE, timeout_ms, nullptr, &error);
  g_object_unref(proxy);
  if (!result) {
    if (error) g_error_free(error);
    return false;
  }
  g_variant_unref(result);
  return true;
}

bool shell_hide_candidate_menu(const char *bus_name, const char *token, int timeout_ms) {
  GError *error = nullptr;
  GDBusProxy *proxy = g_dbus_proxy_new_for_bus_sync(
      G_BUS_TYPE_SESSION,
      static_cast<GDBusProxyFlags>(G_DBUS_PROXY_FLAGS_DO_NOT_AUTO_START |
                                   G_DBUS_PROXY_FLAGS_DO_NOT_LOAD_PROPERTIES),
      nullptr, bus_name, kObjectPath, kInterface, nullptr, &error);
  if (!proxy) {
    if (error) g_error_free(error);
    return false;
  }
  GVariant *result = g_dbus_proxy_call_sync(proxy, "HideCandidateMenu",
                                            g_variant_new("(s)", token), G_DBUS_CALL_FLAGS_NONE,
                                            timeout_ms, nullptr, &error);
  g_object_unref(proxy);
  if (!result) {
    if (error) g_error_free(error);
    return false;
  }
  g_variant_unref(result);
  return true;
}

}  // namespace msime::linux_host
