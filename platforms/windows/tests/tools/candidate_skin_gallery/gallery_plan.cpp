// 候选皮肤截图工具里不碰 Win32 的部分（CandidateSkinGallery.h）：文件名、假候选页经 Server 同一个投影后的样子、展开网格的后续候选，以及空白截图的判断。
#include "CandidateSkinGallery.h"
#include <algorithm>
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
    require(candidate.pronunciation == page_readings[index]);
    require(candidate_secondary_text(candidate) ==
            (*page_readings[index] ? std::string(page_translations[index]) + "\n" + page_readings[index]
                                   : std::string(page_translations[index])));
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

void edges() {
  constexpr uint32_t backdrop = 0xE6E6E6;
  // 20 x 12 的截图全是底色：四条边都干净。
  constexpr int width = 20, height = 12;
  std::vector<uint8_t> pixels;
  for (int index = 0; index < width * height; ++index)
    pixels.insert(pixels.end(), {0xe6, 0xe6, 0xe6, 0xff});
  require(edge_bleed(pixels, width, height, backdrop).worst() == 0.0);
  // 中间画着卡片、边上只有零星一个淡阴影像素：仍在限度内。
  const auto set = [&](int x, int y, uint8_t value) {
    const auto offset = (static_cast<size_t>(y) * width + static_cast<size_t>(x)) * 4;
    pixels[offset] = pixels[offset + 1] = pixels[offset + 2] = value;
  };
  for (int y = 1; y < height - 1; ++y)
    for (int x = 1; x < width - 1; ++x)
      set(x, y, 0xff);
  set(width - 1, 2, 0xd0);
  auto bleed = edge_bleed(pixels, width, height, backdrop);
  require(bleed.left == 0.0 && bleed.top == 0.0 && bleed.bottom == 0.0);
  require(bleed.right > 0.0 && bleed.right <= maximum_edge_fraction);
  // 卡片一直画到右边和下边（被窗口裁掉的样子）：那两条边超过限度。
  for (int y = 1; y < height; ++y)
    set(width - 1, y, 0xff);
  for (int x = 1; x < width; ++x)
    set(x, height - 1, 0xff);
  bleed = edge_bleed(pixels, width, height, backdrop);
  require(bleed.right > maximum_edge_fraction && bleed.bottom > maximum_edge_fraction);
  require(bleed.left == 0.0 && bleed.top == 0.0);
  require(bleed.worst() == (std::max)(bleed.right, bleed.bottom));
  // 尺寸和像素数对不上时不读越界。
  require(edge_bleed(pixels, width + 1, height, backdrop).worst() == 0.0);
}

void striped_backdrop() {
  // 条纹每 24 像素换一种颜色，六种一轮，深浅各一组；负坐标（副显示器在主显示器左边）按绝对值算，不越界。
  require(backdrop_stripe_color(0, false) == backdrop_stripe_color(23, false));
  require(backdrop_stripe_color(0, false) != backdrop_stripe_color(24, false));
  require(backdrop_stripe_color(0, false) == backdrop_stripe_color(stripe_width * 6, false));
  require(backdrop_stripe_color(0, true) != backdrop_stripe_color(0, false));
  require(backdrop_stripe_color(-30, false) == backdrop_stripe_color(30, false));
  // 4 x 3 的屏幕裁出中间 2 x 2：逐行拷对位置。
  std::vector<uint8_t> screen;
  for (int y = 0; y < 3; ++y)
    for (int x = 0; x < 4; ++x)
      screen.insert(screen.end(), {static_cast<uint8_t>(x), static_cast<uint8_t>(y), 0, 0xff});
  const auto window = crop(screen, 4, 3, {1, 1, 3, 3});
  require((window == std::vector<uint8_t>{1, 1, 0, 0xff, 2, 1, 0, 0xff, 1, 2, 0, 0xff, 2, 2, 0, 0xff}));
  require(crop(screen, 4, 3, {3, 1, 5, 3}).empty());
  require(crop(screen, 4, 3, {-1, 0, 2, 2}).empty());
  require(crop(screen, 4, 3, {2, 2, 2, 3}).empty());
  require(crop(screen, 5, 3, {0, 0, 1, 1}).empty());
  // 和截图之前的屏幕比：一样的不算画了东西，改了一个像素的那一个算；大小对不上的一个都不算。
  auto shot = window;
  require(capture_verdict(shot, window).differing == 0 && capture_verdict(shot, window).total == 4);
  require(capture_blank(capture_verdict(shot, window)));
  shot[4] = 0x80;
  require(capture_verdict(shot, window).differing == 1);
  require(capture_verdict(shot, screen).total == 0);
  // 边：只有右上那个像素改了，右边一半、上边一半不同。
  const auto bleed = edge_bleed(shot, window, 2, 2);
  require(bleed.left == 0.0 && bleed.bottom == 0.0 && bleed.right == 0.5 && bleed.top == 0.5);
  require(edge_bleed(shot, screen, 2, 2).worst() == 0.0);
}

void card_geometry() {
  // 592 x 182 的窗口，阴影边距 32/20/32/40：卡片应在 (32,20)-(560,142)。
  const PixelRect expected{32, 20, 560, 142};
  require(card_fits({32, 20, 560, 142}, 592, 182, expected));
  require(card_fits({33, 20, 561, 141}, 592, 182, expected));
  // 按 640 宽的交换链排出来的卡片：右边越过窗口。
  require(!card_fits({32, 20, 608, 142}, 592, 182, expected));
  // 还在窗口里，但和窗口推出的卡片不是一个尺寸。
  require(!card_fits({32, 20, 540, 142}, 592, 182, expected));
  require(!card_fits({32, 20, 560, 190}, 592, 182, expected));
  require(!card_fits({32, 20, 32, 142}, 592, 182, {32, 20, 32, 142}));
}
} // namespace

int main() {
  try {
    file_names();
    presentation();
    expanded();
    blank_capture();
    edges();
    striped_backdrop();
    card_geometry();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "candidate skin gallery plan ok\n";
  return 0;
}
