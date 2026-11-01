#pragma once
// 候选皮肤截图工具（main.cpp）里不碰 Win32 的部分：截图的文件名、假的候选页和后续候选、判断一张截图是不是空白。windows-candidate-skin-gallery-plan 在任何平台上都能测它们。
#include "CandidatePresentation.h"
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdlib>
#include <string>
#include <vector>

namespace msime::windows::gallery {
// 一张截图：哪款皮肤（空是不带皮肤的基线）、横排还是竖排、浅色还是深色、卷轴展开是否打开。
struct GalleryShot {
  std::string skin;
  bool horizontal = true;
  bool dark = false;
  bool expanded = false;
};

// 基线截图的文件名前缀。它也是一个合法的皮肤 id，截图工具遇到叫这个名字的皮肤目录时直接报错，免得两组截图写进同一批文件。
inline constexpr const char *baseline_name = "no-skin";

// `<皮肤 id 或 no-skin>__<horizontal|vertical>-<light|dark>[-expanded].png`。两个下划线隔开 id，id 自己可以带一个下划线或连字符。
inline std::string shot_file_name(const GalleryShot &shot) {
  std::string name = shot.skin.empty() ? std::string(baseline_name) : shot.skin;
  name += "__";
  name += shot.horizontal ? "horizontal" : "vertical";
  name += shot.dark ? "-dark" : "-light";
  if (shot.expanded)
    name += "-expanded";
  return name + ".png";
}

// 日志里说这张截图时用的标签，和文件名同一套字段。
inline std::string shot_label(const GalleryShot &shot) {
  std::string label = shot.skin.empty() ? std::string(baseline_name) : shot.skin;
  label += shot.horizontal ? " horizontal" : " vertical";
  label += shot.dark ? " dark" : " light";
  if (shot.expanded)
    label += " expanded";
  return label;
}

// 假的这一页：拼音 shuru 的九个候选，每个带一行英文释义，第一页、共三页，第一个高亮。全部是合成的，不是任何人的输入。
inline constexpr std::array<const char *, 9> page_texts = {"输入", "书", "数", "树", "熟",
                                                          "术", "属", "鼠", "舒"};
inline constexpr std::array<const char *, 9> page_translations = {
    "input", "book", "number", "tree", "ripe", "technique", "belong", "mouse", "relaxed"};
// 前两个候选带读音（打开「显示读音」时翻译工作线程算出来的那种），截图里看得到读音在释义下面单独一行、按释义色减淡；其余的没有读音，横排时和它们同一行的格子照样留出这一行。
inline constexpr std::array<const char *, 9> page_readings = {"/ˈɪnpʊt/", "/bʊk/", "", "", "", "", "", "", ""};
// 卷轴展开网格里排在这一页之后的候选，长短不一，够排出好几行、出现滚动条。
inline constexpr std::array<const char *, 36> later_texts = {
    "输", "叔", "殊", "梳", "蔬", "枢", "淑", "疏", "抒", "暑", "署", "曙",
    "薯", "竖", "恕", "墅", "述", "束", "书法", "数据", "树木", "熟悉", "属于", "舒服",
    "输入法", "书架", "数学", "树叶", "熟练", "术语", "鼠标", "舒适", "叔叔", "疏忽", "暑假", "输出"};
inline constexpr uint64_t gallery_session = 1;
inline constexpr uint64_t gallery_generation = 1;

// Engine view 的形状（msime_client 的 View），candidate_presentation_from_view 照读：全拼方案、不在任何本地模式里。
inline nlohmann::json gallery_view() {
  nlohmann::json candidates = nlohmann::json::array();
  for (size_t index = 0; index < page_texts.size(); ++index)
    candidates.push_back({{"id",
                           {{"session", gallery_session},
                            {"generation", gallery_generation},
                            {"index", index}}},
                          {"text", page_texts[index]},
                          {"highlighted", index == 0},
                          {"translation", page_translations[index]}});
  return {{"session", gallery_session},
          {"generation", gallery_generation},
          {"focused", true},
          {"editing_text", "shuru"},
          {"preedit", "shuru"},
          {"caret_position", 5},
          {"scheme", 0},
          {"local_mode", "none"},
          {"dedicated_english", false},
          {"page", 0},
          {"page_count", 3},
          {"page_size", page_texts.size()},
          {"candidates", std::move(candidates)}};
}

// 截图用的候选快照，锚点 (x, y) 是物理像素的光标底边左端。经和 Server 同一个投影（candidate_presentation_from_view）得到，再补上一个固定的租约和快照编号。
inline CandidatePresentation gallery_presentation(int x, int y) {
  FocusLease lease;
  lease.transport.client = 1;
  lease.epoch = 1;
  lease.token = 1;
  auto value = candidate_presentation_from_view(lease, gallery_view(), x, y, std::string{});
  CandidateReadings readings;
  for (size_t index = 0; index < page_texts.size(); ++index)
    if (*page_readings[index])
      readings[page_texts[index]] = {page_translations[index], page_readings[index]};
  attach_candidate_readings(value.candidates, readings);
  value.render_serial = 1;
  return value;
}

// 卷轴展开打开后 Server 交回的这一代完整候选（msime_client_all_candidates 解析后的样子）：这一页的九个在前，后续候选的下标接着往下。
inline std::vector<ExpandedCandidate> gallery_expanded_candidates() {
  std::vector<ExpandedCandidate> items;
  items.reserve(page_texts.size() + later_texts.size());
  for (size_t index = 0; index < page_texts.size(); ++index)
    items.push_back({index, page_texts[index]});
  for (size_t index = 0; index < later_texts.size(); ++index)
    items.push_back({page_texts.size() + index, later_texts[index]});
  return items;
}

// 截图和底色比有多少像素不同。pixels 是逐行的 BGRA（每像素四字节，alpha 不看），backdrop 是 0xRRGGBB；任一通道差超过 tolerance 就算不同。
struct CaptureVerdict {
  size_t differing = 0;
  size_t total = 0;
  double fraction() const { return total ? static_cast<double>(differing) / static_cast<double>(total) : 0.0; }
};
// 一个 BGRA 像素和底色（0xRRGGBB）是否不同：任一通道差超过 tolerance。
inline bool pixel_differs(const uint8_t *bgra, uint32_t backdrop, int tolerance) {
  return std::abs(bgra[0] - static_cast<int>(backdrop & 0xff)) > tolerance ||
         std::abs(bgra[1] - static_cast<int>((backdrop >> 8) & 0xff)) > tolerance ||
         std::abs(bgra[2] - static_cast<int>((backdrop >> 16) & 0xff)) > tolerance;
}
inline CaptureVerdict capture_verdict(const std::vector<uint8_t> &pixels, uint32_t backdrop,
                                      int tolerance = 12) {
  CaptureVerdict verdict;
  for (size_t offset = 0; offset + 3 < pixels.size(); offset += 4) {
    ++verdict.total;
    if (pixel_differs(&pixels[offset], backdrop, tolerance))
      ++verdict.differing;
  }
  return verdict;
}
// 两个 BGRA 像素是否不同：任一颜色通道差超过 tolerance，alpha 不看。
inline bool pixel_differs(const uint8_t *bgra, const uint8_t *reference, int tolerance) {
  return std::abs(bgra[0] - reference[0]) > tolerance || std::abs(bgra[1] - reference[1]) > tolerance ||
         std::abs(bgra[2] - reference[2]) > tolerance;
}
// 和同一块屏幕在候选窗出现之前的样子（reference，同样大小的 BGRA）比，有多少像素不同。底色是条纹而不是纯色时（要系统材质的皮肤，见 backdrop_stripe_color）用它代替和底色比；大小对不上时一个像素也不算。
inline CaptureVerdict capture_verdict(const std::vector<uint8_t> &pixels, const std::vector<uint8_t> &reference,
                                      int tolerance = 12) {
  CaptureVerdict verdict;
  if (reference.size() != pixels.size())
    return verdict;
  for (size_t offset = 0; offset + 3 < pixels.size(); offset += 4) {
    ++verdict.total;
    if (pixel_differs(&pixels[offset], &reference[offset], tolerance))
      ++verdict.differing;
  }
  return verdict;
}
// 窗口最外一圈像素里和底色不同的比例，四条边分开算。窗口四周是阴影边距，阴影到边上已经淡没了，最外一圈应该就是底色；卡片的底色、外框或文字出现在这一圈上，说明卡片画到了窗口边上、被窗口裁掉了。
struct EdgeBleed {
  double left = 0.0, top = 0.0, right = 0.0, bottom = 0.0;
  double worst() const { return (std::max)((std::max)(left, right), (std::max)(top, bottom)); }
};
namespace detail {
// differs(x, y) 说这个像素和后面的画面是否不同；四条边分开数。
template <class Differs> EdgeBleed edge_bleed(int width, int height, Differs differs) {
  EdgeBleed bleed;
  const auto column = [&](int x) {
    size_t count = 0;
    for (int y = 0; y < height; ++y)
      count += differs(x, y) ? 1 : 0;
    return static_cast<double>(count) / static_cast<double>(height);
  };
  const auto row = [&](int y) {
    size_t count = 0;
    for (int x = 0; x < width; ++x)
      count += differs(x, y) ? 1 : 0;
    return static_cast<double>(count) / static_cast<double>(width);
  };
  bleed.left = column(0);
  bleed.right = column(width - 1);
  bleed.top = row(0);
  bleed.bottom = row(height - 1);
  return bleed;
}
inline size_t pixel_offset(int width, int x, int y) {
  return (static_cast<size_t>(y) * static_cast<size_t>(width) + static_cast<size_t>(x)) * 4;
}
} // namespace detail
inline EdgeBleed edge_bleed(const std::vector<uint8_t> &pixels, int width, int height,
                            uint32_t backdrop, int tolerance = 12) {
  if (width <= 0 || height <= 0 ||
      pixels.size() < static_cast<size_t>(width) * static_cast<size_t>(height) * 4)
    return {};
  return detail::edge_bleed(width, height, [&](int x, int y) {
    return pixel_differs(&pixels[detail::pixel_offset(width, x, y)], backdrop, tolerance);
  });
}
// 同上，和候选窗出现之前同一块屏幕的样子比（条纹底色）。
inline EdgeBleed edge_bleed(const std::vector<uint8_t> &pixels, const std::vector<uint8_t> &reference, int width,
                            int height, int tolerance = 12) {
  if (width <= 0 || height <= 0 ||
      pixels.size() < static_cast<size_t>(width) * static_cast<size_t>(height) * 4 ||
      reference.size() != pixels.size())
    return {};
  return detail::edge_bleed(width, height, [&](int x, int y) {
    const auto offset = detail::pixel_offset(width, x, y);
    return pixel_differs(&pixels[offset], &reference[offset], tolerance);
  });
}
// 最外一圈里不同的像素超过这个比例就算卡片被裁：画到边上的卡片占满那一边的大半，淡阴影和抗锯齿的零星像素远不到这个量。
inline constexpr double maximum_edge_fraction = 0.1;
// 候选窗的整块窗口（含四周透明的阴影边距）里，和底色不同的像素不到这个比例就当作没画出来：截到的只是底色，或者整块是一种颜色。哪怕皮肤的底色恰好和底色一样，候选文字、序号和外框也远超过这个量。
inline constexpr double minimum_drawn_fraction = 0.005;
inline bool capture_blank(const CaptureVerdict &verdict) {
  return verdict.total == 0 || verdict.fraction() < minimum_drawn_fraction;
}
// 像素矩形，不用 Win32 的 RECT，这样纯逻辑在任何平台上都能测。
struct PixelRect {
  long left = 0, top = 0, right = 0, bottom = 0;
};
// 从 source（source_width x source_height 的逐行 BGRA）裁出 rect；rect 为空、越界或 source 大小对不上时返回空。
inline std::vector<uint8_t> crop(const std::vector<uint8_t> &source, int source_width, int source_height,
                                 const PixelRect &rect) {
  if (source_width <= 0 || source_height <= 0 ||
      source.size() != static_cast<size_t>(source_width) * static_cast<size_t>(source_height) * 4 ||
      rect.left < 0 || rect.top < 0 || rect.right > source_width || rect.bottom > source_height ||
      rect.right <= rect.left || rect.bottom <= rect.top)
    return {};
  std::vector<uint8_t> result;
  const auto row_bytes = static_cast<size_t>(rect.right - rect.left) * 4;
  result.reserve(row_bytes * static_cast<size_t>(rect.bottom - rect.top));
  for (long y = rect.top; y < rect.bottom; ++y) {
    const auto begin = source.begin() + static_cast<std::ptrdiff_t>(
                                            detail::pixel_offset(source_width, static_cast<int>(rect.left),
                                                                 static_cast<int>(y)));
    result.insert(result.end(), begin, begin + static_cast<std::ptrdiff_t>(row_bytes));
  }
  return result;
}
// 要系统材质（`backdrop`）的皮肤截图时铺在候选窗后面的竖条纹，每条 stripe_width 像素宽，在几种饱和色之间循环，返回 0xRRGGBB。纯色底上看不出模糊；条纹在亚克力卡片后面糊成一片混色，卡片外面仍是清楚的条纹，截图里一眼就能看出材质垫没垫上。深色截图用暗一些的一组。
inline constexpr int stripe_width = 24;
inline uint32_t backdrop_stripe_color(int x, bool dark) {
  static constexpr std::array<uint32_t, 6> light = {0xE53935, 0xFDD835, 0x43A047, 0x1E88E5, 0x8E24AA, 0xFFFFFF};
  static constexpr std::array<uint32_t, 6> darker = {0x8E1B1B, 0x8A7400, 0x1B5E20, 0x0D47A1, 0x4A148C, 0x000000};
  const int stripe = (x >= 0 ? x : -x) / stripe_width;
  return (dark ? darker : light)[static_cast<size_t>(stripe) % light.size()];
}
// 上一帧画出的卡片（CandidateWindow::painted_card）是否落在客户区里，且和按窗口尺寸推出的卡片（card_on_screen 换到客户区坐标）每条边相差不超过 tolerance 像素。画卡片用的尺寸和定窗口用的尺寸不是同一个时，这里不成立。
inline bool card_fits(const PixelRect &painted, long client_width, long client_height,
                      const PixelRect &expected, long tolerance = 2) {
  const auto near = [tolerance](long a, long b) { return std::labs(a - b) <= tolerance; };
  return painted.left >= 0 && painted.top >= 0 && painted.right <= client_width &&
         painted.bottom <= client_height && painted.right > painted.left &&
         painted.bottom > painted.top && near(painted.left, expected.left) &&
         near(painted.top, expected.top) && near(painted.right, expected.right) &&
         near(painted.bottom, expected.bottom);
}
} // namespace msime::windows::gallery
