#define GDK_VERSION_MIN_REQUIRED GDK_VERSION_4_8
#define GDK_VERSION_MAX_ALLOWED GDK_VERSION_4_8

#include <gtk/gtk.h>
#include <nlohmann/json.hpp>
#include "../../../common/CloudQuickPhraseClearModel.h"
#include "../../../crates/host-api/include/msime_client.h"
#include "../core/RuntimeOptionsFile.h"

#include <chrono>
#include <future>
#include <optional>
#include <string>
#include <unistd.h>

namespace {
using Json = nlohmann::json;

std::string options_path(int argc, char **argv) {
  if (argc == 3 && std::string(argv[1]) == "--options") return argv[2];
  if (const char *value = g_getenv("MSIME_CLIENT_HOST_OPTIONS"); value && *value)
    return value;
  if (const char *value = g_getenv("MSIME_FCITX5_OPTIONS"); value && *value)
    return value;
  return {};
}

std::string request_id() {
  return "quick-clear-" + std::to_string(static_cast<unsigned long>(::getpid())) + "-" +
         std::to_string(std::chrono::steady_clock::now().time_since_epoch().count());
}

Json call_clear(const std::string &path) {
  const auto options = msime::linux_host::read_runtime_options(path);
  const auto request = Json{{"options", Json::parse(options)},
                            {"action", {{"operation", "clear_quick_phrases"},
                                         {"request_id", request_id()}}}};
  const auto bytes = request.dump();
  char *response = msime_client_settings_dictionary(
      reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size());
  if (!response) return Json{{"ok", false}, {"error", "dictionary_unavailable"}};
  struct Response { char *value; ~Response() { msime_client_string_free(value); } } owned{response};
  auto parsed = Json::parse(response, nullptr, false);
  return parsed.is_discarded() ? Json{{"ok", false}, {"error", "dictionary_unavailable"}} : parsed;
}

struct Window {
  explicit Window(std::string path) : options(std::move(path)) {}

  std::string options;
  GtkApplication *app = nullptr;
  GtkWidget *window = nullptr;
  GtkWidget *status = nullptr;
  GtkWidget *clear = nullptr;
  GtkWidget *cloud_clear = nullptr;
  GtkWidget *cloud_dialog = nullptr;
  std::optional<msime::cloud_quick_clear::Model> cloud;
  std::future<Json> pending;
  bool closing = false;

  bool busy() const {
    return pending.valid() || (cloud && cloud->busy()) || cloud_dialog;
  }

  void update_actions() {
    gtk_widget_set_sensitive(clear, !closing && !busy());
    gtk_widget_set_sensitive(cloud_clear, !closing && !busy() && cloud.has_value());
  }

  void cloud_confirmation(uint64_t entries, std::string token) {
    cloud_dialog = gtk_message_dialog_new(GTK_WINDOW(window), GTK_DIALOG_MODAL,
        GTK_MESSAGE_WARNING, GTK_BUTTONS_NONE, "清空 %llu 条云端快捷短语？", static_cast<unsigned long long>(entries));
    gtk_message_dialog_format_secondary_text(GTK_MESSAGE_DIALOG(cloud_dialog), "%s", "其他云端词库保留。此操作无法撤销。");
    gtk_dialog_add_buttons(GTK_DIALOG(cloud_dialog), "取消", GTK_RESPONSE_CANCEL, "清空", GTK_RESPONSE_ACCEPT, nullptr);
    gtk_dialog_set_default_response(GTK_DIALOG(cloud_dialog), GTK_RESPONSE_CANCEL);
    g_object_set_data_full(G_OBJECT(cloud_dialog), "quick-clear-token", new std::string(std::move(token)), +[](gpointer value) { delete static_cast<std::string *>(value); });
    g_signal_connect(cloud_dialog, "response", G_CALLBACK(+[](GtkDialog *dialog, int response, gpointer data) {
      auto &self = *static_cast<Window *>(data);
      const auto token = *static_cast<std::string *>(g_object_get_data(G_OBJECT(dialog), "quick-clear-token"));
      self.cloud_dialog = nullptr;
      if (response == GTK_RESPONSE_ACCEPT && !self.closing) { self.cloud->apply(token); self.show_status("正在清空云端快捷短语…"); }
      else { self.cloud->cancel(); self.show_status("已取消云端清空。"); }
      gtk_window_destroy(GTK_WINDOW(dialog)); self.update_actions();
    }), this);
    gtk_window_present(GTK_WINDOW(cloud_dialog));
  }

