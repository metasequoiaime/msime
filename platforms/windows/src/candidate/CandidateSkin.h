#pragma once
#include <filesystem>
#include "CandidatePalette.h"
#include <nlohmann/json.hpp>
#include <string>

namespace msime::windows {
// Reads the package assets (decoration and minimum width) from the catalog msime_client_skin_catalog returns. Its colours come from msime_client_resolve_theme instead, and the assets are drawn exactly when that resolution names the package in candidate_skin. The catalog serializes SkinSummary in camelCase. The shared catalog already validated the manifest; nothing here trusts it further than the bounds below.
// The mascot a package draws above the card, if it has one.
//
// The catalog parses these and the settings preview renders them, but the live
// card never touched any of it - a mascot skin looked right in settings and
// plain while actually typing.
struct CandidateSkinDecoration {
  // Absolute path to the image, empty when the package has none.
  std::wstring image;
  // How far the artwork rises above the card's top edge, in DIPs.
  double top_dip = 0.0;
  // Drawn width in DIPs; the height follows the image's own aspect ratio.
  double width_dip = 0.0;
};
inline CandidateSkinDecoration
candidate_skin_decoration(const nlohmann::json &catalog, const std::string &id,
                          const std::filesystem::path &root) {
  CandidateSkinDecoration decoration;
  if (id.empty() || !catalog.is_object() || !catalog.contains("packages") ||
      !catalog.at("packages").is_array())
    return decoration;
  for (const auto &package : catalog.at("packages")) {
    if (!package.is_object() || !package.contains("id") ||
        !package.at("id").is_string() || package.at("id") != id)
      continue;
    if (!package.contains("preview") || !package.at("preview").is_string())
      return decoration;
    const auto preview = package.at("preview").get<std::string>();
    // The catalog already refused a preview that escapes the package
    // directory; refuse an empty or over-long one here rather than building a
    // path from it.
    if (preview.empty() || preview.size() > 256)
      return decoration;
    const auto top = package.value("decorationTopDip", 0.0);
    const auto width = package.value("decorationWidthDip", 0.0);
    // Both are required: a decoration with no width would draw nothing, and
    // one that does not rise above the card is not a decoration.
    if (!(top > 0.0) || !(width > 0.0) || top > 512.0 || width > 1024.0)
      return decoration;
    auto path = root / std::filesystem::u8path(id) /
                std::filesystem::u8path(preview);
    std::error_code error;
    if (!std::filesystem::is_regular_file(path, error))
      return decoration;
    decoration.image = path.wstring();
    decoration.top_dip = top;
    decoration.width_dip = width;
    return decoration;
  }
  return decoration;
}
// The minimum card width a package asks for, in DIPs, or 0 when it asks for
// none. A mascot skin is drawn against a card of a particular width; a narrower
// card makes the decoration overhang it.
inline double candidate_skin_min_width(const nlohmann::json &catalog,
                                       const std::string &id) {
  if (id.empty() || !catalog.is_object() || !catalog.contains("packages") ||
      !catalog.at("packages").is_array())
    return 0.0;
  for (const auto &package : catalog.at("packages")) {
    if (!package.is_object() || !package.contains("id") ||
        !package.at("id").is_string() || package.at("id") != id)
      continue;
    if (!package.contains("minWidthDip") ||
        !package.at("minWidthDip").is_number())
      return 0.0;
    const auto value = package.at("minWidthDip").get<double>();
    // The catalog validates the manifest, but a value this card cannot use is
    // still refused here rather than propagated into the geometry.
    if (!(value > 0.0) || value > 2000.0)
      return 0.0;
    return value;
  }
  return 0.0;
}
} // namespace msime::windows
