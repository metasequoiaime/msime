#pragma once

// macOS 候选窗结构的原生画布（NativeCanvasAssets::chrome）。几何规则逐条对照 platforms/macos/src/input/InputController.mm 的 renderCandidates、candidatePageGeometry 和 platforms/macos/src/candidate/CandidateChrome.h 的 MSIMECandidateButton；行排版、尺寸常量、装饰摆放和动效数值都取自 shared/candidate，与 macOS 同一份。canvas 皮肤包的 Windows 版式在 NativeCanvas.h（measure_native_canvas_skin）。
#include "NativeCanvas.h"

namespace msime::linux_host {

// 动效在某一帧的状态，由 X11 窗口按时间算好交给绘制；默认值是静止的完整画面。
struct NativeCanvasMotion {
  msime::candidate::SelectionMotionFrame selection;
  msime::candidate::DecorationMotionFrame decoration;
  // 错峰浮现时每一行的不透明度和抬起量；空表示不错峰。
  std::vector<msime::candidate::CardMotionFrame> rows;
};

// 同一族字体换绝对字号和字重；Pango 把绝对字号写成「…px」。
inline std::string native_canvas_font_variant(const std::string &font, double size,
                                              PangoWeight weight = PANGO_WEIGHT_NORMAL) {
  auto *description = pango_font_description_from_string(font.c_str());
  if (size > 0) pango_font_description_set_absolute_size(description, size * PANGO_SCALE);
  pango_font_description_set_weight(description, weight);
  char *value = pango_font_description_to_string(description);
  std::string result(value);
  g_free(value);
  pango_font_description_free(description);
  return result;
}

// 一行字的高度：空串是字体本身的行高（macOS 的 MSIMECandidateTextHeight(@"", font)），回退字体更高的字形由调用方按实际文字再量一次。
inline double native_canvas_line_height(cairo_t *cr, const std::string &font, const std::string &text = {}) {
  return native_canvas_text_size(cr, font, text).second;
}

// 释义行的文字：训音在最前，然后是各目标语言的释义（每行的读音在它下面单独一行，macOS 的 MSIMECandidateGlossDisplay），插件注释单独占最后一行；与 macOS 的 MSIMECandidateGlossRun、MSIMECandidateGlossWithScriptAnnotation 相同。
struct NativeCanvasGloss {
  std::string text;
  std::size_t translation_start = 0, script_start = std::string::npos;
  // 释义连同读音，位置相对 translation_start。
  CandidateGlossReadings translation;
};
inline NativeCanvasGloss native_canvas_gloss(const NativeCanvasItem &item) {
  NativeCanvasGloss gloss;
  const auto append = [&](const std::string &line) {
    if (line.empty()) return std::size_t(std::string::npos);
    if (!gloss.text.empty()) gloss.text += "\n";
    const auto start = gloss.text.size();
    gloss.text += line;
    return start;
  };
  append(item.reading);
  gloss.translation = candidate_gloss_readings(item.translation, item.pronunciation);
  const auto start = append(gloss.translation.text);
  gloss.translation_start = start == std::string::npos ? gloss.text.size() : start;
  gloss.script_start = append(item.script_annotation);
  return gloss;
}

// 皮肤长度：em 按候选字号取整，DIP 乘整体大小（macOS 的 MSIMESkinLengthValue）。
inline double native_canvas_chrome_length(const nlohmann::json &geometry, const char *key, double font_size,
                                          double scale, double fallback) {
  const auto at = geometry.find(key);
  if (at == geometry.end() || !at->is_object()) return fallback;
  const auto value = at->find("value");
  if (value == at->end() || !value->is_number()) return fallback;
  const bool em = at->value("em", false);
  const double result = em ? std::round(value->get<double>() * font_size) : value->get<double>() * scale;
  return std::isfinite(result) && result >= 0 ? result : fallback;
}

// 一页候选（或网格里的一行）按 macOS candidatePageGeometry 排出来的结果。
struct NativeCanvasChromePage {
  double width = 0, line_width = 0, rows_height = 0, content_left = 0;
  std::vector<msime::candidate::CandidateRowLayout> rows;
};

inline NativeCanvasLayout measure_native_canvas_chrome(cairo_t *cr, const NativeCanvasAssets &assets,
    const NativeCanvasContent &content, double visible_width = -1) {
  namespace shared = msime::candidate;
  NativeCanvasLayout layout;
  layout.chrome = true;
  layout.metrics = native_canvas_metrics(cr, assets, content);
  auto &metrics = layout.metrics;
  const double s = std::isfinite(assets.scale) && assets.scale > 0 ? assets.scale : 1.0;
  const double font_size = metrics.font_size;
  auto geometry = assets.geometry;
  geometry.update(content.horizontal ? assets.horizontal : assets.vertical);
  const bool skin_layout = !geometry.empty();
  const auto length = [&](const char *key, double fallback) {
    return native_canvas_chrome_length(geometry, key, font_size, s, fallback);
  };
  // 卡片边缘到候选行：内边距可以小到 0，但不压住描边（CandidateInset）。
  double inset = metrics.content[0];
  if (geometry.contains("padding")) inset = length("padding", inset);
  inset = std::max({0.0, inset, assets.border_width});
  metrics.content = {inset, inset, inset, inset};
  const double number_scale = geometry.contains("numberScale") && geometry["numberScale"].is_number()
      ? std::clamp(geometry["numberScale"].get<double>(), 0.0, 2.0) : shared::kCandidateNumberScale;
  metrics.number_scale = number_scale;
  metrics.number_font = native_canvas_font_variant(metrics.font, std::max(0.01, font_size * number_scale));
  layout.gloss_font = native_canvas_font_variant(metrics.font, shared::kCandidateTranslationPointSize * s);
  layout.preedit_font = native_canvas_font_variant(metrics.font, content.preedit_size > 0 ? content.preedit_size : font_size,
                                                   PANGO_WEIGHT_SEMIBOLD);
  layout.indicator_font = native_canvas_font_variant(metrics.font, shared::kCandidatePageIndicatorPointSize * s);
  layout.symbol_font = native_canvas_font_variant(metrics.font, shared::kCandidatePageIndicatorPointSize * s);
  const bool custom_height = geometry.contains("itemExtraHeight");
  const double row_floor = custom_height ? font_size * 1.35 + length("itemExtraHeight", 0) : 0;
  const double row_padding = custom_height ? 0 : shared::kCandidateRowPadding * s;
  const double line = native_canvas_line_height(cr, metrics.font);
  const double gloss_padding = shared::kCandidateGlossPadding * s;
  const bool fit_content = assets.fit_content;
  const double text_left = skin_layout && geometry.contains("numberInset")
      ? std::max(5.0 * s, length("numberInset", 0)) : shared::CandidateTextLeft(assets.show_selected_bar, s);
  const double number_gap = skin_layout ? length("numberGap", shared::kCandidateNumberGap * s) : shared::kCandidateNumberGap * s;
  double text_right = skin_layout ? length("itemPaddingRight", shared::kCandidateTextRight * s) : shared::kCandidateTextRight * s;
  if (skin_layout && fit_content) text_right = text_left;
  metrics.number_inset = text_left;
  metrics.right = text_right;
  metrics.custom_height = custom_height;
  metrics.extra_height = custom_height ? length("itemExtraHeight", 0) : row_padding;
  metrics.bar_width = skin_layout && geometry.contains("selectedBarWidth")
      ? std::max(3.0 * s, length("selectedBarWidth", 3 * s)) : 3.0 * s;
  metrics.bar_height = skin_layout && geometry.contains("selectedBarHeightEm")
      ? std::max(metrics.bar_width * 2, geometry.value("selectedBarHeightEm", 0.0) * font_size)
      : std::max(10.0 * s, font_size * 0.8);
  metrics.bar_inside = skin_layout && geometry.value("selectedBarInside", false);
  layout.text_left = text_left;
  layout.text_right = text_right;
  layout.skin_layout = skin_layout;
  const double gloss_line = native_canvas_line_height(cr, layout.gloss_font, "X") + gloss_padding;

  // 一页（或网格的一行）候选的宽度与行：不含首行，`header_width` 只参与卡片宽度。
  const double visible = visible_width > 0 && std::isfinite(visible_width) ? visible_width : 1920.0;
  const auto page_geometry = [&](std::size_t begin, std::size_t end, bool paging, double header_width,
                                 double minimum_width, int reserved_lines, double row_visible) {
    NativeCanvasChromePage page;
    double candidate_row = std::max(row_floor, line + row_padding);
    double number_width = 0;
    std::vector<std::string> glosses;
    for (std::size_t at = begin; at < end; ++at) {
      const auto &item = content.items[at];
      glosses.push_back(native_canvas_gloss(item).text);
      // 网格里只有高亮所在的行画序号，其余行的 label 是空的；序号列仍按这一行该有的「1」…「n」量（macOS 不论画不画都量），高亮换行时各行文字才不左右跳。
      const auto label = !item.label.empty() ? item.label
          : content.grid_columns > 0 ? candidate_index_label(at - begin) : std::string{};
      if (number_scale > 0 && !label.empty())
        number_width = std::max(number_width, native_canvas_text_size(cr, metrics.number_font, label).first);
      // 回退字体的字形可能比主字体高，单行的行高取这一页里最高的。
      candidate_row = std::max(candidate_row, native_canvas_line_height(cr, metrics.font, item.text) + row_padding);
    }
    if (skin_layout) number_width = std::max(9.0 * s, number_width);
    page.content_left = text_left + std::ceil(number_width) + number_gap;
    shared::CandidateLayoutMetrics lm;
    lm.candidateRow = candidate_row;
    lm.chrome = page.content_left + text_right;
    lm.annotationGap = 0;
    lm.annotationLine = line;
    lm.translationGap = font_size * 0.65;
    lm.translationLine = gloss_line;
    if (skin_layout) {
      lm.itemGap = length("itemGap", 0);
      lm.columnGap = lm.itemGap + (fit_content ? 0 : 6.0 * s);
      lm.justifyRows = geometry.value("justifyRows", false);
      lm.itemMinWidth = content.horizontal ? length("itemMinWidth", 0) : 0;
    }
    std::vector<shared::CandidateItemWidths> widths;
    double natural = 0;
    for (std::size_t at = begin; at < end; ++at) {
      shared::CandidateItemWidths width;
      width.text = std::ceil(native_canvas_text_size(cr, metrics.font, content.items[at].text).first);
      const auto &gloss = glosses[at - begin];
      if (!gloss.empty()) {
        const auto size = native_canvas_text_size(cr, layout.gloss_font, gloss);
        width.translation = std::ceil(size.first);
        width.translationHeight = std::ceil(size.second) + gloss_padding;
      }
      widths.push_back(width);
      const double item = std::ceil(shared::CandidateItemNaturalWidth(widths.back(), lm, content.horizontal));
      natural = content.horizontal ? natural + item : std::max(natural, item);
    }
    if (content.horizontal && widths.size() > 1)
      natural += (lm.columnGap > 0 ? lm.columnGap : lm.itemGap) * static_cast<double>(widths.size() - 1);
    double width = std::max(20.0, natural + 2 * inset);
    width = std::max(width, header_width);
    // 卡片以可见区域的一半封顶；横排一页的候选行在一整行里放得下时可以长过半屏，直到屏宽减两边的留白，释义一起不折行。
    double cap = std::max(80.0, std::floor(row_visible * shared::kCandidateWidthCapRatio));
    if (content.horizontal) {
      const double screen_cap = std::max(cap, std::floor(row_visible - 2 * shared::kCandidateScreenMargin));
      if (std::ceil(shared::SingleLineMinimumWidth(widths, lm)) + 2 * inset <= screen_cap)
        cap = std::max(cap, std::min(screen_cap, natural + 2 * inset));
    }
    width = std::min(width, cap);
    if (paging) width = std::max(width, shared::kCandidatePagingMinimumWidth * s);
    width = std::max(width, shared::CandidateCardMinimumWidth(font_size, minimum_width, cap,
        skin_layout ? std::optional<bool>(fit_content) : std::nullopt, s));
    page.width = width;
    page.line_width = std::max(1.0, width - 2 * inset);
    const auto measure = [&](std::size_t row, shared::CandidateRun run, double run_width) -> double {
      const auto &item = content.items[begin + row];
      if (run == shared::CandidateRun::text)
        return native_canvas_text_size(cr, metrics.font, item.text, run_width).second + row_padding;
      if (run == shared::CandidateRun::translation)
        return native_canvas_text_size(cr, layout.gloss_font, glosses[row], run_width).second + gloss_padding;
      return 0.0;
    };
    // 一行候选加上 lines 行还没到的释义有多高：横排整张卡片的预留和竖排高亮行的预留用同一个量法。
    const auto reserved_height = [&](int lines) {
      std::string placeholder = "X";
      for (int at = 1; at < lines; ++at) placeholder += "\nX";
      return candidate_row + native_canvas_text_size(cr, layout.gloss_font, placeholder).second + gloss_padding;
    };
    double minimum_height = 0;
    if (content.horizontal && reserved_lines > 0) minimum_height = reserved_height(reserved_lines);
    page.rows = shared::LayoutCandidatePage(widths, page.line_width, lm, content.horizontal, measure, minimum_height);
    // 竖排「仅当前高亮」：高亮行（translation_reserved）一直留出与横排同样行数的释义高度，回答没到也留，正文仍从行顶开始；它下面的行跟着下移。
    if (!content.horizontal) {
      double shift = 0;
      for (std::size_t row = 0; row < page.rows.size(); ++row) {
        auto &placed = page.rows[row];
        placed.y += shift;
        const int lines = content.items[begin + row].reserved_gloss_lines;
        if (lines <= 0) continue;
        const double reserved = reserved_height(lines);
        if (placed.height < reserved) {
          shift += reserved - placed.height;
          placed.height = reserved;
        }
      }
    }
    for (auto &row : page.rows) {
      // 整数坐标让相邻的行贴齐，文字落在像素格上。
      const double left = std::round(row.x), right = std::round(row.x + row.width);
      const double top = std::round(row.y), bottom = std::round(row.y + row.height);
      row.x = left; row.width = right - left;
      row.y = top; row.height = bottom - top;
    }
    page.rows_height = std::ceil(shared::CandidatePageHeight(page.rows));
    return page;
  };

  // 首行：logo、拼音（半粗体，candidate_preedit_font_size），右侧「1 / 3」、‹ › 和展开键。
  const bool grid = content.grid_columns > 0;
  const bool menu = content.menu;
  const bool has_preedit = !content.preedit.empty() && !menu;
  const double preedit_height = has_preedit
      ? std::max(shared::kCandidatePreeditMinHeight * s,
                 native_canvas_line_height(cr, layout.preedit_font, content.preedit) + shared::kCandidatePreeditPadding * s)
      : 0;
  const bool logo = content.logo && !menu;
  const double logo_side = shared::kCandidateLogoSide * s;
  const double logo_width = logo ? logo_side + shared::kCandidateLogoGap * s : 0;
  const double header_row = shared::kCandidateHeaderHeight * s;
  const double arrow_width = shared::kCandidatePageArrowWidth * s;
  const double indicator_gap = shared::kCandidatePageIndicatorGap * s;
  // 「1 / 3」和 ‹ › 跟着 show_candidate_page_number 一起显示或隐藏；网格首行是高亮所在的行「2 / 7」和收起键。
  const bool page_controls = !menu && content.page_arrows && (grid || content.page_count > 1);
  layout.page_controls = page_controls;
  if (page_controls)
    layout.indicator_text = grid ? std::to_string(content.grid_row + 1) + " / " + std::to_string(content.grid_rows)
                                 : candidate_page_indicator(static_cast<std::size_t>(std::max(0, content.page_index)),
                                                            static_cast<std::size_t>(content.page_count));
  const double indicator_width = page_controls ? std::ceil(native_canvas_text_size(cr, layout.indicator_font, layout.indicator_text).first) : 0;
  const double indicator_height = page_controls ? std::ceil(native_canvas_text_size(cr, layout.indicator_font, layout.indicator_text).second) : 0;
  const bool expandable = !grid && !menu && content.expandable;
  // 首行什么都不画时，展开键放在第一行候选的右端，不为它多占一行。
  const bool compact_expand = expandable && !page_controls && !has_preedit && !logo;
  const double controls_width = grid ? (page_controls ? indicator_width + indicator_gap + arrow_width : 0)
      : (page_controls ? indicator_width + indicator_gap + 2 * arrow_width : 0) + (expandable && !compact_expand ? arrow_width : 0);
  std::array<double, 4> margin{};
  if (has_preedit) margin = {metrics.preedit_margin[0] * s, metrics.preedit_margin[1] * s,
                             metrics.preedit_margin[2] * s, metrics.preedit_margin[3] * s};
  const double header_content = grid
      ? (has_preedit || logo || page_controls ? std::max(preedit_height, header_row) : 0)
      : std::max(preedit_height, page_controls || logo || (expandable && !compact_expand) ? header_row : 0);
  const double header_height = header_content + margin[0] + margin[2];
  const double preedit_width = has_preedit
      ? std::ceil(native_canvas_text_size(cr, layout.preedit_font, content.preedit).first) + 4 * s +
            shared::kCandidatePreeditCaretGap + margin[3] + margin[1]
      : 0;
  const double header_width = has_preedit || page_controls || logo
      ? 2 * inset + logo_width + preedit_width + (has_preedit && controls_width > 0 ? indicator_gap : 0) + controls_width
      : 0;
  // 光标后编辑的提示算进卡片的最小宽度，模式生效期间总能画全；和本地模式、连击同在一行时它们也算进去，左右两段才不会叠在一起。没有提示时底部一行和以前一样不参与宽度。
  const double footer_hint_width = content.auxiliary_hint.empty()
      ? 0 : std::ceil(native_canvas_text_size(cr, layout.gloss_font, content.auxiliary_hint).first);
  const double footer_width = footer_hint_width > 0
      ? 2 * inset + layout.text_left + layout.text_right + footer_hint_width +
            (content.auxiliary.empty() ? 0 : std::ceil(native_canvas_text_size(cr, layout.gloss_font, content.auxiliary).first) + 8 * s)
      : 0;
  const double top_width = std::max(header_width, footer_width);
  const double row_visible = std::max(0.0, visible - (compact_expand ? arrow_width : 0));
  // 记住的宽度含首行右端的展开键；这一页把展开键放在行尾时先扣掉。
  const double minimum = std::max(assets.min_width, content.minimum_width - (compact_expand ? arrow_width : 0));

  std::vector<NativeCanvasChromePage> pages;
  double width = 0;
  if (grid) {
    width = top_width;
    for (std::size_t begin = 0; begin < content.items.size(); begin += content.grid_columns) {
      const auto end = std::min(content.items.size(), begin + content.grid_columns);
      pages.push_back(page_geometry(begin, end, false, 0, assets.min_width, 0, visible));
      width = std::max(width, pages.back().width);
    }
    width = std::ceil(width);
  } else {
    pages.push_back(page_geometry(0, content.items.size(), page_controls, top_width,
                                  content.horizontal ? minimum : assets.min_width, content.reserved_gloss_lines, row_visible));
    width = pages.back().width + (compact_expand ? arrow_width : 0);
  }
  double rows_height = 0;
  for (const auto &page : pages) rows_height += page.rows_height;

  // 底部一行：本地模式和打字连击，macOS 候选窗没有这一行，Linux 用它保留原来辅助行里的信息；光标后编辑的提示（auxiliary_hint）靠右画在同一行，只有提示时这一行也要出现。
  // 行高和释义行一样从释义字体本身的行高（gloss_line）起算，回退字体更高时再按实际文字加高。只按文字量时，整段落在比主字体矮的回退字体里的提示（没有中文字体时 Pango 1.57 的缺字方框也是这样）会让这一行比释义行矮，连击「×12」里的主字体字形一出现卡片就跟着长高，每次连击出现和消失卡片都要跳一下。
  const bool footer = !content.auxiliary.empty() || !content.auxiliary_hint.empty();
  const double footer_height = footer ? std::max({gloss_line,
      content.auxiliary.empty() ? 0.0 : native_canvas_line_height(cr, layout.gloss_font, content.auxiliary) + gloss_padding,
      content.auxiliary_hint.empty() ? 0.0 : native_canvas_line_height(cr, layout.gloss_font, content.auxiliary_hint) + gloss_padding})
      : 0;
  const double band = assets.band.image && assets.band.top > 0 && !menu ? assets.band.top : 0;
  const double card_height = rows_height + 2 * inset + header_height + footer_height;
  layout.panel = {0, band, width, card_height};
  const double header_top = band + inset + margin[0];
  const double rows_top = band + inset + header_height;
  layout.rows_area = {inset, rows_top, std::max(0.0, width - 2 * inset), rows_height};
  if (logo) layout.logo = {inset + 2 * s, header_top + std::floor((header_content - logo_side) / 2), logo_side, logo_side};
  if (has_preedit) {
    layout.preedit = {inset + logo_width + margin[3], header_top + std::floor((header_content - preedit_height) / 2),
        std::max(0.0, width - 2 * inset - logo_width - margin[3] - margin[1] - (controls_width > 0 ? controls_width + indicator_gap : 0)),
        preedit_height};
    layout.preedit_width = layout.preedit.width;
  }
  const double controls_top = header_top + std::floor((header_content - header_row) / 2);
  if (grid) {
    if (page_controls) {
      const double collapse_left = width - inset - arrow_width;
      layout.expand = {collapse_left, controls_top, arrow_width, header_row};
      layout.indicator = {collapse_left - indicator_gap - indicator_width,
          controls_top + std::floor((header_row - indicator_height) / 2), indicator_width, indicator_height};
    }
  } else {
    if (page_controls) {
      const double arrows_left = width - inset - (expandable ? 3 : 2) * arrow_width;
      layout.indicator = {arrows_left - indicator_gap - indicator_width,
          controls_top + std::floor((header_row - indicator_height) / 2), indicator_width, indicator_height};
      layout.previous = {arrows_left, controls_top, arrow_width, header_row};
      layout.next = {arrows_left + arrow_width, controls_top, arrow_width, header_row};
    }
    if (expandable) {
      if (compact_expand && !pages.front().rows.empty()) {
        const double first = pages.front().rows.front().height;
        const double height = std::min(header_row, first);
        layout.expand = {width - inset - arrow_width, rows_top + std::floor((first - height) / 2), arrow_width, height};
      } else {
        layout.expand = {width - inset - arrow_width, controls_top, arrow_width, header_row};
      }
    }
  }
  double top = rows_top;
  for (const auto &page : pages) {
    for (const auto &row : page.rows) {
      layout.items.push_back({inset + row.x, top + row.y, row.width, row.height});
      layout.rows.push_back(row.item);
      layout.content_lefts.push_back(page.content_left);
      layout.item_text_widths.push_back(row.item.textWidth);
    }
    top += page.rows_height;
  }
  if (footer) {
    const double left = inset + text_left, available = std::max(1.0, width - 2 * inset - text_left - text_right);
    const double top = rows_top + rows_height + std::floor(gloss_padding / 2), height = footer_height - gloss_padding;
    // 提示贴右边距，放不下时最多占满整行；左边的本地模式和连击让出提示的宽度再隔 8pt。
    double hint_width = 0;
    if (!content.auxiliary_hint.empty()) {
      hint_width = std::min(available, std::ceil(native_canvas_text_size(cr, layout.gloss_font, content.auxiliary_hint).first));
      layout.auxiliary_hint = {left + available - hint_width, top, hint_width, height};
    }
    if (!content.auxiliary.empty()) {
      layout.auxiliary = {left, top, std::max(1.0, available - (hint_width > 0 ? hint_width + 8 * s : 0)), height};
      layout.auxiliary_width = layout.auxiliary.width;
    }
  }
  // 顶带装饰：画在卡片顶边上方的透明带里，有待机动效时四周留出余量。
  if (band > 0) {
    const auto &image = assets.band.image->frames.front();
    const auto placed = shared::DecorationPlacement(assets.band.align, width, inset, band, assets.band.width,
        cairo_image_surface_get_width(image.get()), cairo_image_surface_get_height(image.get()),
        assets.band.motion != shared::DecorationMotionKind::none ? shared::kDecorationMotionMargin : 0.0);
    if (placed) layout.band = {placed->x, placed->top, placed->width, placed->height};
  }
  layout.width = std::ceil(width);
  layout.height = std::ceil(band + card_height);
  if (assets.band.candidate && layout.band.width > 0) {
    const double align = assets.band.align == shared::DecorationAlign::left ? 0 :
        assets.band.align == shared::DecorationAlign::center ? 0.5 : 1;
    const NativeCanvasRect declared{layout.band.x + (layout.band.width - assets.band.width) * align,
        layout.band.y + layout.band.height - assets.band.top, assets.band.width, assets.band.top};
    layout.overlay = native_canvas_overlay_layout(cr, *assets.band.candidate, content, layout, declared);
    if (layout.overlay) {
      double left = 0, top = 0, right = layout.width, bottom = layout.height;
      for (const auto &box : {layout.overlay->box, layout.overlay->ink}) {
        left = std::min(left, box.x); top = std::min(top, box.y);
        right = std::max(right, box.x + box.width); bottom = std::max(bottom, box.y + box.height);
      }
      // 最小字号后的完整文字及字形外伸计入窗口；所有命中矩形与绘制一起平移。
      const auto shift = [&](NativeCanvasRect &box) { box.x -= left; box.y -= top; };
      for (auto *box : {&layout.panel, &layout.preedit, &layout.auxiliary, &layout.auxiliary_hint, &layout.previous, &layout.next,
          &layout.page_divider, &layout.expand, &layout.logo, &layout.indicator, &layout.band, &layout.rows_area,
          &layout.overlay->box, &layout.overlay->ink}) shift(*box);
      for (auto &box : layout.items) shift(box);
      layout.width = std::ceil(right - left); layout.height = std::ceil(bottom - top);
    }
  }
  return layout;
}

// 卡片投影：macOS 给非 canvas 皮肤的候选窗留着系统投影（refreshCandidateSkin 的 hasShadow），X11 上没有窗口管理器替 override-redirect 窗口画，所以有合成器时原生画布自己画，取 classicui 主题那一层（FcitxPanelGeometry：向下 4px、sigma 5、22% 黑）。窗口四周按这些边距放大，装饰带比上边距高时上边不再多留。没有合成器时窗口只有二值透明，不画投影，边距都是 0。单位是逻辑像素，不随 candidate_scale_percent 缩放（与 classicui 相同）。
struct NativeCanvasShadowInsets {
  double left = 0, top = 0, right = 0, bottom = 0;
};

inline NativeCanvasShadowInsets native_canvas_shadow_insets(const NativeCanvasLayout &layout, bool composited) {
  using G = FcitxPanelGeometry;
  if (!composited || !layout.chrome) return {};
  return {static_cast<double>(G::shadow_left), std::max(0.0, G::shadow_top - layout.panel.y),
          static_cast<double>(G::shadow_right), static_cast<double>(G::shadow_bottom)};
}

// 投影画成 width×height 设备像素的图，`card` 是卡片在窗口里的位置（逻辑像素），`scale` 是显示倍率；窗口尺寸和卡片位置不变时调用方缓存它。卡片底下的投影按圆角覆盖率擦掉，半透明的卡片底下透出桌面而不是投影，与 classicui 的 fcitx_clear_rounded 相同。
inline NativeCanvasImage native_canvas_card_shadow(int width, int height, const NativeCanvasRect &card, double radius,
                                                   double scale) {
  using G = FcitxPanelGeometry;
  if (width <= 0 || height <= 0 || !(scale > 0)) return {};
  NativeCanvasImage image(cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width, height), cairo_surface_destroy);
  if (cairo_surface_status(image.get()) != CAIRO_STATUS_SUCCESS) return {};
  cairo_surface_flush(image.get());
  auto *data = cairo_image_surface_get_data(image.get());
  const int stride = cairo_image_surface_get_stride(image.get());
  const FcitxRect body{card.x, card.y, card.x + card.width, card.y + card.height};
  const FcitxRect cast{body.left, body.top + G::shadow_offset, body.right, body.bottom + G::shadow_offset};
  const double spread = G::shadow_sigma * std::sqrt(2.0), unit = 1 / scale;
  // 卡片里离圆角和边缘都至少一个设备像素的部分整块被擦掉（到卡片的距离不大于 -unit，覆盖率为 1），不必逐像素算两次距离和 erfc；横排、竖排换页时卡片尺寸常变，投影要重算，这一块占了窗口的大部分。
  const double corner = std::clamp(radius, 0.0, std::min(card.width, card.height) / 2) + unit;
  const FcitxRect interior{body.left + corner, body.top + corner, body.right - corner, body.bottom - corner};
  // 内部在每一行里覆盖的设备像素列 [skip_from, skip_to]：像素中心 (x + 0.5) * unit 严格落在 interior 的左右边之间。
  const int skip_from = static_cast<int>(std::floor(interior.left * scale - 0.5)) + 1;
  const int skip_to = static_cast<int>(std::ceil(interior.right * scale - 0.5)) - 1;
  for (int y = 0; y < height; ++y) {
    const double py = (y + 0.5) * unit;
    const bool interior_row = py > interior.top && py < interior.bottom && skip_from <= skip_to;
    for (int x = 0; x < width; ++x) {
      if (interior_row && x == skip_from) {
        x = skip_to;
        continue;
      }
      const double px = (x + 0.5) * unit;
      const double keep = 1.0 - fcitx_coverage(fcitx_rounded_rect_distance(px, py, body, radius), unit);
      const double distance = fcitx_rounded_rect_distance(px, py, cast, radius);
      // erfc(4) 已小于 1e-7，再远的像素没有可见的投影。
      if (keep <= 0 || distance > 4 * spread) continue;
      const double alpha = G::shadow_alpha * 0.5 * std::erfc(distance / spread) * keep;
      // 预乘的黑色只有 alpha 通道。
      const std::uint32_t pixel = static_cast<std::uint32_t>(std::lround(std::clamp(alpha, 0.0, 1.0) * 255)) << 24;
      std::memcpy(data + y * stride + x * 4, &pixel, 4);
    }
  }
  cairo_surface_mark_dirty(image.get());
  return image;
}

