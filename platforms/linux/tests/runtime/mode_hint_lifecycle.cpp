#include "core/ClientEngine.h"

#include <iostream>
#include <stdexcept>
#include <string>

namespace {
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}

template <typename Predicate> void wait_until(Predicate ready) {
  const auto deadline = g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
  while (!ready() && g_get_monotonic_time() < deadline) {
    while (g_main_context_iteration(nullptr, FALSE)) {
    }
    g_usleep(1000);
  }
  require(ready(), "未收到预期的 IBus 辅助区域信号");
}

struct Observation {
  int shown = 0;
  int hidden = 0;
  bool visible = false;
};

void signal(GDBusConnection *, const gchar *, const gchar *, const gchar *,
            const gchar *name, GVariant *parameters, gpointer data) {
  auto &seen = *static_cast<Observation *>(data);
  if (std::string(name) == "UpdateAuxiliaryText") {
    GVariant *text = nullptr;
    gboolean visible = FALSE;
    g_variant_get(parameters, "(vb)", &text, &visible);
    g_variant_unref(text);
    if (visible) {
      ++seen.shown;
      seen.visible = true;
    }
  } else if (std::string(name) == "HideAuxiliaryText") {
    ++seen.hidden;
    seen.visible = false;
  }
}

// 只启动真实原生宿主和私有 D-Bus，不需要词库，也不接触已安装的输入法。
struct Fixture {
  GTestDBus *bus = nullptr;
  GDBusConnection *server = nullptr;
  GDBusConnection *client = nullptr;
  IBusEngine *engine = nullptr;
  guint subscription = 0;
  Observation seen;

  Fixture() {
    g_unsetenv("XDG_CURRENT_DESKTOP");
    g_setenv("MSIME_DISABLE_IBUS_PROPERTIES", "1", TRUE);
    msime_ibus_configure(
        R"({"preferences":{"default_ime_mode":"english","input_mode_hud":true,"clipboard_history":false,"voice_input":{"enabled":false},"floating_toolbar":{"enabled":false}}})");
    ibus_init();
    bus = g_test_dbus_new(G_TEST_DBUS_NONE);
    g_test_dbus_up(bus);
    const auto connect = [&] {
      return g_dbus_connection_new_for_address_sync(
          g_test_dbus_get_bus_address(bus),
          static_cast<GDBusConnectionFlags>(
              G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
              G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION),
          nullptr, nullptr, nullptr);
    };
    server = connect();
    client = connect();
    require(server && client, "无法建立测试私有 D-Bus");
    engine = IBUS_ENGINE(
        g_object_new(msime_ibus_engine_get_type(), "engine-name", "msime-linux",
                     "object-path", "/app/msime/test/mode_hint", "connection",
                     server, nullptr));
    g_object_ref_sink(engine);
    subscription = g_dbus_connection_signal_subscribe(
        client, g_dbus_connection_get_unique_name(server),
        "org.freedesktop.IBus.Engine", nullptr, "/app/msime/test/mode_hint",
        nullptr, G_DBUS_SIGNAL_FLAGS_NONE, signal, &seen, nullptr);
  }

  void destroy_engine() {
    ibus_object_destroy(IBUS_OBJECT(engine));
    g_object_unref(engine);
    engine = nullptr;
  }

  ~Fixture() {
    if (engine)
      destroy_engine();
    g_dbus_connection_signal_unsubscribe(client, subscription);
    g_dbus_connection_close_sync(client, nullptr, nullptr);
    g_dbus_connection_close_sync(server, nullptr, nullptr);
    g_object_unref(client);
    g_object_unref(server);
    g_test_dbus_down(bus);
    g_object_unref(bus);
  }
};
} // 匿名命名空间

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
  const std::string scenario = argv[1];
  if (scenario != "english" && scenario != "blocked" && scenario != "destroyed")
    return 2;
  try {
    Fixture fixture;
    IBUS_ENGINE_GET_CLASS(fixture.engine)->focus_in(fixture.engine);
    wait_until([&] { return fixture.seen.shown == 1; });
    require(fixture.seen.visible, "英文模式没有显示输入模式提示");
    const auto hidden_before = fixture.seen.hidden;
    if (scenario == "blocked") {
      // 提示仍在等待时走 content_type -> close()，留下 null 视图。
      IBUS_ENGINE_GET_CLASS(fixture.engine)
          ->set_content_type(fixture.engine, IBUS_INPUT_PURPOSE_PASSWORD, 0);
    } else if (scenario == "destroyed") {
      fixture.destroy_engine();
    }

    // 哨兵比 1.2 秒提示定时器晚到，证明回调确实获得执行机会。
    bool elapsed = false;
    const auto timeout = g_timeout_add(
        1500,
        +[](gpointer data) -> gboolean {
          *static_cast<bool *>(data) = true;
          return G_SOURCE_REMOVE;
        },
        &elapsed);
    try {
      wait_until([&] { return elapsed; });
    } catch (...) {
      // 旧回调抛异常时也先撤掉哨兵，避免清理 D-Bus 时访问失效的栈变量。
      if (!elapsed)
        g_source_remove(timeout);
      throw;
    }
    if (scenario == "destroyed") {
      require(fixture.seen.hidden == hidden_before,
              "已销毁上下文的提示回调仍操作辅助区域");
    } else {
      wait_until([&] { return fixture.seen.hidden > hidden_before; });
      require(!fixture.seen.visible, "没有会话时输入模式提示没有收起");
      if (scenario == "english") {
        // 回调正常收起提示以后，宿主仍能处理下一次焦点事件。
        IBUS_ENGINE_GET_CLASS(fixture.engine)->focus_out(fixture.engine);
        IBUS_ENGINE_GET_CLASS(fixture.engine)->focus_in(fixture.engine);
        wait_until([&] { return fixture.seen.shown == 2; });
      }
    }
    std::cout << "IBus mode hint lifecycle: " << scenario << " passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
