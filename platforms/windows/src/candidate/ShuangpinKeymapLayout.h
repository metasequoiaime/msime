#pragma once
#include "CandidatePalette.h"
#include "ToolbarLayout.h"
#include <nlohmann/json.hpp>
#include <algorithm>
#include <array>
#include <cmath>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace msime::windows {
// 双拼键位提示：双拼组字时在候选窗旁显示当前方案的键位图，高亮刚按下的键，上屏后隐藏，对应 macOS 的 MSIMEShuangpinKeymapPanel。键位表不在这里另存一份，取自 host-api 的 msime_client_shuangpin_key_hints / msime_client_shuangpin_zero_initials（Engine 自己的方案表）。这里是纯计算：要不要显示、显示哪几行、摆在哪里，不依赖 Windows 头文件，主机上的单测直接包含。

// 一帧要显示的键位提示：方案名和要高亮的键（'A'–'Z' 或 ';'，0 表示不高亮）。
struct ShuangpinKeymapHint {
  std::string profile;
  char key = 0;
  bool operator==(const ShuangpinKeymapHint &other) const {
    return profile == other.profile && key == other.key;
  }
  bool operator!=(const ShuangpinKeymapHint &other) const { return !(*this == other); }
};

// 共享偏好 `shuangpin_keymap_hint`：没选过时文档里没有这一项，按关处理；不是布尔值时同样按关。
inline bool shuangpin_keymap_preference(const nlohmann::json &preferences) {
  if (!preferences.is_object())
    return false;
  const auto value = preferences.find("shuangpin_keymap_hint");
  return value != preferences.end() && value->is_boolean() && value->get<bool>();
}

// 四个方案的名字，与 Engine 的 profile.name() 相同；不认识的名字按小鹤处理，和 macOS 的 NormalizeShuangpinSchema 一致。
inline std::string_view normalize_shuangpin_profile(std::string_view name) {
  for (const std::string_view known : {"xiaohe", "ziranma", "shoudao", "microsoft"})
    if (name == known)
      return known;
  return "xiaohe";
}

// 组字串的最后一个字符是字母或分号时高亮那个键，和 macOS 的 MSIMEShuangpinKeymapHighlightedKey 一样只看 editing_text（原始按键，与预编辑显示成什么无关）。
inline char shuangpin_keymap_highlighted_key(std::string_view editing) {
  if (editing.empty())
    return 0;
  const char last = editing.back();
  if (last >= 'a' && last <= 'z')
    return static_cast<char>(last - 'a' + 'A');
  if ((last >= 'A' && last <= 'Z') || last == ';')
    return last;
  return 0;
}

// 从 Engine 的 view 判断这一帧要不要显示键位图，条件与 macOS 的 updateKeymapPanel 相同：双拼方案（scheme 1）、没有局部模式、不在 Engine 自己的英文模式、有方案名、正在组字。开关和候选窗是否可见由调用方另外判断。自定义方案（`custom`）不显示：键位表只按方案名从 host-api 取，`custom` 的表在偏好里，msime_client_shuangpin_key_hints 对它返回空；按小鹤画会标错键，标错比不标更糟。
inline std::optional<ShuangpinKeymapHint> shuangpin_keymap_hint(const nlohmann::json &view) {
  if (!view.is_object())
    return std::nullopt;
  const auto scheme = view.find("scheme");
  const auto mode = view.find("local_mode");
  const auto english = view.find("dedicated_english");
  const auto profile = view.find("shuangpin_profile");
  const auto editing = view.find("editing_text");
  if (scheme == view.end() || !scheme->is_number_integer() || scheme->get<int64_t>() != 1 ||
      mode == view.end() || !mode->is_string() || mode->get_ref<const std::string &>() != "none" ||
      english == view.end() || !english->is_boolean() || english->get<bool>() ||
      profile == view.end() || !profile->is_string() || profile->get_ref<const std::string &>().empty() ||
      editing == view.end() || !editing->is_string() || editing->get_ref<const std::string &>().empty() ||
      profile->get_ref<const std::string &>() == "custom")
    return std::nullopt;
  return ShuangpinKeymapHint{std::string(normalize_shuangpin_profile(profile->get_ref<const std::string &>())),
                             shuangpin_keymap_highlighted_key(editing->get_ref<const std::string &>())};
}