inline int native_canvas_hit_chrome(const NativeCanvasContent &content, const NativeCanvasLayout &layout,
                                    double x, double y) {
  for (std::size_t at = 0; at < layout.items.size(); ++at)
    if (layout.items[at].contains(x, y)) return static_cast<int>(at);
  if (layout.page_controls && content.previous && layout.previous.contains(x, y)) return kNativeCanvasHitPrevious;
  if (layout.page_controls && content.next && layout.next.contains(x, y)) return kNativeCanvasHitNext;
  if (layout.expand.width > 0 && layout.expand.contains(x, y)) return kNativeCanvasHitExpand;
  return -1;
}

// 高亮的四角：皮肤要求贴卡片圆角（itemCornersFollowCard，或用户自己设了圆角）时，边上的行在靠卡片的角上取卡片圆角，其余取行圆角；首行有内容时上面一排不贴。
inline std::array<double, 4> native_canvas_chrome_corners(const NativeCanvasLayout &layout, std::size_t at,
                                                          double radius, double card_radius, bool follow) {
  std::array<double, 4> radii;
  radii.fill(radius);
  if (!follow || at >= layout.items.size()) return radii;
  const auto &item = layout.items[at];
  const auto &area = layout.rows_area;
  const bool top = std::abs(area.y - (layout.panel.y + layout.metrics.content[0])) < 0.5 && item.y <= area.y + 0.5;
  const bool bottom = item.y + item.height >= area.y + area.height - 0.5;
  const bool left = item.x <= area.x + 0.5;
  const bool right = item.x + item.width >= area.x + area.width - 0.5;
  if (top && left) radii[0] = card_radius;
  if (top && right) radii[1] = card_radius;
  if (bottom && right) radii[2] = card_radius;
  if (bottom && left) radii[3] = card_radius;
  return radii;
}

