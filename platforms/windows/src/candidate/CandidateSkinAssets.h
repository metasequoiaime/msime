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
};
inline CandidateSkinAssets
candidate_skin_assets(const nlohmann::json &catalog, const std::string &id,
                      const std::filesystem::path &root) {
  if (!valid_candidate_skin_id(id))
    return {};
  return {candidate_skin_min_width(catalog, id),
          candidate_skin_decoration(catalog, id, root),
          candidate_skin_corner_radius(catalog, id),
          candidate_skin_background(catalog, id, root),
          candidate_skin_toolbar(catalog, id)};
}
} // namespace msime::windows
