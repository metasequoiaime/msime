#include "../core/ClientEngine.h"
#include "../core/FirstRunGuidance.h"
#include "../core/LinuxEdition.h"
#include "../core/RuntimeOptionsRefresh.h"
#include "../system/SystemTheme.h"
#include <array>
#include <fstream>
#include <iostream>
#include <iterator>
#include <exception>
#include <filesystem>
#include <cstdlib>
#include <nlohmann/json.hpp>
#include "Telemetry.h"

namespace {
struct OptionsWatch {
  const char *path;
  GFile *file;
  guint debounce = 0;
  std::string last_document;
};
void reload_options(OptionsWatch &watch) {
  std::ifstream file(watch.path);
  if (!file)
    return;
  std::array<char, 16385> buffer;
  file.read(buffer.data(), buffer.size());
  if (file.bad() || file.gcount() == 0 ||
      static_cast<std::size_t>(file.gcount()) >= buffer.size())
    return;
  const std::string document(buffer.data(), static_cast<size_t>(file.gcount()));
  if (document == watch.last_document)
    return;
  // Avoid reparsing identical content, including an invalid intermediate save.
  // A later different document is always eligible for another attempt.
  watch.last_document = document;
  try {
    msime_ibus_configure(document);
  } catch (...) {
    g_warning("MSIME settings reload failed");
  }
}

// Only for a process the launcher restarted after a crash. ibus-daemon drops the dead engine from every context it served and never brings it back, so without this the focused editor stays on no input method until the user reselects MSIME. A different global engine means the user has moved on since the crash, and that choice is left alone. An empty one cannot be told apart further: when a registered component goes away the daemon also clears a global engine that came from another component's XML (an xkb layout, say), so a crash while the user was on such an engine brings them back to MSIME rather than to no input method at all.
// Both calls are asynchronous: the synchronous getter logs an IBus warning when there is no global engine, which is the normal state after a crash, and the daemon answers SetGlobalEngine only after this process's factory has created the engine, which needs the main loop that starts after this returns.
void restore_global_engine(IBusBus *bus) {
  ibus_bus_get_global_engine_async(
      bus, -1, nullptr,
      +[](GObject *source, GAsyncResult *result, gpointer) {
        auto bus = IBUS_BUS(source);
        // An error here is the daemon reporting that no global engine is set.
        auto current = ibus_bus_get_global_engine_async_finish(bus, result, nullptr);
        const gchar *name = current ? ibus_engine_desc_get_name(current) : nullptr;
        const bool restore = name == nullptr || *name == '\0' || g_strcmp0(name, MSIME_EDITION_IBUS_ENGINE) == 0;
        if (current)
          g_object_unref(current);
        if (!restore)
          return;
        ibus_bus_set_global_engine_async(
            bus, MSIME_EDITION_IBUS_ENGINE, -1, nullptr,
            +[](GObject *source, GAsyncResult *result, gpointer) {
              GError *error = nullptr;
              if (!ibus_bus_set_global_engine_async_finish(IBUS_BUS(source), result, &error))
                g_warning("MSIME could not reselect itself after a restart: %s",
                          error ? error->message : "unknown error");
              if (error)
                g_error_free(error);
            },
            nullptr);
      },
      nullptr);
}

// Downloaded dictionaries are not replaced by a package upgrade, so a release that raises the dictionary version leaves this user on the previous generation until they fetch the new files. The guide script beside this executable posts the notification, limited to once per login session; it is asynchronous so startup does not wait on it, and GLib reaps the intermediate child.
void notify_dictionary_outdated() {
  std::error_code error;
  const auto executable = std::filesystem::read_symlink("/proc/self/exe", error);
  if (error) return;
  const auto guide = (executable.parent_path() / std::string(msime::linux_host::kFirstRunGuideProgram)).string();
  if (!g_file_test(guide.c_str(), G_FILE_TEST_IS_EXECUTABLE)) return;
  gchar *argv[] = {const_cast<gchar *>(guide.c_str()), const_cast<gchar *>("--reason"),
                   const_cast<gchar *>("dictionary-outdated"), nullptr};
  g_spawn_async(nullptr, argv, nullptr, G_SPAWN_STDOUT_TO_DEV_NULL, nullptr, nullptr, nullptr, nullptr);
}
} // namespace

