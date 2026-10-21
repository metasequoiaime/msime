#include "CandidateSkin.h"
#include "CandidateSkinAssets.h"
#include "CandidateSkinLayout.h"
#include <cmath>
#include <stdexcept>

using namespace msime::windows;
using Json = nlohmann::json;
namespace {
void require(bool value) {
  if (!value)
    throw std::runtime_error("Candidate skin layout validation failed");
}
bool near(double value, double expected) {
  return std::fabs(value - expected) < 0.001;
}
Json length(double value, bool em = false) { return Json{{"value", value}, {"em", em}}; }
// msime_client_skin_catalog 的一项，几何字段按 SkinSummary 的 camelCase 序列化；数值是合成的。
Json package(const char *id) {
  return Json{{"id", id},
              {"name", "Sample"},
              {"version", "1.0"},
              {"base", "system"},
              {"layouts", Json::array({"vertical", "horizontal"})},
              {"themes", Json::array({"dark", "light"})},
              {"minWidthDip", 0.0}};
}
} // namespace

int main() {
  // 共享层合并后的排列表优先；原始表仍是编辑器看到的声明，不能重新覆盖解析结果。
  {
    auto skin = package("factory");
    skin["geometry"] = {{"padding", length(19)}, {"itemGap", length(13)}};
    skin["resolvedLayout"] = {
        {"cornerRadiusDip", 0}, {"borderWidthDip", 0}, {"itemCornerRadiusDip", 4},
        {"shadow", "none"}, {"pageArrows", true}, {"fitContent", false},
        {"expandPanel", false}, {"expandRows", 4},
        {"geometry", {{"padding", length(5)}}},
        {"horizontal", {{"padding", length(5)}, {"itemGap", length(0.15, true)},
                        {"itemMinWidth", length(3.5, true)}}},
        {"vertical", {{"padding", length(5)}, {"itemGap", length(13)}}}};
    skin["backdrop"] = "mica";
    skin["borderImage"] = Json::object();
    const Json catalog{{"packages", Json::array({skin})}};
    const auto layout = candidate_skin_layout(catalog, "factory");
    const auto horizontal = candidate_skin_metrics(layout, true, 24);
    const auto vertical = candidate_skin_metrics(layout, false, 24);
    require(horizontal.padding == 5 && horizontal.item_gap == 4 && horizontal.item_min_width == 84);
    require(vertical.padding == 5 && vertical.item_gap == 13 && !vertical.item_min_width);
    require(layout.item_corner_radius == 4 && layout.border_width == 0);
    require(layout.shadow == CandidateSkinShadow::none && layout.page_arrows == true);
    require(layout.backdrop == CandidateSkinBackdrop::mica && layout.panel_image);
    require(candidate_skin_corner_radius(catalog, "factory") == 0);
    // 带着共享层合并好的版式：最小宽度和 macOS 一样用 160 DIP。
    require(layout.shared && horizontal.shared_layout && vertical.shared_layout);
    // 旧 catalog 与损坏的可选扩展仍使用既有声明路径。
    skin["resolvedLayout"] = nullptr;
    const auto legacy = candidate_skin_layout(Json{{"packages", Json::array({skin})}}, "factory");
    require(candidate_skin_metrics(legacy, true, 24).padding == 19);
    require(!legacy.shared && !candidate_skin_metrics(legacy, true, 24).shared_layout);
  }
  // 一个键都没写的包解析出空的版式，候选窗用宿主自己的值。
  {
    const Json catalog{{"packages", Json::array({package("plain")})}};
    const auto layout = candidate_skin_layout(catalog, "plain");
    require(!layout.border_width && !layout.item_corner_radius && !layout.shadow &&
            layout.backdrop == CandidateSkinBackdrop::none && !layout.page_arrows &&
            !layout.fit_content && !layout.expand_panel && !layout.expand_rows &&
            !layout.geometry.padding && !layout.horizontal.padding);
    const auto metrics = candidate_skin_metrics(layout, true, 16.0);
    require(metrics.horizontal && !metrics.padding && !metrics.item_gap &&
            !metrics.number_scale && !metrics.selected_bar_height && !metrics.page_arrows &&
            !metrics.fit_content && !metrics.justify_rows && !metrics.corners_follow_card &&
            metrics.page_arrow_style == CandidatePageArrowStyle::chevron &&
            near(metrics.preedit_margin.top + metrics.preedit_margin.left, 0.0));
    // 不在目录里的包同样是空的。
    require(!candidate_skin_layout(catalog, "absent").border_width &&
            !candidate_skin_layout(Json::object(), "plain").geometry.padding);
  }

  // 完整的一份：共享表、横排表、竖排表和窗口级的键都读进来。
  auto full = package("microsoft-like");
  full["borderWidthDip"] = 1.0;
  full["itemCornerRadiusDip"] = 4.0;
  full["shadow"] = "strong";
  full["backdrop"] = "mica";
  full["pageArrows"] = true;
  full["fitContent"] = false;
  full["geometry"] = Json{{"padding", length(0.3, true)},
                          {"itemGap", length(2.0)},
                          {"itemExtraHeight", length(0.24, true)},
                          {"numberInset", length(0.4, true)},
                          {"itemPaddingRight", length(0.3, true)},
                          {"numberScale", 1.0},
                          {"numberGap", length(0.2, true)},
                          {"selectedBarWidth", length(3.0)},
                          {"selectedBarHeightEm", 0.9},
                          {"selectedBarInside", true},
                          {"itemCornersFollowCard", false},
                          {"justifyRows", false},
                          {"trimHiddenPreeditGap", true},
                          {"pageArrowStyle", "triangle"},
                          {"preeditMarginDip",
                           Json{{"top", 1.0}, {"right", 2.0}, {"bottom", 3.0}, {"left", 4.0}}}};
  full["horizontal"] = Json{{"itemGap", length(0.15, true)},
                            {"itemMinWidth", length(3.5, true)},
                            {"itemCornersFollowCard", true}};
  full["vertical"] = Json{{"padding", length(8.0)}, {"justifyRows", true}};
  const Json catalog{{"packages", Json::array({full})}};
  const auto layout = candidate_skin_layout(catalog, "microsoft-like");
  require(layout.border_width && near(*layout.border_width, 1.0));
  require(layout.item_corner_radius && near(*layout.item_corner_radius, 4.0));
  require(layout.shadow == CandidateSkinShadow::strong);
  require(layout.backdrop == CandidateSkinBackdrop::mica);
  require(layout.page_arrows == true && layout.fit_content == false);
  require(layout.geometry.padding && layout.geometry.padding->em &&
          near(layout.geometry.padding->value, 0.3));
  require(layout.geometry.page_arrow_style == CandidatePageArrowStyle::triangle);
  require(layout.geometry.preedit_margin && near(layout.geometry.preedit_margin->top, 1.0) &&
          near(layout.geometry.preedit_margin->right, 2.0) &&
          near(layout.geometry.preedit_margin->bottom, 3.0) &&
          near(layout.geometry.preedit_margin->left, 4.0));

  // em 按候选字号换算后取整（msime-windows ResolveLength）：16 号字 0.3em = 4.8，取 5。
  require(near(candidate_skin_length({0.3, true}, 16.0), 5.0));
  require(near(candidate_skin_length({0.24, true}, 16.0), 4.0));
  require(near(candidate_skin_length({0.24, true}, 24.0), 6.0));
  require(near(candidate_skin_length({7.5, false}, 24.0), 7.5));

  // 横排表盖过共享表，没写的键沿用共享表；竖排表同理，横排表里的键不会漏进竖排。
  {
    const auto horizontal = candidate_skin_metrics(layout, true, 16.0);
    require(horizontal.horizontal);
    require(horizontal.padding && near(*horizontal.padding, 5.0));
    require(horizontal.item_gap && near(*horizontal.item_gap, 2.0));
    require(horizontal.item_min_width && near(*horizontal.item_min_width, 56.0));
    require(horizontal.corners_follow_card && !horizontal.justify_rows);
    require(horizontal.item_extra_height && near(*horizontal.item_extra_height, 4.0));
    require(horizontal.number_inset && near(*horizontal.number_inset, 6.0));
    require(horizontal.number_gap && near(*horizontal.number_gap, 3.0));
    require(horizontal.number_scale && near(*horizontal.number_scale, 1.0));
    require(horizontal.selected_bar_width && near(*horizontal.selected_bar_width, 3.0));
    // 竖条高是字号的倍数，不取整。
    require(horizontal.selected_bar_height && near(*horizontal.selected_bar_height, 14.4));
    require(horizontal.selected_bar_inside && horizontal.page_arrows && !horizontal.fit_content);
    require(horizontal.page_arrow_style == CandidatePageArrowStyle::triangle);
    require(near(horizontal.preedit_margin.left, 4.0));
    const auto vertical = candidate_skin_metrics(layout, false, 16.0);
    require(!vertical.horizontal && vertical.padding && near(*vertical.padding, 8.0));
    require(vertical.item_gap && near(*vertical.item_gap, 2.0) && !vertical.item_min_width);
    require(vertical.justify_rows && !vertical.corners_follow_card);
    // 字号变了，em 长度跟着变。
    const auto large = candidate_skin_metrics(layout, true, 24.0);
    require(near(*large.padding, 7.0) && near(*large.item_gap, 4.0) &&
            near(*large.item_min_width, 84.0));
  }

  // 越界、类型不对或认不出的值当作没写，宿主保留自己的值。
  {
    auto hostile = package("hostile");
    hostile["borderWidthDip"] = 5.0;
    hostile["itemCornerRadiusDip"] = -1.0;
    hostile["shadow"] = "huge";
    hostile["backdrop"] = "glass";
    hostile["pageArrows"] = "yes";
    hostile["fitContent"] = 1;
    hostile["geometry"] = Json{{"padding", length(65.0)},
                               {"itemGap", length(4.5, true)},
                               {"itemMinWidth", length(500.0)},
                               {"numberScale", 2.5},
                               {"selectedBarWidth", length(17.0)},
                               {"pageArrowStyle", "arrow"},
                               {"preeditMarginDip", Json{{"top", 1.0}, {"right", 2.0}}},
                               {"itemExtraHeight", 3.0}};
    hostile["vertical"] = 7;
    hostile["expandPanel"] = "yes";
    hostile["expandRows"] = 4.5;
    const auto parsed =
        candidate_skin_layout(Json{{"packages", Json::array({hostile})}}, "hostile");
    require(!parsed.border_width && !parsed.item_corner_radius && !parsed.shadow &&
            parsed.backdrop == CandidateSkinBackdrop::none && !parsed.page_arrows &&
            !parsed.fit_content && !parsed.expand_panel && !parsed.expand_rows);
    // expand_rows 只收 2-8 的整数。
    for (const double rows : {1.0, 9.0, -2.0}) {
      auto out_of_range = package("rows");
      out_of_range["expandRows"] = rows;
      require(!candidate_skin_layout(Json{{"packages", Json::array({out_of_range})}}, "rows")
                   .expand_rows);
    }
    require(!parsed.geometry.padding && !parsed.geometry.item_gap &&
            !parsed.geometry.number_scale && !parsed.geometry.selected_bar_width &&
            !parsed.geometry.page_arrow_style && !parsed.geometry.preedit_margin &&
            !parsed.geometry.item_extra_height);
    // 上限本身是合法的。
    require(parsed.geometry.item_min_width && near(parsed.geometry.item_min_width->value, 500.0));
    require(!parsed.vertical.padding);
  }

  // shadow 只缩放宿主阴影的不透明度：soft 一半，strong 1.6 倍且不超过 1，none 不画，没写保持原样。
  require(near(candidate_skin_shadow_alpha(std::nullopt, 0.14f), 0.14));
  require(near(candidate_skin_shadow_alpha(CandidateSkinShadow::none, 0.14f), 0.0));
  require(near(candidate_skin_shadow_alpha(CandidateSkinShadow::soft, 0.14f), 0.07));
  require(near(candidate_skin_shadow_alpha(CandidateSkinShadow::strong, 0.14f), 0.224));
  require(near(candidate_skin_shadow_alpha(CandidateSkinShadow::strong, 0.8f), 1.0));

  // 没有系统材质可用：要亚克力或云母的皮肤，底色补成不透明；不要材质时底色原样。
  require(near(candidate_skin_surface_alpha(CandidateSkinBackdrop::none, 0.6f), 0.6));
  require(near(candidate_skin_surface_alpha(CandidateSkinBackdrop::acrylic, 0.6f), 1.0));
  require(near(candidate_skin_surface_alpha(CandidateSkinBackdrop::mica, 0.15f), 1.0));

  // 卷轴展开：expandPanel 与 expandRows 读进来；只在横排生效，打开时 page_arrows 被忽略，expand_rows 没写时是 4。
  {
    auto scroll = package("wechat-scroll-like");
    scroll["expandPanel"] = true;
    scroll["expandRows"] = 6;
    scroll["pageArrows"] = true;
    const auto scroll_layout =
        candidate_skin_layout(Json{{"packages", Json::array({scroll})}}, "wechat-scroll-like");
    require(scroll_layout.expand_panel == true && scroll_layout.expand_rows == 6 && scroll_layout.page_arrows == true);
    const auto horizontal = candidate_skin_metrics(scroll_layout, true, 16.0);
    require(horizontal.expand_panel && horizontal.expand_rows == 6 && !horizontal.page_arrows);
    const auto vertical = candidate_skin_metrics(scroll_layout, false, 16.0);
    require(!vertical.expand_panel && vertical.page_arrows);
    auto defaults = scroll_layout;
    defaults.expand_rows.reset();
    require(candidate_skin_metrics(defaults, true, 16.0).expand_rows == 4);
  }

  // 资源一起带上版式。
  {
    const auto assets = candidate_skin_assets(catalog, "microsoft-like", {});
    require(assets.layout.page_arrows == true && assets.layout.border_width &&
            assets.layout.geometry.padding);
    require(!candidate_skin_assets(catalog, "../bad", {}).layout.border_width);
  }
  return 0;
}
