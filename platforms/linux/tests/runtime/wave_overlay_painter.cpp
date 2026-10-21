#include "../src/overlay/WaveOverlayPainter.h"

#include <cairo/cairo.h>

#include <cassert>
#include <cstdint>
#include <cstdlib>
#include <string>

namespace {

using msime::linux_host::WaveOverlayFrame;
using msime::linux_host::WaveOverlayLayout;
using msime::linux_host::WaveOverlayModel;
using msime::linux_host::WaveOverlayPainter;

struct Image {
  cairo_surface_t *surface = nullptr;
  ~Image() { cairo_surface_destroy(surface); }
  std::uint32_t pixel(int x, int y) const {
    const auto *data = cairo_image_surface_get_data(surface);
    const auto stride = cairo_image_surface_get_stride(surface);
    return *reinterpret_cast<const std::uint32_t *>(data + y * stride + x * 4);
  }
  int alpha(int x, int y) const { return static_cast<int>(pixel(x, y) >> 24); }
};

// 像浮层那样以每逻辑像素 `scale` 个设备像素绘制 `model`，`MSIME_TEST_CANVAS_ARTIFACTS` 指定了目录时存一张 PNG。
Image paint(WaveOverlayPainter &painter, const WaveOverlayModel &model, int scale, bool translucent,
            const std::string &name, WaveOverlayFrame *frame_out = nullptr) {
  const auto frame = painter.frame(model);
  if (frame_out) *frame_out = frame;
  Image image;
  image.surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, frame.width * scale, frame.height * scale);
  auto *cairo = cairo_create(image.surface);
  cairo_scale(cairo, scale, scale);
  painter.paint(cairo, frame, model, translucent);
  cairo_destroy(cairo);
  cairo_surface_flush(image.surface);
  if (const char *directory = std::getenv("MSIME_TEST_CANVAS_ARTIFACTS"); directory && *directory) {
    const auto path = std::string(directory) + "/wave-overlay-" + name + ".png";
    const auto written = cairo_surface_write_to_png(image.surface, path.c_str());
    assert(written == CAIRO_STATUS_SUCCESS);
  }
  return image;
}

WaveOverlayModel listening(bool light) {
  WaveOverlayModel model;
  model.light_theme = light;
  model.listening = true;
  model.set_input_level(0.8f);
  for (int frame = 0; frame < 30; ++frame) model.advance(100.0 + frame * 0.016);
  return model;
}

}  // namespace

