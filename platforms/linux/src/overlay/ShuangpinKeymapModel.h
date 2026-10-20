#pragma once

#include <nlohmann/json.hpp>

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "WaveOverlayPlacement.h"

namespace msime::linux_host {

// 双拼键位提示：双拼组字时在候选窗旁显示当前方案的键位图，高亮刚按下的键，上屏后隐藏，对应 macOS 的 MSIMEShuangpinKeymapPanel（ShuangpinKeymapPanel.mm）。键位表不在这里另存一份，取自 host-api 的 msime_client_shuangpin_key_hints / msime_client_shuangpin_zero_initials（Engine 自己的方案表），与 macOS、Windows 读同一份。
//
// 这里只有纯计算：要不要显示、显示哪几行、几何和摆放，不依赖 X11、cairo 或宿主，单测直接包含。规则与 Windows Server 的 ShuangpinKeymapLayout.h 同源（都照 macOS 写），但没有合并成一份：那份带着 Windows 的调色板和阴影类型，Windows 的键位图仍在并行改动，把它挪到 platforms/common 要同时改 Windows 的构建和测试，放在 Linux 这次改动之外。

// 一帧要显示的键位提示：方案名和要高亮的键（'A'–'Z' 或 ';'，0 表示不高亮）。
struct ShuangpinKeymapHint {
  std::string profile;
  char key = 0;
  bool operator==(const ShuangpinKeymapHint &other) const { return profile == other.profile && key == other.key; }
  bool operator!=(const ShuangpinKeymapHint &other) const { return !(*this == other); }
};

// 共享偏好 `shuangpin_keymap_hint`：没选过时文档里没有这一项，按关处理；不是布尔值时同样按关，与 macOS 和 Windows 一致。
inline bool shuangpin_keymap_preference(const nlohmann::json &preferences) {
  if (!preferences.is_object()) return false;
  const auto value = preferences.find("shuangpin_keymap_hint");
  return value != preferences.end() && value->is_boolean() && value->get<bool>();
}

// 五个内置方案的名字，与 Engine 的 profile.name() 相同；不认识的名字按小鹤处理，和 macOS 的 NormalizeShuangpinSchema 一致。
inline std::string_view normalize_shuangpin_keymap_profile(std::string_view name) {
  for (const std::string_view known : {"xiaohe", "ziranma", "shoudao", "microsoft", "smart_abc"})
    if (name == known) return known;
  return "xiaohe";
}

// 组字串的最后一个字符是字母或分号时高亮那个键，和 macOS 的 MSIMEShuangpinKeymapHighlightedKey 一样只看 editing_text（原始按键，与预编辑显示成什么无关）。
inline char shuangpin_keymap_highlighted_key(std::string_view editing) {
  if (editing.empty()) return 0;
  const char last = editing.back();
  if (last >= 'a' && last <= 'z') return static_cast<char>(last - 'a' + 'A');
  if ((last >= 'A' && last <= 'Z') || last == ';') return last;
  return 0;
}

// 从 Engine 的 view 判断这一帧要不要显示键位图，条件与 macOS 的 updateKeymapPanel 相同：双拼方案（scheme 1）、没有局部模式、不在 Engine 自己的英文模式、有方案名、正在组字。开关、中文输入是否打开和光标是否可用由宿主另外判断。自定义方案（`custom`）不显示：键位表只按方案名从 host-api 取，`custom` 的表在偏好里，按小鹤画会标错键，标错比不标更糟。
inline std::optional<ShuangpinKeymapHint> shuangpin_keymap_hint(const nlohmann::json &view) {
  if (!view.is_object()) return std::nullopt;
  const auto scheme = view.find("scheme");
  const auto mode = view.find("local_mode");
  const auto english = view.find("dedicated_english");
  const auto profile = view.find("shuangpin_profile");
  const auto editing = view.find("editing_text");
  if (scheme == view.end() || !scheme->is_number_integer() || scheme->get<std::int64_t>() != 1 ||
      mode == view.end() || !mode->is_string() || mode->get_ref<const std::string &>() != "none" ||
      english == view.end() || !english->is_boolean() || english->get<bool>() || profile == view.end() ||
      !profile->is_string() || profile->get_ref<const std::string &>().empty() ||
      profile->get_ref<const std::string &>() == "custom" || editing == view.end() || !editing->is_string() ||
      editing->get_ref<const std::string &>().empty())
    return std::nullopt;
  return ShuangpinKeymapHint{std::string(normalize_shuangpin_keymap_profile(profile->get_ref<const std::string &>())),
                             shuangpin_keymap_highlighted_key(editing->get_ref<const std::string &>())};
}

// 键帽上的一个键：键名和它在这个方案里代表的声母、韵母。
struct ShuangpinKeymapKey {
  std::string key;
  std::string codes;
};
using ShuangpinKeymapRows = std::array<std::vector<ShuangpinKeymapKey>, 3>;

inline std::vector<std::string_view> shuangpin_keymap_split(std::string_view text, std::string_view separator) {
  std::vector<std::string_view> parts;
  std::size_t position = 0;
  while (true) {
    const auto found = text.find(separator, position);
    if (found == std::string_view::npos) {
      parts.push_back(text.substr(position));
      return parts;
    }
    parts.push_back(text.substr(position, found - position));
    position = found + separator.size();
  }
}

// Engine 的提示是「声母 / 韵母」，每一侧的单位按空格分开；键帽上用「 · 」分开，与 macOS 的 CodesText 相同。
inline std::string shuangpin_keymap_codes_text(std::string_view hint) {
  std::string result;
  const auto sides = shuangpin_keymap_split(hint, " / ");
  for (std::size_t side = 0; side < sides.size(); ++side) {
    if (side) result += " / ";
    const auto units = shuangpin_keymap_split(sides[side], " ");
    for (std::size_t unit = 0; unit < units.size(); ++unit) {
      if (unit) result += " · ";
      result.append(units[unit].data(), units[unit].size());
    }
  }
  return result;
}

// 三排键帽，键序与 macOS 的 MSIMEShuangpinKeymapRows 相同。`hints` 是 msime_client_shuangpin_key_hints 的 value：大写键名到「声母 / 韵母」的对象。中排的分号只在这个方案给它分了韵母时出现（微软双拼）。
inline ShuangpinKeymapRows shuangpin_keymap_rows(const nlohmann::json &hints) {
  const auto hint = [&](const std::string &key) -> std::string {
    if (!hints.is_object()) return {};
    const auto found = hints.find(key);
    return found != hints.end() && found->is_string() ? shuangpin_keymap_codes_text(found->get_ref<const std::string &>())
                                                     : std::string{};
  };
  ShuangpinKeymapRows rows;
  for (const char *key : {"Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"}) rows[0].push_back({key, hint(key)});
  for (const char *key : {"A", "S", "D", "F", "G", "H", "J", "K", "L"}) rows[1].push_back({key, hint(key)});
  if (!hint(";").empty()) rows[1].push_back({";", hint(";")});
  for (const char *key : {"Z", "X", "C", "V", "B", "N", "M"}) rows[2].push_back({key, hint(key)});
  return rows;
}

// 底部的零声母说明：「零声母  a=aa · ang=ah · …」，按条目排序，v 开头的音节写成 ü，与 macOS 的 MSIMEShuangpinZeroInitialText 相同。`zero` 是 msime_client_shuangpin_zero_initials 的 value。
inline std::string shuangpin_keymap_zero_initial_text(const nlohmann::json &zero) {
  std::vector<std::string> entries;
  if (zero.is_object()) {
    for (const auto &[syllable, code] : zero.items()) {
      if (!code.is_string()) continue;
      const std::string shown = !syllable.empty() && syllable.front() == 'v' ? "ü" + syllable.substr(1) : syllable;
      entries.push_back(shown + "=" + code.get<std::string>());
    }
  }
  std::sort(entries.begin(), entries.end());
  std::string text = "零声母  ";
  for (std::size_t index = 0; index < entries.size(); ++index) {
    if (index) text += " · ";
    text += entries[index];
  }
  return text;
}

// 标题行：「<方案>键位」，右侧是「当前按键会高亮 · 上屏后自动隐藏」，与 macOS 相同。方案名用 macOS 的 ShuangpinSchemaTitle 的叫法。
inline std::string shuangpin_keymap_title(std::string_view profile) {
  if (profile == "ziranma") return "自然码双拼键位";
  if (profile == "shoudao") return "首道双拼键位";
  if (profile == "microsoft") return "微软双拼键位";
  if (profile == "smart_abc") return "智能 ABC 双拼键位";
  return "小鹤双拼键位";
}
inline constexpr const char *kShuangpinKeymapSubtitle = "当前按键会高亮 · 上屏后自动隐藏";

// 键位图的几何，逻辑像素，取 macOS 面板的尺寸：620×203 的卡片、13 圆角，左右各留 14；标题行顶部留 10，键帽三排各高 38、圆角 7、键间距 5，第一排离标题 7；零声母一行高 13、离下排 6、底部留 10。macOS 的标题行没有固定高度，被这些约束撑到 33，两段字在里面垂直居中，这里直接写成 33。中排、下排按键数往里收，与 macOS 的 KeyRowInset 相同。表面四周另留 shadow_margin 的透明边画阴影（只在有合成器时）。
struct ShuangpinKeymapGeometry {
  static constexpr double width = 620.0;
  static constexpr double height = 203.0;
  static constexpr double radius = 13.0;
  static constexpr double inset = 14.0;
  static constexpr double header_top = 10.0;
  static constexpr double header_height = 33.0;
  static constexpr double first_row_gap = 7.0;
  static constexpr double row_gap = 5.0;
  static constexpr double key_height = 38.0;
  static constexpr double key_spacing = 5.0;
  static constexpr double key_radius = 7.0;
  static constexpr double footer_gap = 6.0;
  static constexpr double footer_height = 13.0;
  static constexpr double title_size = 12.0;
  static constexpr double subtitle_size = 10.0;
  static constexpr double key_label_size = 11.0;
  static constexpr double codes_size = 9.0;
  static constexpr double footer_size = 10.0;
  static constexpr double shadow_margin = 12.0;
  // 离候选窗 8，离工作区边缘 16，与 macOS 的 MSIMEShuangpinKeymapPanelFrame 相同。
  static constexpr double candidate_gap = 8.0;
  static constexpr double screen_margin = 16.0;
};

inline double shuangpin_keymap_row_inset(std::size_t count) {
  if (count >= 10) return 14.0;
  if (count == 9) return 31.0;
  return 58.0;
}

inline double shuangpin_keymap_row_top(std::size_t row) {
  using G = ShuangpinKeymapGeometry;
  return G::header_top + G::header_height + G::first_row_gap + static_cast<double>(row) * (G::key_height + G::row_gap);
}

// 一个键帽在卡片里的矩形（相对卡片左上角，逻辑像素）。
struct ShuangpinKeymapRect {
  double x = 0.0, y = 0.0, width = 0.0, height = 0.0;
};
inline ShuangpinKeymapRect shuangpin_keymap_key_rect(std::size_t row, std::size_t index, std::size_t count) {
  using G = ShuangpinKeymapGeometry;
  const double inset = shuangpin_keymap_row_inset(count);
  const double available = G::width - inset * 2.0 - G::key_spacing * static_cast<double>(count ? count - 1 : 0);
  const double key_width = count ? available / static_cast<double>(count) : 0.0;
  return {inset + static_cast<double>(index) * (key_width + G::key_spacing), shuangpin_keymap_row_top(row), key_width,
          G::key_height};
}

// 零声母一行的顶边：下排键帽之下 footer_gap。
inline double shuangpin_keymap_footer_top() {
  using G = ShuangpinKeymapGeometry;
  return shuangpin_keymap_row_top(2) + G::key_height + G::footer_gap;
}

// 候选窗估计占的高度（逻辑像素，乘过候选缩放），键位图要让开它。候选窗由桌面面板（IBus 面板、Fcitx5 经典界面）画时宿主拿不到它的矩形，所以与 macOS 在算出精确几何之前的基线一样按字号估计：横排一行是字号加 42，竖排每行字号加 10、再加 24 的上下边。面板画拼音行时再加一行 22 的标题。Fcitx5 的经典界面用的字体由插件按同一个候选字号偏好写进去，所以估计用的字号与它画的一致；Fcitx5 自己画候选窗（原生画布）时改用 shuangpin_keymap_measured_clearance。
inline double shuangpin_keymap_candidate_clearance(double font_size, double scale, bool horizontal,
                                                   std::size_t candidates, bool preedit_row) {
  const double rows = static_cast<double>(std::max<std::size_t>(candidates, 1));
  double clearance = horizontal ? font_size + 42.0 : (font_size + 10.0) * rows + 24.0;
  if (preedit_row) clearance += 22.0;
  return clearance * scale;
}

// 宿主自己画的候选窗（Fcitx5 的原生画布）在根窗口里的真实矩形换成要让开的高度（设备像素）：候选窗在光标下方时是它的底边离光标底边多远，翻到光标上方时是它的顶边离光标顶边多远，对应 macOS 用 candidatePageGeometry 算出的精确高度。
inline int shuangpin_keymap_measured_clearance(int caret_y, int caret_height, int window_y, int window_height) {
  const int caret_bottom = caret_y + caret_height;
  if (window_y >= caret_bottom) return window_y + window_height - caret_bottom;
  return std::max(0, caret_y - window_y);
}

// 键位图本体（不含阴影边）左上角的根窗口坐标，照搬 macOS 的 MSIMEShuangpinKeymapPanelFrame，只是换成 y 轴向下：左边与光标对齐，放在光标下方、让开候选窗再隔 gap；下面放不下就翻到光标上方同样的距离；最后夹在工作区四边各 margin 以内。所有长度都是设备像素。
inline WaveOverlayPosition shuangpin_keymap_position(int caret_x, int caret_y, int caret_height, int panel_width,
                                                     int panel_height, int clearance, WaveOverlayWorkArea work, int gap,
                                                     int margin) {
  const auto clamp = [](int value, int minimum, int maximum) {
    return maximum < minimum ? minimum : std::clamp(value, minimum, maximum);
  };
  const int minimum_x = work.x + margin;
  const int maximum_x = work.x + work.width - margin - panel_width;
  const int minimum_y = work.y + margin;
  const int maximum_y = work.y + work.height - margin - panel_height;
  const int below = caret_y + caret_height + clearance + gap;
  const int preferred = below <= maximum_y ? below : caret_y - clearance - gap - panel_height;
  return {clamp(caret_x, minimum_x, maximum_x), clamp(preferred, minimum_y, maximum_y)};
}

// 键位图要画的颜色，全部不透明（0xRRGGBB），取自候选窗解析出的主题：卡片是主题底色，键帽在底色上叠一层很淡的文字色（对应 macOS 的 controlBackgroundColor），描边是主题的分隔线，高亮键填主题的强调色、字是白色，与 macOS 的 MSIMEKeymapAccent（皮肤 tokens.accent）相同。系统主题因此是 Adwaita 的蓝色，皮肤画自己的强调色。
struct ShuangpinKeymapColors {
  std::uint32_t surface = 0xFFFFFF;
  std::uint32_t text = 0x2E2E2E;
  std::uint32_t secondary = 0x777777;
  std::uint32_t key = 0xF4F4F4;
  std::uint32_t key_border = 0xDADADA;
  std::uint32_t accent = 0x3584E4;
  std::uint32_t accent_text = 0xFFFFFF;
  std::optional<std::uint32_t> border;
};

}  // namespace msime::linux_host