inline void native_canvas_set_color(cairo_t *cr, ThemeColor color, double alpha_scale = 1) {
  cairo_set_source_rgba(cr, ((color.rgb >> 16) & 255) / 255.0, ((color.rgb >> 8) & 255) / 255.0,
                        (color.rgb & 255) / 255.0, color.alpha / 255.0 * std::clamp(alpha_scale, 0.0, 1.0));
}

// 首行的 ‹ ›：按 macOS 的按钮标题字形画，居中在 28pt 宽的按钮里。
inline void native_canvas_chrome_glyph(cairo_t *cr, const std::string &font, const std::string &glyph,
                                       const NativeCanvasRect &box) {
  auto *line = native_canvas_text_layout(cr, font, glyph);
  int width = 0, height = 0;
  pango_layout_get_pixel_size(line, &width, &height);
  cairo_move_to(cr, box.x + (box.width - width) / 2, box.y + (box.height - height) / 2);
  pango_cairo_show_layout(cr, line);
  g_object_unref(line);
}

// 展开、收起键：macOS 画 SF Symbol 的 chevron.down / chevron.up（11pt 半粗），这里画同样比例的折线。
inline void native_canvas_chrome_chevron(cairo_t *cr, const NativeCanvasRect &box, double scale, bool up) {
  const double size = msime::candidate::kCandidateExpandSymbolPointSize * scale;
  const double half = size * 0.42, rise = size * 0.22;
  const double cx = box.x + box.width / 2, cy = box.y + box.height / 2;
  const double tip = up ? cy - rise : cy + rise, tail = up ? cy + rise : cy - rise;
  cairo_move_to(cr, cx - half, tail);
  cairo_line_to(cr, cx, tip);
  cairo_line_to(cr, cx + half, tail);
  cairo_set_line_width(cr, std::max(1.2, size * 0.14));
  cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
  cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);
  cairo_stroke(cr);
}

