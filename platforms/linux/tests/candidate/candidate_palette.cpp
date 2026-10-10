#include "../src/candidates/CandidateColors.h"
#include "../src/candidates/CandidatePalette.h"

#include <cassert>

int main() {
  using Json = nlohmann::json;
  namespace host = msime::linux_host;

  // The native tokens are the design's Adwaita ones (tok('linux')): the translucent ink is composited over the surface because both frontends paint opaque colours.
  const auto light = host::candidate_native_palette(false);
  assert(light.surface == 0xFFFFFFu && light.text == 0x2E2E2Eu && light.number == 0x737373u);
  assert(light.accent == 0x1C71D8u && light.border == 0xE5E5E5u);
  assert(light.selected == 0x3584E4u && light.selected_text == 0xFFFFFFu && light.selected_number == 0xE1EDFBu);
  const auto dark = host::candidate_native_palette(true);
  assert(dark.surface == 0x303030u && dark.text == 0xFFFFFFu && dark.number == 0xA2A2A2u);
  assert(dark.accent == 0x78AEEDu && dark.border == 0x404040u);
  assert(dark.selected == 0x3584E4u && dark.selected_text == 0xFFFFFFu);

  // "follow" takes the mode preference, as Windows resolves theme_cand against theme_mode: only "system" consults the desktop appearance, and an explicit candidate mode wins over both.
  using host::candidate_dark_theme;
  assert(!candidate_dark_theme(Json{{"candidate_theme", "follow"}}, false));
  assert(candidate_dark_theme(Json::object(), true));
  assert(!candidate_dark_theme(Json{{"theme", "light"}, {"candidate_theme", "follow"}}, true));
  assert(candidate_dark_theme(Json{{"theme", "dark"}, {"candidate_theme", "follow"}}, false));
  assert(candidate_dark_theme(Json{{"theme", "light"}, {"candidate_theme", "dark"}}, false));
  assert(!candidate_dark_theme(Json{{"theme", "dark"}, {"candidate_theme", "light"}}, true));

  // A floating surface's own mode key (toolbar_theme) wins when it names a mode and otherwise defers to the global mode, whose default "system" follows the desktop; the candidate mode plays no part.
  using host::surface_dark_theme;
  assert(surface_dark_theme(Json{{"toolbar_theme", "dark"}, {"candidate_theme", "light"}}, "toolbar_theme", false));
  assert(!surface_dark_theme(Json{{"toolbar_theme", "light"}, {"theme", "dark"}}, "toolbar_theme", true));
  assert(surface_dark_theme(Json{{"toolbar_theme", "follow"}, {"theme", "dark"}}, "toolbar_theme", false));
  assert(!surface_dark_theme(Json{{"toolbar_theme", "follow"}, {"candidate_theme", "dark"}}, "toolbar_theme", false));
  assert(surface_dark_theme(Json::object(), "toolbar_theme", true));

  // 运行时选项里的 preferences 可能是 null、数组或数字（runtime-options.json 被手工改过，或写入被截断）。
  // 两个宿主共用的入口必须照一份空文档处理：跟随系统明暗、默认纵向布局、解析成 system，而不是抛 nlohmann 的 type_error。
  for (const auto &document : {Json(nullptr), Json::array({Json("paper")}), Json(7)}) {
    assert(candidate_dark_theme(document, true) && !candidate_dark_theme(document, false));
    assert(surface_dark_theme(document, "toolbar_theme", true));
    assert(!surface_dark_theme(document, "toolbar_theme", false));
    assert(host::candidate_layout_id(document) == "vertical");
    assert(host::candidate_theme_request(document, false, Json::object()) ==
           Json::parse(R"({"dark":false,"global_theme":"system","layout":"vertical"})"));
  }
  assert(host::candidate_layout_id(Json{{"candidate_layout", "horizontal"}}) == "horizontal");

  // Its palette is the resolved card: surface, text and accent, and the outline only when the theme draws one.
  const auto native_floating = host::floating_surface_colors(host::candidate_theme_colors(Json::object(), true));
  assert(native_floating.surface == dark.surface && native_floating.text == dark.text);
  assert(native_floating.accent == dark.accent && native_floating.border == dark.border);
  const auto borderless = host::floating_surface_colors(host::candidate_theme_colors(
      Json{{"candidate", {{"surface", "#102030"}, {"text", "#F0F0F0"}, {"accent", "#FF8800"}, {"border", "#00000000"}}}},
      false));
  assert(borderless.surface == 0x102030u && borderless.text == 0xF0F0F0u && borderless.accent == 0xFF8800u);
  assert(!borderless.border);

  // Slot colours: the shared layer writes #RRGGBB or #RRGGBBAA and nothing else.
  assert(host::theme_color(Json("#FF000080"))->alpha == 0x80);
  assert(host::theme_color(Json("#ABCDEF"))->rgb == 0xABCDEFu && host::theme_color(Json("#ABCDEF"))->alpha == 0xFF);
  assert(!host::theme_color(Json("transparent")));
  assert(!host::theme_color(Json("rgba(0,0,0,0.1)")));
  assert(!host::theme_color(Json("#12345")));
  assert(!host::theme_color(Json("#1234567g")));
  assert(!host::theme_color(Json(nullptr)));

  // `system` resolves to a null palette: the native tokens in the host's mode, solid #3584E4 selection with white text, a one-pixel outline, and no package decoration.
  const Json system = Json::parse(R"({"id":"system","source":"system","appearance":null,"candidate":null,"keyboard":null,"candidate_skin":null})");
  const auto native_light = host::candidate_theme_colors(system, false);
  assert(!native_light.dark && native_light.candidate_skin.empty());
  assert(native_light.colors.background == 0xFFFFFFu && native_light.colors.text == 0x2E2E2Eu);
  assert(native_light.colors.number == 0x737373u && native_light.colors.accent == 0x1C71D8u);
  assert(native_light.colors.selected == 0x3584E4u && native_light.colors.selected_text == 0xFFFFFFu);
  assert(native_light.colors.selected_number == 0xE1EDFBu);
  assert(native_light.colors.border == 0xE5E5E5u && native_light.colors.border_width == 1);
  const auto native_dark = host::candidate_theme_colors(system, true);
  assert(native_dark.dark && native_dark.colors.background == 0x303030u && native_dark.colors.border == 0x404040u);
  // A failed call draws exactly the same.
  assert(host::candidate_theme_colors(Json::object(), true).colors.background == 0x303030u);

  // A built-in theme fixes its appearance whatever the host's mode, and its translucent slots are composited over its surface. 浅色 (light) as msime_client_resolve_theme answers it:
  const Json builtin_light = Json::parse(
      R"({"id":"light","source":"builtin","appearance":"light","candidate":{"surface":"#FFFFFF","border":"#0000001F",)"
      R"("text":"#1A1A1A","number":"#6A6F76","secondary":"#6A6F76","accent":"#005FB8","selected":"#005FB824",)"
      R"("selected_text":"#005FB8","selected_number":"#6A6F76","hover":"#1A1A1A0F","show_selected_bar":null},)"
      R"("keyboard":null,"candidate_skin":null})");
  const auto fixed = host::candidate_theme_colors(builtin_light, true);
  assert(!fixed.dark);
  // 有效系统主题使用原生配色绘制水杉样式；无覆盖自定义与解析失败不能借原生配色接管。
  assert(fixed.covers_candidates && native_light.covers_candidates && native_dark.covers_candidates);
  assert(!host::candidate_theme_colors(Json::object(), true).covers_candidates);
  assert(fixed.colors.background == 0xFFFFFFu && fixed.colors.text == 0x1A1A1Au && fixed.colors.number == 0x6A6F76u);
  assert(fixed.colors.accent == 0x005FB8u);
  assert(fixed.colors.selected == host::composite_color(0x005FB8u, 0x24, 0xFFFFFFu));
  assert(fixed.colors.selected_text == 0x005FB8u && fixed.colors.selected_number == 0x6A6F76u);
  assert(fixed.colors.border == host::composite_color(0x000000u, 0x1F, 0xFFFFFFu) && fixed.colors.border_width == 1);
  // 水杉 is dark on a light desktop.
  const Json shuishan = Json::parse(
      R"({"id":"shuishan","source":"builtin","appearance":"dark","candidate":{"surface":"#2A2B27","border":"#0000001F",)"
      R"("text":"#FFFFFF","number":"#9FB5A3","secondary":"#9FB5A3","accent":"#7FE08E","selected":"#7FE08E24",)"
      R"("selected_text":"#7FE08E","selected_number":"#9FB5A3","hover":"#FFFFFF0F","show_selected_bar":null},)"
      R"("keyboard":null,"candidate_skin":null})");
  const auto shuishan_colors = host::candidate_theme_colors(shuishan, false);
  assert(shuishan_colors.dark && shuishan_colors.colors.background == 0x2A2B27u);
  assert(shuishan_colors.colors.selected_text == 0x7FE08Eu);

  // A custom theme over `system` sets only some slots; the rest are native. An accent without a selected fill is drawn as the design's solid selection, with readable text.
  const Json custom = Json::parse(
      R"({"id":"custom","source":"custom","appearance":null,"candidate":{"surface":null,"border":null,"text":"#123456",)"
      R"("number":"#1234569D","secondary":"#1234569D","accent":"#FFD400","selected":null,"selected_text":null,)"
      R"("selected_number":null,"hover":null,"show_selected_bar":null},"keyboard":null,"candidate_skin":null})");
  const auto picked = host::candidate_theme_colors(custom, true);
  assert(picked.dark && picked.colors.background == 0x303030u && picked.colors.text == 0x123456u);
  assert(picked.colors.number == host::composite_color(0x123456u, 0x9D, 0x303030u));
  assert(picked.colors.selected == 0xFFD400u && picked.colors.selected_text == 0x000000u);
  assert(picked.colors.selected_number == 0x000000u && picked.colors.border == 0x404040u);
  // A selected fill without its own foreground takes the theme's text when the theme sets one.
  auto filled = custom;
  filled["candidate"]["selected"] = "#654321";
  const auto filled_colors = host::candidate_theme_colors(filled, false);
  assert(filled_colors.colors.selected == 0x654321u && filled_colors.colors.selected_text == 0x123456u);
  // The contract's secondary equals number unless a skin gives the translation its own colour, which is then drawn over the surface.
  assert(!filled_colors.colors.translation && !picked.colors.translation);
  auto glossed = custom;
  glossed["candidate"]["secondary"] = "#9FB4E0";
  assert(host::candidate_theme_colors(glossed, true).colors.translation == 0x9FB4E0u);
  glossed["candidate"]["secondary"] = "#9FB4E080";
  assert(host::candidate_theme_colors(glossed, true).colors.translation ==
         host::composite_color(0x9FB4E0u, 0x80, 0x303030u));
  // A package draws its decoration only when the shared layer names it, and a transparent border means no outline.
  filled["candidate_skin"] = "sakura";
  filled["candidate"]["border"] = "#00000000";
  const auto package = host::candidate_theme_colors(filled, false);
  assert(package.candidate_skin == "sakura" && !package.colors.border && package.colors.border_width == 0);
  assert(package.covers_candidates);
  // 皮肤被画出来但清单没给它颜色槽位（candidate 是空对象）时仍然算覆盖：装饰图要画。
  assert(host::candidate_theme_colors(
             Json::parse(R"({"id":"custom","source":"custom","appearance":null,"candidate":null,)"
                         R"("keyboard":null,"candidate_skin":"sakura"})"),
             true)
             .covers_candidates);
  // 退役的皮肤 id、未知主题和解析失败都落到空文档上，同样没有覆盖。
  assert(!host::candidate_theme_colors(Json::object(), false).covers_candidates);

  // The request: the stored theme, the mode and the layout being drawn, and the package entry only when a custom theme names a listed one.
  const Json catalog = Json::parse(
      R"({"packages":[{"id":"sakura","title":"樱花","base":"light","layouts":["vertical"],"candidate":{"light":{}}}]})");
  const auto plain = host::candidate_theme_request(Json::object(), true, catalog);
  assert(plain == Json::parse(R"({"global_theme":"system","dark":true,"layout":"vertical"})"));
  const auto horizontal = host::candidate_theme_request(Json{{"global_theme", "paper"}, {"candidate_layout", "horizontal"}}, false, catalog);
  assert(horizontal.at("global_theme") == "paper" && horizontal.at("layout") == "horizontal" && !horizontal.contains("package"));
  const Json with_package = {{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}, {"base", "light"}}}};
  const auto request = host::candidate_theme_request(with_package, false, catalog);
  assert(request.at("custom_theme") == with_package.at("custom_theme"));
  assert(request.at("package") == catalog.at("packages").at(0));
  // Not while another theme is selected, and not for a package the catalogue does not list.
  auto elsewhere = with_package;
  elsewhere["global_theme"] = "ink";
  assert(!host::candidate_theme_request(elsewhere, false, catalog).contains("package"));
  assert(host::candidate_theme_request(elsewhere, false, catalog).contains("custom_theme"));
  auto unlisted = with_package;
  unlisted["custom_theme"]["candidate_skin"] = "absent";
  assert(!host::candidate_theme_request(unlisted, false, catalog).contains("package"));
  // 两个槽位：请求带的是这个明暗下槽位指名的那一项。深色取 candidate_skin_dark，没设时取 candidate_skin；浅色只取 candidate_skin。
  const Json slotted_catalog = Json::parse(
      R"({"packages":[{"id":"sakura","title":"樱花","base":"light","layouts":["vertical"],"candidate":{"light":{}}},)"
      R"({"id":"dusk","title":"黄昏","base":"night","layouts":["vertical"],"candidate":{"dark":{}}}]})");
  const Json both_slots = {{"global_theme", "custom"},
                           {"custom_theme", {{"candidate_skin", "sakura"}, {"candidate_skin_dark", "dusk"}}}};
  assert(host::candidate_theme_request(both_slots, true, slotted_catalog).at("package") ==
         slotted_catalog.at("packages").at(1));
  assert(host::candidate_theme_request(both_slots, false, slotted_catalog).at("package") ==
         slotted_catalog.at("packages").at(0));
  assert(host::candidate_theme_request(with_package, true, slotted_catalog).at("package") ==
         slotted_catalog.at("packages").at(0));
  const Json dark_only = {{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin_dark", "dusk"}}}};
  assert(!host::candidate_theme_request(dark_only, false, slotted_catalog).contains("package"));
  assert(host::candidate_theme_request(dark_only, true, slotted_catalog).at("package") ==
         slotted_catalog.at("packages").at(1));
  // 深色槽位指名的包不在目录里时不退回浅色槽位：共享层只认当前模式槽位指名的那一项。
  auto dark_absent = both_slots;
  dark_absent["custom_theme"]["candidate_skin_dark"] = "absent";
  assert(!host::candidate_theme_request(dark_absent, true, slotted_catalog).contains("package"));

  // 主题选择：只有 global_theme 与 custom_theme 参与比较；缺失或非字符串的 global_theme 与 system 等价，其他偏好字段的变化不算选主题。
  using host::candidate_theme_selection;
  assert(candidate_theme_selection(Json::object()) == candidate_theme_selection(Json{{"global_theme", "system"}}));
  assert(candidate_theme_selection(Json{{"global_theme", 7}}) == candidate_theme_selection(Json::object()));
  assert(candidate_theme_selection(Json{{"global_theme", "paper"}}) !=
         candidate_theme_selection(Json{{"global_theme", "ink"}}));
  assert(candidate_theme_selection(Json{{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "sakura"}}}}) !=
         candidate_theme_selection(Json{{"global_theme", "custom"}, {"custom_theme", {{"candidate_skin", "kite"}}}}));
  assert(candidate_theme_selection(Json{{"global_theme", "paper"}, {"candidate_layout", "horizontal"}}) ==
         candidate_theme_selection(Json{{"global_theme", "paper"}, {"candidate_layout", "vertical"}}));
  assert(candidate_theme_selection(Json(nullptr)) == candidate_theme_selection(Json::array({Json(1)})));

  assert(msime::linux_host::candidate_preedit_with_caret("nihao", "nihao", 0) ==
         "|nihao");
  assert(msime::linux_host::candidate_preedit_with_caret("nihao", "nihao", 2) ==
         "ni|hao");
  assert(msime::linux_host::candidate_preedit_with_caret("nihao", "nihao", 5) ==
         "nihao|");
  assert(msime::linux_host::candidate_preedit_with_caret("ni'hao", "nihao", 2) ==
         "ni'hao");
  assert(msime::linux_host::candidate_preedit_with_caret("nihao", "nihao", 9) ==
         "nihao");
}
