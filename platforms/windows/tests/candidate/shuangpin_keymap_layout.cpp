#include "ShuangpinKeymapLayout.h"
#include <cmath>
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Shuangpin keymap layout test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)

nlohmann::json view(const std::string &editing) {
  return nlohmann::json{{"scheme", 1},
                        {"local_mode", "none"},
                        {"dedicated_english", false},
                        {"shuangpin_profile", "ziranma"},
                        {"editing_text", editing}};
}

void preference() {
  // 没选过时文档里没有这一项，按关处理。
  require(!shuangpin_keymap_preference(nlohmann::json::object()));
  require(shuangpin_keymap_preference(nlohmann::json{{"shuangpin_keymap_hint", true}}));
  require(!shuangpin_keymap_preference(nlohmann::json{{"shuangpin_keymap_hint", false}}));
  require(!shuangpin_keymap_preference(nlohmann::json{{"shuangpin_keymap_hint", "yes"}}));
  require(!shuangpin_keymap_preference(nlohmann::json()));
}

void hint_follows_the_view() {
  // 双拼组字时显示，高亮最后一个键，键名大写。
  const auto shown = shuangpin_keymap_hint(view("ni"));
  require(shown && shown->profile == "ziranma" && shown->key == 'I');
  require(shuangpin_keymap_hint(view("x;"))->key == ';');
  // 最后一个字符不是字母或分号时只显示、不高亮。
  require(shuangpin_keymap_hint(view("ni'"))->key == 0);
  // 没有组字、不是双拼、局部模式、Engine 的英文模式、没有方案名时都不显示。
  require(!shuangpin_keymap_hint(view("")));
  auto other = view("ni");
  other["scheme"] = 0;
  require(!shuangpin_keymap_hint(other));
  other = view("ni");
  other["local_mode"] = "unicode";
  require(!shuangpin_keymap_hint(other));
  other = view("ni");
  other["dedicated_english"] = true;
  require(!shuangpin_keymap_hint(other));
  other = view("ni");
  other["shuangpin_profile"] = "";
  require(!shuangpin_keymap_hint(other));
  other = view("ni");
  other.erase("dedicated_english");
  require(!shuangpin_keymap_hint(other));
  // 不认识的方案名按小鹤处理。
  other = view("ni");
  other["shuangpin_profile"] = "unknown";
  require(shuangpin_keymap_hint(other)->profile == "xiaohe");
  // 自定义方案没有按名字取得到的键位表，不显示，免得按小鹤标错键。
  other = view("ni");
  other["shuangpin_profile"] = "custom";
  require(!shuangpin_keymap_hint(other));
  require(!shuangpin_keymap_hint(nlohmann::json::array()));
}

void keys_and_texts() {
  require(shuangpin_keymap_codes_text("q / iu") == "q / iu");
  require(shuangpin_keymap_codes_text("zh / ui v") == "zh / ui · v");
  require(shuangpin_keymap_codes_text("ing") == "ing");
  require(shuangpin_keymap_codes_text("").empty());

  const nlohmann::json hints{{"Q", "q / iu"}, {"A", "a"}, {"V", "zh / ui v"}};
  auto rows = shuangpin_keymap_rows(hints);
  require(rows[0].size() == 10 && rows[1].size() == 9 && rows[2].size() == 7);
  require(rows[0][0].key == "Q" && rows[0][0].codes == "q / iu");
  require(rows[1][0].key == "A" && rows[1][0].codes == "a");
  require(rows[2][3].key == "V" && rows[2][3].codes == "zh / ui · v");
  require(rows[0][1].codes.empty());
  // 分号只在方案给它分了韵母时出现在中排末尾（微软双拼）。
  rows = shuangpin_keymap_rows(nlohmann::json{{";", "ing"}});
  require(rows[1].size() == 10 && rows[1][9].key == ";" && rows[1][9].codes == "ing");
  require(shuangpin_keymap_rows(nlohmann::json()).at(1).size() == 9);

  require(shuangpin_keymap_zero_initial_text(nlohmann::json{{"o", "oo"}, {"a", "aa"}, {"ang", "ah"}}) ==
          "零声母  a=aa · ang=ah · o=oo");
  require(shuangpin_keymap_zero_initial_text(nlohmann::json{{"ve", "vt"}}) == "零声母  üe=vt");
  require(shuangpin_keymap_zero_initial_text(nlohmann::json::object()) == "零声母  ");
}