// 释义行的 Pango 属性：字号是 12pt 的释义字体，颜色按 macOS 的规则（见调用方），训音与释义同色，插件注释斜体；释义里共享层识别出的词性和义项分隔符另上颜色。
inline PangoLayout *native_canvas_gloss_layout(cairo_t *cr, const std::string &font, const NativeCanvasItem &item,
    const NativeCanvasGloss &gloss, ThemeColor color, const nlohmann::json &colors, bool highlighted, double max_width) {
  auto *line = native_canvas_text_layout(cr, font, gloss.text, max_width);
  auto *attributes = pango_attr_list_new();
  const auto insert = [&](PangoAttribute *attribute, std::size_t from, std::size_t to) {
    attribute->start_index = static_cast<guint>(from);
    attribute->end_index = static_cast<guint>(to);
    pango_attr_list_change(attributes, attribute);
  };
  const auto paint = [&](ThemeColor value, std::size_t from, std::size_t to) {
    insert(pango_attr_foreground_new(((value.rgb >> 16) & 255) * 257, ((value.rgb >> 8) & 255) * 257, (value.rgb & 255) * 257), from, to);
    insert(pango_attr_foreground_alpha_new(static_cast<guint16>(value.alpha * 257)), from, to);
  };
  paint(color, 0, gloss.text.size());
  if (gloss.script_start != std::string::npos)
    insert(pango_attr_style_new(PANGO_STYLE_OBLIQUE), gloss.script_start, gloss.text.size());
  const auto slot = [&](const char *key) { return theme_color(colors.value(key, nlohmann::json{})); };
  const auto visible = [](const std::optional<ThemeColor> &value) { return value && value->alpha > 0; };
  if (item.translation_runs.is_array()) for (const auto &run : item.translation_runs) {
    if (!run.is_object() || !run.contains("start") || !run.contains("length") ||
        !run["start"].is_number_integer() || !run["length"].is_number_integer() ||
        run["start"] < 0 || run["length"] <= 0 || run["start"] > item.translation.size() ||
        !run.contains("role") || !run["role"].is_string()) continue;
    const auto from = run["start"].get<std::size_t>(), size = run["length"].get<std::size_t>();
    if (size > item.translation.size() - from ||
        !g_utf8_validate(item.translation.data() + from, static_cast<gssize>(size), nullptr)) continue;
    const auto role = run["role"].get<std::string>();
    if (role != "pos" && role != "separator") continue;
    auto run_color = highlighted ? slot(role == "pos" ? "selected_translation_pos" : "selected_translation_separator")
                                 : std::optional<ThemeColor>{};
    if (!visible(run_color)) run_color = slot(role == "pos" ? "translation_pos" : "translation_separator");
    const auto at = gloss.translation_start + gloss.translation.position(from);
    if (visible(run_color)) paint(*run_color, at, at + size);
  }
  // 读音按释义色的 0.72 倍透明度画（macOS 的 MSIMECandidatePronunciationOpacity），比它跟着的释义轻一些；选中行不减淡，那时的释义色已经是按选中底色挑的。
  if (!gloss.translation.readings.empty()) {
    auto reading = color;
    if (!highlighted)
      reading.alpha = static_cast<std::uint8_t>(std::round(reading.alpha * msime::candidate::kCandidatePronunciationOpacity));
    for (const auto &[from, length] : gloss.translation.readings)
      paint(reading, gloss.translation_start + from, gloss.translation_start + from + length);
  }
  pango_layout_set_attributes(line, attributes);
  pango_attr_list_unref(attributes);
  return line;
}

