#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <optional>
#include <stdexcept>
#include <vector>
#include "CandidateSkinLayout.h"

namespace msime::windows {
// Card geometry ported from the shipped Windows presenter. Widths arrive
// already measured in device independent pixels; this header only composes
// them, so it stays free of DirectWrite and is exercised without a renderer.
// Measured single-line widths of one candidate's three runs, each at its own font size: the text (with its badge) and the annotation at the candidate size, the translation at CandidateCardMetrics::translation_font. All three zero hides the row.
struct CandidateItemWidths {
  double text = 0.0, annotation = 0.0, translation = 0.0;
  // 释义那一段折行之前有几行，`translation` 是其中最宽一行的宽度。两行以上（韩文汉字的 훈음 在上、译文在下，每种目标语言一行，插件注释一行）时总放在候选文字下面，高度按全部行量；唯一的例外是竖排里只有一行释义加它的读音（reading_lines），见 candidate_translation_beside。
  size_t translation_lines = 1;
  // view 候选的 translation_reserved：候选词翻译范围是「仅当前高亮」时高亮的那一行。竖排给这一行一直留出 CandidateCardInput::reserved_secondary_lines 行释义高（candidate_reserved_row_height），回答还没到也留，译文总画在候选文字下面，填进留出来的高度，所以移动高亮、译文到达时卡片都不变高。横排每行本来就统一预留，不看它；一行也不留（reserved_secondary_lines 为 0）时同样不起作用，见 candidate_row_holds_translation。
  bool translation_reserved = false;
  // translation_lines 里读音占的行数：「显示读音」打开时每条读音在它那一行释义下面单独一行（candidate_gloss_display）。只用来认出竖排里「一行释义加它的读音」这种两行，见 candidate_translation_beside。
  size_t reading_lines = 0;
};
// 竖排时释义那一段能不能留在候选文字右边（放得下的话）：只有一行，或者只有一行释义加它下面的读音。后者和 macOS 竖排一样，读音在释义下面、两行一起在候选右侧，候选字号下两行释义通常就在一行候选的高度里，行不变高；如果按 Windows 两行以上一律放到候选下面的规则，竖排每个有读音的候选都要多出两行释义高，一页英文释义的候选高出一倍多，而竖排本来就没有横排挤到相邻格的问题。韩文 훈음 加译文、两种目标语言、插件注释这些两行仍放在候选文字下面。
inline bool candidate_translation_beside(const CandidateItemWidths &item) {
  return item.translation_lines <= 1 || (item.translation_lines == 2 && item.reading_lines == 1);
}
enum class CandidateRun { text, annotation, translation };
// Height of candidate `index`'s run once wrapped to `width` DIPs. The text run is the candidate text with its badge, at the candidate size. The window answers with DirectWrite; without one the layout estimates from the single-line width.
using CandidateWrapMeasure =
    std::function<double(size_t index, CandidateRun run, double width)>;
struct CandidateCardInput {
  double preedit_width = 0.0;
  // One entry per candidate.
  std::vector<CandidateItemWidths> items;
  CandidateWrapMeasure wrapped;
  bool horizontal = false;
  bool preedit_visible = true;
  double font_size = 16.0;
  double preedit_font_size = 16.0;
  // Work area caps. At most one pixel means the axis stays uncapped.
  double max_width = 0.0;
  double max_height = 0.0;
  // How far a horizontal card may grow past max_width so its candidates' own lines stay on one line, their translations wrapping underneath: the work area less a margin on each side. At most one pixel keeps max_width.
  double max_single_line_width = 0.0;
  // Minimum card width asked for by an external skin package, in DIPs. Zero keeps the width derived from the font size (7em, CandidateCardMetrics::min_width). A mascot skin needs it: the artwork is drawn against a card of a particular width, and a narrow card makes the decoration overhang.
  double skin_min_width = 0.0;
  // 候选字体一行的高度（DirectWrite 量出的行高，回退字体比主字体高时取这一页里最高的），见 candidate_card_metrics 的 line_height。0 时按字号估算。
  double line_height = 0.0;
  // 页码（「1 / 3」）按 CandidateCardMetrics::pager_font 量出的宽度，为 0 时不画翻页。翻页和拼音共用首行；只要有 logo 或翻页，拼音隐藏时首行也照样画。
  double page_width = 0.0;
  // 首行右侧提示（光标后编辑的「候选编辑：光标后」，candidate_header_hint）按 CandidateCardMetrics::pager_font 量出的宽度，为 0 时不画也不占位。有它时首行一定在，拼音、logo 和翻页都不画时也照样画。
  double hint_width = 0.0;
  // 共享偏好 `show_app_logo`：首行左端画不画水杉 logo。不画时拼音顶到左边，拼音隐藏又没有翻页时首行不占高度，和 macOS 一致。
  bool logo_visible = true;
  // 横排时每行候选预留的释义行数，见 candidate_reserved_row_height：释义几秒后才到，先留出高度，卡片就不会在用户打字时突然变高。竖排只给标了 translation_reserved 的那一行留（候选词翻译范围是「仅当前高亮」时的高亮行），见 CandidateItemWidths::translation_reserved。
  size_t reserved_secondary_lines = 0;
  // 这几行释义用释义字体量出来的高度：窗口拿同一个 DirectWrite 格式量 reserved_secondary_lines 行占位文字，和释义到达后多行释义的测量（CandidateWrapMeasure）同一条路，预留的和画出来的才一样高。0 时按 translation_line 估算。
  double reserved_secondary_height = 0.0;
  // 皮肤包按当前排列和字号解析好的版式（candidate_skin_metrics），空的键用宿主自己的值。
  CandidateSkinMetrics skin;
  // 这一页最宽的序号按序号字号量出的宽度。缺省版式里候选文字从最宽的序号之后 1.5 DIP 处开始；皮肤写了 number_inset 时不窄于 msime-windows 的下限 9 DIP。0 时按序号字号估算一个数字的宽。
  double number_width = 0.0;
  // canvas 皮肤的面板比这张卡片左右多出的宽度（candidate_canvas_content_offset 的左右之和，DIP，可以是负数）。最小宽度量的是整块面板（msime-windows 把最小宽度设在带着内容内边距的卡片上），所以比最小宽度时先加上它；0 时与以前相同。
  double canvas_extra_width = 0.0;
};
struct CandidateCardSize {
  double width, height;
};
// Screen pixels, including negative monitor origins.
struct CandidateBounds {
  int x, y, width, height;
};
// One source for the card's spacing, so sizing, drawing and hit testing cannot drift apart. Rows are laid out from the top padding downwards. 缺省值照 macOS 候选窗（CandidateSkin.h 的 pad、CandidateChrome.h）：卡片四边到候选行 4 DIP，行与行之间不留空；皮肤写了对应的键时按 msime-windows 的含义改，见 candidate_card_metrics。
struct CandidateCardMetrics {
  double pad_x = 8.0, pad_y = 4.0, slack_x = 0.0, slack_y = 4.0;
  double number_and_bar = 0.0, preedit_row = 0.0, candidate_row = 0.0,
         min_width = 0.0;
  // 缺省行高里文字上下的留白之和（macOS 的 MSIMECandidateRowPadding）。折行的候选文字也在量出的高度上加它，和一行的候选一样上下各留 6 DIP；皮肤写了 item_extra_height 时为 0。
  double row_padding = 12.0;
  // 注释和释义两段：注释包在括号里紧贴候选文字（「你好(wqvb)」，括号就是分隔，所以间隔为 0），和候选同一个字号；释义固定 12 DIP，不随候选字号变大，离文字 0.65 倍字号。挪到第一行下面的一段至少占它自己字体的一行。
  double annotation_gap = 0.0, annotation_line = 0.0, translation_font = 0.0,
         translation_gap = 0.0, translation_line = 0.0;
  // Space every horizontal column keeps after its content. It belongs to the column, so neighbouring columns touch and a click between two candidates still lands on one of them. 缺省为 0：每列的留白已经在 item_padding_right 里。
  double column_gap = 0.0;
  // Vertical space between two stacked rows, and between two lines of a horizontal page. It belongs to no row, so a click in it selects nothing. 缺省为 0，和 macOS 一样行挨着行；皮肤的 item_gap 改它。
  double item_gap = 0.0;
  // 高亮和悬停底色比行四边各缩进多少：行挨着行时，相邻两行的底色之间留出这条缝（macOS 的 NSInsetRect(bounds, 1, 1)）。皮肤写了 item_gap 时行间距由皮肤给，底色铺满整行。
  double selection_inset = 1.0;
  // The pager at the right of the preedit row: the page indicator and the previous and next arrows, in the secondary colour at 13 DIP. 和 macOS 一样，页码离拼音和箭头各 4 DIP，每个箭头是 28 DIP 宽、与首行等高的点击区。
  double pager_font = 13.0, pager_gap = 4.0, pager_arrow = 28.0;
  // 首行打头的全彩水杉 logo，和 macOS 候选窗、悬浮工具栏一样：pad_x 处一个 logo_side 见方的图，再隔 logo_gap 才是拼音。只要有 logo 或翻页，拼音隐藏时首行也至少 header_row 高（macOS 的 26pt）；`show_app_logo` 关掉时 logo_visible 为 false，logo 和它的间隔都不占宽度。
  double logo_side = 16.0, logo_gap = 6.0, header_row = 26.0;
  bool logo_visible = true;
  // 以下是皮肤包能改的版式，缺省值就是没有皮肤时的样子。
  // 序号：字号（缺省是候选字号的 0.8 倍），从行左缘 number_left 处左对齐（缺省 8 DIP，调色板打开选中竖条时 14 DIP，皮肤写了 number_inset 时是它）。number_column 是 msime-windows 版式里序号那一栏的宽度。number_left 也是收紧宽度和竖排翻页箭头对齐的位置。
  double number_font = 0.0, number_column = 0.0, number_left = 0.0;
  std::optional<double> number_inset;
  // 候选内容之后、行（横排是列）右缘之前的留白，缺省 8 DIP（macOS 的 MSIMECandidateTextRight）；横排每一列至少这么宽（0 为不限）。
  double item_padding_right = 8.0, item_min_width = 0.0;
  // 横排时把一行多出的宽度均分给这一行的各列；落在列表四角上的高亮角跟卡片同圆角。
  bool justify_rows = false, corners_follow_card = false;
  // 选中竖条：宽，高（空时是单行行高的 40%），是否落在高亮左缘之内，左缘离行左缘多远（缺省是 macOS 的行内 1 DIP；皮肤版式下是 0 或负半个宽，见 candidate_card_metrics）。
  double selection_bar_width = 3.0;
  std::optional<double> selection_bar_height;
  bool selection_bar_inside = false;
  double selection_bar_offset = 1.0;
  // 拼音行的外边距，只在显示拼音时生效。上下两段算在 preedit_row 里，logo 和首行翻页在去掉它们之后的那一段里居中。
  CandidateSkinInsets preedit_margin{};
  // 皮肤 `page_arrows` 打开时的翻页箭头：不再画在首行，而是横排时候选右侧的一窄列（column 宽）、竖排时最后一行下方的一矮行（row 高），按 msime-windows 的 PagerArrows 摆放。
  struct ListArrows {
    bool triangle = false;
    double button_width = 0.0, button_height = 0.0, gap = 0.0;
    double margin_left = 0.0, margin_right = 0.0, margin_bottom = 0.0;
    // 三角箭头横排时前面那条竖线，宽为 0 时不画也不占宽度。
    double divider_width = 0.0, divider_gap = 0.0;
    // 箭头字形的高；chevron 是线宽，triangle 是三个角各圆掉多少。
    double glyph_size = 0.0, stroke = 0.0;
    double column = 0.0, row = 0.0;
    // 一个字形的宽，竖排时据此让「‹」的左端对齐序号左端。
    double glyph_width() const {
      return triangle ? glyph_size * 0.84 : glyph_size * 0.5 * 0.55;
    }
  };
  std::optional<ListArrows> list_arrows;
  // 皮肤 `expand_panel` 打开时（只在横排）：候选右侧一窄列放展开按钮「∨」，翻页箭头不画，首行也不画翻页。尺寸照 msime-windows candidate_skin_scene.cpp：按钮边长是 1.1 倍字号取整，左右外边距 2 和 1，竖直居中在第一行上；网格每格不窄于 2.6 倍字号取整；滚动条预留 10 DIP，只画 4 DIP 宽的滑块。网格、滚动和点击见 CandidateExpandPanel.h。
  struct ExpandPanel {
    double button = 0.0, glyph = 0.0, stroke = 0.0;
    double margin_left = 2.0, margin_right = 1.0, margin_top = 0.0;
    double column = 0.0, cell_min = 0.0;
    // 展开后一屏的网格行数（`expand_rows`）。
    int rows = 4;
    double scrollbar = 10.0, thumb_width = 4.0, thumb_inset = 2.0,
           thumb_right_inset = 2.0, thumb_min = 20.0, thumb_radius = 2.0;
  };
  std::optional<ExpandPanel> expand_panel;
};
// 首行里 logo、拼音和首行翻页所在的那一段（卡片坐标的上下边）：拼音行外边距之内。
struct CandidateHeaderBand {
  double top = 0.0, bottom = 0.0, height = 0.0;
};
inline CandidateHeaderBand candidate_header_band(const CandidateCardMetrics &metrics) {
  const auto &margin = metrics.preedit_margin;
  return {metrics.pad_y + margin.top,
          metrics.pad_y + metrics.preedit_row - margin.bottom,
          metrics.preedit_row - margin.top - margin.bottom};
}
// logo_visible 和 pager_visible 只决定首行：两者都没有、拼音也隐藏时首行高度为 0。pager_visible 是这一页有没有翻页可画（`show_candidate_page_number` 关掉时没有）；皮肤打开 `page_arrows` 时翻页改画在列表里，首行就不再为它留高度。
// line_height 是候选字体一行的实测高度（CandidateCardInput::line_height），缺省版式的行高是它加上下各 6 DIP，和 macOS 的 MSIMECandidateRowPadding 一样；0 时按 1.35 倍字号估算。
// header_hint 是首行右侧有没有提示要画（CandidateCardInput::hint_width 大于 0）：有时首行和有 logo 时一样不矮于 header_row。
inline CandidateCardMetrics candidate_card_metrics(double font_size,
                                                   double preedit_font_size,
                                                   bool preedit_visible,
                                                   bool logo_visible = true,
                                                   bool pager_visible = true,
                                                   const CandidateSkinMetrics &skin = {},
                                                   double number_width = 0.0,
                                                   double line_height = 0.0,
                                                   bool header_hint = false) {
  if (!std::isfinite(font_size) || font_size < 12.0 || font_size > 32.0 ||
      !std::isfinite(preedit_font_size) || preedit_font_size < 12.0 ||
      preedit_font_size > 32.0)
    throw std::invalid_argument("Invalid candidate card font size");
  CandidateCardMetrics metrics;
  // 卷轴展开打开时 page_arrows 被忽略（candidate_skin_metrics 已经清掉），首行的翻页也不画。
  const bool expand_panel = skin.expand_panel && skin.horizontal;
  const bool list_arrows = skin.page_arrows && pager_visible && !expand_panel;
  // `padding` 是卡片边缘到候选的距离，四边相同：行左右各缩进 pad_x / 2，上边 pad_y，下边 slack_y。没写时是 macOS 的 4 DIP。
  if (skin.padding) {
    metrics.pad_x = *skin.padding * 2.0;
    metrics.pad_y = *skin.padding;
    metrics.slack_y = *skin.padding;
  }
  // 序号缺省是 0.8 倍候选字号，从行左缘 8 DIP 处左对齐，候选文字从最宽的序号之后 1.5 DIP 处开始，和 macOS 的 MSIMECandidateNumberScale、MSIMECandidateTextLeft、MSIMECandidateNumberGap 一样。皮肤的 number_scale 改字号；number_inset 改成从行左缘 number_inset 处量起（msime-windows 不让它小于 5），序号栏不窄于 9 DIP，之后的空缺省是 msime-windows 的 0.2 倍字号加 8；number_gap 改序号到候选的空。
  metrics.number_font = font_size * skin.number_scale.value_or(0.8);
  metrics.number_column = font_size * 0.8;
  const double digits = std::isfinite(number_width) && number_width > 0.0
                            ? number_width
                            : metrics.number_font * 0.6;
  if (skin.number_inset) {
    metrics.number_inset = (std::max)(*skin.number_inset, 5.0);
    metrics.number_left = *metrics.number_inset;
    metrics.number_and_bar = *metrics.number_inset + (std::max)(number_width, 9.0) +
                             skin.number_gap.value_or(font_size * 0.2 + 8.0);
  } else {
    // 调色板打开选中竖条时再让出 6 DIP 给竖条，和 macOS 的 MSIMECandidateTextLeft(showSelectedBar) 一样是 14。
    metrics.number_left = skin.show_selected_bar ? 14.0 : 8.0;
    metrics.number_and_bar =
        metrics.number_left + std::ceil(digits) + skin.number_gap.value_or(1.5);
  }
  // 卡片的最小宽度是 7 倍候选字号（macOS 的 kCandidateMinWidthEm，源皮肤的 `min-width: 7em`），超过半个工作区时由 candidate_card_size 的封顶压回去。带着共享层出厂版式的皮肤和 macOS 一样用 msime-windows 固定的 160 DIP（kCandidateMinWidthDip），与字号无关。
  metrics.min_width = skin.shared_layout ? 160.0 : font_size * 7.0;
  // `item_gap` 是两行之间、横排两列之间的空。横排每列自带的 column_gap 原本是 msime-windows 文字后的 6 DIP 加上这段间距。皮肤给了行间距时高亮铺满整行。
  if (skin.item_gap) {
    metrics.item_gap = *skin.item_gap;
    metrics.column_gap = 6.0 + *skin.item_gap;
    metrics.selection_inset = 0.0;
  }
  // 皮肤写了 item_padding_right 时按 msime-windows 的版式：内容后留它，卡片右侧另有宿主的 14 DIP 余量。
  if (skin.item_padding_right) {
    metrics.item_padding_right = *skin.item_padding_right;
    metrics.slack_x = 14.0;
  }
  if (skin.horizontal) {
    metrics.item_min_width = skin.item_min_width.value_or(0.0);
    metrics.justify_rows = skin.justify_rows;
  }
  metrics.corners_follow_card = skin.corners_follow_card;
  // `fit_content`：不垫 7em 的最小宽度，文字后也不留宿主的余量，行右侧的留白等于序号左侧的留白，卡片两边对称（msime-windows 让 contentPadRight = contentPadLeft + textPadLeft）。皮肤的 min_width_dip 照常生效。
  if (skin.fit_content) {
    metrics.min_width = 0.0;
    metrics.slack_x = 0.0;
    metrics.column_gap = metrics.item_gap;
    metrics.item_padding_right = metrics.number_left;
  }
  // msime-windows 的选中竖条不窄于 3 DIP，高不矮于宽的两倍。
  if (skin.selected_bar_width)
    metrics.selection_bar_width = (std::max)(*skin.selected_bar_width, 3.0);
  metrics.selection_bar_height = skin.selected_bar_height;
  metrics.selection_bar_inside = skin.selected_bar_inside;
  // 竖条的位置：带着共享层版式的皮肤（macOS 的 hasSkinLayout）和写了 selected_bar_inside 的皮肤按 msime-windows，贴在行左缘之内或骑在左缘上；缺省版式和 macOS 一样画在行内 1 DIP 处（max(1, scale)），落在缩进 1 DIP 的高亮底色里。
  if (skin.shared_layout || skin.selected_bar_inside)
    metrics.selection_bar_offset =
        skin.selected_bar_inside ? 0.0 : -metrics.selection_bar_width * 0.5;
  // 带着共享层出厂版式的皮肤是照 msime-windows 的几何画的，皮肤没有键能改的这几项也保留它的值：注释离候选 4 DIP，首行 22 DIP，页码离拼音和箭头 12 DIP，箭头 16 DIP 见方；释义字号在下面。
  if (skin.shared_layout) {
    metrics.annotation_gap = 4.0;
    metrics.header_row = 22.0;
    metrics.pager_gap = 12.0;
    metrics.pager_arrow = 16.0;
  }
  // 首行：显示拼音时是拼音那一行，有 logo 或翻页时不矮于它们需要的高度；三者都没有时不占高度。
  metrics.logo_visible = logo_visible;
  metrics.preedit_row = (std::max)(
      preedit_visible ? preedit_font_size * 1.4 + 6.0 : 0.0,
      logo_visible || header_hint || (pager_visible && !list_arrows && !expand_panel)
          ? metrics.header_row
          : 0.0);
  if (preedit_visible) {
    metrics.preedit_margin = skin.preedit_margin;
    metrics.preedit_row += metrics.preedit_margin.top + metrics.preedit_margin.bottom;
  }
  // 一行候选的最小高度：缺省是实测行高加上下各 6 DIP（macOS 的 MSIMECandidateRowPadding）。皮肤写了 item_extra_height 时是 msime-windows 的 CandidateList itemHeight，1.35 倍字号加上它。
  const double line = std::isfinite(line_height) && line_height > 0.0 ? line_height
                                                                       : font_size * 1.35;
  if (skin.item_extra_height)
    metrics.row_padding = 0.0;
  metrics.candidate_row = skin.item_extra_height ? font_size * 1.35 + *skin.item_extra_height
                                                 : line + metrics.row_padding;
  metrics.annotation_line = font_size * 1.25;
  // 释义固定 12 DIP，和 macOS 的 MSIMECandidateTranslationPointSize 一样：候选字号调大，扫一眼的释义不跟着变大。用户的整体缩放（candidate_scale_percent）照样作用在它上面，和别的 DIP 一样。带着共享层出厂版式的皮肤仍是 msime-windows 的 0.78 倍候选字号。
  metrics.translation_font = skin.shared_layout ? font_size * 0.78 : 12.0;
  metrics.translation_gap = font_size * 0.65;
  metrics.translation_line = metrics.translation_font * 1.25;
  if (list_arrows) {
    // msime-windows candidate_skin_scene.cpp 的翻页箭头外观：跟着候选字号缩放；三角箭头按钮与候选行等高，横排时更宽、前面隔一条竖线。
    CandidateCardMetrics::ListArrows arrows;
    arrows.triangle = skin.page_arrow_style == CandidatePageArrowStyle::triangle;
    if (arrows.triangle) {
      arrows.glyph_size = font_size * 0.52;
      arrows.stroke = arrows.glyph_size * 0.375;
      arrows.button_height = std::round(metrics.candidate_row);
      if (skin.horizontal) {
        arrows.button_width = std::round(font_size * 0.96);
        arrows.divider_width = 1.0;
        arrows.divider_gap = std::round(font_size * 0.12);
        arrows.margin_left = std::round(font_size * 0.3);
      } else {
        arrows.button_width = std::round(font_size * 0.81);
        arrows.button_height = std::round(font_size * 1.2);
      }
    } else {
      arrows.button_width = std::round(font_size * 0.72);
      arrows.button_height = std::round(font_size * 0.9);
      arrows.gap = 1.0;
      arrows.glyph_size = font_size * 0.5;
      arrows.stroke = (std::max)(1.2, font_size / 13.0);
      // 横排的一窄列和候选之间留一点空，箭头竖直居中在最后一行上。
      if (skin.horizontal) {
        arrows.margin_left = 2.0;
        arrows.margin_right = 1.0;
        arrows.margin_bottom =
            (std::max)(0.0, (metrics.candidate_row - arrows.button_height) * 0.5);
      }
    }
    if (skin.horizontal) {
      arrows.column = arrows.margin_left +
                      (arrows.divider_width > 0.0
                           ? arrows.divider_width + arrows.divider_gap
                           : 0.0) +
                      arrows.button_width * 2.0 + arrows.gap + arrows.margin_right;
      // 箭头列接在候选后面，候选列自带的 column_gap 已经是它们之间的空。
      metrics.slack_x = 0.0;
    } else {
      // 竖排那一矮行：msime-windows 的 body 行距 2 DIP，加上箭头上 -1、下 1 的外边距。
      arrows.row = arrows.button_height + 2.0;
    }
    metrics.list_arrows = arrows;
  }
  if (expand_panel) {
    CandidateCardMetrics::ExpandPanel panel;
    panel.button = std::round(font_size * 1.1);
    panel.glyph = std::round(font_size * 0.6);
    panel.stroke = (std::max)(1.2, font_size / 13.0);
    panel.margin_top = (std::max)(0.0, (metrics.candidate_row - panel.button) * 0.5);
    panel.column = panel.margin_left + panel.button + panel.margin_right;
    panel.cell_min = std::round(font_size * 2.6);
    panel.rows = std::clamp(skin.expand_rows, 2, 8);
    // 按钮列接在候选后面，和翻页箭头那一列一样，候选列自带的 column_gap 已经是它们之间的空。
    metrics.slack_x = 0.0;
    metrics.expand_panel = panel;
  }
  return metrics;
}
// 列表右侧那一窄列占去的宽度：横排的列表翻页箭头，或卷轴展开的按钮；没有时为 0。
inline double candidate_list_arrow_column(const CandidateCardMetrics &metrics) {
  if (metrics.list_arrows)
    return metrics.list_arrows->column;
  return metrics.expand_panel ? metrics.expand_panel->column : 0.0;
}
// Row rectangle in card coordinates for rows of one line each; the caller supplies the card width it actually got. candidate_page_layout starts a vertical page from these and grows the rows that carry wrapped runs; the first row's top and minimum height are the same either way. The horizontal branch is an even split of the width, which candidate_page_layout no longer uses: a horizontal page gets columns of each candidate's natural width instead.
struct CandidateRowBounds {
  double left, top, right, bottom;
};
inline CandidateRowBounds candidate_row_bounds(size_t index, size_t count,
                                               double width,
                                               const CandidateCardMetrics &metrics,
                                               bool horizontal) {
  if (index >= count || count > 9 || !std::isfinite(width) || width <= 0.0)
    throw std::invalid_argument("Invalid candidate row");
  const double top = metrics.pad_y + metrics.preedit_row;
  const double stride = metrics.candidate_row + metrics.item_gap;
  if (!horizontal)
    return {metrics.pad_x / 2.0, top + stride * static_cast<double>(index),
            width - metrics.pad_x / 2.0,
            top + stride * static_cast<double>(index) + metrics.candidate_row};
  const double column =
      (width - metrics.pad_x) / static_cast<double>(count);
  return {metrics.pad_x / 2.0 + column * static_cast<double>(index), top,
          metrics.pad_x / 2.0 + column * static_cast<double>(index + 1),
          top + metrics.candidate_row};
}
// Width of the pager for an indicator of `indicator_width`: the indicator, the gap and the two arrows. Zero without an indicator.
inline double candidate_pager_width(double indicator_width,
                                    const CandidateCardMetrics &metrics) {
  if (!(indicator_width > 0.0) || !std::isfinite(indicator_width))
    return 0.0;
  return indicator_width + metrics.pager_gap + metrics.pager_arrow * 2.0;
}
// Where the pager sits in the preedit row of a card `width` wide, in card coordinates: right aligned to the rows' right edge, the arrows last. `left` is the pager's left edge, which the preedit is clipped before, less pager_gap.
// in_list 为 true 时是皮肤 `page_arrows` 画在列表里的两枚箭头（candidate_list_arrows_layout），没有页码，indicator 宽为 0，首行也不因它让出位置。
struct CandidatePagerLayout {
  double left = 0.0;
  CandidateRowBounds indicator{}, previous{}, next{};
  bool in_list = false;
};
inline std::optional<CandidatePagerLayout>
candidate_pager_layout(double width, double indicator_width,
                       const CandidateCardMetrics &metrics) {
  const double pager = candidate_pager_width(indicator_width, metrics);
  if (!(pager > 0.0) || metrics.preedit_row <= 0.0 || !std::isfinite(width) ||
      width <= 0.0)
    return std::nullopt;
  const auto band = candidate_header_band(metrics);
  const double top = band.top, bottom = band.bottom;
  const double right = width - metrics.pad_x / 2.0;
  CandidatePagerLayout layout;
  layout.next = {right - metrics.pager_arrow, top, right, bottom};
  layout.previous = {layout.next.left - metrics.pager_arrow, top,
                     layout.next.left, bottom};
  layout.indicator = {layout.previous.left - metrics.pager_gap - indicator_width,
                      top, layout.previous.left - metrics.pager_gap, bottom};
  layout.left = layout.indicator.left;
  return layout;
}
// 首行右侧提示（candidate_header_hint）在卡片里的位置：和翻页同一段高度，右对齐到首行翻页左边再让开 pager_gap；首行没有翻页（关掉了页码，或皮肤把箭头画进列表）时右对齐到行的右缘，和翻页的右缘一样。没有提示或首行不占高度时为空。
inline std::optional<CandidateRowBounds>
candidate_header_hint_bounds(double width, double hint_width,
                             const std::optional<CandidatePagerLayout> &pager,
                             const CandidateCardMetrics &metrics) {
  if (!(hint_width > 0.0) || !std::isfinite(hint_width) || metrics.preedit_row <= 0.0 ||
      !std::isfinite(width) || width <= 0.0)
    return std::nullopt;
  const auto band = candidate_header_band(metrics);
  const double right =
      pager && !pager->in_list ? pager->left - metrics.pager_gap : width - metrics.pad_x / 2.0;
  return CandidateRowBounds{right - hint_width, band.top, right, band.bottom};
}
// 拼音在首行里画到哪里为止（卡片坐标）：首行右侧提示左边再让开 pager_gap，没有提示时是首行翻页左边再让开 pager_gap，两者都没有时到右边距再减去皮肤给拼音行的右外边距。候选窗画拼音和读屏报拼音的位置都按它算。
inline double candidate_preedit_right(double width,
                                      const std::optional<CandidatePagerLayout> &pager,
                                      const std::optional<CandidateRowBounds> &hint,
                                      const CandidateCardMetrics &metrics) {
  if (hint)
    return hint->left - metrics.pager_gap;
  if (pager && !pager->in_list)
    return pager->left - metrics.pager_gap;
  return width - metrics.pad_x / 2.0 - metrics.preedit_margin.right;
}
// The brand mark's square in the preedit row, in card coordinates: at the row's left padding and centred in the row.
inline CandidateRowBounds candidate_logo_bounds(const CandidateCardMetrics &metrics) {
  const auto band = candidate_header_band(metrics);
  const double top = band.top + (band.height - metrics.logo_side) / 2.0;
  return {metrics.pad_x, top, metrics.pad_x + metrics.logo_side,
          top + metrics.logo_side};
}
// 拼音在首行里从哪里开始（卡片坐标）：logo 和它的间隔之后，再加上皮肤给拼音行的左外边距；不画 logo 时就在左边距处。
inline double candidate_preedit_left(const CandidateCardMetrics &metrics) {
  return metrics.pad_x +
         (metrics.logo_visible ? metrics.logo_side + metrics.logo_gap : 0.0) +
         metrics.preedit_margin.left;
}
// 拼音里的插入点，和 macOS 候选窗的 MSIMECandidatePreeditField 一样：一条 1.25 DIP 宽的竖线，光标在拼音中间时文字在它两边各让开 0.85 DIP（光标后面的字整体右移 caret_slot），不压字；在末尾时离最后一个字 1.5 DIP。拼音超出可用宽度时整行左移，刚好让插入点留在可见的一段里。
inline constexpr double candidate_preedit_caret_width = 1.25;
inline constexpr double candidate_preedit_caret_side_air = 0.85;
inline constexpr double candidate_preedit_caret_end_air = 1.5;
// 光标在拼音中间时它占去的宽度，量拼音宽度时要加上。
inline constexpr double candidate_preedit_caret_slot =
    candidate_preedit_caret_width + 2.0 * candidate_preedit_caret_side_air;
struct CandidatePreeditCaret {
  // 相对拼音区左缘：整行左移多少，光标后面那段文字从哪里开始，插入点的左缘。后两者已经减去了 scroll。
  double scroll = 0.0, after = 0.0, caret = 0.0;
};
// before 是光标前那段拼音的宽，after_text 为 true 表示光标后面还有字；available 是拼音区的宽。
inline CandidatePreeditCaret candidate_preedit_caret(double before, bool after_text,
                                                     double available) {
  before = std::isfinite(before) ? (std::max)(before, 0.0) : 0.0;
  const double offset = before + (after_text ? candidate_preedit_caret_side_air
                                             : candidate_preedit_caret_end_air);
  const double room = std::isfinite(available)
                          ? (std::max)(available - candidate_preedit_caret_width, 0.0)
                          : offset;
  CandidatePreeditCaret caret;
  caret.scroll = (std::max)(offset - room, 0.0);
  caret.caret = offset - caret.scroll;
  caret.after = before + (after_text ? candidate_preedit_caret_slot : 0.0) - caret.scroll;
  return caret;
}
// Which arrow of a drawn pager a card point falls on: true for the previous page, false for the next. The previous arrow is inert on the first page. The next one is always live, because the Engine hands candidates over lazily and the page count grows as the user pages.
inline std::optional<bool>
candidate_pager_hit(double x, double y,
                    const std::optional<CandidatePagerLayout> &pager,
                    bool first_page) {
  if (!pager || !std::isfinite(x) || !std::isfinite(y))
    return std::nullopt;
  auto inside = [x, y](const CandidateRowBounds &box) {
    return x >= box.left && x < box.right && y >= box.top && y < box.bottom;
  };
  if (inside(pager->previous) && !first_page)
    return true;
  if (inside(pager->next))
    return false;
  return std::nullopt;
}
// The Fluent selection pill on the selected row: 3 DIP wide, starting `offset` DIPs right of the row's left edge (centred on that edge when no offset is given), 40% of a one-line row tall (never less than twice its width) and centred in the row, so a one-line row has it from 30% to 70% of its height. A row that grows because its text or annotation wrapped keeps the same pill in its middle instead of a stretched one; a row shorter than the pill starts it at the row top. The corner radius is half the width.
struct CandidateSelectionBar {
  double left, right, top, bottom;
};
inline constexpr double candidate_selection_bar_width = 3.0;
// 皮肤可以改竖条的宽（`selected_bar_width`）、高（`selected_bar_height_em`，给出时代替 40% 行高）和位置（`selected_bar_inside`：贴在高亮左缘之内，而不是骑在左缘上）；窗口传 CandidateCardMetrics::selection_bar_offset，缺省版式是 macOS 的行内 1 DIP，见 candidate_card_metrics。
inline CandidateSelectionBar
candidate_selection_bar(double row_left, double row_top, double row_bottom,
                        double line_height,
                        double width = candidate_selection_bar_width,
                        std::optional<double> bar_height = std::nullopt,
                        std::optional<double> offset = std::nullopt) {
  const double height = (std::max)(bar_height.value_or(line_height * 0.4), width * 2.0);
  const double top =
      row_top + (std::max)((row_bottom - row_top - height) * 0.5, 0.0);
  const double left = row_left + offset.value_or(-width * 0.5);
  return {left, left + width, top, top + height};
}
// A run's box relative to the row's text column: x from where the candidate text starts, y from the row top. Zero width means the run is absent. An inline run shares the first line and is centred in it; one below the first line is top aligned and may wrap.
struct CandidateRunBox {
  double x = 0.0, y = 0.0, width = 0.0, height = 0.0;
  bool below = false;
};
struct CandidateItemLayout {
  double text_width = 0.0;
  // Height of the text's box from the row top: one candidate_row, or the measured wrapped height when the text is wider than the column. The text is centred in it, and runs placed below the first line start under it.
  double text_height = 0.0;
  // Whether the text is wider than the column and so is drawn wrapped.
  bool text_wrapped = false;
  CandidateRunBox annotation, translation;
  // At least one candidate_row; grows by every run placed below the first line.
  double height = 0.0;
};
// Port of the shipped presenter's per-item geometry (CandidateList::MeasureItem). Text wider than the column wraps inside it and the row takes the measured height, at least one candidate_row. Short runs stay on the text's line; a run that does not fit moves under the text and wraps to the column. A horizontal list always puts the translation under the text, and a translation never goes back up once the annotation has moved down.
inline CandidateItemLayout
candidate_item_layout(const CandidateItemWidths &item, double content_width,
                      const CandidateCardMetrics &metrics, bool horizontal,
                      const std::function<double(CandidateRun, double)> &wrapped) {
  content_width = (std::max)(content_width, 1.0);
  // 卡片宽度是内容宽加上序号栏和留白，排版时再减回来，浮点误差会让正好放得下的一段比列窄上一点点而被折行；差在这么多以内的算放得下。
  constexpr double fits = 1e-6;
  CandidateItemLayout layout;
  layout.text_width = (std::min)(item.text, content_width);
  // Height of a run of `natural` width and `lines` lines once it sits in `width`: one `line` each unless it has to wrap. A run of several lines is always measured, because its height is more than one line even when none of them wraps.
  auto run_height = [&](CandidateRun run, double natural, double width,
                        double line, size_t lines = 1) {
    const double unwrapped = line * static_cast<double>((std::max)(lines, size_t{1}));
    double height = unwrapped;
    if (natural > width + fits || lines > 1)
      height = wrapped ? wrapped(run, width)
                       : std::ceil(natural / width) * unwrapped;
    // A failed or nonsense measurement still reserves the lines.
    return std::isfinite(height) ? (std::max)(height, unwrapped) : unwrapped;
  };
  // Without a measure a wrapped text line is estimated at the candidate size's line height, like an annotation line.
  layout.text_wrapped = item.text > content_width + fits;
  layout.text_height =
      layout.text_wrapped
          ? (std::max)(run_height(CandidateRun::text, item.text,
                                  content_width, metrics.annotation_line) +
                           metrics.row_padding,
                       metrics.candidate_row)
          : metrics.candidate_row;
  layout.height = layout.text_height;
  double line_end = layout.text_width;
  auto below = [&](CandidateRun run, double natural, double width, double line,
                   size_t lines = 1) {
    const double height = run_height(run, natural, width, line, lines);
    CandidateRunBox box{0.0, layout.height, width, height, true};
    layout.height += height;
    return box;
  };
  if (item.annotation > 0.0) {
    const double width = (std::min)(item.annotation, content_width);
    if (line_end + metrics.annotation_gap + width <= content_width + fits) {
      layout.annotation = {line_end + metrics.annotation_gap, 0.0, width,
                           metrics.candidate_row, false};
      line_end = layout.annotation.x + width;
    } else {
      layout.annotation = below(CandidateRun::annotation, item.annotation,
                                width, metrics.annotation_line);
    }
  }
  if (item.translation > 0.0) {
    const double width = (std::min)(item.translation, content_width);
    if (!horizontal && !item.translation_reserved && !layout.annotation.below &&
        candidate_translation_beside(item) &&
        line_end + metrics.translation_gap + width <= content_width + fits) {
      // 一行释义加读音的两行在右边按量出的高度画，竖直居中；比一行候选高时这一行跟着长高。
      const double height =
          item.translation_lines > 1
              ? (std::max)(layout.height,
                           run_height(CandidateRun::translation, item.translation, width,
                                      metrics.translation_line, item.translation_lines))
              : metrics.candidate_row;
      layout.translation = {line_end + metrics.translation_gap, 0.0, width, height, false};
      layout.height = (std::max)(layout.height, height);
    } else
      layout.translation = below(CandidateRun::translation, item.translation,
                                 width, metrics.translation_line,
                                 item.translation_lines);
  }
  return layout;
}
// 一行候选什么都不折行时要的宽度：序号和竖条加上内容。竖排把一行释义（或一行释义加它的读音，见 candidate_translation_beside）留在候选文字同一行；横排，以及其他两行以上的释义，放在候选文字下面，宽度取两者里较宽的一个。隐藏的候选三段都量出 0，宽度为 0。
// 竖排里留着释义行的那一行（translation_reserved，调用方已按 candidate_row_holds_translation 判过）同样把译文放在下面。
inline double candidate_item_natural_width(const CandidateItemWidths &item,
                                           const CandidateCardMetrics &metrics,
                                           bool horizontal) {
  if (item.text <= 0.0 && item.annotation <= 0.0 && item.translation <= 0.0)
    return 0.0;
  const double line =
      item.text +
      (item.annotation > 0.0 ? metrics.annotation_gap + item.annotation : 0.0);
  const double content =
      horizontal || !candidate_translation_beside(item) || item.translation_reserved
          ? (std::max)(line, item.translation)
          : line + (item.translation > 0.0
                        ? metrics.translation_gap + item.translation
                        : 0.0);
  // 皮肤的 item_padding_right 留在内容之后；横排每列不窄于 item_min_width（msime-windows 的 minItemWidth，列间距不算在内）。
  const double width = content + metrics.number_and_bar + metrics.item_padding_right;
  return horizontal ? (std::max)(width, metrics.item_min_width) : width;
}
// The narrowest a horizontal page's columns can be on one line, column gaps included: each candidate's line (text and annotation) with its number and bar, the translations left to wrap under them. The macOS port is SingleLineMinimumWidth.
inline double
candidate_single_line_minimum_width(const std::vector<CandidateItemWidths> &items,
                                    const CandidateCardMetrics &metrics) {
  double total = 0.0;
  for (const auto &item : items) {
    auto line = item;
    line.translation = 0.0;
    const double wide = candidate_item_natural_width(item, metrics, true);
    if (wide > 0.0)
      total += (std::min)((std::max)(candidate_item_natural_width(
                                         line, metrics, true),
                                     metrics.number_and_bar),
                          wide) +
               metrics.column_gap;
  }
  return total;
}
// The columns of a horizontal page on a single line `line_width` wide, each including the column gap. Columns that fit keep their natural widths. Otherwise each column whose translation is wider than its candidate line gives up room, never below that line and in proportion to how much it could give, so the translations wrap under their text instead of the page starting a second line. None when the candidate lines alone do not fit. The macOS and Linux port is SingleLineColumns in shared/candidate/CandidateItemLayout.h.
inline std::optional<std::vector<double>>
candidate_single_line_columns(const std::vector<CandidateItemWidths> &items,
                              double line_width,
                              const CandidateCardMetrics &metrics) {
  std::vector<double> natural, firm;
  natural.reserve(items.size());
  firm.reserve(items.size());
  double natural_total = 0.0, firm_total = 0.0;
  for (const auto &item : items) {
    auto line = item;
    line.translation = 0.0;
    const double wide = candidate_item_natural_width(item, metrics, true);
    // A translation-only candidate still needs a column; its line is the number and bar.
    const double narrow =
        wide > 0.0
            ? (std::max)(candidate_item_natural_width(line, metrics, true),
                         metrics.number_and_bar)
            : 0.0;
    natural.push_back(wide > 0.0 ? wide + metrics.column_gap : 0.0);
    firm.push_back(wide > 0.0 ? (std::min)(narrow, wide) + metrics.column_gap
                              : 0.0);
    natural_total += natural.back();
    firm_total += firm.back();
  }
  if (natural_total <= line_width)
    return natural;
  if (firm_total > line_width || !(natural_total > firm_total))
    return std::nullopt;
  const double keep = (line_width - firm_total) / (natural_total - firm_total);
  for (size_t index = 0; index < natural.size(); ++index)
    natural[index] = firm[index] + (natural[index] - firm[index]) * keep;
  return natural;
}
// 这一行在竖排里是否一直留出释义行（CandidateItemWidths::translation_reserved）：只在竖排、view 标了这一行、而且确实有要留的高度（reserved_row 是 candidate_reserved_row_height，预留 0 行时为 0）时成立。不成立时这一行和别的行一样排。
inline bool candidate_row_holds_translation(const CandidateItemWidths &item, bool horizontal,
                                            double reserved_row) {
  return !horizontal && item.translation_reserved && std::isfinite(reserved_row) &&
         reserved_row > 0.0;
}
// 横排候选在释义到达前就留出的行高：一行候选文字加 `lines` 行释义，和 macOS 的 reservedGlossHeightForFont 一样。lines 为 0 时不预留。释义到达后，一行的释义按 translation_line 算高，多行的总是用 DirectWrite 量（candidate_item_layout 的 run_height，不矮于 translation_line 乘行数）；所以多行时用 `measured`（窗口用同一种测量量出的 lines 行占位文字的高度），规则与 run_height 相同，预留的和到达后的一样高。measured 为 0 或不是有限值时按 translation_line 估算。
inline double candidate_reserved_row_height(const CandidateCardMetrics &metrics,
                                            size_t lines, double measured = 0.0) {
  if (lines == 0)
    return 0.0;
  const double estimated = metrics.translation_line * static_cast<double>(lines);
  const double gloss = lines > 1 && std::isfinite(measured) ? (std::max)(measured, estimated) : estimated;
  return metrics.candidate_row + gloss;
}
// One laid out row: its rectangle in card coordinates and the runs inside it.
struct CandidateRowLayout {
  CandidateRowBounds bounds;
  CandidateItemLayout item;
};
// Rows for a whole page at the card width actually drawn. A vertical list stacks rows of their own heights. A horizontal list keeps every candidate on one line: candidate_single_line_columns narrows the translations to fit, and only when the candidate lines themselves do not fit does it fall back to the shipped presenter's CandidateList::Measure, where each column is its candidate's natural width and one that would pass the card's inner edge starts a new line (a candidate wider than a whole line is narrowed to it, and its text and runs wrap inside that). Every column on a line takes the line's tallest height, so the selection fills evenly. Sizing, painting and hit testing all read this one result.
// minimum_row_height 是 candidate_reserved_row_height：横排每一行候选至少这么高；竖排只作用于留着释义行的那一行（candidate_row_holds_translation）。
inline std::vector<CandidateRowLayout>
candidate_page_layout(const std::vector<CandidateItemWidths> &items,
                      double width, const CandidateCardMetrics &metrics,
                      bool horizontal, const CandidateWrapMeasure &wrapped = {},
                      double minimum_row_height = 0.0) {
  if (items.size() > 9 || !std::isfinite(width) || width <= 0.0)
    throw std::invalid_argument("Invalid candidate page");
  std::vector<CandidateRowLayout> rows;
  rows.reserve(items.size());
  double top = metrics.pad_y + metrics.preedit_row, tallest = 0.0;
  // Horizontal cursor: x within the line, and where the current line's columns begin. 皮肤的翻页箭头在横排时占去右侧一窄列，候选只排在它左边。
  const double line_width =
      (std::max)(width - metrics.pad_x - candidate_list_arrow_column(metrics), 1.0);
  double x = 0.0;
  size_t line_start = 0;
  const auto single_line =
      horizontal ? candidate_single_line_columns(items, line_width, metrics)
                 : std::nullopt;
  auto close_line = [&](size_t end) {
    for (size_t index = line_start; index < end; ++index)
      rows[index].bounds.bottom = rows[index].bounds.top + tallest;
    // 皮肤的 justify_rows：这一行比列表窄时，多出的宽度均分给这一行看得见的各列，末列贴到列表右缘（msime-windows CandidateList::ItemRect）。列里的文字跟着列一起右移，排版宽度不变。
    if (metrics.justify_rows && horizontal) {
      size_t columns = 0;
      for (size_t index = line_start; index < end; ++index)
        if (rows[index].bounds.right > rows[index].bounds.left)
          ++columns;
      const double extra = line_width - x;
      if (columns > 0 && extra > 0.0) {
        const double share = extra / static_cast<double>(columns);
        double shift = 0.0;
        for (size_t index = line_start; index < end; ++index) {
          auto &bounds = rows[index].bounds;
          bounds.left += shift;
          bounds.right += shift;
          if (bounds.right > bounds.left) {
            bounds.right += share;
            shift += share;
          }
        }
      }
    }
    line_start = end;
  };
  for (size_t index = 0; index < items.size(); ++index) {
    auto bounds =
        candidate_row_bounds(index, items.size(), width, metrics, horizontal);
    if (horizontal) {
      const double natural =
          candidate_item_natural_width(items[index], metrics, true);
      const double column =
          single_line ? (*single_line)[index]
          : natural > 0.0
              ? (std::min)(natural + metrics.column_gap, line_width)
              : 0.0;
      if (!single_line && x > 0.0 && x + column > line_width) {
        close_line(index);
        top += tallest + metrics.item_gap;
        tallest = 0.0;
        x = 0.0;
      }
      bounds = {metrics.pad_x / 2.0 + x, top, metrics.pad_x / 2.0 + x + column,
                top + metrics.candidate_row};
      x += column;
    }
    std::function<double(CandidateRun, double)> measure;
    if (wrapped)
      measure = [&wrapped, index](CandidateRun run, double run_width) {
        return wrapped(index, run, run_width);
      };
    auto widths = items[index];
    widths.translation_reserved =
        candidate_row_holds_translation(widths, horizontal, minimum_row_height);
    auto item = candidate_item_layout(
        widths,
        bounds.right - bounds.left - metrics.number_and_bar - metrics.item_padding_right,
        metrics, horizontal, measure);
    if (!horizontal) {
      // 「仅当前高亮」的高亮行：译文还没到也留出释义行，到了正好填进去。
      if (widths.translation_reserved)
        item.height = (std::max)(item.height, minimum_row_height);
      bounds.top = top;
      bounds.bottom = top + item.height;
      top = bounds.bottom + metrics.item_gap;
    }
    tallest = (std::max)(tallest, item.height);
    if (horizontal && std::isfinite(minimum_row_height))
      tallest = (std::max)(tallest, minimum_row_height);
    rows.push_back({bounds, item});
  }
  if (horizontal)
    close_line(rows.size());
  return rows;
}
// Hit testing runs on the same rows the renderer drew.
inline std::optional<size_t>
candidate_card_hit(double x, double y, double width, double height,
                   const std::vector<CandidateRowLayout> &rows) {
  if (rows.empty() || rows.size() > 9 || !std::isfinite(x) ||
      !std::isfinite(y) || !std::isfinite(width) || !std::isfinite(height) ||
      width <= 0.0 || height <= 0.0 || x < 0.0 || y < 0.0 || x >= width ||
      y >= height)
    return std::nullopt;
  for (size_t index = 0; index < rows.size(); ++index) {
    const auto &row = rows[index].bounds;
    if (x >= row.left && x < row.right && y >= row.top && y < row.bottom)
      return index;
  }
  return std::nullopt;
}
inline CandidateCardSize candidate_card_size(const CandidateCardInput &input) {
  auto measured = [](double value) {
    return std::isfinite(value) && value >= 0.0;
  };
  if (input.items.size() > 9 || !measured(input.preedit_width) ||
      !measured(input.page_width) || !measured(input.hint_width) || !measured(input.max_width) ||
      !measured(input.max_height))
    throw std::invalid_argument("Invalid candidate card measurement");
  for (const auto &item : input.items)
    if (!measured(item.text) || !measured(item.annotation) ||
        !measured(item.translation))
      throw std::invalid_argument("Invalid candidate card measurement");
  const auto shape = candidate_card_metrics(
      input.font_size, input.preedit_font_size, input.preedit_visible,
      input.logo_visible, input.page_width > 0.0, input.skin,
      std::isfinite(input.number_width) ? input.number_width : 0.0, input.line_height,
      input.hint_width > 0.0);
  // 皮肤把翻页画在列表里时，横排在候选右侧加一窄列，竖排在最后一行下方加一矮行；卷轴展开的按钮也是候选右侧的一窄列。
  const double arrow_column = candidate_list_arrow_column(shape);
  const double arrow_row = shape.list_arrows ? shape.list_arrows->row : 0.0;
  const double pad_x = shape.pad_x, pad_y = shape.pad_y,
               slack_x = shape.slack_x, slack_y = shape.slack_y;
  // The skin's floor only ever raises the built-in one, never lowers it: a
  // package must not be able to shrink the card below what the text needs.
  const double min_width =
      (std::max)(shape.min_width, std::isfinite(input.skin_min_width) &&
                                          input.skin_min_width > 0.0
                                      ? input.skin_min_width
                                      : 0.0);
  const double preedit_row = shape.preedit_row;
  const double candidate_row = shape.candidate_row;

  double width = 0.0;
  double height = pad_y + slack_y;
  // 首行：显示时的 logo、显示时的拼音、首行右侧的提示，以及翻页。都没有时首行高度为 0。
  {
    const double pager = shape.list_arrows || shape.expand_panel
                             ? 0.0
                             : candidate_pager_width(input.page_width, shape);
    width = (std::max)(width, (shape.logo_visible ? shape.logo_side + shape.logo_gap : 0.0) +
                                  (input.preedit_visible
                                       ? input.preedit_width + 6.0 +
                                             shape.preedit_margin.left +
                                             shape.preedit_margin.right
                                       : 0.0) +
                                  (pager > 0.0 ? shape.pager_gap + pager : 0.0) +
                                  (input.hint_width > 0.0 ? shape.pager_gap + input.hint_width
                                                          : 0.0));
    height += preedit_row;
  }
  auto visible = [](const CandidateItemWidths &item) {
    return item.text > 0.0 || item.annotation > 0.0 || item.translation > 0.0;
  };
  // 释义行的预留高度：横排每一行都留，竖排只给留着释义行的那一行（candidate_row_holds_translation）。
  const double reserved = candidate_reserved_row_height(shape, input.reserved_secondary_lines,
                                                        input.reserved_secondary_height);
  // A horizontal card asks for every column side by side, the same columns candidate_page_layout places.
  double row_width_sum = 0.0, row_width_max = 0.0;
  for (auto item : input.items) {
    if (!visible(item))
      continue;
    item.translation_reserved =
        candidate_row_holds_translation(item, input.horizontal, reserved);
    const double row =
        candidate_item_natural_width(item, shape, input.horizontal);
    row_width_sum += row + shape.column_gap;
    row_width_max = (std::max)(row_width_max, row);
  }
  width = (std::max)(input.horizontal ? row_width_sum : row_width_max, width);
  const double canvas_extra =
      std::isfinite(input.canvas_extra_width) ? input.canvas_extra_width : 0.0;
  width = (std::max)(width + pad_x + slack_x + arrow_column + canvas_extra, min_width) -
          canvas_extra;
  auto clamp = [](double value, double cap) {
    value = (std::max)(value, 1.0);
    return cap > 1.0 ? (std::min)(value, cap) : value;
  };
  // Half the work area caps the card, except that a horizontal page grows past it as far as its candidates' own lines need so the page stays on one line.
  double width_cap = input.max_width;
  if (input.horizontal && input.max_width > 1.0 &&
      input.max_single_line_width > 1.0)
    width_cap = (std::max)(
        width_cap,
        (std::min)(input.max_single_line_width,
                   candidate_single_line_minimum_width(input.items, shape) +
                       pad_x + slack_x + arrow_column));
  width = clamp(width, width_cap);
  // Heights come from the width the card will actually get: a capped card wraps the runs that no longer fit, and grows by exactly what the painter will draw.
  const auto rows = candidate_page_layout(input.items, width, shape,
                                          input.horizontal, input.wrapped, reserved);
  // A horizontal page ends at its last line's bottom, however many lines the capped width broke it into.
  // A vertical page is its visible rows plus one item_gap between each two of them.
  double rows_height = 0.0;
  bool stacked = false;
  for (size_t index = 0; index < rows.size(); ++index) {
    if (!visible(input.items[index]))
      continue;
    const auto &row = rows[index];
    rows_height =
        input.horizontal
            ? (std::max)(rows_height, row.bounds.bottom - pad_y - preedit_row)
            : rows_height + (stacked ? shape.item_gap : 0.0) + row.item.height;
    stacked = true;
  }
  // 空列表也留一行，卡片不会塌掉；横排时留的是预留了释义的行高。
  height += (std::max)(rows_height,
                       (std::max)(candidate_row, input.horizontal ? reserved : 0.0)) +
            arrow_row;
  return {width, clamp(height, input.max_height)};
}
// 列表占的矩形（卡片坐标）：左右是行的左右缘（横排时右缘让开翻页箭头那一列），上下是首行的顶和最后一行的底。没有看得见的行时为空。
inline std::optional<CandidateRowBounds>
candidate_rows_area(double width, const std::vector<CandidateRowLayout> &rows,
                    const CandidateCardMetrics &metrics) {
  std::optional<CandidateRowBounds> area;
  for (const auto &row : rows) {
    if (!(row.bounds.right > row.bounds.left))
      continue;
    if (!area)
      area = CandidateRowBounds{metrics.pad_x / 2.0, row.bounds.top,
                                width - metrics.pad_x / 2.0 -
                                    candidate_list_arrow_column(metrics),
                                row.bounds.bottom};
    area->top = (std::min)(area->top, row.bounds.top);
    area->bottom = (std::max)(area->bottom, row.bounds.bottom);
  }
  return area;
}
// 皮肤 `page_arrows` 画在列表里的两枚翻页箭头（卡片坐标），按 msime-windows 的 PagerArrows 摆：横排时在候选右侧那一窄列里贴右下角，竖直居中在最后一行上；竖排时在最后一行下方那一矮行里靠左，「‹」的左端与序号左端对齐。没有列表箭头或没有行时为空。点击沿用 candidate_pager_hit。
inline std::optional<CandidatePagerLayout>
candidate_list_arrows_layout(double width, const std::vector<CandidateRowLayout> &rows,
                             const CandidateCardMetrics &metrics) {
  const auto area = candidate_rows_area(width, rows, metrics);
  if (!metrics.list_arrows || !area || !std::isfinite(width) || width <= 0.0)
    return std::nullopt;
  const auto &arrows = *metrics.list_arrows;
  CandidatePagerLayout layout;
  layout.in_list = true;
  if (arrows.column > 0.0) {
    const double right = width - metrics.pad_x / 2.0 - arrows.margin_right;
    const double bottom = area->bottom - arrows.margin_bottom;
    const double height =
        (std::min)(arrows.button_height, (std::max)(bottom - area->top, 0.0));
    layout.next = {right - arrows.button_width, bottom - height, right, bottom};
    layout.previous = {layout.next.left - arrows.gap - arrows.button_width,
                       bottom - height, layout.next.left - arrows.gap, bottom};
  } else {
    const double left = metrics.pad_x / 2.0 + metrics.number_left -
                        (arrows.button_width - arrows.glyph_width()) * 0.5;
    const double top = area->bottom + 1.0;
    layout.previous = {left, top, left + arrows.button_width,
                       top + arrows.button_height};
    layout.next = {layout.previous.right + arrows.gap, top,
                   layout.previous.right + arrows.gap + arrows.button_width,
                   top + arrows.button_height};
  }
  layout.indicator = {layout.previous.left, layout.previous.top,
                      layout.previous.left, layout.previous.bottom};
  layout.left = layout.previous.left;
  return layout;
}
// 皮肤 `item_corners_follow_card`：高亮落在列表四角上的那几个角改用卡片圆角（msime-windows CandidateList::Render 的 outerCornerRadius），其余角仍是高亮圆角 `inner`。顶上两个角只在 `top_edge` 时算（首行不占高度，列表贴着卡片内容的顶边）。返回左上、右上、右下、左下。
inline std::array<double, 4>
candidate_row_corner_radii(const CandidateRowBounds &row, const CandidateRowBounds &area,
                           bool top_edge, double inner, double outer) {
  constexpr double edge = 0.5;
  const bool top = top_edge && row.top <= area.top + edge;
  const bool bottom = row.bottom >= area.bottom - edge;
  const bool left = row.left <= area.left + edge;
  const bool right = row.right >= area.right - edge;
  return {top && left ? outer : inner, top && right ? outer : inner,
          bottom && right ? outer : inner, bottom && left ? outer : inner};
}
// Where the card sits relative to the caret. All values are physical pixels
// except scale, which is dpi/96; the design offsets below are DIPs so the gaps
// stay visually stable at 150% and 200%.
struct CandidatePlacementInput {
  // The line's bottom-left, as TSF reports the text extent.
  int anchor_x = 0;
  int anchor_y = 0;
  int width = 0;
  int height = 0;
  // The height the flip decision is made with. For a vertical list this is the
  // tallest the list has been this composition, not its current height, so a
  // list that grows as the user types does not jump below-to-above mid-word.
  // The card is still *placed* with its current height.
  int decision_height = 0;
  int work_left = 0, work_top = 0, work_right = 0, work_bottom = 0;
  double scale = 1.0;
};
// The height a vertical list's flip decision starts from before any list has been measured this composition: the shipped presenter seeds its memory with DEFAULT_WINDOW_HEIGHT_DIP (CANDIDATE_WINDOW_HEIGHT, 232 DIP) and resets it to that on hide, so a short first page already flips above the caret when a full one would not fit below. The decision then follows the tallest list seen, capped at the work area height.
inline constexpr double candidate_vertical_decision_seed_dip = 232.0;
inline int64_t candidate_vertical_decision_height(int64_t tallest, double scale,
                                                  int64_t available_height) {
  const double factor = std::isfinite(scale) && scale > 0.0 ? scale : 1.0;
  const auto seed = static_cast<int64_t>(
      std::ceil(candidate_vertical_decision_seed_dip * factor));
  return (std::min)((std::max)(tallest, seed), available_height);
}
struct CandidatePlacement {
  int x = 0;
  int y = 0;
  bool above = false;
};
// Position the card against the caret, flipping above the input line when a
// full list would not fit below it. Clamping alone is not enough: near the
// bottom of a screen it slides the card up over the very text being composed.
inline CandidatePlacement
candidate_card_placement(const CandidatePlacementInput &input) {
  const double scale = input.scale > 0.0 ? input.scale : 1.0;
  const int caret_gap = static_cast<int>(std::lround(3.0 * scale));
  const int edge_pad = static_cast<int>(std::lround(2.0 * scale));
  CandidatePlacement placement{input.anchor_x, input.anchor_y + caret_gap,
                               false};
  if (placement.x + input.width > input.work_right - edge_pad)
    placement.x = input.work_right - input.width - edge_pad;
  if (placement.x < input.work_left + edge_pad)
    placement.x = input.work_left + edge_pad;
  if (placement.y < input.work_top + edge_pad)
    placement.y = input.work_top + edge_pad;
  const int decision = (std::max)(input.decision_height, input.height);
  if (placement.y + decision > input.work_bottom) {
    // anchor_y is the line's bottom, so stepping back one line height puts the
    // card's lower edge at the line's top. A fixed gap instead of a line height
    // leaves a hole under short cards.
    const int line_height = static_cast<int>(std::lround(24.0 * scale));
    placement.y = input.anchor_y - input.height - line_height;
    placement.above = true;
    if (placement.y < input.work_top + edge_pad)
      placement.y = input.work_top + edge_pad;
  }
  return placement;
}
} // namespace msime::windows
