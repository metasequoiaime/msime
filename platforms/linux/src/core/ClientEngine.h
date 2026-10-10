#pragma once
#include <ibus.h>
#include <string>

// IBUS_INPUT_HINT_PRIVATE（输入框要求不更新个人数据）从 IBus 1.5.26 起才在头文件里。这一位的值属于 D-Bus 协议，是固定的 1 << 11；用更旧的头文件构建时（Debian 10 基线的 legacy 包，IBus 1.5.19）按同一个值补上，客户端带着这一位时行为不变，不带时这一位本来就是 0。
#if !IBUS_CHECK_VERSION(1, 5, 26)
#define IBUS_INPUT_HINT_PRIVATE (1u << 11)
#endif

GType msime_ibus_engine_get_type();
// Set once before registering the factory. The document is prepared by
// host-api.
void msime_ibus_configure(const std::string &options);
void msime_ibus_set_system_dark(bool dark);
// Called once the IBus main loop has returned, before the factory and its engines go: writes every engine's pending key presses on this thread, makes any later flush do the same, and waits for the writes still running on worker threads, so the process does not exit under them.
void msime_ibus_shutdown_key_presses();
// Exit status of msime-linux-ibus after the Ctrl+Shift+Alt+T maintenance stop. msime-linux-ibus-launcher supervises the process and restarts it after a crash; this status tells it the stop was deliberate. Keep the value in step with stop_status in that script.
constexpr int msime_ibus_maintenance_stop_exit = 77;
// True once the maintenance stop shortcut has quit the IBus main loop.
bool msime_ibus_maintenance_stop_requested();
// Exit status of msime-linux-ibus after it found its own program replaced by an upgrade and quit so the new build can take over. The launcher restarts it at once with --recovered, without the crash backoff; keep the value in step with upgraded_status in that script.
constexpr int msime_ibus_upgraded_exit = 78;
// True once an upgrade restart has quit the IBus main loop.
bool msime_ibus_upgrade_restart_requested();
