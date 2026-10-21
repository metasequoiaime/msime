#include "core/ClientEngine.h"

#include <iostream>
#include <optional>
#include <stdexcept>
#include <string>
#include <fstream>
#include <filesystem>
#include <xkbcommon/xkbcommon-keysyms.h>
#ifdef MSIME_TEST_X11_BADGE
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#endif

namespace {
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}

template <typename Predicate>
void wait_until(Predicate ready, const char *message = "未收到预期的 IBus 辅助区域信号") {
  const auto deadline = g_get_monotonic_time() + 5 * G_USEC_PER_SEC;
  while (!ready() && g_get_monotonic_time() < deadline) {
    while (g_main_context_iteration(nullptr, FALSE)) {
    }
    g_usleep(1000);
  }
  require(ready(), message);
}

#ifdef MSIME_TEST_X11_BADGE
// 让主循环空转 milliseconds 毫秒，期间到期的定时器都有机会执行。
void run_for(int milliseconds) {
  bool elapsed = false;
  const auto timeout = g_timeout_add(
      milliseconds,
      +[](gpointer data) -> gboolean {
        *static_cast<bool *>(data) = true;
        return G_SOURCE_REMOVE;
      },
      &elapsed);
  try {
    wait_until([&] { return elapsed; }, "主循环没有按时走完");
  } catch (...) {
    // 旧回调抛异常时也先撤掉哨兵，避免清理 D-Bus 时访问失效的栈变量。
    if (!elapsed)
      g_source_remove(timeout);
    throw;
  }
}

struct BadgeWindow {
  int x = 0;
  int y = 0;
};
// 根窗口下正在显示的 override-redirect 窗口：这个测试进程里只有切换徽章会建这种窗口。
std::optional<BadgeWindow> visible_badge() {
  Display *display = XOpenDisplay(nullptr);
  require(display != nullptr, "连不上 Xvfb");
  Window root = 0, parent = 0;
  Window *children = nullptr;
  unsigned count = 0;
  std::optional<BadgeWindow> found;
  if (XQueryTree(display, DefaultRootWindow(display), &root, &parent, &children, &count)) {
    for (unsigned index = 0; index < count; ++index) {
      XWindowAttributes attributes{};
      if (XGetWindowAttributes(display, children[index], &attributes) && attributes.override_redirect &&
          attributes.map_state == IsViewable)
        found = BadgeWindow{attributes.x, attributes.y};
    }
  }
  if (children)
    XFree(children);
  XCloseDisplay(display);
  return found;
}

