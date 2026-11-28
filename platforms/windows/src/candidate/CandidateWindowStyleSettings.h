#pragma once
#include "CandidateWindowStyle.h"
#include <cstdint>
#include <mutex>
#include <nlohmann/json.hpp>
#include <optional>

namespace msime::windows {
// 从偏好文档里取出样式字段，和 candidate_font_settings 取字体一样：缺字段就是它的缺省值，类型不对或超出范围则整份投影被拒，卡片保留原来的样式。
inline std::optional<CandidateWindowStyle>
candidate_window_style(const nlohmann::json &preferences) {
  try {
    if (!preferences.is_object())
      return std::nullopt;
    CandidateWindowStyle result;
    for (const auto &[key, target] :
         {std::pair{"candidate_scale_percent", &result.scale_percent},
          std::pair{"candidate_opacity_percent", &result.opacity_percent}}) {
      if (!preferences.contains(key))
        continue;
      if (!preferences.at(key).is_number_integer())
        return std::nullopt;
      const auto value = preferences.at(key).get<int64_t>();
      if (value < 0 || value > 1000)
        return std::nullopt;
      *target = static_cast<unsigned>(value);
    }
    // 行高与行距：null 和缺字段一样表示跟随皮肤与窗口布局（设置页的「跟随皮肤」写 null），整数 0–16 才是用户的值。
    for (const auto &[key, target] :
         {std::pair{"candidate_item_padding_vertical", &result.item_padding_vertical},
          std::pair{"candidate_item_spacing", &result.item_spacing}}) {
      if (!preferences.contains(key) || preferences.at(key).is_null())
        continue;
      if (!preferences.at(key).is_number_integer())
        return std::nullopt;
      const auto value = preferences.at(key).get<int64_t>();
      if (value < 0 || value > 16)
        return std::nullopt;
      *target = static_cast<unsigned>(value);
    }
    if (preferences.contains("candidate_corner_radius") &&
        !preferences.at("candidate_corner_radius").is_null()) {
      const auto &radius = preferences.at("candidate_corner_radius");
      if (!radius.is_number_integer())
        return std::nullopt;
      const auto value = radius.get<int64_t>();
      if (value < 0 || value > 32)
        return std::nullopt;
      result.corner_radius = static_cast<unsigned>(value);
    }
    return result.valid() ? std::optional{result} : std::nullopt;
  } catch (...) {
    return std::nullopt;
  }
}

// Latest style by preference revision, handed from the monitor thread to the UI thread like CandidateFontMailbox.
class CandidateWindowStyleMailbox {
public:
  bool publish(uint64_t revision, CandidateWindowStyle value) {
    if (!value.valid())
      return false;
    std::lock_guard lock(mutex_);
    if (revision_ && revision <= *revision_)
      return false;
    revision_ = revision;
    pending_ = value;
    return true;
  }
  std::optional<CandidateWindowStyle> take() {
    std::lock_guard lock(mutex_);
    auto result = pending_;
    pending_.reset();
    return result;
  }

private:
  std::mutex mutex_;
  std::optional<uint64_t> revision_;
  std::optional<CandidateWindowStyle> pending_;
};
} // namespace msime::windows
