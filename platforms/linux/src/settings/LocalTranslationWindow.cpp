// 保持 Debian 12 的 GTK 4.8 API 下限，新系统也使用相同原生控件。
#define GDK_VERSION_MIN_REQUIRED GDK_VERSION_4_8
#define GDK_VERSION_MAX_ALLOWED GDK_VERSION_4_8
#include <gtk/gtk.h>
#include "../../../common/LocalTranslationSettings.h"
#include <filesystem>

namespace {
using Json = nlohmann::json;
struct Window {
  explicit Window(std::string root) : model(std::move(root)) {}
  msime::translation::SettingsModel model;
  GtkApplication *app = nullptr;
  GtkWidget *window = nullptr, *enabled = nullptr, *language = nullptr, *name = nullptr;
  GtkWidget *status = nullptr, *progress = nullptr, *download = nullptr, *import = nullptr, *remove = nullptr, *cancel = nullptr;
  GtkNativeDialog *picker = nullptr;
  GtkWidget *confirmation = nullptr;
  guint timer = 0;
  bool updating = false, closing = false;
  std::vector<std::string> languages;

  template<class Action> void guard(Action action) noexcept {
    try { action(); } catch (...) { updating = false; model.notice = "unavailable";
      if (status) gtk_label_set_text(GTK_LABEL(status), "操作失败，请重试"); }
  }
  void update(bool values) {
    updating = true;
    const auto &view = model.view;
    const bool loaded = view.contains("settings") && view.contains("catalog");
    const bool available = loaded && view.at("catalog").value("available", false);
    const bool installed = loaded && view.value("installed", false);
    if (loaded && values) {
      const auto &selection = view.at("settings").at("local_translation");
      gtk_check_button_set_active(GTK_CHECK_BUTTON(enabled), selection.at("enabled").get<bool>());
      auto *strings = gtk_string_list_new(nullptr);
      languages.clear();
      guint selected = 0;
      for (const auto &item : view.at("catalog").at("languages")) {
        gtk_string_list_append(strings, item.at("name").get_ref<const std::string &>().c_str());
        if (item.at("code") == selection.at("target_language")) selected = static_cast<guint>(languages.size());
        languages.push_back(item.at("code").get<std::string>());
      }
      gtk_drop_down_set_model(GTK_DROP_DOWN(language), G_LIST_MODEL(strings));
      gtk_drop_down_set_selected(GTK_DROP_DOWN(language), selected);
      g_object_unref(strings);
      const auto text = view.at("catalog").at("model").get<std::string>() + " · " +
          std::to_string(view.at("catalog").at("size").get<uint64_t>() / 1'000'000) + " MB";
      gtk_label_set_text(GTK_LABEL(name), text.c_str());
    }
    const bool on = gtk_check_button_get_active(GTK_CHECK_BUTTON(enabled));
    gtk_widget_set_sensitive(enabled, available && !model.busy() && (installed || on));
    gtk_widget_set_sensitive(language, available && !model.busy());
    gtk_widget_set_sensitive(download, available && !model.busy());
    gtk_widget_set_sensitive(import, available && !model.busy());
    gtk_widget_set_sensitive(remove, loaded && installed && !model.busy());
    gtk_widget_set_visible(cancel, model.installing());
    gtk_widget_set_visible(progress, model.installing());
    gtk_progress_bar_set_fraction(GTK_PROGRESS_BAR(progress), model.progress() / 100.0);
    const char *message = model.busy() ? "正在处理…" : !loaded ? "无法读取设置，请重新打开窗口" :
        !available ? "此构建未包含本地推理库" : installed ? "已安装 · 翻译无需联网" : "未安装";
    if (!model.notice.empty()) message = model.notice.find("cancelled") != std::string::npos ? "已取消" :
        model.notice.find("checksum") != std::string::npos ? "模型校验失败，请重新下载或导入" :
        model.notice.find("preferences changed") != std::string::npos ? "设置已在其他窗口更新，请重新选择" : "操作失败，请重试";
    gtk_label_set_text(GTK_LABEL(status), message);
    updating = false;
  }
  void save() {
    if (updating || model.busy()) return;
    const auto index = gtk_drop_down_get_selected(GTK_DROP_DOWN(language));
    if (index < languages.size()) model.save(gtk_check_button_get_active(GTK_CHECK_BUTTON(enabled)), languages[index]);
    update(false);
  }
  void import_model() {
    if (picker || model.busy()) return;
    picker = GTK_NATIVE_DIALOG(gtk_file_chooser_native_new("导入 Hy-MT2 Q4_K_M 模型", GTK_WINDOW(window), GTK_FILE_CHOOSER_ACTION_OPEN, "导入", "取消"));
    auto *filter = gtk_file_filter_new();
    gtk_file_filter_set_name(filter, "GGUF 模型"); gtk_file_filter_add_pattern(filter, "*.gguf");
    gtk_file_chooser_add_filter(GTK_FILE_CHOOSER(picker), filter);
    g_signal_connect(picker, "response", G_CALLBACK(+[](GtkNativeDialog *dialog, int response, gpointer data) {
      auto &self = *static_cast<Window *>(data);
      if (response == GTK_RESPONSE_ACCEPT) {
        GFile *file = gtk_file_chooser_get_file(GTK_FILE_CHOOSER(dialog));
        char *path = file ? g_file_get_path(file) : nullptr;
        if (path) self.guard([&] { self.model.install({path}); });
        g_free(path); if (file) g_object_unref(file);
      }
      self.picker = nullptr; g_object_unref(dialog); self.guard([&] { self.update(false); });
    }), this);
    gtk_native_dialog_show(picker);
  }
  void remove_model() {
    if (confirmation || model.busy()) return;
    confirmation = gtk_dialog_new_with_buttons("删除本地翻译模型？", GTK_WINDOW(window), GTK_DIALOG_MODAL,
        "取消", GTK_RESPONSE_CANCEL, "删除", GTK_RESPONSE_ACCEPT, nullptr);
    gtk_dialog_set_default_response(GTK_DIALOG(confirmation), GTK_RESPONSE_CANCEL);
    auto *text = gtk_label_new("删除后需重新下载或导入才能离线翻译。");
    gtk_widget_set_margin_top(text, 24); gtk_widget_set_margin_bottom(text, 24);
    gtk_widget_set_margin_start(text, 24); gtk_widget_set_margin_end(text, 24);
    gtk_box_append(GTK_BOX(gtk_dialog_get_content_area(GTK_DIALOG(confirmation))), text);
    g_signal_connect(confirmation, "response", G_CALLBACK(+[](GtkDialog *dialog, int response, gpointer data) {
      auto &self = *static_cast<Window *>(data);
      if (response == GTK_RESPONSE_ACCEPT) self.guard([&] { self.model.remove(); });
      self.confirmation = nullptr; gtk_window_destroy(GTK_WINDOW(dialog)); self.guard([&] { self.update(false); });
    }), this);
    gtk_window_present(GTK_WINDOW(confirmation));
  }
  void create() {
    window = gtk_application_window_new(app);
    gtk_window_set_title(GTK_WINDOW(window), "本地模型翻译");
    gtk_window_set_default_size(GTK_WINDOW(window), 520, 300);
    gtk_window_set_resizable(GTK_WINDOW(window), FALSE);
    auto *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 20);
    for (auto setter : {gtk_widget_set_margin_top, gtk_widget_set_margin_bottom, gtk_widget_set_margin_start, gtk_widget_set_margin_end}) setter(box, 24);
    gtk_window_set_child(GTK_WINDOW(window), box);
    enabled = gtk_check_button_new_with_label("启用本地翻译"); gtk_box_append(GTK_BOX(box), enabled);
    auto *grid = gtk_grid_new(); gtk_grid_set_row_spacing(GTK_GRID(grid), 16); gtk_grid_set_column_spacing(GTK_GRID(grid), 24);
    language = gtk_drop_down_new(nullptr, nullptr); gtk_widget_set_hexpand(language, TRUE);
    name = gtk_label_new("正在读取…"); gtk_label_set_xalign(GTK_LABEL(name), 0);
    auto *language_label = gtk_label_new_with_mnemonic("目标语言(_L)"); gtk_label_set_mnemonic_widget(GTK_LABEL(language_label), language);
    gtk_grid_attach(GTK_GRID(grid), language_label, 0, 0, 1, 1); gtk_grid_attach(GTK_GRID(grid), language, 1, 0, 1, 1);
    gtk_grid_attach(GTK_GRID(grid), gtk_label_new("模型"), 0, 1, 1, 1); gtk_grid_attach(GTK_GRID(grid), name, 1, 1, 1, 1);
    gtk_box_append(GTK_BOX(box), grid);
    auto *source = gtk_link_button_new_with_label("https://huggingface.co/tencent/HY-MT2-1.8B-GGUF", "来源与许可 · 腾讯 Hy-MT2 · Apache-2.0");
    gtk_widget_set_halign(source, GTK_ALIGN_START); gtk_box_append(GTK_BOX(box), source);
    status = gtk_label_new("正在读取…"); gtk_label_set_xalign(GTK_LABEL(status), 0); gtk_label_set_wrap(GTK_LABEL(status), TRUE); gtk_box_append(GTK_BOX(box), status);
    progress = gtk_progress_bar_new(); gtk_progress_bar_set_show_text(GTK_PROGRESS_BAR(progress), TRUE); gtk_box_append(GTK_BOX(box), progress);
    auto *actions = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    download = gtk_button_new_with_label("下载模型"); import = gtk_button_new_with_label("从文件导入…");
    remove = gtk_button_new_with_label("删除模型…"); cancel = gtk_button_new_with_label("取消");
    for (auto *button : {download, import, remove, cancel}) gtk_box_append(GTK_BOX(actions), button);
    gtk_box_append(GTK_BOX(box), actions);
    g_signal_connect(enabled, "toggled", G_CALLBACK(+[](GtkCheckButton *, gpointer data) { auto &self = *static_cast<Window *>(data); self.guard([&] { self.save(); }); }), this);
    g_signal_connect(language, "notify::selected", G_CALLBACK(+[](GObject *, GParamSpec *, gpointer data) { auto &self = *static_cast<Window *>(data); self.guard([&] { self.save(); }); }), this);
    g_signal_connect(download, "clicked", G_CALLBACK(+[](GtkButton *, gpointer data) { auto &self = *static_cast<Window *>(data); self.guard([&] { self.model.install(); self.update(false); }); }), this);
    g_signal_connect(import, "clicked", G_CALLBACK(+[](GtkButton *, gpointer data) { auto &self = *static_cast<Window *>(data); self.guard([&] { self.import_model(); }); }), this);
    g_signal_connect(remove, "clicked", G_CALLBACK(+[](GtkButton *, gpointer data) { auto &self = *static_cast<Window *>(data); self.guard([&] { self.remove_model(); }); }), this);
    g_signal_connect(cancel, "clicked", G_CALLBACK(+[](GtkButton *, gpointer data) { static_cast<Window *>(data)->model.cancel(); }), this);
    g_signal_connect(window, "close-request", G_CALLBACK(+[](GtkWindow *, gpointer data) -> gboolean {
      auto &self = *static_cast<Window *>(data); self.closing = true; self.model.cancel();
      if (self.model.busy()) gtk_widget_set_visible(self.window, FALSE); else g_application_quit(G_APPLICATION(self.app));
      return TRUE;
    }), this);
    timer = g_timeout_add(100, +[](gpointer data) -> gboolean {
      auto &self = *static_cast<Window *>(data);
      try {
        const bool changed = self.model.poll(); self.update(changed);
        if (self.closing && !self.model.busy()) g_application_quit(G_APPLICATION(self.app));
      } catch (...) { gtk_label_set_text(GTK_LABEL(self.status), "操作失败，请重试"); }
      return G_SOURCE_CONTINUE;
    }, this);
    update(false); gtk_window_present(GTK_WINDOW(window));
  }
};
}
int main(int argc, char **argv) {
  if (argc != 3 || std::string(argv[1]) != "--state-root" || !std::filesystem::path(argv[2]).is_absolute()) return 2;
  try {
    Window state(argv[2]);
    state.app = gtk_application_new(nullptr, G_APPLICATION_NON_UNIQUE);
    g_signal_connect(state.app, "activate", G_CALLBACK(+[](GtkApplication *, gpointer data) { auto &self = *static_cast<Window *>(data); self.guard([&] { self.create(); }); }), &state);
    const int result = g_application_run(G_APPLICATION(state.app), 1, argv);
    if (state.timer) g_source_remove(state.timer);
    if (state.picker) { g_signal_handlers_disconnect_by_data(state.picker, &state); gtk_native_dialog_destroy(state.picker); g_object_unref(state.picker); }
    if (state.confirmation) { g_signal_handlers_disconnect_by_data(state.confirmation, &state); gtk_window_destroy(GTK_WINDOW(state.confirmation)); }
    if (state.window) gtk_window_destroy(GTK_WINDOW(state.window));
    g_object_unref(state.app);
    return result;
  } catch (...) { return 1; }
}
