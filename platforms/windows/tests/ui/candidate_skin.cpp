#include "CandidateSkin.h"
#include <chrono>
#include <fstream>
#include <stdexcept>

using namespace msime::windows;
using Json = nlohmann::json;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Candidate skin validation failed");
}
// One msime_client_skin_catalog entry. SkinSummary is serialized in camelCase, and its colours are not read here: they reach the card through msime_client_resolve_theme.
Json package(const char *id, Json min_width, Json top, Json width) {
  return Json{{"id", id},
              {"name", "Sample"},
              {"version", "1.0"},
              {"base", "system"},
              {"layouts", Json::array({"vertical", "horizontal"})},
              {"themes", Json::array({"dark", "light"})},
              {"preview", "preview.png"},
              {"minWidthDip", std::move(min_width)},
              {"decorationTopDip", std::move(top)},
              {"decorationWidthDip", std::move(width)}};
}
int main() {
  const auto root =
      std::filesystem::temp_directory_path() /
      ("msime-candidate-skin-" +
       std::to_string(
           std::chrono::steady_clock::now().time_since_epoch().count()));
  for (const char *id : {"mascot", "wide", "hostile"}) {
    std::filesystem::create_directories(root / id);
    // A synthetic file is enough: the reader only checks that the preview exists.
    std::ofstream(root / id / "preview.png") << "synthetic";
  }
  const Json catalog{
      {"packages", Json::array({package("mascot", 320.0, 50.0, 180.0),
                                package("wide", 2400.0, 600.0, 180.0),
                                package("hostile", "320", -1.0, 0.0)})},
      {"issues", Json::array()}};

  // A package that declares artwork is drawn with it, against its own minimum width.
  require(candidate_skin_min_width(catalog, "mascot") == 320.0);
  const auto mascot = candidate_skin_decoration(catalog, "mascot", root);
  require(mascot.image == (root / "mascot" / "preview.png").wstring());
  require(mascot.top_dip == 50.0 && mascot.width_dip == 180.0);

  // Values this card cannot use are refused rather than propagated into the geometry.
  require(candidate_skin_min_width(catalog, "wide") == 0.0);
  require(candidate_skin_decoration(catalog, "wide", root).image.empty());
  require(candidate_skin_min_width(catalog, "hostile") == 0.0);
  const auto hostile = candidate_skin_decoration(catalog, "hostile", root);
  require(hostile.image.empty() && hostile.top_dip == 0.0 &&
          hostile.width_dip == 0.0);

  // Unknown, unnamed and malformed catalogs draw no package.
  for (const auto &missing : {Json::object(), Json{{"packages", 7}},
                              Json::array(), catalog}) {
    require(candidate_skin_min_width(missing, "absent") == 0.0);
    require(candidate_skin_decoration(missing, "absent", root).image.empty());
  }
  require(candidate_skin_min_width(catalog, "") == 0.0);
  require(candidate_skin_decoration(catalog, "", root).image.empty());

  // A preview that is missing on disk, empty or over-long is not drawn.
  Json unsafe = catalog;
  unsafe["packages"][0]["preview"] = "absent.png";
  require(candidate_skin_decoration(unsafe, "mascot", root).image.empty());
  unsafe["packages"][0]["preview"] = "";
  require(candidate_skin_decoration(unsafe, "mascot", root).image.empty());
  unsafe["packages"][0]["preview"] = std::string(300, 'p');
  require(candidate_skin_decoration(unsafe, "mascot", root).image.empty());

  std::error_code error;
  std::filesystem::remove_all(root, error);
}
