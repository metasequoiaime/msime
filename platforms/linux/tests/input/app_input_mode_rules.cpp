#include "../src/core/AppInputModeRules.h"

#include <cassert>

int main() {
  using msime::linux_host::app_input_mode_rule;
  using msime::linux_host::app_input_mode_rule_start;
  using msime::linux_host::AppInputModeRuleStay;
  using msime::linux_host::ibus_app_rule_program;

  const auto preferences = nlohmann::json{
      {"app_input_mode_rules",
       {{"Code", "english"}, {"gedit", "chinese"}, {"com.apple.Terminal", "english"},
        {"broken", 3}, {"other", "japanese"}}}};
  // 按程序名不分 ASCII 大小写查找；没有规则、空程序名、坏条目都不命中。
  assert(app_input_mode_rule(preferences, "code") == false);
  assert(app_input_mode_rule(preferences, "CODE") == false);
  assert(app_input_mode_rule(preferences, "gedit") == true);
  assert(!app_input_mode_rule(preferences, "firefox"));
  assert(!app_input_mode_rule(preferences, ""));
  assert(!app_input_mode_rule(preferences, "broken"));
  assert(!app_input_mode_rule(preferences, "other"));
  assert(!app_input_mode_rule(nlohmann::json::object(), "code"));
  assert(!app_input_mode_rule(nlohmann::json{{"app_input_mode_rules", "code"}}, "code"));

  // IBus 只有 GTK 输入法模块带程序名。
  assert(ibus_app_rule_program("gtk3-im:gedit") == "gedit");
  assert(ibus_app_rule_program("gtk4-im:org.gnome.TextEditor") == "org.gnome.TextEditor");
  assert(ibus_app_rule_program("gtk-im:firefox") == "firefox");
  assert(ibus_app_rule_program("QIBusInputContext").empty());
  assert(ibus_app_rule_program("xim").empty());
  assert(ibus_app_rule_program("fake").empty());
  assert(ibus_app_rule_program("").empty());
  assert(ibus_app_rule_program("other:gedit").empty());

  // 进入有规则的程序从规则的模式开始；同一次停留里手动切换后让位，换到别的程序再回来重新生效。
  AppInputModeRuleStay stay;
  assert(stay.enter("code"));
  assert(app_input_mode_rule_start(preferences, "code", stay) == false);
  stay.yield("code");
  assert(!app_input_mode_rule_start(preferences, "code", stay));
  // 同一程序的另一个窗口不是新的停留。
  assert(!stay.enter("Code"));
  assert(!app_input_mode_rule_start(preferences, "code", stay));
  // 认不出程序的焦点不结束这次停留。
  assert(!stay.enter(""));
  assert(!app_input_mode_rule_start(preferences, "code", stay));
  assert(stay.enter("gedit"));
  assert(app_input_mode_rule_start(preferences, "gedit", stay) == true);
  // 让位只对当时所在的程序。
  stay.yield("code");
  assert(app_input_mode_rule_start(preferences, "gedit", stay) == true);
  assert(stay.enter("code"));
  assert(app_input_mode_rule_start(preferences, "code", stay) == false);
  assert(!app_input_mode_rule_start(preferences, "firefox", stay));
  return 0;
}
