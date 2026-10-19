#pragma once
// 候选皮肤截图工具（main.cpp）里不碰 Win32 的部分：截图的文件名、假的候选页和后续候选、判断一张截图是不是空白。windows-candidate-skin-gallery-plan 在任何平台上都能测它们。
#include "CandidatePresentation.h"
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
inline CaptureVerdict capture_verdict(const std::vector<uint8_t> &pixels, uint32_t backdrop,
                                      int tolerance = 12) {
  CaptureVerdict verdict;
  const int red = static_cast<int>((backdrop >> 16) & 0xff);
  const int green = static_cast<int>((backdrop >> 8) & 0xff);
  const int blue = static_cast<int>(backdrop & 0xff);
  for (size_t offset = 0; offset + 3 < pixels.size(); offset += 4) {
    ++verdict.total;
    if (std::abs(pixels[offset] - blue) > tolerance ||
        std::abs(pixels[offset + 1] - green) > tolerance ||
        std::abs(pixels[offset + 2] - red) > tolerance)
      ++verdict.differing;
  }
  return verdict;
}
// 候选窗的整块窗口（含四周透明的阴影边距）里，和底色不同的像素不到这个比例就当作没画出来：截到的只是底色，或者整块是一种颜色。哪怕皮肤的底色恰好和底色一样，候选文字、序号和外框也远超过这个量。
inline constexpr double minimum_drawn_fraction = 0.005;
inline bool capture_blank(const CaptureVerdict &verdict) {
  return verdict.total == 0 || verdict.fraction() < minimum_drawn_fraction;
}
} // namespace msime::windows::gallery