int main() {
  WaveOverlayPainter painter;
  for (const bool light : {false, true}) {
    const std::string theme = light ? "light" : "dark";

    // 紧凑胶囊：78x32，圆角之外透明，面板是 macOS 的 0.90 不透明度。
    WaveOverlayFrame frame;
    auto compact = listening(light);
    const auto compact_image = paint(painter, compact, 2, true, "compact-" + theme, &frame);
    assert(frame.layout == WaveOverlayLayout::Compact && frame.width == 78 && frame.height == 32);
    assert(!frame.actions && !frame.label && frame.transcript.empty());
    assert(compact_image.alpha(0, 0) == 0);
    const int panel_alpha = compact_image.alpha(78, 8);
    assert(panel_alpha >= 226 && panel_alpha <= 234);
    // 没有 alpha 通道时面板不透明。
    const auto opaque = paint(painter, compact, 1, false, "compact-opaque-" + theme);
    assert(opaque.alpha(39, 4) == 255);

    // 锁定的录音：142x40 的操作胶囊，波形两侧各一个中性色的圆。
    auto locked = listening(light);
    locked.locked = true;
    const auto locked_image = paint(painter, locked, 2, true, "locked-" + theme, &frame);
    assert(frame.layout == WaveOverlayLayout::Action && frame.width == 142 && frame.height == 40 && frame.actions);
    assert(locked_image.pixel(36, 24) != locked_image.pixel(142, 8));

    // 识别中：操作按钮保留，macOS 的标签代替波形。
    WaveOverlayModel processing;
    processing.light_theme = light;
    processing.compact_status = WaveOverlayModel::CompactStatus::Recognizing;
    paint(painter, processing, 2, true, "recognizing-" + theme, &frame);
    assert(frame.layout == WaveOverlayLayout::Action && frame.label && frame.label_text == "识别中...");

    // 流式转写：420x112 的卡片在「…」之后保留最新的三行。
    auto transcript = listening(light);
    std::string text;
    for (int i = 0; i < 40; ++i) text += "这是一段比较长的实时转写文字，用来检查只显示最新的三行。";
    text += "最新的结尾";
    transcript.set_transcript(text);
    paint(painter, transcript, 2, true, "transcript-" + theme, &frame);
    assert(frame.layout == WaveOverlayLayout::Transcript && frame.width == 420 && frame.height == 112);
    assert(frame.transcript.rfind("…", 0) == 0);
    assert(text.size() >= frame.transcript.size() - 3 &&
           text.compare(text.size() - (frame.transcript.size() - 3), std::string::npos, frame.transcript.substr(3)) == 0);
    assert(painter.transcript_line_count(frame.transcript, 392) <= 3);
    // 短的转写文字完整显示。
    transcript.set_transcript("你好，世界");
    assert(painter.frame(transcript).transcript == "你好，世界");

    // 失败：没有波形，没有操作按钮，只有消息，画在加宽到能放下它的胶囊里。
    WaveOverlayModel failure;
    failure.light_theme = light;
    failure.failure = true;
    failure.status = "语音输入失败，请检查语音服务、麦克风及提供商配置后重试";
    paint(painter, failure, 2, true, "failure-" + theme, &frame);
    assert(frame.layout == WaveOverlayLayout::Processing && frame.height == 40 && !frame.actions && frame.label);
    assert(frame.width > 112 && frame.width <= WaveOverlayPainter::kMaxFailureWidth);
    failure.status = "失败";
    assert(painter.frame(failure).width == 112);
  }

  // 主题配色取代内置颜色，主题没有描边时面板也没有描边。
  {
    auto skinned = listening(false);
    skinned.palette = msime::linux_host::FloatingSurfaceColors{0x336699u, 0xFFFFFFu, 0xFFCC00u, std::nullopt};
    const auto image = paint(painter, skinned, 1, false, "skin");
    const auto panel = image.pixel(39, 4);
    assert(((panel >> 16) & 0xff) == 0x33 && ((panel >> 8) & 0xff) == 0x66 && (panel & 0xff) == 0x99);
    // 波形条用文字颜色，与 macOS 用前景色填充一样，从不用强调色。
    const auto bars = paint(painter, skinned, 2, false, "skin-2x");
    bool text_seen = false;
    for (int y = 0; y < 64; ++y)
      for (int x = 0; x < 156; ++x) {
        const auto pixel = bars.pixel(x, y);
        const int r = (pixel >> 16) & 0xff, g = (pixel >> 8) & 0xff, b = pixel & 0xff;
        assert(!(r > 0xe0 && g > 0xb0 && g < 0xe0 && b < 0x40));
        text_seen = text_seen || (r > 0xf0 && g > 0xf0 && b > 0xf0);
      }
    assert(text_seen);
  }

  // 提供商的 NUL 到不了 Pango：模型把它丢掉，长转写的最新几行照样切出并画出。
  {
    WaveOverlayModel nul;
    nul.listening = true;
    std::string text(1, '\0');
    for (int i = 0; i < 200; ++i) text += "你";
    nul.set_transcript(text);
    assert(nul.transcript.find('\0') == std::string::npos && nul.transcript.size() == 600);
    WaveOverlayFrame frame;
    paint(painter, nul, 1, true, "transcript-nul", &frame);
    assert(frame.layout == WaveOverlayLayout::Transcript && frame.transcript.find('\0') == std::string::npos);
  }

  // 切分从不把基字母和它的组合符号分开。
  {
    WaveOverlayModel marks;
    marks.listening = true;
    std::string text;
    for (int i = 0; i < 600; ++i) text += "e\xcc\x81 ";
    marks.set_transcript(text);
    const auto visible = painter.frame(marks).transcript;
    assert(visible.rfind("…", 0) == 0 && visible.size() > 3);
    assert(visible.compare(3, 2, "\xcc\x81") != 0);
  }
}