// 根窗口下正在显示、WM_CLASS 是 resource_class 的窗口，例如悬浮工具栏的 MSIMEFloatingToolbar。
bool window_shown(const char *resource_class) {
  Display *display = XOpenDisplay(nullptr);
  require(display != nullptr, "连不上 Xvfb");
  Window root = 0, parent = 0;
  Window *children = nullptr;
  unsigned count = 0;
  bool found = false;
  if (XQueryTree(display, DefaultRootWindow(display), &root, &parent, &children, &count)) {
    for (unsigned index = 0; index < count; ++index) {
      XWindowAttributes attributes{};
      XClassHint klass{};
      if (!XGetWindowAttributes(display, children[index], &attributes) || attributes.map_state != IsViewable ||
          !XGetClassHint(display, children[index], &klass))
        continue;
      found = found || std::string(klass.res_class) == resource_class;
      XFree(klass.res_name);
      XFree(klass.res_class);
    }
  }
  if (children)
    XFree(children);
  XCloseDisplay(display);
  return found;
}
#endif

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

  explicit Fixture(bool x11, bool toolbar = false) {
    g_unsetenv("XDG_CURRENT_DESKTOP");
    // 默认验证辅助文本那条路：没有显示服务器时宿主拿不到自绘徽章，只能用辅助文本提示。x11 场景由 xvfb-run 给出 DISPLAY，宿主把整个会话当成 X11，在光标下画徽章。
    if (!x11)
      g_unsetenv("DISPLAY");
    g_unsetenv("WAYLAND_DISPLAY");
    const std::string display = x11 && g_getenv("DISPLAY") ? g_getenv("DISPLAY") : "";
    g_setenv("MSIME_DISABLE_IBUS_PROPERTIES", "1", TRUE);
    // 悬浮工具栏默认关着：别的场景只看切换徽章，屏幕上不该多出一块窗口。
    msime_ibus_configure(
        toolbar
            ? R"({"preferences":{"default_ime_mode":"english","input_mode_hud":false,"clipboard_history":false,"voice_input":{"enabled":false},"floating_toolbar":{"enabled":true}}})"
            : R"({"preferences":{"default_ime_mode":"english","input_mode_hud":true,"clipboard_history":false,"voice_input":{"enabled":false},"floating_toolbar":{"enabled":false}}})");
    ibus_init();
    bus = g_test_dbus_new(G_TEST_DBUS_NONE);
    g_test_dbus_up(bus);
    // g_test_dbus_up 会清掉 DISPLAY，免得测试连到用户的会话；x11 场景要的正是 xvfb-run 给的那个，放回去。
    if (!display.empty())
      g_setenv("DISPLAY", display.c_str(), TRUE);
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

  // 像客户端那样经 D-Bus 报光标位置，走 IBus 自己写 cursor_area 的那段处理。异步调用：服务端在这条线程的主循环里应答，同步调用会把自己卡住。
  void report_cursor(int x, int y, int width, int height) {
    bool replied = false;
    g_dbus_connection_call(
        client, g_dbus_connection_get_unique_name(server), "/app/msime/test/mode_hint",
        "org.freedesktop.IBus.Engine", "SetCursorLocation", g_variant_new("(iiii)", x, y, width, height), nullptr,
        G_DBUS_CALL_FLAGS_NONE, -1, nullptr,
        +[](GObject *source, GAsyncResult *result, gpointer data) {
          GError *error = nullptr;
          if (auto *reply = g_dbus_connection_call_finish(G_DBUS_CONNECTION(source), result, &error))
            g_variant_unref(reply);
          if (error)
            g_error_free(error);
          *static_cast<bool *>(data) = true;
        },
        &replied);
    wait_until([&] { return replied; }, "SetCursorLocation 没有应答");
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
  const bool toolbar = scenario == "x11-toolbar";
  const bool x11 = scenario == "x11-badge" || scenario == "x11-badge-no-caret" || scenario == "x11-badge-point" || toolbar;
  if (scenario != "english" && scenario != "blocked" && scenario != "destroyed" &&
      scenario != "compose" && !x11)
    return 2;
  try {
    std::filesystem::path compose_directory;
    if (scenario == "compose") {
      gchar *temporary = g_dir_make_tmp("msime-compose-test-XXXXXX", nullptr);
      require(temporary != nullptr, "无法创建 Compose 测试目录");
      compose_directory = temporary;
      g_free(temporary);
      std::ofstream(compose_directory / "compose.fixture")
          << "<dead_circumflex> <e> : \"ê\"\n";
      g_setenv("XCOMPOSEFILE", (compose_directory / "compose.fixture").c_str(), TRUE);
    }
    struct ComposeCleanup {
      std::filesystem::path directory;
      ~ComposeCleanup() {
        if (directory.empty())
          return;
        g_unsetenv("XCOMPOSEFILE");
        std::error_code error;
        std::filesystem::remove_all(directory, error);
      }
    } compose_cleanup{compose_directory};
    Fixture fixture(x11, toolbar);
    if (x11) {
#ifdef MSIME_TEST_X11_BADGE
      IBUS_ENGINE_GET_CLASS(fixture.engine)->focus_in(fixture.engine);
      if (toolbar) {
        // 获得焦点就显示悬浮工具栏；失焦照 macOS 留着（X11 会话）；换到别的输入法（disable）收起。
        wait_until([] { return window_shown("MSIMEFloatingToolbar"); }, "获得焦点后没有显示悬浮工具栏");
        IBUS_ENGINE_GET_CLASS(fixture.engine)->focus_out(fixture.engine);
        run_for(100);
        require(window_shown("MSIMEFloatingToolbar"), "失焦后悬浮工具栏不该收起");
        IBUS_ENGINE_GET_CLASS(fixture.engine)->disable(fixture.engine);
        wait_until([] { return !window_shown("MSIMEFloatingToolbar"); }, "换输入法后悬浮工具栏没有收起");
        // 引擎销毁时放开属主，之后再获得焦点的新引擎接手。
        fixture.destroy_engine();
        run_for(50);
        std::cout << "IBus mode hint lifecycle: " << scenario << " passed\n";
        return 0;
      }
      if (scenario == "x11-badge") {
        // 获得焦点时先不画，等客户端报来光标再画在它下方，并且不发辅助文本：一次切换只出一处提示。
        run_for(20);
        require(!visible_badge(), "客户端还没报光标就画了徽章");
        fixture.report_cursor(200, 300, 2, 20);
        std::optional<BadgeWindow> badge;
        wait_until([&] { return (badge = visible_badge()).has_value(); }, "报来光标后没有画徽章");
        require(badge->y >= 300 + 20 && badge->y < 300 + 20 + 40 && badge->x > 100 && badge->x < 220,
                "徽章没有放在光标下方");
        require(fixture.seen.shown == 0, "画了徽章还发辅助文本");
        // 引擎在徽章显示期间销毁：徽章属于整个进程，淡出回调照常收起它，不碰已销毁的引擎。
        fixture.destroy_engine();
        run_for(1500);
        require(!visible_badge(), "引擎销毁后徽章没有收起");
      } else if (scenario == "x11-badge-point") {
        // 全 0 的报告还不是光标，接着等；XIM 客户端报的是一个点（宽高为 0），那就是光标：徽章放在它下方，不放到屏幕下方的回退位置，也不发辅助文本。
        fixture.report_cursor(0, 0, 0, 0);
        run_for(20);
        require(!visible_badge(), "全 0 的光标报告就画了徽章");
        fixture.report_cursor(200, 300, 0, 0);
        std::optional<BadgeWindow> badge;
        wait_until([&] { return (badge = visible_badge()).has_value(); }, "报来点状光标后没有画徽章");
        require(badge->y >= 300 && badge->y < 300 + 40 && badge->x > 100 && badge->x < 220,
                "徽章没有放在点状光标下方");
        require(fixture.seen.shown == 0, "画了徽章还发辅助文本");
        // 离开输入框时立刻收起，不等 0.6 秒后的淡出，与 macOS 的 deactivateServer: 相同。
        IBUS_ENGINE_GET_CLASS(fixture.engine)->focus_out(fixture.engine);
        run_for(50);
        require(!visible_badge(), "失焦后徽章没有立刻收起");
        // 再次获得焦点后、光标报来之前就开始打字：那条获得焦点提示作废，徽章不再盖到随后出现的候选窗上，也不改发辅助文本。
        IBUS_ENGINE_GET_CLASS(fixture.engine)->focus_in(fixture.engine);
        IBUS_ENGINE_GET_CLASS(fixture.engine)->process_key_event(fixture.engine, XKB_KEY_a, 30, 0);
        fixture.report_cursor(250, 400, 0, 0);
        run_for(300);
        require(!visible_badge(), "开始打字后仍放了获得焦点的徽章");
        require(fixture.seen.shown == 0, "开始打字后改发了辅助文本");
      } else {
        // 等满时间仍没报光标：不把徽章放到屏幕下方，改用辅助文本。
        wait_until([&] { return fixture.seen.shown == 1; });
        require(!visible_badge(), "没有光标时仍画了徽章");
      }
      std::cout << "IBus mode hint lifecycle: " << scenario << " passed\n";
      return 0;
#else
      return 2;
#endif
    }
    IBUS_ENGINE_GET_CLASS(fixture.engine)->focus_in(fixture.engine);
    wait_until([&] { return fixture.seen.shown == 1; });
    require(fixture.seen.visible, "英文模式没有显示输入模式提示");
    const auto hidden_before = fixture.seen.hidden;
    if (scenario == "compose") {
      auto *engine_class = IBUS_ENGINE_GET_CLASS(fixture.engine);
      require(engine_class->process_key_event(fixture.engine, XKB_KEY_dead_circumflex, 0, 0),
              "英文模式没有消费 Compose 死键");
      require(engine_class->process_key_event(fixture.engine, XKB_KEY_e, 0, 0),
              "英文模式没有消费 Compose 完成键");
      std::cout << "IBus native Compose without an Engine session passed\n";
      return 0;
    }
    if (scenario == "blocked") {
      // 提示仍在等待时走 content_type -> close()，留下 null 视图。
      IBUS_ENGINE_GET_CLASS(fixture.engine)
          ->set_content_type(fixture.engine, IBUS_INPUT_PURPOSE_PASSWORD, 0);
    } else if (scenario == "destroyed") {
      fixture.destroy_engine();
    }

    // 哨兵比 0.6 秒提示定时器晚到，证明回调确实获得执行机会。
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