// 键帽上的一个键：键名和它在这个方案里代表的声母、韵母。
struct ShuangpinKeymapKey {
  std::string key;
  std::string codes;
};
using ShuangpinKeymapRows = std::array<std::vector<ShuangpinKeymapKey>, 3>;

// 按分隔符切开，保留空段。
inline std::vector<std::string_view> shuangpin_keymap_split(std::string_view text, std::string_view separator) {
  std::vector<std::string_view> parts;
  size_t position = 0;
  while (true) {
    const size_t found = text.find(separator, position);
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
  for (size_t side = 0; side < sides.size(); ++side) {
    if (side)
      result += " / ";
    const auto units = shuangpin_keymap_split(sides[side], " ");
    for (size_t unit = 0; unit < units.size(); ++unit) {
      if (unit)
        result += " · ";
      result.append(units[unit].data(), units[unit].size());
    }
  }
  return result;
}

// 三排键帽，键序与 macOS 的 MSIMEShuangpinKeymapRows 相同。`hints` 是 msime_client_shuangpin_key_hints 的 value：大写键名到「声母 / 韵母」的对象。中排的分号只在这个方案给它分了韵母时出现（微软双拼）。
inline ShuangpinKeymapRows shuangpin_keymap_rows(const nlohmann::json &hints) {
  const auto hint = [&](const std::string &key) -> std::string {
    if (!hints.is_object())
      return {};
    const auto found = hints.find(key);
    return found != hints.end() && found->is_string() ? shuangpin_keymap_codes_text(found->get_ref<const std::string &>())
                                                     : std::string{};
  };
  ShuangpinKeymapRows rows;
  for (const char *key : {"Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"})
    rows[0].push_back({key, hint(key)});
  for (const char *key : {"A", "S", "D", "F", "G", "H", "J", "K", "L"})
    rows[1].push_back({key, hint(key)});
  if (!hint(";").empty())
    rows[1].push_back({";", hint(";")});
  for (const char *key : {"Z", "X", "C", "V", "B", "N", "M"})
    rows[2].push_back({key, hint(key)});
  return rows;
}

// 底部的零声母说明：「零声母  a=aa · ang=ah · …」，按条目排序，v 开头的音节写成 ü，与 macOS 的 MSIMEShuangpinZeroInitialText 相同。`zero` 是 msime_client_shuangpin_zero_initials 的 value。
inline std::string shuangpin_keymap_zero_initial_text(const nlohmann::json &zero) {
  std::vector<std::string> entries;
  if (zero.is_object()) {
    for (const auto &[syllable, code] : zero.items()) {
      if (!code.is_string())
        continue;
      const std::string shown = !syllable.empty() && syllable.front() == 'v' ? "ü" + syllable.substr(1) : syllable;
      entries.push_back(shown + "=" + code.get<std::string>());
    }
  }
  std::sort(entries.begin(), entries.end());
  std::string text = "零声母  ";
  for (size_t index = 0; index < entries.size(); ++index) {
    if (index)
      text += " · ";
    text += entries[index];
  }
  return text;
}

// 键位图的几何，设备无关像素，取 macOS 面板的尺寸：620×203 的卡片，左右各留 14，三排键帽各高 38、键间距 5；中排、下排按键数往里收，与 macOS 的 KeyRowInset 相同。窗口四周再加阴影的边距。
struct ShuangpinKeymapMetrics {
  double width = 620.0;
  double height = 203.0;
  double inset = 14.0;
  double header_top = 10.0;
  double header_height = 20.0;
  double row_gap = 5.0;
  double first_row_gap = 7.0;
  double key_height = 38.0;
  double key_spacing = 5.0;
  double key_radius = 7.0;
  double footer_height = 16.0;
  double footer_bottom = 10.0;
  ToolbarShadow shadow = toolbar_shadow(true);
  double window_width() const { return shadow.left + width + shadow.right; }
  double window_height() const { return shadow.top + height + shadow.bottom; }
  double row_top(size_t row) const {
    return header_top + header_height + first_row_gap + static_cast<double>(row) * (key_height + row_gap);
  }
};

inline double shuangpin_keymap_row_inset(size_t count) {
  if (count >= 10)
    return 14.0;
  if (count == 9)
    return 31.0;
  return 58.0;
}

// 一个键帽在卡片里的矩形（相对卡片左上角）。
struct ShuangpinKeymapRect {
  double left = 0.0, top = 0.0, right = 0.0, bottom = 0.0;
};
inline ShuangpinKeymapRect shuangpin_keymap_key_rect(const ShuangpinKeymapMetrics &metrics, size_t row, size_t index,
                                                    size_t count) {
  const double inset = shuangpin_keymap_row_inset(count);
  const double available = metrics.width - inset * 2.0 - metrics.key_spacing * static_cast<double>(count ? count - 1 : 0);
  const double key_width = count ? available / static_cast<double>(count) : 0.0;
  const double left = inset + static_cast<double>(index) * (key_width + metrics.key_spacing);
  const double top = metrics.row_top(row);
  return {left, top, left + key_width, top + metrics.key_height};
}

// 高亮键帽上的字色：强调色够暗时用白字，够亮时用黑字（Windows 深色主题的强调色是浅蓝，白字看不清），按 WCAG 的相对亮度取对比更大的那一个。
inline CandidateColor shuangpin_keymap_on_accent(const CandidateColor &accent) {
  const auto linear = [](float channel) {
    return channel <= 0.04045f ? channel / 12.92f
                               : static_cast<float>(std::pow((channel + 0.055f) / 1.055f, 2.4f));
  };
  const float luminance = 0.2126f * linear(accent.r) + 0.7152f * linear(accent.g) + 0.0722f * linear(accent.b);
  // 白字对比度 (1.05)/(L+0.05)，黑字 (L+0.05)/0.05，两者相等时 L 约为 0.179。
  return luminance > 0.179f ? CandidateColor{0.0f, 0.0f, 0.0f, 1.0f} : CandidateColor{1.0f, 1.0f, 1.0f, 1.0f};
}

// 屏幕上的矩形，物理像素。
struct ShuangpinKeymapScreenRect {
  long left = 0, top = 0, right = 0, bottom = 0;
};
struct ShuangpinKeymapPlacementInput {
  // 候选卡片在屏幕上的矩形（不含透明的阴影边距，CandidateWindow::card_on_screen）和光标所在行的左下角（TSF 报的锚点，候选窗也从它摆起）。
  ShuangpinKeymapScreenRect candidate;
  long anchor_x = 0, anchor_y = 0;
  // 光标所在显示器的工作区。
  ShuangpinKeymapScreenRect work;
  // 卡片本身的像素尺寸和它左上方阴影的像素边距。
  long card_width = 0, card_height = 0;
  long shadow_left = 0, shadow_top = 0;
  // 离候选窗的距离、离屏幕边缘的距离和一行文字的高度（与候选窗翻到上方时跨过的行高相同，24 DIP）。
  long gap = 8, margin = 16, line_height = 24;
};
struct ShuangpinKeymapPoint {
  long x = 0, y = 0;
};

inline long shuangpin_keymap_clamp(long value, long minimum, long maximum) {
  return maximum < minimum ? minimum : std::clamp(value, minimum, maximum);
}

// 键位图放在候选窗离光标远的那一侧，和 macOS 一样不挡住正在输入的那一行：候选窗在光标下方时放在候选窗下面，放不下就放到光标所在行的上方；候选窗翻到光标上方时放在候选窗上面，放不下就放到光标下方。候选窗在光标哪一侧按卡片的竖直中线判断，卡片被工作区底边往上推了几像素、顶边略高于锚点时仍算在下方。左边与候选窗对齐，再收进工作区。返回窗口左上角（已减去阴影边距）。
inline ShuangpinKeymapPoint shuangpin_keymap_placement(const ShuangpinKeymapPlacementInput &input) {
  const long min_x = input.work.left + input.margin;
  const long max_x = input.work.right - input.margin - input.card_width;
  const long min_y = input.work.top + input.margin;
  const long max_y = input.work.bottom - input.margin - input.card_height;
  long y = 0;
  if (input.candidate.top + input.candidate.bottom >= 2 * input.anchor_y) {
    const long below = input.candidate.bottom + input.gap;
    y = below + input.card_height <= input.work.bottom - input.margin
            ? below
            : input.anchor_y - input.line_height - input.gap - input.card_height;
  } else {
    const long above = input.candidate.top - input.gap - input.card_height;
    y = above >= input.work.top + input.margin ? above : input.anchor_y + input.gap;
  }
  return {shuangpin_keymap_clamp(input.candidate.left, min_x, max_x) - input.shadow_left,
          shuangpin_keymap_clamp(y, min_y, max_y) - input.shadow_top};
}
} // namespace msime::windows
