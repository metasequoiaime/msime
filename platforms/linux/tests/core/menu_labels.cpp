#include "../src/core/InputSchemes.h"
#include "../src/core/MenuLabels.h"

#include <cassert>
#include <string>
#include <string_view>

int main() {
  namespace host = msime::linux_host;

  // 子菜单顺序与 Engine 的方案编号相同，也就是 macOS 子菜单的顺序。
  for (std::size_t index = 0; index < host::kSchemeMenuOrder.size(); ++index)
    assert(host::kInputSchemeIds[index] == host::kSchemeMenuOrder[index]);

  // 方案名与 macOS 菜单相同：双拼带键位方案，五笔带码表版本，外语方案是「日语」「韩语」「越南语」「藏文」。
  assert(host::scheme_menu_title("quanpin", "xiaohe", "wubi86") == "全拼");
  assert(host::scheme_menu_title("shuangpin", "xiaohe", "wubi86") == "双拼（小鹤）");
  assert(host::scheme_menu_title("shuangpin", "smart_abc", "wubi86") == "双拼（智能 ABC）");
  assert(host::scheme_menu_title("shuangpin", "unknown", "wubi86") == "双拼");
  assert(host::scheme_menu_title("wubi", "xiaohe", "wubi86") == "五笔 86");
  assert(host::scheme_menu_title("wubi", "xiaohe", "wubi98") == "五笔 98");
  assert(host::scheme_menu_title("japanese", "", "") == "日语");
  assert(host::scheme_menu_title("korean", "", "") == "韩语");
  assert(host::scheme_menu_title("cantonese", "", "") == "粤拼");
  assert(host::scheme_menu_title("zhuyin", "", "") == "注音");
  assert(host::scheme_menu_title("vietnamese", "", "") == "越南语");
  assert(host::scheme_menu_title("tibetan", "", "") == "藏文");
  assert(host::scheme_menu_title("stroke", "", "") == "笔画");

  // 子菜单那一行带上当前项，写法与 macOS 的「输入方案（双拼（小鹤））」「主题（水杉）」相同。
  assert(host::menu_heading(host::menu_label::kScheme, "双拼（小鹤）") == "输入方案（双拼（小鹤））");
  assert(host::menu_heading(host::menu_label::kTheme, "跟随系统") == "主题（跟随系统）");
  assert(host::menu_heading(host::menu_label::kTheme, "") == "主题");

  // 设置和关于带上本版本的名字和省略号；快捷键写进名字里。
  assert(std::string(host::menu_label::kSettings) == MSIME_EDITION_DISPLAY_NAME "设置…");
  assert(std::string(host::menu_label::kAbout) == "关于" MSIME_EDITION_DISPLAY_NAME "…");
  assert(std::string_view(host::menu_label::kEnglishCandidateMode) == "英文候选模式（Ctrl+Shift+E）");
  assert(std::string_view(host::menu_label::kFullwidth) == "全角字符（Ctrl+Shift+Space）");
  assert(std::string_view(host::menu_label::kChinesePunctuation) == "中文标点（Ctrl+.）");
  return 0;
}