void geometry() {
  const ShuangpinKeymapMetrics metrics;
  // 上排十个键占满两边各 14 的内距，中排九个键往里收到 31，下排七个键收到 58。
  const auto first = shuangpin_keymap_key_rect(metrics, 0, 0, 10);
  const auto last = shuangpin_keymap_key_rect(metrics, 0, 9, 10);
  require(std::abs(first.left - 14.0) < 1e-9 && std::abs(last.right - (620.0 - 14.0)) < 1e-9);
  require(std::abs(shuangpin_keymap_key_rect(metrics, 1, 0, 9).left - 31.0) < 1e-9);
  require(std::abs(shuangpin_keymap_key_rect(metrics, 2, 6, 7).right - (620.0 - 58.0)) < 1e-9);
  // 三排键帽和底部说明都在卡片里，彼此不重叠。
  const auto bottom = shuangpin_keymap_key_rect(metrics, 2, 0, 7);
  require(bottom.top > shuangpin_keymap_key_rect(metrics, 1, 0, 9).bottom);
  require(bottom.bottom + 4.0 <= metrics.height - metrics.footer_bottom - metrics.footer_height);
  require(metrics.window_width() > metrics.width && metrics.window_height() > metrics.height);
}

void accent_text() {
  // Windows 深色主题的浅蓝强调色上用黑字，浅色主题的深蓝上用白字。
  require(shuangpin_keymap_on_accent(candidate_rgb(0x60CDFF)).r == 0.0f);
  require(shuangpin_keymap_on_accent(candidate_rgb(0x005FB8)).r == 1.0f);
  require(shuangpin_keymap_on_accent(candidate_rgb(0x127D73)).r == 1.0f);
}

void placement() {
  ShuangpinKeymapPlacementInput input;
  input.work = {0, 0, 1920, 1040};
  input.anchor_x = 100;
  input.anchor_y = 300;
  input.candidate = {100, 303, 400, 500};
  input.card_width = 620;
  input.card_height = 203;
  input.shadow_left = 18;
  input.shadow_top = 16;
  // 候选窗在光标下方：键位图接在候选窗下面，左边对齐。
  auto placed = shuangpin_keymap_placement(input);
  require(placed.x == 100 - 18 && placed.y == 500 + 8 - 16);
  // 下面放不下时跨过光标所在行，放到它上方。
  input.anchor_y = 800;
  input.candidate = {100, 803, 400, 1000};
  placed = shuangpin_keymap_placement(input);
  require(placed.y == 800 - 24 - 8 - 203 - 16);
  // 候选窗翻到光标上方时，键位图放在候选窗上面。
  input.anchor_y = 900;
  input.candidate = {100, 600, 400, 876};
  placed = shuangpin_keymap_placement(input);
  require(placed.y == 600 - 8 - 203 - 16);
  // 候选窗上方也放不下时放到光标下方。
  input.anchor_y = 230;
  input.candidate = {100, 10, 400, 206};
  placed = shuangpin_keymap_placement(input);
  require(placed.y == 230 + 8 - 16);
  // 贴着右边缘时收进工作区。
  input.candidate.left = 1800;
  placed = shuangpin_keymap_placement(input);
  require(placed.x == 1920 - 16 - 620 - 18);
  // 候选卡片被往上推了几像素、顶边略高于锚点：卡片大半在光标下方，仍算在下方，键位图接在卡片下面，不跳到光标上方去盖住正在输入的行。
  input.anchor_y = 300;
  input.candidate = {100, 296, 400, 500};
  placed = shuangpin_keymap_placement(input);
  require(placed.x == 100 - 18 && placed.y == 500 + 8 - 16);
  // 候选窗口的外框带着透明阴影边距（上 20、下 40、左 32），顶边在锚点上方。按中线判断仍认得出卡片在光标下方；摆放用的是卡片本身的矩形，这里只确认方向不会因外框而翻转。
  input.candidate = {100 - 32, 303 - 20, 400 + 32, 500 + 40};
  placed = shuangpin_keymap_placement(input);
  require(placed.y > input.anchor_y);
}
} // namespace

int main() {
  try {
    preference();
    hint_follows_the_view();
    keys_and_texts();
    geometry();
    accent_text();
    placement();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
