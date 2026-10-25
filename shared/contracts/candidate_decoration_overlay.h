#pragma once

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <optional>
#include <string>

namespace msime::candidate {

// 来源 0978b5c2 的装饰词契约；只存共享扫描器输出，不解析 TOML 或候选原文。
struct DecorationCandidate {
  int index = 1;
  double fontSizeDip = 16;
  double minFontSizeDip = 8;
  double maxWidthDip = 0;
  std::string color = "#FFFFFFFF";
  std::string alignH = "center", alignV = "center";
  double offsetXDip = 0, offsetYDip = 0;
  bool stripBadge = true;

  bool valid() const {
    const auto bounded = [](double value, double low, double high) {
      return std::isfinite(value) && value >= low && value <= high;
    };
    return index >= 0 && index <= 100 && fontSizeDip > 0 && minFontSizeDip > 0 &&
      bounded(fontSizeDip, 0, 200) && bounded(minFontSizeDip, 0, 200) &&
      bounded(maxWidthDip, 0, 2000) && bounded(offsetXDip, -2000, 2000) && bounded(offsetYDip, -2000, 2000) &&
      (alignH == "left" || alignH == "center" || alignH == "right") &&
      (alignV == "top" || alignV == "center" || alignV == "bottom");
  }

  std::optional<std::size_t> seat(std::size_t count, int highlight) const {
    const int value = index == 0 ? highlight : index - 1;
    if (value < 0 || static_cast<std::size_t>(value) >= count) return std::nullopt;
    return static_cast<std::size_t>(value);
  }

  double fittedFont(double naturalWidth, double boxWidth) const {
    const double limit = maxWidthDip > 0 ? maxWidthDip : boxWidth;
    return naturalWidth > limit && naturalWidth > 0
      ? (std::max)((std::min)(fontSizeDip, minFontSizeDip), fontSizeDip * limit / naturalWidth)
      : fontSizeDip;
  }

  double left(double boxWidth, double textWidth) const {
    return offsetXDip + (boxWidth - textWidth) * (alignH == "left" ? 0 : alignH == "center" ? 0.5 : 1);
  }
  double top(double boxHeight, double textHeight) const {
    return offsetYDip + (boxHeight - textHeight) * (alignV == "top" ? 0 : alignV == "center" ? 0.5 : 1);
  }

  DecorationCandidate scaled(double factor) const {
    auto result = *this;
    result.fontSizeDip *= factor;
    result.minFontSizeDip *= factor;
    result.maxWidthDip *= factor;
    result.offsetXDip *= factor;
    result.offsetYDip *= factor;
    return result;
  }
};

} // namespace msime::candidate
