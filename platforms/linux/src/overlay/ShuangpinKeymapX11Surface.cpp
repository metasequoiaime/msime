#include "ShuangpinKeymapX11Surface.h"

#include <cmath>
#include <cstdlib>
#include <cstring>

#include "OverlayX11Window.h"
#include "WaveOverlayX11Placement.h"
#include "msime_client.h"

namespace msime::linux_host {

namespace {

// host-api 的一张方案表：{"ok":true,"value":{...}} 的 value；读不到或不是对象时是空对象，键帽上只剩键名。
nlohmann::json profile_table(char *(*query)(const uint8_t *, size_t), const std::string &profile) {
  char *raw = query(reinterpret_cast<const uint8_t *>(profile.data()), profile.size());
  if (!raw) return nlohmann::json::object();
  auto envelope = nlohmann::json::parse(raw, nullptr, false);
  msime_client_string_free(raw);
  if (!envelope.is_object() || envelope.value("ok", false) != true) return nlohmann::json::object();
  const auto value = envelope.find("value");
  return value != envelope.end() && value->is_object() ? *value : nlohmann::json::object();
}

}  // namespace

ShuangpinKeymapX11Surface::ShuangpinKeymapX11Surface() = default;
ShuangpinKeymapX11Surface::~ShuangpinKeymapX11Surface() { close(); }

bool ShuangpinKeymapX11Surface::visible() const { return window_ && window_->visible(); }

bool ShuangpinKeymapX11Surface::open(const std::string &display) {
  // 连接断了（X 服务器退出或重启）就关掉重连；键位图只在宿主的按键路径上画，不在这条连接的回调里，这里关是安全的。
  if (display_ && display == display_name_ && !lost_) return true;
  close();
  const char *name = display.empty() ? std::getenv("DISPLAY") : display.c_str();
  if (!name || !*name) return false;
  display_ = open_overlay_display(name, &lost_);
  if (!display_) return false;
  display_name_ = display;
  randr_monitors_ = x11_overlay_randr_monitors(display_);
  window_ = std::make_unique<OverlayX11Window>(display_, "msime-shuangpin-keymap", "MSIMEShuangpinKeymap",
                                               "_NET_WM_WINDOW_TYPE_TOOLTIP", false);
  if (!window_->valid()) {
    close();
    return false;
  }
  return true;
}

void ShuangpinKeymapX11Surface::close() {
  window_.reset();
  close_overlay_display(display_);
  display_ = nullptr;
  lost_ = false;
  display_name_.clear();
}

const ShuangpinKeymapContent &ShuangpinKeymapX11Surface::content(const std::string &profile) {
  auto found = tables_.find(profile);
  if (found != tables_.end()) return found->second;
  ShuangpinKeymapContent table;
  table.title = shuangpin_keymap_title(profile);
  table.rows = shuangpin_keymap_rows(profile_table(msime_client_shuangpin_key_hints, profile));
  table.zero_initials = shuangpin_keymap_zero_initial_text(profile_table(msime_client_shuangpin_zero_initials, profile));
  return tables_.emplace(profile, std::move(table)).first->second;
}

bool ShuangpinKeymapX11Surface::show(const std::string &display, const ShuangpinKeymapHint &hint,
                                     const ShuangpinKeymapColors &colors, const ShuangpinKeymapAnchor &anchor) {
  // 与 macOS 的 MSIMEValidCaret 相同：高度为 0 的矩形是输入框没报光标，不知道放在哪里就不画。
  if (anchor.caret_height <= 0 || !open(display)) {
    hide();
    return false;
  }
  auto content = this->content(hint.profile);
  content.highlighted = hint.key;
  // 显示器、工作区、缩放和合成器在两次按键之间都可能变，这里每次都重读：键位图只在组字时出现，一次读取比一次重画便宜得多。
  const double scale = x11_overlay_scale(display_);
  const bool shadow = window_->composited();
  const int width = static_cast<int>(std::ceil(shuangpin_keymap_surface_width(shadow) * scale));
  const int height = static_cast<int>(std::ceil(shuangpin_keymap_surface_height(shadow) * scale));
  const int margin = static_cast<int>(std::lround(shuangpin_keymap_margin(shadow) * scale));
  const int plate_width = width - 2 * margin;
  const int plate_height = height - 2 * margin;
  const WaveOverlayPosition caret_center{anchor.caret_x + anchor.caret_width / 2,
                                         anchor.caret_y + anchor.caret_height / 2};
  const auto monitor = x11_overlay_monitor(display_, randr_monitors_, caret_center);
  const auto plate = shuangpin_keymap_position(
      anchor.caret_x, anchor.caret_y, anchor.caret_height, plate_width, plate_height,
      anchor.measured_clearance ? *anchor.measured_clearance : static_cast<int>(std::lround(anchor.clearance * scale)),
      monitor.work,
      static_cast<int>(std::lround(ShuangpinKeymapGeometry::candidate_gap * scale)),
      static_cast<int>(std::lround(ShuangpinKeymapGeometry::screen_margin * scale)));
  window_->place(plate.x - margin, plate.y - margin, width, height);
  const bool was_visible = window_->visible();
  if (!was_visible) window_->show();
  window_->paint(scale, colors.surface, [&](cairo_t *cairo) { paint_shuangpin_keymap(cairo, content, colors, shadow); });
  // 窗口不收输入，只有 Expose 会来；内容每次按键都整块重画，排掉它们免得队列越积越长。
  while (XPending(display_)) {
    XEvent event{};
    XNextEvent(display_, &event);
  }
  return true;
}

void ShuangpinKeymapX11Surface::hide() {
  if (window_) window_->hide();
}

}  // namespace msime::linux_host
