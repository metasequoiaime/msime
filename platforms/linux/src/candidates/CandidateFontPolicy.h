#pragma once

#include <algorithm>
#include <cctype>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace msime::linux_host {

// Windows draws its own candidate window and reads candidate_font_family, the fallback chain and
// candidate_font_size straight into it. Neither Linux host draws the list: IBus hands it to the
// desktop panel and Fcitx5 to its classic UI, and both of those take one Pango font description.
// So the Linux answer to the same three settings is to write that description where the panel
// reads it, rather than to draw a second candidate window next to the one the desktop already has.
struct CandidateFont {
  std::string family;
  std::vector<std::string> fallbacks;
  int size_px = 0;
  // candidate_english_font. Windows draws candidate text in this face first and falls back for the glyphs it lacks, and macOS and Android name it ahead of the primary family; a Pango family list resolves per glyph the same way, so the Linux answer is to put it first. Empty when unset, which leaves the description exactly as it was before the setting existed.
  std::string english_family{};
};

inline constexpr const char *kDefaultCandidateFontFamily = "Noto Sans SC";
inline constexpr int kDefaultCandidateFontSize = 18;

inline const std::vector<std::string> &default_candidate_fallback_fonts() {
  static const std::vector<std::string> fonts{"Noto Sans SC", "Microsoft YaHei"};
  return fonts;
}

// Reads the shared font preferences the way the preference store defaults them, so a document written before a key existed describes the same font as one that spells the default out.
inline CandidateFont read_candidate_font(const nlohmann::json &preferences) {
  CandidateFont font{kDefaultCandidateFontFamily, default_candidate_fallback_fonts(),
                     kDefaultCandidateFontSize};
  if (!preferences.is_object()) return font;
  if (const auto family = preferences.find("candidate_font_family");
      family != preferences.end() && family->is_string())
    font.family = family->get<std::string>();
  if (const auto fallbacks = preferences.find("candidate_fallback_fonts");
      fallbacks != preferences.end() && fallbacks->is_array()) {
    font.fallbacks.clear();
    for (const auto &fallback : *fallbacks)
      if (fallback.is_string()) font.fallbacks.push_back(fallback.get<std::string>());
  }
  if (const auto size = preferences.find("candidate_font_size");
      size != preferences.end() && size->is_number_integer())
    font.size_px = size->get<int>();
  // Optional and absent by default; null is how the store writes "not chosen".
  if (const auto english = preferences.find("candidate_english_font");
      english != preferences.end() && english->is_string())
    font.english_family = english->get<std::string>();
  return font;
}

inline std::string trimmed_font_family(const std::string &value) {
  auto begin = value.begin();
  auto end = value.end();
  while (begin != end && std::isspace(static_cast<unsigned char>(*begin))) ++begin;
  while (end != begin && std::isspace(static_cast<unsigned char>(*(end - 1)))) --end;
  std::string family(begin, end);
  // A comma separates families in a Pango description; one inside a name would split it in two.
  family.erase(std::remove(family.begin(), family.end(), ','), family.end());
  return family;
}

// 字号写成什么单位。共享偏好里的字号是像素，IBus 面板照写像素。Fcitx5 5.1.18 起的经典界面要写成磅：它画
// 候选序号时取描述里的字号乘上主题的 LabelTextSizeFactor 再按磅设回去（pango_font_description_set_size），
// 像素字号因此被当成同样数值的磅，序号比候选大三分之一。5.1.22 起它在 X11 上按 DPI 整窗缩放、字体 DPI 固定
// 为 96，Wayland 上除非用户设了 ForceWaylandDPI 也是 96，像素乘 3/4 就是同样大小的磅。5.1.18 到 5.1.21 的
// X11 把 Xft.dpi（没有时取不低于 96 的屏幕 DPI）作为字体 DPI，磅数随它放大，大小与 5.1.22 起整窗缩放后的
// 相同。更早的版本序号与候选用同一个描述，本来就一样大，照写像素（见 CMakeLists 的 MSIME_FCITX5_LABEL_POINTS）。
enum class CandidateFontUnit { Pixels, Points };

// 像素换成磅是乘 3/4，偏好的字号是整数，所以磅数总是 0.25 的整数倍，照原样写出，不经浮点格式化。
inline std::string candidate_font_size_text(int size_px, CandidateFontUnit unit) {
  if (unit == CandidateFontUnit::Pixels) return std::to_string(size_px) + "px";
  static constexpr const char *quarters[] = {"", ".25", ".5", ".75"};
  return std::to_string(size_px * 3 / 4) + quarters[size_px * 3 % 4];
}

// 描述先列选了的英文字体，再列主字体和回退字体，去掉重复；字体列表以逗号结尾，名字以 Pango 认得的样式词（"Bold"、"Light"）结尾时仍算名字的一部分。
inline std::string candidate_pango_font(const CandidateFont &font,
                                        CandidateFontUnit unit = CandidateFontUnit::Pixels) {
  std::vector<std::string> families;
  families.reserve(font.fallbacks.size() + 2);
  auto add = [&](const std::string &value) {
    auto family = trimmed_font_family(value);
    if (!family.empty() && std::find(families.begin(), families.end(), family) == families.end())
      families.push_back(std::move(family));
  };
  add(font.english_family);
  add(font.family);
  for (const auto &fallback : font.fallbacks) add(fallback);
  std::string description;
  for (const auto &family : families) {
    if (!description.empty()) description += ", ";
    description += family;
  }
  if (!description.empty()) description += ",";
  const int size = font.size_px >= 12 && font.size_px <= 32 ? font.size_px : kDefaultCandidateFontSize;
  if (!description.empty()) description += ' ';
  description += candidate_font_size_text(size, unit);
  return description;
}

// The default has no English family, so a document that never set one still counts as untouched and leaves the desktop's panel font alone, while choosing one counts as a change like any other font choice.
inline bool candidate_font_is_default(const CandidateFont &font) {
  return candidate_pango_font(font) ==
         candidate_pango_font({kDefaultCandidateFontFamily, default_candidate_fallback_fonts(),
                               kDefaultCandidateFontSize, std::string{}});
}

// The panel font belongs to the whole desktop, not to this input method, so the host only writes
// it in answer to the user choosing a font. Untouched defaults leave whatever the desktop already
// has; after that, every change the user makes is written once, and an unchanged value is not
// written again on every preference refresh.
class CandidateFontSync {
public:
  explicit CandidateFontSync(CandidateFontUnit unit = CandidateFontUnit::Pixels) : unit_(unit) {}

  // current 是面板配置里现有的描述。偏好仍是默认值时本来不写；但它正是旧版本按像素写下的同一个字体时，按磅
  // 重写一次，否则升级前写过像素的用户要等再改一次字体，序号才与候选一样大。
  std::optional<std::string> next(const CandidateFont &font, const std::optional<std::string> &current = std::nullopt) {
    auto description = candidate_pango_font(font, unit_);
    if (!last_ && candidate_font_is_default(font)) {
      last_ = description;
      if (unit_ == CandidateFontUnit::Points && current &&
          *current == candidate_pango_font(font, CandidateFontUnit::Pixels))
        return description;
      return std::nullopt;
    }
    if (last_ == description) return std::nullopt;
    last_ = description;
    return description;
  }
  // 是否已经同步过一次；之后 next 不再看 current。
  bool primed() const { return last_.has_value(); }

private:
  CandidateFontUnit unit_;
  std::optional<std::string> last_;
};

}  // namespace msime::linux_host
