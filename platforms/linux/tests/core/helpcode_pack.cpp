#include "../src/core/HelpcodePack.h"

#include <cassert>
#include <string>

int main() {
  using namespace msime::linux_host;
  using Json = nlohmann::json;

  // 偏好文档里各方案的辅助码表包互不相干；只有全拼和双拼有包，空字符串与缺省同义。
  const auto preferences = Json::parse(R"json({
    "scheme": "quanpin",
    "quanpin_helpcode": {"enabled": true, "schema": "xiaohe"},
    "shuangpin_helpcode": {"enabled": true, "schema": "jiajia"},
    "plugins": {"command_tables": [], "helpcode_pack_quanpin": "radicals", "helpcode_pack_shuangpin": ""}
  })json");
  assert(helpcode_pack(preferences, "quanpin") == "radicals");
  assert(helpcode_pack(preferences, "shuangpin").empty());
  assert(helpcode_pack(preferences, "wubi").empty());
  assert(helpcode_pack(Json::object(), "quanpin").empty());
  assert(helpcode_pack(Json::parse(R"({"plugins": {"helpcode_pack_wubi": "x"}})"), "wubi").empty());

  // 选了包时状态栏写明插件在生效，而不是那个只作回退的内置方案；没选包时照旧是方案名，缺省方案按方案各自的默认值。
  assert(helpcode_status_label(preferences, "quanpin") == "辅助码：插件 radicals");
  assert(helpcode_status_label(preferences, "shuangpin") == "辅助码：加加");
  assert(helpcode_status_label(Json::object(), "quanpin") == "辅助码：自然码");
  assert(helpcode_status_label(Json::object(), "shuangpin") == "辅助码：蓝天小雨点");

  // 在菜单里选内置方案与设置页同一约定：写方案的同时停用这个方案的包，于是 Engine 改按刚选的方案出辅助码；另一个方案的包和其它插件设置不动。
  auto chosen = Json::parse(R"json({
    "quanpin_helpcode": {"enabled": true, "schema": "xiaohe", "show_in_candidate_window": true},
    "plugins": {"command_tables": ["sample"], "helpcode_pack_quanpin": "radicals", "helpcode_pack_shuangpin": "strokes"}
  })json");
  apply_helpcode_schema_choice(chosen, "quanpin", "xiaohe");
  assert(chosen["quanpin_helpcode"]["schema"] == "xiaohe");
  assert(chosen["quanpin_helpcode"]["show_in_candidate_window"] == true);
  assert(helpcode_pack(chosen, "quanpin").empty());
  assert(!chosen["plugins"].contains("helpcode_pack_quanpin"));
  assert(helpcode_pack(chosen, "shuangpin") == "strokes");
  assert(chosen["plugins"]["command_tables"] == Json::array({"sample"}));
  assert(helpcode_status_label(chosen, "quanpin") == "辅助码：小鹤");

  // 文档里没有任何插件设置时不为清除新建 plugins：Rust 侧在插件设置全为默认时同样省略它。
  auto bare = Json::parse(R"({"shuangpin_helpcode": {"enabled": false}})");
  apply_helpcode_schema_choice(bare, "shuangpin", "ziranma");
  assert(bare == Json::parse(R"({"shuangpin_helpcode": {"enabled": false, "schema": "ziranma"}})"));

  // Fcitx5 把存储里的包抄进运行时偏好：非空写入，空则删键。
  auto runtime = Json::object();
  set_helpcode_pack(runtime, "shuangpin", "strokes");
  assert(helpcode_pack(runtime, "shuangpin") == "strokes");
  set_helpcode_pack(runtime, "shuangpin", "");
  assert(runtime["plugins"].empty());

  return 0;
}
