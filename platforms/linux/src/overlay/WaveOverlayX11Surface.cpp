#include "WaveOverlayX11Surface.h"

#include "WaveOverlayPlacement.h"
#include "WaveOverlayX11Placement.h"

#include <X11/Xlib.h>
#include <X11/Xatom.h>
#include <X11/Xresource.h>
#include <X11/Xutil.h>
#include <X11/extensions/Xfixes.h>
#include <X11/extensions/Xrandr.h>
#include <X11/extensions/shape.h>

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <locale>
#include <optional>
#include <sstream>
#include <string>
#include <vector>

namespace msime::linux_host {
namespace {

// Logical geometry at a scale of 1; every length is multiplied by the current scale before it reaches the X server.
constexpr int kLogicalWidth = 420;
constexpr int kLogicalHeight = 132;
constexpr int kBarCount = 12;
constexpr int kActionCenterInset = 24;
constexpr int kActionCenterY = 66;
constexpr int kActionRadius = 14;
constexpr int kFontPixelSize = 14;
// Matches the Windows voice bar, which sits 10 px above the bottom of the work area.
constexpr int kBottomMargin = 10;
// update() runs for every input-level change, so the monitor, work area and scale are re-read at most this often while visible; show() always re-reads them.
constexpr auto kPlacementRefreshInterval = std::chrono::milliseconds(500);

unsigned long color(Display *display, int screen, const char *value,
                    unsigned long fallback) {
  XColor exact{};
  XColor allocated{};
  const auto colormap = DefaultColormap(display, screen);
  if (!XParseColor(display, colormap, value, &exact) ||
      !XAllocColor(display, colormap, &exact))
    return fallback;
  allocated = exact;
  return allocated.pixel;
}

std::string one_line(std::string text) {
  for (auto &character : text)
    if (character == '\r' || character == '\n' || character == '\t')
      character = ' ';
  return text;
}

std::string utf8_prefix(const std::string &text, std::size_t characters) {
  std::size_t count = 0;
  std::size_t end = 0;
  while (end < text.size() && count < characters) {
    if ((static_cast<unsigned char>(text[end]) & 0xc0) != 0x80)
      ++count;
    ++end;
  }
  return text.substr(0, end);
}

}  // namespace

WaveOverlayX11Surface::~WaveOverlayX11Surface() { destroy_window(); }

bool WaveOverlayX11Surface::ensure_window() {
  if (display_)
    return true;
  const auto *display_name = std::getenv("DISPLAY");
  if (!display_name || !*display_name)
    return false;
  display_ = XOpenDisplay(display_name);
  if (!display_)
    return false;
  const auto screen = DefaultScreen(display_);
  int fixes_event = 0;
  int fixes_error = 0;
  if (!XFixesQueryExtension(display_, &fixes_event, &fixes_error)) {
    XCloseDisplay(display_);
    display_ = nullptr;
    return false;
  }
  randr_monitors_ = x11_overlay_randr_monitors(display_);
  const auto root = RootWindow(display_, screen);
  XSetWindowAttributes attributes{};
  attributes.override_redirect = True;
  attributes.background_pixel = color(display_, screen, "#202124", BlackPixel(display_, screen));
  attributes.border_pixel = color(display_, screen, "#4a4d52", WhitePixel(display_, screen));
  window_ = XCreateWindow(display_, root, 0, 0, kLogicalWidth, kLogicalHeight, 1,
                           CopyFromParent, InputOutput, CopyFromParent,
                           CWOverrideRedirect | CWBackPixel | CWBorderPixel,
                           &attributes);
  if (!window_) {
    XCloseDisplay(display_);
    display_ = nullptr;
    return false;
  }
  gc_ = XCreateGC(display_, window_, 0, nullptr);
  background_ = attributes.background_pixel;
  foreground_ = color(display_, screen, "#f5f7fa", WhitePixel(display_, screen));
  accent_ = color(display_, screen, "#73a7ff", WhitePixel(display_, screen));
  light_background_ = color(display_, screen, "#f5f7fa", WhitePixel(display_, screen));
  light_foreground_ = color(display_, screen, "#202124", BlackPixel(display_, screen));
  light_accent_ = color(display_, screen, "#3367d6", BlackPixel(display_, screen));
  border_ = attributes.border_pixel;
  current_border_ = border_;
  palette_pixels_.clear();
  // Zero size makes the first place() apply the scale, resize the window and load the font set.
  scale_ = 1.0;
  width_ = 0;
  height_ = 0;
  XSelectInput(display_, window_, ExposureMask | ButtonPressMask |
                                       ButtonReleaseMask);
  set_input_region(false);
  return true;
}

int WaveOverlayX11Surface::scaled(int logical) const {
  return wave_overlay_scaled(logical, scale_);
}

void WaveOverlayX11Surface::load_font_set() {
  if (font_set_) {
    XFreeFontSet(display_, font_set_);
    font_set_ = nullptr;
  }
  const auto create = [this](int pixel_size, int &missing_count) {
    const auto pattern = "-misc-fixed-*-*-*-*-" + std::to_string(pixel_size) +
                         "-*-*-*-*-*-*-*";
    char **missing = nullptr;
    char *default_string = nullptr;
    missing_count = 0;
    const auto font_set = XCreateFontSet(display_, pattern.c_str(), &missing,
                                         &missing_count, &default_string);
    if (missing)
      XFreeStringList(missing);
    return font_set;
  };
  // The bitmap fixed fonts only come in a few sizes. When the scaled size is missing, or covers fewer charsets (CJK above all), the 1x size stays: small text beats unreadable text.
  int scaled_missing = 0;
  const auto pixel_size = scaled(kFontPixelSize);
  font_set_ = create(pixel_size, scaled_missing);
  if (pixel_size == kFontPixelSize || (font_set_ && scaled_missing == 0))
    return;
  int base_missing = 0;
  const auto base = create(kFontPixelSize, base_missing);
  if (base && (!font_set_ || base_missing < scaled_missing)) {
    if (font_set_)
      XFreeFontSet(display_, font_set_);
    font_set_ = base;
  } else if (base) {
    XFreeFontSet(display_, base);
  }
}

// Mirrors the Windows voice bar's update_window_bounds: the scale and the target monitor are re-read, the window resized when the scale changed, and the bar centred on the monitor holding the focused window, just above its work-area bottom.
void WaveOverlayX11Surface::place(bool force) {
  if (!display_ || !window_)
    return;
  const auto now = std::chrono::steady_clock::now();
  if (!force && width_ != 0 && now - placed_at_ < kPlacementRefreshInterval)
    return;
  placed_at_ = now;
  const auto scale = x11_overlay_scale(display_);
  const auto width =
      static_cast<unsigned>(wave_overlay_scaled(kLogicalWidth, scale));
  const auto height =
      static_cast<unsigned>(wave_overlay_scaled(kLogicalHeight, scale));
  if (width != width_ || height != height_ || scale != scale_) {
    scale_ = scale;
    width_ = width;
    height_ = height;
    XResizeWindow(display_, window_, width_, height_);
    const auto line_width = scaled(1);
    XSetLineAttributes(display_, gc_, line_width > 1 ? line_width : 0,
                       LineSolid, CapRound, JoinRound);
    load_font_set();
  }

  const auto target = x11_overlay_monitor(display_, randr_monitors_);
  const auto position = wave_overlay_monitor_bottom_center(
      target, static_cast<int>(width_), static_cast<int>(height_),
      scaled(kBottomMargin));
  XMoveWindow(display_, window_, position.x, position.y);
}

void WaveOverlayX11Surface::set_input_region(bool actions_visible) {
  if (!display_ || !window_)
    return;
  const auto radius = scaled(kActionRadius);
  const auto top = scaled(kActionCenterY) - radius;
  const auto left_center = scaled(kActionCenterInset);
  const auto right_center = static_cast<int>(width_) - left_center;
  XRectangle buttons[2] = {
      {static_cast<short>(left_center - radius), static_cast<short>(top),
       static_cast<unsigned short>(2 * radius),
       static_cast<unsigned short>(2 * radius)},
      {static_cast<short>(right_center - radius), static_cast<short>(top),
       static_cast<unsigned short>(2 * radius),
       static_cast<unsigned short>(2 * radius)}};
  XserverRegion region = XFixesCreateRegion(
      display_, actions_visible ? buttons : nullptr, actions_visible ? 2 : 0);
  if (region) {
    XFixesSetWindowShapeRegion(display_, window_, ShapeInput, 0, 0, region);
    XFixesDestroyRegion(display_, region);
  }
}

bool WaveOverlayX11Surface::hit_test_action(
    int x, int y, WaveOverlayModel::Action &action) const {
  if (!actions_visible_)
    return false;
  const auto radius = scaled(kActionRadius);
  const auto center_y = scaled(kActionCenterY);
  const auto left_center = scaled(kActionCenterInset);
  const auto inside = [x, y, radius, center_y](int center_x) {
    const int dx = x - center_x;
    const int dy = y - center_y;
    return dx * dx + dy * dy <= radius * radius;
  };
  if (inside(left_center)) {
    action = WaveOverlayModel::Action::Cancel;
    return true;
  }
  if (inside(static_cast<int>(width_) - left_center)) {
    action = WaveOverlayModel::Action::Confirm;
    return true;
  }
  return false;
}

void WaveOverlayX11Surface::pump_events() {
  if (!display_ || !visible_)
    return;
  while (XPending(display_)) {
    XEvent event{};
    XNextEvent(display_, &event);
    if (event.type == ButtonPress && event.xbutton.button == Button1) {
      WaveOverlayModel::Action action;
      if (hit_test_action(event.xbutton.x, event.xbutton.y, action)) {
        pressed_action_ = action;
        action_pressed_ = true;
        XGrabPointer(display_, window_, False, ButtonReleaseMask,
                     GrabModeAsync, GrabModeAsync, None, None, CurrentTime);
      }
    } else if (event.type == ButtonRelease && event.xbutton.button == Button1 &&
               action_pressed_) {
      WaveOverlayModel::Action action;
      const bool activated = hit_test_action(event.xbutton.x, event.xbutton.y,
                                             action) &&
                             action == pressed_action_;
      action_pressed_ = false;
      XUngrabPointer(display_, CurrentTime);
      if (activated && action_handler_)
        action_handler_(action);
    }
  }
}

void WaveOverlayX11Surface::destroy_window() {
  if (!display_)
    return;
  if (action_pressed_)
    XUngrabPointer(display_, CurrentTime);
  if (font_set_)
    XFreeFontSet(display_, font_set_);
  if (gc_)
    XFreeGC(display_, gc_);
  if (window_)
    XDestroyWindow(display_, window_);
  XCloseDisplay(display_);
  display_ = nullptr;
  window_ = 0;
  gc_ = 0;
  font_set_ = 0;
  visible_ = false;
}

unsigned long WaveOverlayX11Surface::palette_pixel(std::uint32_t rgb, unsigned long fallback) {
  for (const auto &[cached, pixel] : palette_pixels_)
    if (cached == rgb) return pixel;
  char value[8];
  std::snprintf(value, sizeof(value), "#%06x", rgb & 0xffffffu);
  const auto pixel = color(display_, DefaultScreen(display_), value, fallback);
  // A theme switch brings new colours; a bounded cache keeps a long session from growing it, and TrueColor visuals allocate nothing to free.
  if (palette_pixels_.size() >= 16) palette_pixels_.clear();
  palette_pixels_.emplace_back(rgb, pixel);
  return pixel;
}

void WaveOverlayX11Surface::draw(const WaveOverlayModel &model) {
  if (!display_ || !window_ || !gc_)
    return;
  place(false);
  set_input_region(model.actions_visible);
  if (!model.actions_visible)
    action_pressed_ = false;
  auto background = model.light_theme ? light_background_ : background_;
  auto foreground = model.light_theme ? light_foreground_ : foreground_;
  auto accent = model.light_theme ? light_accent_ : accent_;
  auto border = border_;
  // The resolved theme's palette when the frontend gave one: its surface, text and accent, and its outline (the surface itself when the theme draws none, so the 1 px border disappears).
  if (model.palette) {
    background = palette_pixel(model.palette->surface, background);
    foreground = palette_pixel(model.palette->text, foreground);
    accent = palette_pixel(model.palette->accent, accent);
    border = model.palette->border ? palette_pixel(*model.palette->border, border_) : background;
  }
  if (border != current_border_) {
    XSetWindowBorder(display_, window_, border);
    current_border_ = border;
  }
  XSetForeground(display_, gc_, background);
  XFillRectangle(display_, window_, gc_, 0, 0, width_, height_);
  XSetForeground(display_, gc_, accent);
  const auto inset = scaled(16);
  const auto bar_step = (static_cast<int>(width_) - 2 * inset) / kBarCount;
  const auto bar_width = std::max(2, bar_step - scaled(3));
  const auto bar_center = scaled(56);
  for (int index = 0; index < kBarCount; ++index) {
    const auto slot = static_cast<std::size_t>(index);
    const auto level = slot < model.levels.size() ? model.levels[slot] : 0.0f;
    const auto height =
        std::max(scaled(4), static_cast<int>(level * 38.0 * scale_));
    XFillRectangle(display_, window_, gc_, inset + index * bar_step,
                   bar_center - height / 2, static_cast<unsigned>(bar_width),
                   static_cast<unsigned>(height));
  }
  if (font_set_) {
    const auto font = font_set_;
    XSetForeground(display_, gc_, foreground);
    std::string status = model.locked
                             ? "录音已锁定 · 再按快捷键或点击语音菜单结束 · Esc 取消"
                             : one_line(model.status);
    if (status.empty())
      status = "正在录音…";
    Xutf8DrawString(display_, window_, font, gc_, inset, scaled(104),
                    status.c_str(), static_cast<int>(status.size()));
    if (model.show_transcript && !model.transcript.empty()) {
      auto transcript = utf8_prefix(one_line(model.transcript), 72);
      Xutf8DrawString(display_, window_, font, gc_, inset, scaled(124),
                      transcript.c_str(), static_cast<int>(transcript.size()));
    }
  }
  if (model.actions_visible) {
    const auto radius = scaled(kActionRadius);
    const auto center_y = scaled(kActionCenterY);
    const auto cancel_x = scaled(kActionCenterInset);
    const auto confirm_x = static_cast<int>(width_) - cancel_x;
    const auto diameter = static_cast<unsigned>(2 * radius);
    XSetForeground(display_, gc_, accent);
    XFillArc(display_, window_, gc_, cancel_x - radius, center_y - radius,
             diameter, diameter, 0, 360 * 64);
    XFillArc(display_, window_, gc_, confirm_x - radius, center_y - radius,
             diameter, diameter, 0, 360 * 64);
    XSetForeground(display_, gc_, background);
    XDrawLine(display_, window_, gc_, cancel_x - scaled(5),
              center_y - scaled(5), cancel_x + scaled(5), center_y + scaled(5));
    XDrawLine(display_, window_, gc_, cancel_x + scaled(5),
              center_y - scaled(5), cancel_x - scaled(5), center_y + scaled(5));
    XDrawLine(display_, window_, gc_, confirm_x - scaled(5), center_y,
              confirm_x - scaled(1), center_y + scaled(4));
    XDrawLine(display_, window_, gc_, confirm_x - scaled(1),
              center_y + scaled(4), confirm_x + scaled(6), center_y - scaled(5));
  }
  XFlush(display_);
}

bool WaveOverlayX11Surface::show(const WaveOverlayModel &model) {
  if (!ensure_window())
    return false;
  visible_ = true;
  actions_visible_ = model.actions_visible;
  // Placed before mapping so the bar never flashes at its previous monitor or size.
  place(true);
  XMapRaised(display_, window_);
  draw(model);
  pump_events();
  return true;
}

void WaveOverlayX11Surface::update(const WaveOverlayModel &model) {
  if (visible_) {
    actions_visible_ = model.actions_visible;
    pump_events();
    draw(model);
  }
}

void WaveOverlayX11Surface::hide() {
  if (display_ && window_ && visible_) {
    XUnmapWindow(display_, window_);
    XFlush(display_);
  }
  if (display_ && action_pressed_)
    XUngrabPointer(display_, CurrentTime);
  visible_ = false;
  action_pressed_ = false;
}

}  // namespace msime::linux_host
