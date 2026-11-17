#include "wlr-data-control-unstable-v1-client-protocol.h"
#include <wayland-client.h>
#include <algorithm>
#include <csignal>
#include <cstring>
#include <iostream>
#include <iterator>
#include <string>
#include <unistd.h>

namespace {
wl_seat *seat = nullptr;
zwlr_data_control_manager_v1 *manager = nullptr;
std::string text;
void global(void *, wl_registry *registry, uint32_t name, const char *interface, uint32_t version) {
  if (!manager && std::strcmp(interface, zwlr_data_control_manager_v1_interface.name) == 0) {
    manager = static_cast<zwlr_data_control_manager_v1 *>(wl_registry_bind(
        registry, name, &zwlr_data_control_manager_v1_interface, std::min(version, 2u)));
  } else if (!seat && std::strcmp(interface, wl_seat_interface.name) == 0) {
    seat = static_cast<wl_seat *>(wl_registry_bind(registry, name, &wl_seat_interface, 1));
  }
}
void removed(void *, wl_registry *, uint32_t) {}
void mime(void *, zwlr_data_control_offer_v1 *, const char *) {}
void offer(void *, zwlr_data_control_device_v1 *, zwlr_data_control_offer_v1 *value) {
  static const zwlr_data_control_offer_v1_listener types{mime};
  zwlr_data_control_offer_v1_add_listener(value, &types, nullptr);
}
void selection(void *, zwlr_data_control_device_v1 *, zwlr_data_control_offer_v1 *value) {
  if (value) zwlr_data_control_offer_v1_destroy(value);
}
void finished(void *, zwlr_data_control_device_v1 *) { std::exit(0); }
void send(void *, zwlr_data_control_source_v1 *, const char *, int32_t fd) {
  // 本测试只发短合成文本；敏感标记与明文同时提供，不能靠缺文本类型碰巧通过。
  (void)write(fd, text.data(), text.size());
  close(fd);
}
void cancelled(void *, zwlr_data_control_source_v1 *) { std::exit(0); }
} // namespace

int main(int argc, char **argv) {
  signal(SIGPIPE, SIG_IGN);
  text.assign(std::istreambuf_iterator<char>(std::cin), std::istreambuf_iterator<char>());
  auto *display = wl_display_connect(nullptr);
  if (!display) return 1;
  auto *registry = wl_display_get_registry(display);
  const wl_registry_listener globals{global, removed};
  wl_registry_add_listener(registry, &globals, nullptr);
  if (wl_display_roundtrip(display) < 0 || !seat || !manager) return 2;
  auto *device = zwlr_data_control_manager_v1_get_data_device(manager, seat);
  const zwlr_data_control_device_v1_listener devices{offer, selection, finished, selection};
  zwlr_data_control_device_v1_add_listener(device, &devices, nullptr);
  auto *source = zwlr_data_control_manager_v1_create_data_source(manager);
  const zwlr_data_control_source_v1_listener sources{send, cancelled};
  zwlr_data_control_source_v1_add_listener(source, &sources, nullptr);
  for (int index = 1; index < argc; ++index) zwlr_data_control_source_v1_offer(source, argv[index]);
  zwlr_data_control_device_v1_set_selection(device, source);
  while (wl_display_dispatch(display) >= 0) {}
  return 0;
}
