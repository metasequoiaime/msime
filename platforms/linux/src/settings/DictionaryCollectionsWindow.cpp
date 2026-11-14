#include <gtk/gtk.h>
#include "../../../common/DictionaryCollectionsModel.h"
#include "../core/RuntimeOptionsFile.h"
#include <algorithm>
#include <array>
#include <filesystem>
#include <set>

namespace {
using Json = nlohmann::json;
struct Window {
  explicit Window(Json options) : model(std::move(options)) {}
  msime::dictionary_collections::Model model;
  GtkApplication *app = nullptr;
  GtkWidget *window = nullptr, *list = nullptr, *name = nullptr, *status = nullptr;
  GtkWidget *kind = nullptr, *format = nullptr, *target = nullptr, *actions = nullptr;
  GtkNativeDialog *picker = nullptr;
  GtkWidget *confirmation = nullptr;
  guint timer = 0;
  std::vector<std::string> rows, targets;
  const std::array<const char *, 7> kinds{{"pinyin", "wubi", "wubi98", "wubixsj", "english", "quickPhrase", "cantonese"}};
  const std::array<const char *, 5> formats{{"standard", "windows", "rime", "hans", "scel"}};

  std::vector<std::string> selected() const {
    std::vector<std::string> ids;
    auto *selection = gtk_list_box_get_selected_rows(GTK_LIST_BOX(list));
    for (auto *item = selection; item; item = item->next) {
      const int index = gtk_list_box_row_get_index(GTK_LIST_BOX_ROW(item->data));
      if (index >= 0 && static_cast<size_t>(index) < rows.size()) ids.push_back(rows[static_cast<size_t>(index)]);
    }
    g_list_free(selection);
    return ids;
  }
  void perform(Json action) { model.perform(std::move(action)); update(false); }
  void update(bool rebuild) {
    if (rebuild) {
      const auto selected_ids = selected();
      const std::set<std::string> preserved(selected_ids.begin(), selected_ids.end());
      while (auto *child = gtk_widget_get_first_child(list)) gtk_list_box_remove(GTK_LIST_BOX(list), child);
      rows.clear();
      auto *names = gtk_string_list_new(nullptr);
      gtk_string_list_append(names, "新建分组"); targets = {""};
      for (const auto *source : {"user", "import", "community"}) {
        for (const auto &item : model.view.at("collections")) {
          if (item.at("source").at("type") != source) continue;
          const auto id = item.at("id").get<std::string>();
          const auto title = std::string(msime::dictionary_collections::Model::source_title(source)) + " · " +
              item.at("name").get<std::string>() + " · " + std::to_string(item.at("entry_count").get<unsigned>()) +
              " 条 · " + (item.at("enabled").get<bool>() ? "已启用" : "已停用");
          auto *label = gtk_label_new(title.c_str());
          gtk_label_set_xalign(GTK_LABEL(label), 0);
          gtk_widget_set_margin_top(label, 10); gtk_widget_set_margin_bottom(label, 10);
          gtk_list_box_append(GTK_LIST_BOX(list), label);
          const auto index = static_cast<int>(rows.size()); rows.push_back(id);
          if (preserved.count(id)) gtk_list_box_select_row(GTK_LIST_BOX(list), gtk_list_box_get_row_at_index(GTK_LIST_BOX(list), index));
          if (item.at("kind") == kinds.at(gtk_drop_down_get_selected(GTK_DROP_DOWN(kind))) && std::string(source) != "community") {
            targets.push_back(id); gtk_string_list_append(names, item.at("name").get_ref<const std::string &>().c_str());
          }
        }
      }
      gtk_drop_down_set_model(GTK_DROP_DOWN(target), G_LIST_MODEL(names));
      gtk_drop_down_set_selected(GTK_DROP_DOWN(target), 0); g_object_unref(names);
    }
    gtk_widget_set_sensitive(actions, !model.busy());
    gtk_widget_set_sensitive(list, !model.busy());
    std::string message = model.busy() ? "正在处理…" : msime::dictionary_collections::Model::message(model.error);
    if (!model.busy() && model.error.empty() && model.report.is_object()) {
      message = "导入 " + std::to_string(model.report.value("imported", 0)) + " 条，跳过重复 " +
          std::to_string(model.report.value("duplicates", 0)) + " 条，失败 " + std::to_string(model.report.value("failed", 0)) + " 条";
      if (model.report.value("truncated", false)) message += "；分组已满";
    }
    gtk_label_set_text(GTK_LABEL(status), message.c_str());
  }
  void batch(const char *operation, bool enabled = false) {
    const auto ids = selected();
    if (ids.empty()) { gtk_label_set_text(GTK_LABEL(status), "请先选择词库"); return; }
    Json action{{"operation", operation}, {"ids", ids}};
    if (std::string(operation) == "set_enabled_many") action["enabled"] = enabled;
    perform(std::move(action));
  }
  void remove() {
    if (selected().empty() || confirmation) return;
    confirmation = gtk_message_dialog_new(GTK_WINDOW(window), GTK_DIALOG_MODAL, GTK_MESSAGE_WARNING,
        GTK_BUTTONS_NONE, "删除所选词库？");
    gtk_message_dialog_format_secondary_text(GTK_MESSAGE_DIALOG(confirmation), "%s", "删除分组及其词条；其他已启用分组和原有个人词条会保留。");
    gtk_dialog_add_buttons(GTK_DIALOG(confirmation), "取消", GTK_RESPONSE_CANCEL, "删除", GTK_RESPONSE_ACCEPT, nullptr);
    g_signal_connect(confirmation, "response", G_CALLBACK(+[](GtkDialog *dialog, int response, gpointer data) {
      auto &self = *static_cast<Window *>(data);
      if (response == GTK_RESPONSE_ACCEPT) self.batch("delete_many");
      self.confirmation = nullptr; gtk_window_destroy(GTK_WINDOW(dialog));
    }), this);
    gtk_window_present(GTK_WINDOW(confirmation));
  }
  void import_file() {
    if (picker) return;
    picker = GTK_NATIVE_DIALOG(gtk_file_chooser_native_new("导入词库", GTK_WINDOW(window), GTK_FILE_CHOOSER_ACTION_OPEN, "导入", "取消"));
    g_signal_connect(picker, "response", G_CALLBACK(+[](GtkNativeDialog *dialog, int response, gpointer data) {
      auto &self = *static_cast<Window *>(data);
      if (response == GTK_RESPONSE_ACCEPT) {
        auto *file = gtk_file_chooser_get_file(GTK_FILE_CHOOSER(dialog));
        if (file) {
          GError *failure = nullptr;
          auto *stream = g_file_read(file, nullptr, &failure);
          gchar *basename = g_file_get_basename(file);
          const auto index = gtk_drop_down_get_selected(GTK_DROP_DOWN(self.target));
          std::string group = gtk_editable_get_text(GTK_EDITABLE(self.name));
          if (group.empty() && basename) group = std::filesystem::path(basename).stem().string();
          const auto chosen_format = self.formats.at(gtk_drop_down_get_selected(GTK_DROP_DOWN(self.format)));
          const size_t maximum = std::string(chosen_format) == "scel" ? 8 * 1024 * 1024 : 16 * 1024 * 1024;
          std::string bytes;
          if (stream) {
            std::array<char, 16384> buffer{};
            while (bytes.size() <= maximum) {
              const auto count = g_input_stream_read(G_INPUT_STREAM(stream), buffer.data(),
                  std::min(buffer.size(), maximum + 1 - bytes.size()), nullptr, &failure);
              if (count <= 0) break;
              bytes.append(buffer.data(), static_cast<size_t>(count));
            }
            g_object_unref(stream);
          }
          if (!stream || failure || bytes.size() > maximum) {
            gtk_label_set_text(GTK_LABEL(self.status), "文件无法读取或超出容量限制");
          } else {
            Json action{{"operation", "import"}, {"kind", self.kinds.at(gtk_drop_down_get_selected(GTK_DROP_DOWN(self.kind)))},
                {"format", chosen_format}, {"name", group}};
            if (index < self.targets.size() && !self.targets[index].empty()) action["id"] = self.targets[index];
            if (std::string(chosen_format) == "scel") {
              gchar *encoded = g_base64_encode(reinterpret_cast<const guchar *>(bytes.data()), bytes.size());
              action["bytes_base64"] = encoded; g_free(encoded);
            } else action["text"] = bytes;
            self.perform(std::move(action));
          }
          if (failure) g_error_free(failure);
          g_free(basename); g_object_unref(file);
        }
      }
      self.picker = nullptr; gtk_native_dialog_destroy(dialog); g_object_unref(dialog);
    }), this);
    gtk_native_dialog_show(picker);
  }
  void create() {
    window = gtk_application_window_new(app);
    gtk_window_set_title(GTK_WINDOW(window), "词库分组"); gtk_window_set_default_size(GTK_WINDOW(window), 760, 560);
    auto *root = gtk_box_new(GTK_ORIENTATION_VERTICAL, 12); gtk_window_set_child(GTK_WINDOW(window), root);
    for (auto margin : {GTK_POS_LEFT, GTK_POS_RIGHT, GTK_POS_TOP, GTK_POS_BOTTOM}) {
      if (margin == GTK_POS_LEFT) gtk_widget_set_margin_start(root, 20);
      if (margin == GTK_POS_RIGHT) gtk_widget_set_margin_end(root, 20);
      if (margin == GTK_POS_TOP) gtk_widget_set_margin_top(root, 20);
      if (margin == GTK_POS_BOTTOM) gtk_widget_set_margin_bottom(root, 20);
    }
    list = gtk_list_box_new(); gtk_list_box_set_selection_mode(GTK_LIST_BOX(list), GTK_SELECTION_MULTIPLE);
    auto *scroll = gtk_scrolled_window_new(); gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scroll), list);
    gtk_widget_set_vexpand(scroll, true); gtk_box_append(GTK_BOX(root), scroll);
    actions = gtk_box_new(GTK_ORIENTATION_VERTICAL, 10); gtk_box_append(GTK_BOX(root), actions);
    auto *fields = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8); gtk_box_append(GTK_BOX(actions), fields);
    name = gtk_entry_new(); gtk_entry_set_placeholder_text(GTK_ENTRY(name), "分组名称（导入时默认文件名）");
    gtk_widget_set_hexpand(name, true); gtk_box_append(GTK_BOX(fields), name);
    const char *kind_names[] = {"拼音", "86 五笔", "98 五笔", "新世纪五笔", "英文", "快捷短语", "粤语", nullptr};
    kind = gtk_drop_down_new_from_strings(kind_names); gtk_box_append(GTK_BOX(fields), kind);
    const char *format_names[] = {"词在前（TSV）", "编码在前（TSV）", "Rime", "汉字词表", "搜狗细胞词库", nullptr};
    format = gtk_drop_down_new_from_strings(format_names); gtk_box_append(GTK_BOX(fields), format);
    const char *target_names[] = {"新建分组", nullptr};
    target = gtk_drop_down_new_from_strings(target_names); gtk_box_append(GTK_BOX(actions), target);
    auto *buttons = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8); gtk_box_append(GTK_BOX(actions), buttons);
    const auto button = [&](const char *title, GCallback callback) {
      auto *item = gtk_button_new_with_label(title); gtk_box_append(GTK_BOX(buttons), item); g_signal_connect(item, "clicked", callback, this);
    };
    button("新建", G_CALLBACK(+[](GtkButton *, gpointer data) { auto &s = *static_cast<Window *>(data);
      s.perform({{"operation", "create"}, {"name", gtk_editable_get_text(GTK_EDITABLE(s.name))}, {"kind", s.kinds.at(gtk_drop_down_get_selected(GTK_DROP_DOWN(s.kind)))}}); }));
    button("改名", G_CALLBACK(+[](GtkButton *, gpointer data) { auto &s = *static_cast<Window *>(data); const auto ids = s.selected();
      if (ids.size() == 1) s.perform({{"operation", "rename"}, {"id", ids[0]}, {"name", gtk_editable_get_text(GTK_EDITABLE(s.name))}});
      else gtk_label_set_text(GTK_LABEL(s.status), "改名时请选择一个词库"); }));
    button("导入…", G_CALLBACK(+[](GtkButton *, gpointer data) { static_cast<Window *>(data)->import_file(); }));
    button("启用", G_CALLBACK(+[](GtkButton *, gpointer data) { static_cast<Window *>(data)->batch("set_enabled_many", true); }));
    button("停用", G_CALLBACK(+[](GtkButton *, gpointer data) { static_cast<Window *>(data)->batch("set_enabled_many", false); }));
    button("删除…", G_CALLBACK(+[](GtkButton *, gpointer data) { static_cast<Window *>(data)->remove(); }));
    button("刷新", G_CALLBACK(+[](GtkButton *, gpointer data) { static_cast<Window *>(data)->perform({{"operation", "load"}}); }));
    status = gtk_label_new(""); gtk_label_set_xalign(GTK_LABEL(status), 0); gtk_label_set_wrap(GTK_LABEL(status), true); gtk_box_append(GTK_BOX(root), status);
    g_signal_connect(kind, "notify::selected", G_CALLBACK(+[](GObject *, GParamSpec *, gpointer data) { static_cast<Window *>(data)->update(true); }), this);
    timer = g_timeout_add(100, +[](gpointer data) -> gboolean { auto &s = *static_cast<Window *>(data);
      try { if (s.model.poll()) s.update(true); } catch (...) { gtk_label_set_text(GTK_LABEL(s.status), "操作失败，请重试"); }
      return G_SOURCE_CONTINUE;
    }, this);
    update(false); gtk_window_present(GTK_WINDOW(window));
  }
};
}
int main(int argc, char **argv) {
  if (argc != 3 || std::string(argv[1]) != "--state-root" || !std::filesystem::path(argv[2]).is_absolute()) return 2;
  try {
    Window state(Json::parse(msime::linux_host::read_runtime_options(std::filesystem::path(argv[2]) / "runtime-options.json")));
    state.app = gtk_application_new(nullptr, G_APPLICATION_NON_UNIQUE);
    g_signal_connect(state.app, "activate", G_CALLBACK(+[](GtkApplication *, gpointer data) { static_cast<Window *>(data)->create(); }), &state);
    const int result = g_application_run(G_APPLICATION(state.app), 1, argv);
    if (state.timer) g_source_remove(state.timer);
    if (state.picker) { g_signal_handlers_disconnect_by_data(state.picker, &state); gtk_native_dialog_destroy(state.picker); g_object_unref(state.picker); }
    if (state.confirmation) { g_signal_handlers_disconnect_by_data(state.confirmation, &state); gtk_window_destroy(GTK_WINDOW(state.confirmation)); }
    g_object_unref(state.app); return result;
  } catch (...) { return 1; }
}