int main(int argc, char **argv) {
  // Crash capture only writes this session's crash record to disk; the next start reports it. Armed by telemetry::begin below, so a crash before that records nothing.
  std::set_terminate([] { msime::telemetry::record_terminate(); std::abort(); });
  msime::telemetry::install_crash_handlers();
  // --recovered is passed only by the launcher's crash supervisor when it restarts this process.
  const bool recovered = argc == 3 && g_strcmp0(argv[1], "--recovered") == 0;
  if ((argc != 2 && !recovered) || argv[argc - 1][0] != '/') {
    std::cerr << "usage: msime-linux-ibus [--recovered] /absolute/runtime-options.json\n";
    return 2;
  }
  const char *options_path = argv[argc - 1];
  // Panel actions launched from the IBus property menu inherit this process's
  // environment. Keep the direct binary invocation equivalent to the
  // packaged launcher, which already exports the HostOptions path.
  if (!g_setenv("MSIME_CLIENT_HOST_OPTIONS", options_path, FALSE)) {
    std::cerr << "Cannot export runtime options path\n";
    return 1;
  }
  // Before any session exists: a package upgrade leaves the options on the previous dictionary generation until this re-prepares it.
  try {
    if (msime::linux_host::refresh_runtime_options(options_path))
      std::cerr << "Dictionary updated to the installed generation\n";
  } catch (const msime::linux_host::DictionaryOutdated &) {
    std::cerr << "Installed dictionaries are older than this version; keeping the current ones. Run msime-linux-setup --update --download\n";
    notify_dictionary_outdated();
  } catch (...) {
    std::cerr << "Cannot update the dictionary to the installed generation; keeping the current one\n";
  }
  std::string preferences_directory;
  try {
    std::ifstream file(options_path);
    if (!file)
      throw std::runtime_error("Missing configuration");
    std::array<char, 16385> buffer;
    file.read(buffer.data(), buffer.size());
    if (file.bad() || file.gcount() == 0 ||
        static_cast<std::size_t>(file.gcount()) >= buffer.size())
      throw std::runtime_error("Cannot read configuration");
    std::string options(buffer.data(), static_cast<size_t>(file.gcount()));
    msime_ibus_configure(options);
    if (const auto parsed = nlohmann::json::parse(options, nullptr, false); parsed.is_object())
      preferences_directory = parsed.value("preferences_directory", std::string{});
  } catch (...) {
    std::cerr << "Cannot load configuration\n";
    return 1;
  }
  // One reporting session per host process, a supervisor restart included: the session the crash ended is closed by this start (session_crash only when it left a crash record), and active is queued at most once a day whatever the number of starts. The usage_reporting switch is read from the shared preferences; off clears what is queued and sends nothing. begin is file I/O only, and runs before the bus so a crash while starting is recorded too.
  // 使用统计目录按版本分开：default_directory() 是 $XDG_STATE_HOME/msime，其他版本换成同级的 msime-<id>（LinuxEdition.h），full 的结果不变。
  auto telemetry_directory = msime::telemetry::default_directory();
  if (!telemetry_directory.empty())
    telemetry_directory = telemetry_directory.parent_path() / MSIME_EDITION_TELEMETRY_DIRECTORY;
  msime::telemetry::begin({"linux", MSIME_LINUX_VERSION, telemetry_directory, std::nullopt,
                           preferences_directory.empty() || preferences_directory.front() != '/'
                               ? std::filesystem::path()
                               : std::filesystem::path(preferences_directory)});
  ibus_init();
  auto bus = ibus_bus_new();
  if (!ibus_bus_is_connected(bus)) {
    g_object_unref(bus);
    // A restarted host that finds no bus means ibus-daemon went away during the backoff without stopping the supervisor (SIGKILL or a daemon crash). Exit 0 like a disconnect so the supervisor ends instead of retrying forever as an orphan; a later daemon starts its own launcher. The first run still fails loudly.
    return recovered ? 0 : 1;
  }
  auto factory = ibus_factory_new(ibus_bus_get_connection(bus));
  ibus_factory_add_engine(factory, MSIME_EDITION_IBUS_ENGINE,
                          msime_ibus_engine_get_type());
#if IBUS_CHECK_VERSION(1, 5, 27)
  g_signal_connect(factory, "create-engine",
      G_CALLBACK(+[](IBusFactory *factory, const gchar *name, gpointer) -> IBusEngine * {
        if (g_strcmp0(name, MSIME_EDITION_IBUS_ENGINE) != 0)
          return nullptr;
        static guint64 sequence = 0;
        auto path = g_strdup_printf("/org/freedesktop/IBus/Engine/MSIME/%" G_GUINT64_FORMAT,
                                    ++sequence);
        auto engine = IBUS_ENGINE(g_object_new(
            msime_ibus_engine_get_type(), "engine-name", name,
            "object-path", path, "connection",
            ibus_service_get_connection(IBUS_SERVICE(factory)),
            "has-focus-id", TRUE, nullptr));
        g_free(path);
        return engine;
      }), nullptr);
#endif
  // 组件名、引擎名和显示名按版本取（LinuxEdition.h）：几个版本的 IBus 宿主是各自的进程，名字不同才能同时注册。
  auto component = ibus_component_new(
      MSIME_EDITION_TAURI_IDENTIFIER, MSIME_EDITION_IBUS_LONGNAME, "0.1.0",
      "GPL-3.0-only", "MSIME contributors",
      "https://github.com/metasequoiaime/msime", "", "");
  ibus_component_add_engine(
      component,
      ibus_engine_desc_new(MSIME_EDITION_IBUS_ENGINE, MSIME_EDITION_IBUS_LONGNAME,
                           "Shared MSIME Linux input runtime", "zh",
                           "GPL-3.0-only", "MSIME contributors", "", "us"));
  if (!ibus_bus_register_component(bus, component)) {
    g_object_unref(component);
    g_object_unref(factory);
    g_object_unref(bus);
    return 1;
  }
  g_signal_connect(bus, "disconnected",
                   G_CALLBACK(+[](IBusBus *, gpointer) { ibus_quit(); }),
                   nullptr);
  if (recovered)
    restore_global_engine(bus);
  // Delivery runs after registration on a thread that is never joined, so an unreachable endpoint cannot delay the engine and exiting mid-request only leaves the events queued.
  msime::telemetry::start_flushing();
  auto config_file = g_file_new_for_path(options_path);
  OptionsWatch options_watch{options_path, config_file, 0, {}};
  auto config_directory = g_file_get_parent(config_file);
  auto monitor = config_directory
      ? g_file_monitor_directory(config_directory, G_FILE_MONITOR_WATCH_MOVES, nullptr, nullptr)
      : nullptr;
  if (config_directory) g_object_unref(config_directory);
  if (monitor) {
    g_signal_connect(
        monitor, "changed",
        G_CALLBACK(+[](GFileMonitor *, GFile *file, GFile *other, GFileMonitorEvent,
                       gpointer data) {
          auto &watch = *static_cast<OptionsWatch *>(data);
          if ((!file || !g_file_equal(file, watch.file)) &&
              (!other || !g_file_equal(other, watch.file)))
            return;
          if (watch.debounce) g_source_remove(watch.debounce);
          watch.debounce = g_timeout_add(100, +[](gpointer data) -> gboolean {
            auto &watch = *static_cast<OptionsWatch *>(data);
            watch.debounce = 0;
            reload_options(watch);
            return G_SOURCE_REMOVE;
          }, data);
        }),
        &options_watch);
  }
  // Also covers unavailable monitors, replaced parent directories and symlink
  // targets changed outside the watched directory.
  const auto options_poll = g_timeout_add_seconds(5, +[](gpointer data) -> gboolean {
    reload_options(*static_cast<OptionsWatch *>(data));
    return G_SOURCE_CONTINUE;
  }, &options_watch);
  const auto theme_watch = msime_watch_system_theme();
  ibus_main();
  msime_ibus_shutdown_key_presses();
  msime_unwatch_system_theme(theme_watch);
  g_source_remove(options_poll);
  if (options_watch.debounce) g_source_remove(options_watch.debounce);
  if (monitor) {
    g_file_monitor_cancel(monitor);
    g_object_unref(monitor);
  }
  g_object_unref(config_file);
  g_object_unref(component);
  g_object_unref(factory);
  g_object_unref(bus);
  // Every way out of the main loop is a normal end of this session.
  msime::telemetry::end();
  // 0 means the bus went away because ibus-daemon is exiting or restarting, so the supervisor must not restart this host. After `ibus restart` the new daemon starts the launcher again when the engine is selected; after `ibus exit` nothing runs again, which is intended.
  if (msime_ibus_maintenance_stop_requested())
    return msime_ibus_maintenance_stop_exit;
  return msime_ibus_upgrade_restart_requested() ? msime_ibus_upgraded_exit : 0;
}
