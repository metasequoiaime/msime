#include "../src/candidates/CandidateFontPolicy.h"

#include <cassert>

int main() {
  using msime::linux_host::candidate_pango_font;
  using msime::linux_host::CandidateFont;
  using msime::linux_host::CandidateFontSync;

  assert(candidate_pango_font({"Noto Sans SC", {"Noto Sans SC", "Microsoft YaHei"}, 18}) ==
         "Noto Sans SC, Microsoft YaHei, 18px");
  // Blank entries and repeats are dropped, surrounding space is trimmed.
  assert(candidate_pango_font({"  LXGW WenKai ", {"", "LXGW WenKai", "Noto Sans CJK SC"}, 20}) ==
         "LXGW WenKai, Noto Sans CJK SC, 20px");
  // A trailing style word stays part of the family because the list ends with a comma.
  assert(candidate_pango_font({"Source Han Sans Bold", {}, 16}) == "Source Han Sans Bold, 16px");
  // A comma inside a name cannot split it into two families.
  assert(candidate_pango_font({"Odd,Name", {}, 16}) == "OddName, 16px");
  // Out-of-range sizes fall back to the shared default rather than reaching the panel.
  assert(candidate_pango_font({"Sans", {}, 4}) == "Sans, 18px");
  assert(candidate_pango_font({"", {}, 14}) == "14px");

  using msime::linux_host::read_candidate_font;
  // A document without the keys reads as the store's defaults.
  assert(candidate_pango_font(read_candidate_font(nlohmann::json::object())) ==
         "Noto Sans SC, Microsoft YaHei, 18px");
  assert(candidate_pango_font(read_candidate_font(nlohmann::json{
             {"candidate_font_family", "LXGW WenKai"},
             {"candidate_fallback_fonts", nlohmann::json::array({"Noto Sans CJK SC", 3})},
             {"candidate_font_size", 24}})) == "LXGW WenKai, Noto Sans CJK SC, 24px");
  // An emptied fallback list stays empty rather than reverting to the defaults.
  assert(candidate_pango_font(read_candidate_font(nlohmann::json{
             {"candidate_fallback_fonts", nlohmann::json::array()}})) == "Noto Sans SC, 18px");

  // The English family leads the list, as Windows draws Latin from it first; the primary family and the fallbacks follow for the glyphs it lacks, without repeats.
  assert(candidate_pango_font(read_candidate_font(nlohmann::json{
             {"candidate_english_font", "Inter"}})) == "Inter, Noto Sans SC, Microsoft YaHei, 18px");
  assert(candidate_pango_font(read_candidate_font(nlohmann::json{
             {"candidate_english_font", " Microsoft YaHei "},
             {"candidate_font_family", "LXGW WenKai"}})) ==
         "Microsoft YaHei, LXGW WenKai, Noto Sans SC, 18px");
  assert(candidate_pango_font({"Noto Sans SC", {"Noto Sans SC"}, 18, "Noto Sans SC"}) == "Noto Sans SC, 18px");
  // Unset (absent, null or blank) leaves the description exactly as it was before the setting existed.
  assert(candidate_pango_font(read_candidate_font(nlohmann::json{
             {"candidate_english_font", nullptr}})) == "Noto Sans SC, Microsoft YaHei, 18px");
  assert(candidate_pango_font({"Noto Sans SC", {"Noto Sans SC", "Microsoft YaHei"}, 18, "  "}) ==
         "Noto Sans SC, Microsoft YaHei, 18px");

  using msime::linux_host::candidate_font_is_default;
  assert(candidate_font_is_default(read_candidate_font(nlohmann::json::object())));
  assert(candidate_font_is_default(read_candidate_font(nlohmann::json{{"candidate_english_font", nullptr}})));
  assert(!candidate_font_is_default(read_candidate_font(nlohmann::json{{"candidate_english_font", "Inter"}})));

  CandidateFontSync untouched;
  assert(!untouched.next({"Noto Sans SC", {"Noto Sans SC", "Microsoft YaHei"}, 18}));
  assert(!untouched.next({"Noto Sans SC", {"Noto Sans SC", "Microsoft YaHei"}, 18}));
  assert(untouched.next({"Noto Sans SC", {"Noto Sans SC", "Microsoft YaHei"}, 22}) ==
         "Noto Sans SC, Microsoft YaHei, 22px");
  // Going back to the defaults after a change is itself a change the panel must see.
  assert(untouched.next({"Noto Sans SC", {"Noto Sans SC", "Microsoft YaHei"}, 18}) ==
         "Noto Sans SC, Microsoft YaHei, 18px");

  CandidateFontSync chosen;
  assert(chosen.next({"LXGW WenKai", {}, 20}) == "LXGW WenKai, 20px");
  assert(!chosen.next({"LXGW WenKai", {}, 20}));

  // Choosing only an English family is a font choice the panel must see, even on the first refresh.
  CandidateFontSync english;
  assert(english.next(read_candidate_font(nlohmann::json{{"candidate_english_font", "Inter"}})) ==
         "Inter, Noto Sans SC, Microsoft YaHei, 18px");
  assert(!english.next(read_candidate_font(nlohmann::json{{"candidate_english_font", "Inter"}})));

  // Fcitx5 的经典界面要磅：像素乘 3/4，总是 0.25 的整数倍，照原样写出；超出范围同样退回默认的 18 像素。
  using msime::linux_host::CandidateFontUnit;
  assert(candidate_pango_font({"MiSans", {"Noto Sans SC", "Microsoft YaHei"}, 14}, CandidateFontUnit::Points) ==
         "MiSans, Noto Sans SC, Microsoft YaHei, 10.5");
  assert(candidate_pango_font({"Sans", {}, 12}, CandidateFontUnit::Points) == "Sans, 9");
  assert(candidate_pango_font({"Sans", {}, 13}, CandidateFontUnit::Points) == "Sans, 9.75");
  assert(candidate_pango_font({"Sans", {}, 15}, CandidateFontUnit::Points) == "Sans, 11.25");
  assert(candidate_pango_font({"Sans", {}, 32}, CandidateFontUnit::Points) == "Sans, 24");
  assert(candidate_pango_font({"Sans", {}, 4}, CandidateFontUnit::Points) == "Sans, 13.5");
  assert(candidate_pango_font({"", {}, 14}, CandidateFontUnit::Points) == "10.5");
  // 单位不改变「是否仍是默认」的判断：没动过的偏好照样不写。
  CandidateFontSync points(CandidateFontUnit::Points);
  assert(!points.next({"Noto Sans SC", {"Noto Sans SC", "Microsoft YaHei"}, 18}));
  assert(points.next({"MiSans", {"Noto Sans SC", "Microsoft YaHei"}, 14}) ==
         "MiSans, Noto Sans SC, Microsoft YaHei, 10.5");
  assert(!points.next({"MiSans", {"Noto Sans SC", "Microsoft YaHei"}, 14}));
  // 升级前按像素写过同一个字体时，偏好虽是默认值也按磅重写一次，之后照旧不重复写；面板里是别的描述，或者仍写像素时，不动它。
  const CandidateFont defaultFont{"Noto Sans SC", {"Noto Sans SC", "Microsoft YaHei"}, 18};
  CandidateFontSync upgraded(CandidateFontUnit::Points);
  assert(upgraded.next(defaultFont, std::string("Noto Sans SC, Microsoft YaHei, 18px")) ==
         "Noto Sans SC, Microsoft YaHei, 13.5");
  assert(!upgraded.next(defaultFont, std::string("Noto Sans SC, Microsoft YaHei, 13.5")));
  CandidateFontSync foreign(CandidateFontUnit::Points);
  assert(!foreign.next(defaultFont, std::string("Sans 10")));
  CandidateFontSync pixels;
  assert(!pixels.next(defaultFont, std::string("Noto Sans SC, Microsoft YaHei, 18px")));
  return 0;
}
