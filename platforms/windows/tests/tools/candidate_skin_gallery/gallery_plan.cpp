// 候选皮肤截图工具里不碰 Win32 的部分（CandidateSkinGallery.h）：文件名、假候选页经 Server 同一个投影后的样子、展开网格的后续候选，以及空白截图的判断。
#include "CandidateSkinGallery.h"
#include <iostream>
#include <set>
#include <stdexcept>
#include <string>

using namespace msime::windows;
using namespace msime::windows::gallery;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Candidate skin gallery check failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)

void file_names() {
  require(shot_file_name({"", true, false, false}) == "no-skin__horizontal-light.png");
  require(shot_file_name({"verify-basic", false, true, false}) == "verify-basic__vertical-dark.png");
  require(shot_file_name({"verify.scroll_1", true, true, true}) ==
          "verify.scroll_1__horizontal-dark-expanded.png");
  require(shot_label({"verify-basic", true, false, true}) == "verify-basic horizontal light expanded");
  require(shot_label({"", false, true, false}) == "no-skin vertical dark");
  // 一款皮肤的每种排列、明暗和展开状态都落在不同的文件上。
  std::set<std::string> names;
  for (const bool horizontal : {true, false})
    for (const bool dark : {false, true})
      for (const bool expanded : {false, true})
        names.insert(shot_file_name({"verify-basic", horizontal, dark, expanded}));
  require(names.size() == 8);
}

void presentation() {
  const auto value = gallery_presentation(64, 112);
  require(value.visible);
  require(value.x == 64 && value.y == 112);
  require(value.preedit == "shuru");
  require(value.preedit_caret == 5);
  require(value.candidates.size() == 9);
  require(value.page == 0 && value.page_count == 3 && value.page_size == 9);
  require(value.pointer_input);
  require(value.shows_glosses);
  require(!value.shuangpin_keymap);
  require(value.render_serial == 1);
  require(value.lease.epoch == 1 && value.lease.token == 1);
  require(!value.expanded && !value.expansion_open);
  size_t highlighted = 0;
  for (size_t index = 0; index < value.candidates.size(); ++index) {
    const auto &candidate = value.candidates[index];
    require(candidate.index == index);
    require(candidate.session == value.session && candidate.generation == value.generation);
    require(candidate.text == page_texts[index]);
    require(candidate.translation == page_translations[index]);
    require(candidate.badge.empty() && candidate.annotation.empty() && candidate.wubi_code_hint.empty());
    require(candidate_secondary_text(candidate) == page_translations[index]);
    highlighted += candidate.highlighted;
  }
  require(highlighted == 1 && value.candidates.front().highlighted);
}

void expanded() {
  const auto items = gallery_expanded_candidates();
  require(items.size() == page_texts.size() + later_texts.size());
  for (size_t index = 0; index < items.size(); ++index) {
    require(items[index].index == index);
    require(!items[index].text.empty());
  }
  // 网格从这一页最后一个候选之后排起：第一格是后续候选的第一个。
  const auto start = candidate_expand_start(items, page_texts.size() - 1);
  require(start == page_texts.size());
  require(items[start].text == later_texts.front());
}

void blank_capture() {
  constexpr uint32_t backdrop = 0x202020;
  // 100 个像素全是底色（BGRA），或只差一点点：空白。
  std::vector<uint8_t> pixels;
  for (int index = 0; index < 100; ++index)
    pixels.insert(pixels.end(), {0x20, 0x21, 0x1f, 0xff});
  auto verdict = capture_verdict(pixels, backdrop);
  require(verdict.total == 100 && verdict.differing == 0);
  require(capture_blank(verdict));
  // 一个像素不同（1%）就不算空白；alpha 不参与比较。
  pixels[3] = 0x00;
  require(capture_verdict(pixels, backdrop).differing == 0);
  pixels[2] = 0xff;
  verdict = capture_verdict(pixels, backdrop);
  require(verdict.differing == 1);
  require(!capture_blank(verdict));
  // 通道按 BGRA 读：只有蓝通道不同也算。
  pixels[2] = 0x20;
  pixels[4] = 0x80;
  require(capture_verdict(pixels, backdrop).differing == 1);
  // 2000 个像素里只有一个不同（0.05%）仍是空白，没有像素也是。
  std::vector<uint8_t> large;
  for (int index = 0; index < 2000; ++index)
    large.insert(large.end(), {0x20, 0x20, 0x20, 0xff});
  large[0] = 0xff;
  require(capture_blank(capture_verdict(large, backdrop)));
  require(capture_blank(capture_verdict({}, backdrop)));
}
} // namespace

int main() {
  try {
    file_names();
    presentation();
    expanded();
    blank_capture();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "candidate skin gallery plan ok\n";
  return 0;
}