inline void paint_native_canvas_chrome(cairo_t *cr, const NativeCanvasAssets &assets, const NativeCanvasContent &content,
                                       const NativeCanvasLayout &layout, const nlohmann::json &palette,
                                       std::uint64_t elapsed_ms = 0, NativeCanvasPointer pointer = {},
                                       const NativeCanvasMotion &motion = {}) {
  namespace shared = msime::candidate;
  const double s = std::isfinite(assets.scale) && assets.scale > 0 ? assets.scale : 1.0;
  const auto &metrics = layout.metrics;
  const auto color = [&](const char *key, std::uint32_t fallback) {
    return theme_color(palette.value(key, nlohmann::json{})).value_or(ThemeColor{fallback, 255});
  };
  const auto optional_color = [&](const char *key) -> std::optional<ThemeColor> {
    auto value = theme_color(palette.value(key, nlohmann::json{}));
    if (value && value->alpha == 0) return std::nullopt;
    return value;
  };
  const auto text = color("text", 0x202020);
  const auto number = color("number", 0x555555);
  const auto accent = color("accent", 0x3584e4);
  const auto selected_text = optional_color("selected_text").value_or(text);
  const auto selected_number = optional_color("selected_number").value_or(number);
  const auto selected_fill = color("selected", 0xdceaff);
  const auto hover = color("hover", 0x000000);
  const ThemeColor hover_fill = theme_color(palette.value("hover", nlohmann::json{})) ? hover : ThemeColor{0x000000, 0x10};
  // 卡片：底色和描边随候选窗不透明度变淡，文字和高亮不变。
  const auto &panel = layout.panel;
  const double card_alpha = std::clamp(assets.opacity, 0.0, 1.0);
  const double radius = metrics.card_radius;
  native_canvas_set_color(cr, color("surface", 0xf8f8f8), card_alpha);
  native_canvas_round_rect(cr, panel, {radius, radius, radius, radius});
  cairo_fill(cr);
  // 皮肤背景图：cover 等比铺满后居中裁切，contain 等比完整放进并居中，stretch 两个方向各自拉伸（macOS 的 BackgroundRects），裁在卡片圆角里。
  if (auto *image = assets.background.image.get(); image && panel.width > 0 && panel.height > 0) {
    const double width = cairo_image_surface_get_width(image), height = cairo_image_surface_get_height(image);
    double scale_x = panel.width / width, scale_y = panel.height / height;
    if (assets.background.fit == CandidateSkinBackgroundFit::cover) scale_x = scale_y = std::max(scale_x, scale_y);
    else if (assets.background.fit == CandidateSkinBackgroundFit::contain) scale_x = scale_y = std::min(scale_x, scale_y);
    cairo_save(cr);
    native_canvas_round_rect(cr, panel, {radius, radius, radius, radius});
    cairo_clip(cr);
    cairo_translate(cr, panel.x + (panel.width - width * scale_x) / 2, panel.y + (panel.height - height * scale_y) / 2);
    cairo_scale(cr, scale_x, scale_y);
    // 只画图片本身的范围（contain 时两边留出底色）；PAD 让放大时边缘像素不被插值成半透明。
    cairo_rectangle(cr, 0, 0, width, height);
    cairo_clip(cr);
    cairo_set_source_surface(cr, image, 0, 0);
    cairo_pattern_set_filter(cairo_get_source(cr), CAIRO_FILTER_GOOD);
    cairo_pattern_set_extend(cairo_get_source(cr), CAIRO_EXTEND_PAD);
    cairo_paint_with_alpha(cr, std::clamp(assets.background.opacity, 0.0, 1.0) * card_alpha);
    cairo_restore(cr);
  }
  if (assets.border_width > 0) {
    if (const auto border = optional_color("border")) {
      native_canvas_set_color(cr, *border, card_alpha);
      const double half = assets.border_width / 2, inner = std::max(0.0, radius - half);
      native_canvas_round_rect(cr, {panel.x + half, panel.y + half, std::max(0.0, panel.width - 2 * half),
          std::max(0.0, panel.height - 2 * half)}, {inner, inner, inner, inner});
      cairo_set_line_width(cr, assets.border_width);
      cairo_stroke(cr);
    }
  }
  // 首行。
  if (content.logo && layout.logo.width > 0) {
    auto *image = content.logo.get();
    const double width = cairo_image_surface_get_width(image), height = cairo_image_surface_get_height(image);
    cairo_save(cr);
    cairo_translate(cr, layout.logo.x, layout.logo.y);
    cairo_scale(cr, layout.logo.width / std::max(1.0, width), layout.logo.height / std::max(1.0, height));
    cairo_set_source_surface(cr, image, 0, 0);
    cairo_pattern_set_filter(cairo_get_source(cr), CAIRO_FILTER_GOOD);
    cairo_paint(cr);
    cairo_restore(cr);
  }
  if (layout.preedit.width > 0 && !content.preedit.empty()) {
    const auto &field = layout.preedit;
    // 拼音画成强调色（皮肤给了 preedit_text 时用它），光标是 1.25pt 的竖线；光标处让出一小段空隙，与 macOS 的 MSIMECandidatePreeditField 相同。
    const auto reading = optional_color("preedit_text").value_or(accent);
    const auto caret_color = optional_color("preedit_caret").value_or(accent);
    const bool caret = content.preedit_cursor >= 0 && static_cast<std::size_t>(content.preedit_cursor) <= content.preedit.size() &&
        (static_cast<std::size_t>(content.preedit_cursor) == content.preedit.size() ||
         (static_cast<unsigned char>(content.preedit[content.preedit_cursor]) & 0xc0) != 0x80);
    const auto cursor = caret ? static_cast<std::size_t>(content.preedit_cursor) : content.preedit.size();
    const auto before = content.preedit.substr(0, cursor), after = content.preedit.substr(cursor);
    const double before_width = native_canvas_text_size(cr, layout.preedit_font, before).first;
    const double offset = before_width + (caret ? (cursor < content.preedit.size() ? shared::kCandidatePreeditCaretSideAir
                                                                                   : shared::kCandidatePreeditCaretEndAir) : 0);
    const double available = std::max(0.0, field.width - 4.0 - shared::kCandidatePreeditCaretWidth);
    const double origin = 2.0 - std::max(0.0, offset - available);
    auto *line = native_canvas_text_layout(cr, layout.preedit_font, content.preedit);
    int line_width = 0, line_height = 0;
    pango_layout_get_pixel_size(line, &line_width, &line_height);
    g_object_unref(line);
    const double top = field.y + std::max(0.0, (field.height - line_height) / 2);
    cairo_save(cr);
    cairo_rectangle(cr, field.x, field.y, field.width, field.height);
    cairo_clip(cr);
    native_canvas_set_color(cr, reading);
    for (const auto &[piece, x] : {std::pair<std::string, double>{before, 0},
                                   std::pair<std::string, double>{after, before_width + (caret && !after.empty() ? shared::kCandidatePreeditCaretGap : 0)}}) {
      if (piece.empty()) continue;
      auto *part = native_canvas_text_layout(cr, layout.preedit_font, piece);
      cairo_move_to(cr, field.x + origin + x, top);
      pango_cairo_show_layout(cr, part);
      g_object_unref(part);
    }
    if (caret) {
      native_canvas_set_color(cr, caret_color);
      cairo_rectangle(cr, field.x + origin + offset, top, shared::kCandidatePreeditCaretWidth, std::min<double>(line_height, field.height));
      cairo_fill(cr);
    }
    cairo_restore(cr);
  }
  const double control_radius = metrics.item_radius;
  const auto control = [&](const NativeCanvasRect &box, int part, bool enabled) {
    if (!enabled || box.width <= 0) return;
    if (pointer.pressed == part || pointer.hovered == part) {
      native_canvas_set_color(cr, pointer.pressed == part ? selected_fill : hover_fill);
      native_canvas_round_rect(cr, box, {control_radius, control_radius, control_radius, control_radius});
      cairo_fill(cr);
    }
  };
  if (layout.page_controls) {
    native_canvas_set_color(cr, number);
    auto *line = native_canvas_text_layout(cr, layout.indicator_font, layout.indicator_text);
    auto *attributes = pango_attr_list_new();
    pango_attr_list_insert(attributes, pango_attr_font_features_new("tnum"));
    pango_layout_set_attributes(line, attributes);
    pango_attr_list_unref(attributes);
    cairo_move_to(cr, layout.indicator.x, layout.indicator.y);
    pango_cairo_show_layout(cr, line);
    g_object_unref(line);
    if (content.grid_columns == 0) {
      for (const bool next : {false, true}) {
        const auto &box = next ? layout.next : layout.previous;
        const bool enabled = next ? content.next : content.previous;
        control(box, next ? kNativeCanvasHitNext : kNativeCanvasHitPrevious, enabled);
        native_canvas_set_color(cr, number, enabled ? 1.0 : shared::kCandidateDisabledArrowAlpha);
        native_canvas_chrome_glyph(cr, layout.symbol_font, next ? "›" : "‹", box);
      }
    }
  }
  if (layout.expand.width > 0) {
    control(layout.expand, kNativeCanvasHitExpand, true);
    native_canvas_set_color(cr, number);
    native_canvas_chrome_chevron(cr, layout.expand, s, content.grid_columns > 0);
  }
  // 候选行。
  const bool follow_card = metrics.corners_follow_card;
  for (std::size_t at = 0; at < content.items.size() && at < layout.items.size(); ++at) {
    const auto &item = content.items[at];
    const auto &box = layout.items[at];
    const auto &row = layout.rows[at];
    const bool highlighted = static_cast<int>(at) == content.selected || static_cast<int>(at) == pointer.pressed;
    const bool hovered = !highlighted && static_cast<int>(at) == pointer.hovered;
    const auto reveal = at < motion.rows.size() ? motion.rows[at] : shared::CardMotionFrame{};
    if (reveal.opacity <= 0) continue;
    const bool revealing = reveal.opacity < 1 || reveal.lift != 0;
    if (revealing) {
      cairo_push_group(cr);
      cairo_translate(cr, 0, -reveal.lift);
    }
    const NativeCanvasRect fill_box{box.x + 1, box.y + 1, std::max(0.0, box.width - 2), std::max(0.0, box.height - 2)};
    const auto corners = native_canvas_chrome_corners(layout, at, metrics.item_radius, metrics.card_radius, follow_card);
    if (highlighted && selected_fill.alpha > 2) {
      const auto frame = static_cast<int>(at) == content.selected ? motion.selection : shared::SelectionMotionFrame{};
      cairo_save(cr);
      if (frame.scale != 1) {
        cairo_translate(cr, fill_box.x + fill_box.width / 2, fill_box.y + fill_box.height / 2);
        cairo_scale(cr, frame.scale, frame.scale);
        cairo_translate(cr, -(fill_box.x + fill_box.width / 2), -(fill_box.y + fill_box.height / 2));
      }
      native_canvas_set_color(cr, selected_fill, frame.fillOpacity);
      native_canvas_round_rect(cr, fill_box, corners);
      cairo_fill(cr);
      if (frame.ringOpacity > 0) {
        native_canvas_set_color(cr, optional_color("selected_bar").value_or(accent), frame.ringOpacity);
        native_canvas_round_rect(cr, fill_box, corners);
        cairo_set_line_width(cr, 2);
        cairo_stroke(cr);
      }
      cairo_restore(cr);
    } else if (hovered && hover_fill.alpha > 2) {
      native_canvas_set_color(cr, hover_fill);
      native_canvas_round_rect(cr, fill_box, corners);
      cairo_fill(cr);
    }
    if (highlighted && assets.show_selected_bar) {
      const double bar_width = metrics.bar_width, bar_height = metrics.bar_height;
      const double x = !layout.skin_layout || metrics.bar_inside ? std::max(1.0, s) : -bar_width / 2;
      native_canvas_set_color(cr, optional_color("selected_bar").value_or(accent));
      native_canvas_round_rect(cr, {box.x + x, box.y + (box.height - bar_height) / 2, bar_width, bar_height},
          {bar_width / 2, bar_width / 2, bar_width / 2, bar_width / 2});
      cairo_fill(cr);
    }
    // 正文、序号和释义：正文在 textHeight 的框里竖直居中，序号落在正文第一行上。
    auto title = highlighted ? selected_text : text;
    if (item.fixed && !highlighted) title = ThemeColor{kCandidateFixedPositionColor, 255};
    if (item.pending) title = highlighted ? selected_number : number;
    const auto number_color = highlighted ? selected_number : number;
    const double content_left = box.x + layout.content_lefts[at];
    auto *word = native_canvas_text_layout(cr, metrics.font, item.text, row.textWrapped ? row.textWidth : -1);
    // 光标后编辑：首选开头固定的那几个字用这一行序号的颜色画淡（选中行取选中序号色），只改颜色，排版和测量不变。
    if (const auto dim_end = std::min(item.dim_prefix_bytes, item.text.size()); dim_end > 0 && !item.pending) {
      const auto &dim = number_color;
      auto *attributes = pango_attr_list_new();
      for (auto *attribute : {pango_attr_foreground_new(((dim.rgb >> 16) & 255) * 257, ((dim.rgb >> 8) & 255) * 257,
               (dim.rgb & 255) * 257), pango_attr_foreground_alpha_new(static_cast<guint16>(dim.alpha * 257))}) {
        attribute->start_index = 0;
        attribute->end_index = static_cast<guint>(dim_end);
        pango_attr_list_insert(attributes, attribute);
      }
      pango_layout_set_attributes(word, attributes);
      pango_attr_list_unref(attributes);
    }
    int word_width = 0, word_height = 0;
    pango_layout_get_pixel_size(word, &word_width, &word_height);
    const double text_top = box.y + std::max(0.0, (row.textHeight - word_height) / 2);
    const double first_line = row.textWrapped ? native_canvas_line_height(cr, metrics.font) : word_height;
    native_canvas_set_color(cr, title);
    cairo_move_to(cr, content_left, text_top);
    pango_cairo_show_layout(cr, word);
    g_object_unref(word);
    if (metrics.number_scale > 0 && !item.label.empty()) {
      const auto label = native_canvas_text_size(cr, metrics.number_font, item.label);
      native_canvas_set_color(cr, number_color);
      auto *line = native_canvas_text_layout(cr, metrics.number_font, item.label);
      cairo_move_to(cr, box.x + layout.text_left, text_top + (first_line - label.second) / 2);
      pango_cairo_show_layout(cr, line);
      g_object_unref(line);
    }
    const auto gloss = native_canvas_gloss(item);
    if (!gloss.text.empty() && row.translation.width > 0) {
      // 释义色：选中行用选中释义色，否则用皮肤的释义色；都没有时固定排位的行取正文色的 0.62，其余取次要色（序号色）。
      std::optional<ThemeColor> gloss_color = highlighted ? optional_color("selected_translation") : std::nullopt;
      if (!gloss_color) gloss_color = optional_color("translation");
      if (!gloss_color && item.fixed) {
        gloss_color = title;
        gloss_color->alpha = static_cast<std::uint8_t>(std::round(gloss_color->alpha * shared::kCandidateTranslationOpacity));
      }
      if (!gloss_color) gloss_color = number_color;
      const auto &run = row.translation;
      auto *line = native_canvas_gloss_layout(cr, layout.gloss_font, item, gloss, *gloss_color, palette, highlighted,
                                              run.below ? run.width : -1);
      int gloss_width = 0, gloss_height = 0;
      pango_layout_get_pixel_size(line, &gloss_width, &gloss_height);
      const double drawn = std::min<double>(run.height, gloss_height);
      cairo_save(cr);
      cairo_rectangle(cr, box.x, box.y, box.width, box.height);
      cairo_clip(cr);
      cairo_move_to(cr, content_left + run.x, box.y + run.y + std::max(0.0, (run.height - drawn) / 2));
      pango_cairo_show_layout(cr, line);
      cairo_restore(cr);
      g_object_unref(line);
    }
    if (revealing) {
      cairo_pop_group_to_source(cr);
      cairo_paint_with_alpha(cr, reveal.opacity);
    }
  }
  if (!content.auxiliary.empty() && layout.auxiliary.width > 0) {
    native_canvas_set_color(cr, optional_color("secondary").value_or(number));
    auto *line = native_canvas_text_layout(cr, layout.gloss_font, content.auxiliary, layout.auxiliary_width);
    cairo_move_to(cr, layout.auxiliary.x, layout.auxiliary.y);
    pango_cairo_show_layout(cr, line);
    g_object_unref(line);
  }
  if (!content.auxiliary_hint.empty() && layout.auxiliary_hint.width > 0) {
    native_canvas_set_color(cr, optional_color("secondary").value_or(number));
    auto *line = native_canvas_text_layout(cr, layout.gloss_font, content.auxiliary_hint, layout.auxiliary_hint.width);
    cairo_move_to(cr, layout.auxiliary_hint.x, layout.auxiliary_hint.y);
    pango_cairo_show_layout(cr, line);
    g_object_unref(line);
  }
  // 顶带装饰最后画，压在卡片顶边和首行上；待机动效以图片底边中点为轴。
  if (assets.band.image && layout.band.width > 0) {
    const auto &animation = *assets.band.image;
    auto *image = animation.frames[sample_native_canvas_animation(animation, elapsed_ms).index].get();
    const auto &box = layout.band;
    const auto &frame = motion.decoration;
    cairo_save(cr);
    const double pivot_x = box.x + box.width / 2, pivot_y = box.y + box.height;
    cairo_translate(cr, pivot_x, pivot_y - frame.lift);
    cairo_rotate(cr, -frame.angle);
    cairo_scale(cr, frame.scale, frame.scale);
    cairo_translate(cr, -pivot_x, -pivot_y);
    native_canvas_image(cr, image, {0, 0, static_cast<double>(cairo_image_surface_get_width(image)),
        static_cast<double>(cairo_image_surface_get_height(image))}, box);
    if (layout.overlay && assets.band.candidate) {
      const auto &overlay = *layout.overlay;
      auto *line = native_canvas_overlay_line(cr, layout.metrics.font, overlay.text, overlay.font_size);
      native_canvas_color(cr, {{"text", assets.band.candidate->color}}, "text", "#FFFFFFFF");
      cairo_move_to(cr, overlay.box.x, overlay.box.y);
      pango_cairo_show_layout(cr, line);
      g_object_unref(line);
    }
    cairo_restore(cr);
  }
}

