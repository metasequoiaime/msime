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
    // 自定义主题指名的包就是要监视文件的那个。
    assert(candidate_theme_packages(candidate_theme_values(custom(id))) ==
           std::vector<std::string>{id});
  }
  for (const auto &id :
       {std::string(""), std::string("../escape"), std::string("Upper"),
        std::string("/absolute"), std::string(65, 'a')}) {
    assert(!valid_candidate_skin_id(id));
    assert(candidate_theme_packages(candidate_theme_values(custom(id))).empty());
  }
  // Only a custom theme draws a package; the built-in themes watch nothing.
  auto builtin = custom("sample");
  builtin["global_theme"] = "paper";
  assert(candidate_theme_packages(candidate_theme_values(builtin)).empty());
  assert(candidate_theme_packages(nlohmann::json::object()).empty());
  // 两个槽位：浅色 candidate_skin 和深色 candidate_skin_dark 指名的包都监视，同一个包只算一次，无效的 id 略过。
  auto slotted = custom("sakura");
  slotted["custom_theme"]["candidate_skin_dark"] = "dusk";
  assert(candidate_theme_packages(candidate_theme_values(slotted)) ==
         (std::vector<std::string>{"sakura", "dusk"}));
  // candidate_theme_values 把深色槽位原样带过线程边界，解析请求里也有它。
  assert(candidate_theme_request(candidate_theme_values(slotted), true, false,
                                 std::filesystem::path())
             .at("custom_theme")
             .at("candidate_skin_dark") == "dusk");
  auto dark_only = slotted;
  dark_only["custom_theme"].erase("candidate_skin");
  assert(candidate_theme_packages(candidate_theme_values(dark_only)) ==
         std::vector<std::string>{"dusk"});
  auto same = slotted;
  same["custom_theme"]["candidate_skin_dark"] = "sakura";
  assert(candidate_theme_packages(candidate_theme_values(same)) ==
         std::vector<std::string>{"sakura"});
  auto unsafe_dark = slotted;
  unsafe_dark["custom_theme"]["candidate_skin_dark"] = "../escape";
  assert(candidate_theme_packages(candidate_theme_values(unsafe_dark)) ==
         std::vector<std::string>{"sakura"});
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
  assert(candidate_theme_packages(*mailbox.take()) ==
         std::vector<std::string>{"other"});
}
