#include "../../src/candidate/CandidateThemeSettings.h"
#include <cassert>
#include <chrono>
#include <fstream>

int main() {
  using namespace msime::windows;
  const auto custom = [](const std::string &id) {
    return nlohmann::json{{"global_theme", "custom"},
                          {"custom_theme",
                           {{"base", "system"},
                            {"candidate_skin", id},
                            {"candidate_colors", nlohmann::json::object()}}}};
  };
  for (const auto &id : {std::string("willow_green"),
                         std::string("sample.skin-1"), std::string(64, 'a')}) {
    assert(valid_candidate_skin_id(id));
    // The package a custom theme names is the one whose files are watched.
    assert(candidate_theme_package(candidate_theme_values(custom(id))) == id);
  }
  for (const auto &id :
       {std::string(""), std::string("../escape"), std::string("Upper"),
        std::string("/absolute"), std::string(65, 'a')}) {
    assert(!valid_candidate_skin_id(id));
    assert(candidate_theme_package(candidate_theme_values(custom(id))).empty());
  }
  // Only a custom theme draws a package; the built-in themes watch nothing.
  auto builtin = custom("sample");
  builtin["global_theme"] = "paper";
  assert(candidate_theme_package(candidate_theme_values(builtin)).empty());
  assert(candidate_theme_package(nlohmann::json::object()).empty());
  const auto root =
      std::filesystem::temp_directory_path() /
      ("msime-skin-reload-" +
       std::to_string(
           std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(root / "sample");
  // A synthetic file is enough for the resource-selection test; no renderer
  // or real image is involved in this portable test.
  std::ofstream(root / "sample" / "preview.png") << "synthetic";
  const nlohmann::json catalog{
      {"packages", nlohmann::json::array({{{"id", "sample"},
                                           {"preview", "preview.png"},
                                           {"decorationImage", "preview.png"},
                                           {"minWidthDip", 320},
                                           {"decorationTopDip", 50},
                                           {"decorationWidthDip", 180}}})}};
  const auto external = candidate_skin_assets(catalog, "sample", root);
  assert(external.min_width == 320 && external.decoration.top_dip == 50 &&
         external.decoration.width_dip == 180);
  assert(external.decoration.image ==
         (root / "sample" / "preview.png").wstring());
  // The catalog serializes SkinSummary in camelCase; the snake_case names this reader used before never matched, so an external package drew no artwork and no minimum width.
  const nlohmann::json snake{
      {"packages", nlohmann::json::array({{{"id", "sample"},
                                           {"preview", "preview.png"},
                                           {"min_width_dip", 320},
                                           {"decoration_top_dip", 50},
                                           {"decoration_width_dip", 180}}})}};
  const auto unread = candidate_skin_assets(snake, "sample", root);
  assert(unread.min_width == 0 && unread.decoration.image.empty());
  // Switching to no package or to an unavailable one must clear old artwork/width.
  for (const char *id : {"", "missing", "../sample", "Sample"}) {
    const auto reset = candidate_skin_assets(catalog, id, root);
    assert(reset.min_width == 0 && reset.decoration.image.empty());
    assert(reset.decoration.top_dip == 0 && reset.decoration.width_dip == 0);
  }
  std::filesystem::remove(root / "sample" / "preview.png");
  assert(
      candidate_skin_assets(catalog, "sample", root).decoration.image.empty());
  std::filesystem::remove(root / "sample");
  std::filesystem::remove(root);
  CandidateThemeMailbox mailbox;
  mailbox.publish(custom("sample"));
  mailbox.publish(custom("other"));
  assert(candidate_theme_package(*mailbox.take()) == "other");
}
