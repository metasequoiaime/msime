// fcitx5-gtk 的客户端输入面板（GNOME Wayland 这类不让输入法自己弹窗的桌面上，候选窗由程序里的 fcitx5-gtk 模块绘制）用 GKeyFile 读 MSIME 生成的 theme.conf，读不进就退回 default 主题。这里用同一个解析器读一遍，带装饰图和不带的两种主题都要能读，颜色和装饰图也要读得到。
#include "../src/candidates/CandidateFcitxTheme.h"

#include <cassert>
#include <glib.h>
#include <string>

namespace {

namespace host = msime::linux_host;

host::CandidateColors colors_of(const char *candidate, const char *appearance) {
  return host::candidate_theme_colors({{"appearance", appearance}, {"candidate", nlohmann::json::parse(candidate)}}, false)
      .colors;
}

std::string value(GKeyFile *file, const char *group, const char *key) {
  gchar *raw = g_key_file_get_value(file, group, key, nullptr);
  assert(raw);
  std::string result(raw);
  g_free(raw);
  return result;
}

void assert_gkeyfile_reads(const std::string &theme, bool decorated) {
  GKeyFile *file = g_key_file_new();
  GError *error = nullptr;
  const bool loaded = g_key_file_load_from_data(file, theme.data(), theme.size(), G_KEY_FILE_NONE, &error);
  if (error) g_error_free(error);
  assert(loaded);
  assert(value(file, "Metadata", "Name") == "MSIME");
  assert(value(file, "SupportedScale", "Value") == "2");
  assert(value(file, "InputPanel/Background", "Color") == "#201a30");
  assert(g_key_file_has_key(file, "InputPanel/Background", "Overlay", nullptr) == decorated);
  g_key_file_free(file);
}

} // namespace

int main() {
  const auto night = colors_of(R"({"surface":"#201A30","text":"#F6F1FF","accent":"#4FE0C8"})", "dark");
  assert_gkeyfile_reads(host::fcitx_candidate_theme(night, true), false);
  assert_gkeyfile_reads(host::fcitx_candidate_theme(night, true, host::FcitxThemeOverlay{"decoration-ab.png", 112, 112}), true);
  return 0;
}
