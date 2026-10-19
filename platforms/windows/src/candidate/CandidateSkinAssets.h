#pragma once
#include "CandidateSkin.h"
#include <algorithm>

namespace msime::windows {
inline bool valid_candidate_skin_id(const std::string &id) {
  const auto alnum = [](unsigned char c) {
    return (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9');
  };
  return !id.empty() && id.size() <= 64 && alnum(id.front()) &&
         std::all_of(id.begin(), id.end(), [&](unsigned char c) {
           return alnum(c) || c == '.' || c == '_' || c == '-';
         });
}
struct CandidateSkinAssets {
  double min_width = 0.0;
  CandidateSkinDecoration decoration;
  std::optional<float> corner_radius;
  CandidateSkinBackground background;
  CandidateSkinToolbar toolbar;
  CandidateSkinAnimation animation;
  std::string font_family;
  // 完整候选配色，按当前明暗叠加到解析主题；资源字段仍由各自的绘制能力决定。
  nlohmann::json candidate_colors = nlohmann::json::object();
};
inline CandidateSkinAssets
candidate_skin_assets(const nlohmann::json &catalog, const std::string &id,
                      const std::filesystem::path &root) {
  if (!valid_candidate_skin_id(id))
    return {};
  CandidateSkinAssets assets{candidate_skin_min_width(catalog, id),
          candidate_skin_decoration(catalog, id, root),
          candidate_skin_corner_radius(catalog, id),
          candidate_skin_background(catalog, id, root),
          candidate_skin_toolbar(catalog, id),
          candidate_skin_animation(catalog, id),
          std::string{}};
  if (const auto *package = detail::candidate_skin_package(catalog, id);
      package && package->contains("fontFamily") && package->at("fontFamily").is_string())
    assets.font_family = package->at("fontFamily").get<std::string>();
  if (const auto *package = detail::candidate_skin_package(catalog, id);
      package && package->contains("candidate") && package->at("candidate").is_object())
    assets.candidate_colors = package->at("candidate");
  return assets;
}
} // namespace msime::windows