// 菜单（右键的候选操作）仍是简单列表，两种画法共用 canvas 那套。`panel_height_limit` 只约束 canvas 包展开的卷轴（视口变矮、可以滚动）；chrome 的网格按可见行数定高，不读它。
inline NativeCanvasLayout measure_native_canvas(cairo_t *cr, const NativeCanvasAssets &assets,
    const NativeCanvasContent &content, double panel_width_limit = -1, double panel_height_limit = -1) {
  if (assets.chrome && !content.menu) return measure_native_canvas_chrome(cr, assets, content, panel_width_limit);
  return measure_native_canvas_skin(cr, assets, content, panel_width_limit, panel_height_limit);
}

inline int native_canvas_hit(const NativeCanvasContent &content, const NativeCanvasLayout &layout, double x, double y) {
  return layout.chrome ? native_canvas_hit_chrome(content, layout, x, y) : native_canvas_hit_skin(content, layout, x, y);
}

inline void paint_native_canvas(cairo_t *cr, const NativeCanvasAssets &assets, const NativeCanvasContent &content,
                                const NativeCanvasLayout &layout, const nlohmann::json &palette,
                                std::uint64_t elapsed_ms = 0, NativeCanvasPointer pointer = {},
                                const NativeCanvasMotion &motion = {}) {
  if (layout.chrome) paint_native_canvas_chrome(cr, assets, content, layout, palette, elapsed_ms, pointer, motion);
  else paint_native_canvas_skin(cr, assets, content, layout, palette, elapsed_ms, pointer);
}

} // msime::linux_host 命名空间