  void show_status(const std::string &text) { gtk_label_set_text(GTK_LABEL(status), text.c_str()); }

  void poll() {
    if (pending.valid() && pending.wait_for(std::chrono::milliseconds(0)) == std::future_status::ready) {
      const auto result = pending.get();
      if (result.value("ok", false)) {
        const auto cleared = result.value("value", Json::object()).value("cleared", 0u);
        show_status("已清空 " + std::to_string(cleared) + " 条快捷短语。");
      } else show_status("清空失败，请稍后重试。");
    }
    if (cloud && cloud->poll()) {
      if (!cloud->error.empty()) show_status(msime::cloud_quick_clear::Model::message(cloud->error));
      else if (!closing && cloud->operation == "quick_clear_preview") {
        const auto entries = cloud->value.at("entries").get<uint64_t>();
        if (entries == 0) { cloud->cancel(); show_status("云端没有可清空的快捷短语。"); }
        else cloud_confirmation(entries, cloud->value.at("token").get<std::string>());
      } else if (cloud->operation == "quick_clear_apply") {
        show_status("已清空 " + std::to_string(cloud->value.at("cleared").get<uint64_t>()) + " 条云端快捷短语。");
      }
    }
    // 关闭时先等进行中的请求结束，再丢弃内存预览；不留下可重用的清空令牌。
    if (closing && cloud && !cloud->busy() && cloud->operation != "quick_clear_cancel") cloud->cancel();
    update_actions();
  }

  void clear_phrases() {
    if (busy()) return;
    auto *dialog = gtk_dialog_new_with_buttons(
        "清空快捷短语？", GTK_WINDOW(window), GTK_DIALOG_MODAL,
        "取消", GTK_RESPONSE_CANCEL, "清空", GTK_RESPONSE_ACCEPT, nullptr);
    auto *detail = gtk_label_new("只删除本机用户添加的快捷短语，内置短语会保留。此操作无法撤销。");
    gtk_label_set_wrap(GTK_LABEL(detail), TRUE);
    gtk_widget_set_margin_top(detail, 24);
    gtk_widget_set_margin_bottom(detail, 24);
    gtk_widget_set_margin_start(detail, 24);
    gtk_widget_set_margin_end(detail, 24);
    gtk_box_append(GTK_BOX(gtk_dialog_get_content_area(GTK_DIALOG(dialog))), detail);
    gtk_dialog_set_default_response(GTK_DIALOG(dialog), GTK_RESPONSE_CANCEL);
    g_signal_connect(dialog, "response", G_CALLBACK(+[](GtkDialog *dialog, int response, gpointer data) {
      auto &self = *static_cast<Window *>(data);
      gtk_window_destroy(GTK_WINDOW(dialog));
      if (response != GTK_RESPONSE_ACCEPT || self.busy()) return;
      gtk_widget_set_sensitive(self.clear, FALSE);
      gtk_widget_set_sensitive(self.cloud_clear, FALSE);
      self.show_status("正在清空…");
      const auto path = self.options;
      self.pending = std::async(std::launch::async, [path] {
        try { return call_clear(path); }
        catch (...) { return Json{{"ok", false}, {"error", "dictionary_unavailable"}}; }
      });
    }), this);
    gtk_window_present(GTK_WINDOW(dialog));
  }

  void create() {
    window = gtk_application_window_new(app);
    gtk_window_set_title(GTK_WINDOW(window), "快捷短语");
    gtk_window_set_default_size(GTK_WINDOW(window), 420, 230);
    gtk_window_set_resizable(GTK_WINDOW(window), FALSE);
    auto *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 16);
    for (auto setter : {gtk_widget_set_margin_top, gtk_widget_set_margin_bottom,
                        gtk_widget_set_margin_start, gtk_widget_set_margin_end})
      setter(box, 24);
    gtk_window_set_child(GTK_WINDOW(window), box);
    auto *title = gtk_label_new("快捷短语");
    gtk_widget_set_halign(title, GTK_ALIGN_START);
    gtk_box_append(GTK_BOX(box), title);
    clear = gtk_button_new_with_label("清空快捷短语…");
    gtk_widget_set_halign(clear, GTK_ALIGN_START);
    gtk_box_append(GTK_BOX(box), clear);
    cloud_clear = gtk_button_new_with_label("清空云端快捷短语…");
    gtk_widget_set_halign(cloud_clear, GTK_ALIGN_START);
    gtk_box_append(GTK_BOX(box), cloud_clear);
    try {
      const auto document = Json::parse(msime::linux_host::read_runtime_options(options));
      const auto directory = document.at("preferences_directory").get<std::string>();
      if (directory.empty() || directory.front() != '/') throw std::runtime_error("invalid account directory");
      cloud.emplace(Json{{"directory", directory}, {"snapshot_directory", directory + "/native-quick-clear-" + std::to_string(::getpid())}});
    } catch (...) { gtk_widget_set_sensitive(cloud_clear, FALSE); }
    g_signal_connect(cloud_clear, "clicked", G_CALLBACK(+[](GtkButton *, gpointer data) {
      auto &self = *static_cast<Window *>(data);
      if (!self.cloud || self.busy()) return;
      self.cloud->preview(); self.show_status("正在读取云端快捷短语…"); self.update_actions();
    }), this);
    status = gtk_label_new("");
    gtk_label_set_xalign(GTK_LABEL(status), 0);
    gtk_label_set_wrap(GTK_LABEL(status), TRUE);
    gtk_box_append(GTK_BOX(box), status);
    g_signal_connect(clear, "clicked", G_CALLBACK(+[](GtkButton *, gpointer data) {
      static_cast<Window *>(data)->clear_phrases();
    }), this);
    g_signal_connect(window, "close-request", G_CALLBACK(+[](GtkWindow *, gpointer data) -> gboolean {
      auto &self = *static_cast<Window *>(data);
      self.closing = true;
      if (self.cloud_dialog) {
        gtk_dialog_response(GTK_DIALOG(self.cloud_dialog), GTK_RESPONSE_CANCEL);
      }
      self.poll();
      if (!self.busy()) g_application_quit(G_APPLICATION(self.app));
      return TRUE;
    }), this);
    g_timeout_add(100, +[](gpointer data) -> gboolean {
      auto &self = *static_cast<Window *>(data);
      self.poll();
      if (self.closing && !self.busy()) {
        g_application_quit(G_APPLICATION(self.app));
        return G_SOURCE_REMOVE;
      }
      return G_SOURCE_CONTINUE;
    }, this);
    gtk_window_present(GTK_WINDOW(window));
  }
};
}  // namespace

int main(int argc, char **argv) {
  const auto path = options_path(argc, argv);
  if (path.empty() || path.front() != '/') return 2;
  Window state{path};
  state.app = gtk_application_new(nullptr, G_APPLICATION_NON_UNIQUE);
  g_signal_connect(state.app, "activate", G_CALLBACK(+[](GtkApplication *, gpointer data) {
    static_cast<Window *>(data)->create();
  }), &state);
  const int result = g_application_run(G_APPLICATION(state.app), 1, argv);
  if (state.window) gtk_window_destroy(GTK_WINDOW(state.window));
  g_object_unref(state.app);
  return result;
}
