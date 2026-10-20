#include "NativeCanvasX11.h"

#include <fcitx-utils/event.h>
#include "../overlay/WaveOverlayX11Placement.h"
#include <X11/Xutil.h>
#include <X11/cursorfont.h>
#include <X11/extensions/dpms.h>
#include <X11/extensions/shape.h>
#include <cairo-xlib.h>

namespace msime::linux_host {

namespace {
namespace shared = msime::candidate;
// 短动效（进场、退场、选中、错峰）按约 60 帧刷新；装饰图待机动效周期以秒计，30 帧足够平滑，也少一半重画。
constexpr std::uint64_t kMotionFrameMs = 16;
constexpr std::uint64_t kIdleMotionFrameMs = 33;
} // namespace

struct NativeCanvasX11::Impl {
  explicit Impl(fcitx::EventLoop &loop) : loop(loop) {}
  ~Impl() { close(); }
  fcitx::EventLoop &loop;
  Display *display = nullptr;
  Window window = 0;
  Visual *visual = nullptr;
  Colormap colormap = 0;
  Cursor hand = 0;
  std::string name;
  bool visible = false, composited = false, dpms = false, hand_shown = false;
  std::unique_ptr<fcitx::EventSourceIO> input;
  std::unique_ptr<fcitx::EventSourceTime> animation_timer;
  std::uint64_t animation_started_us = 0;
  std::shared_ptr<NativeCanvasAssets> assets;
  NativeCanvasContent content;
  NativeCanvasLayout layout;
  nlohmann::json palette;
  double scale = 1;
  int width = 1, height = 1;
  int measured_work_width = 0, measured_work_height = 0;
  int pressed = -1, hovered = -1;
  int cursor_x = 0, cursor_y = 0, cursor_height = 0;
  // 这次显示以来竖排最高的一页（决定翻不翻到光标上方）和横排最宽的一张卡片（卡片只变宽不变窄），隐藏后从头记。
  double tallest = 0, widest = 0;
  // 有合成器时 macOS 结构的卡片四周留出的投影边距（逻辑像素），卡片和它的命中、形状、定位都按 inset.left、inset.top 平移；投影图按窗口尺寸缓存。
  NativeCanvasShadowInsets inset;
  NativeCanvasImage shadow;
  std::array<double, 8> shadow_key{};
  bool reduce_motion = false;
  // 动效时钟（CLOCK_MONOTONIC 微秒）：进场、退场、选中各自的起点，装饰图待机动效的相位起点和上一次收起的时刻。
  bool entering = false, exiting = false, selecting = false;
  std::uint64_t enter_start_us = 0, exit_start_us = 0, selection_start_us = 0;
  std::uint64_t decoration_anchor_us = 0, last_hide_us = 0;
  shared::CardMotionFrame exit_from;
  // 选中装饰只在同一段组字、同一页里高亮从一行移到另一行时播（macOS 的 decorateCandidateSelection）。
  int highlighted = -1;
  std::string highlight_context;

  void close() {
    animation_timer.reset();
    input.reset();
    if (display) {
      if (hand) XFreeCursor(display, hand);
      if (window) XDestroyWindow(display, window);
      if (colormap) XFreeColormap(display, colormap);
      XCloseDisplay(display);
    }
    display = nullptr; window = 0; colormap = 0; hand = 0; visible = exiting = hand_shown = false;
  }
  bool open(const std::string &target) {
    if (display && target == name) return true;
    close();
    if (target.rfind("x11:", 0) != 0) return false;
    display = XOpenDisplay(target.substr(4).c_str());
    if (!display) return false;
    name = target;
    const int screen = DefaultScreen(display);
    const auto root = RootWindow(display, screen);
    int shape_event = 0, shape_error = 0;
    if (!XShapeQueryExtension(display, &shape_event, &shape_error)) { close(); return false; }
    visual = DefaultVisual(display, screen);
    int depth = DefaultDepth(display, screen);
    // 只有合成管理器存在才选 ARGB；无合成器时用不透明底色保持文字可读。
    const auto selection = "_NET_WM_CM_S" + std::to_string(screen);
    XVisualInfo info{};
    composited = XGetSelectionOwner(display, XInternAtom(display, selection.c_str(), False)) != None &&
        XMatchVisualInfo(display, screen, 32, TrueColor, &info);
    if (composited) {
      visual = info.visual; depth = info.depth;
    }
    colormap = XCreateColormap(display, root, visual, AllocNone);
    XSetWindowAttributes attributes{};
    attributes.override_redirect = True;
    attributes.colormap = colormap;
    attributes.border_pixel = 0;
    attributes.background_pixel = 0;
    attributes.event_mask = ExposureMask | ButtonPressMask | ButtonReleaseMask | PointerMotionMask | EnterWindowMask | LeaveWindowMask;
    window = XCreateWindow(display, root, 0, 0, 1, 1, 0, depth, InputOutput, visual,
        CWOverrideRedirect | CWColormap | CWBorderPixel | CWBackPixel | CWEventMask, &attributes);
    if (!window) { close(); return false; }
    XClassHint klass{};
    char instance[] = "msime-candidate-canvas", resource[] = "MSIMECandidateCanvas";
    klass.res_name = instance; klass.res_class = resource;
    XSetClassHint(display, window, &klass);
    XStoreName(display, window, "MSIME candidate canvas");
    XWMHints hints{};
    hints.flags = InputHint; hints.input = False;
    XSetWMHints(display, window, &hints);
    const auto type = XInternAtom(display, "_NET_WM_WINDOW_TYPE_TOOLTIP", False);
    XChangeProperty(display, window, XInternAtom(display, "_NET_WM_WINDOW_TYPE", False),
        XA_ATOM, 32, PropModeReplace, reinterpret_cast<const unsigned char *>(&type), 1);
    // 候选行和首行的按钮可以点，指针在上面时换成手形，与 macOS 候选按钮的 pointingHandCursor 相同。
    hand = XCreateFontCursor(display, XC_hand2);
    int dpms_event = 0, dpms_error = 0;
    dpms = DPMSQueryExtension(display, &dpms_event, &dpms_error) && DPMSCapable(display);
    input = loop.addIOEvent(ConnectionNumber(display), fcitx::IOEventFlag::In,
        [this](fcitx::EventSourceIO *, int, fcitx::IOEventFlags) { events(); return true; });
    return true;
  }
  int hit(int x, int y) const {
    return native_canvas_hit(content, layout, x / scale - inset.left, y / scale - inset.top);
  }
  void pointer(int at) {
    const bool actionable = at >= 0 || at == kNativeCanvasHitPrevious || at == kNativeCanvasHitNext || at == kNativeCanvasHitExpand;
    if (actionable == hand_shown || !hand) return;
    hand_shown = actionable;
    if (actionable) XDefineCursor(display, window, hand);
    else XUndefineCursor(display, window);
    XFlush(display);
  }
  void events() {
    while (display && XPending(display)) {
      XEvent event{};
      XNextEvent(display, &event);
      if (!visible) continue;
      if (event.type == Expose && event.xexpose.count == 0) paint();
      if (event.type == MotionNotify || event.type == EnterNotify) {
        const int at = event.type == MotionNotify ? hit(event.xmotion.x, event.xmotion.y) : hit(event.xcrossing.x, event.xcrossing.y);
        pointer(at);
        if (hovered != at) { hovered = at; paint(); }
      }
      if (event.type == LeaveNotify) {
        pointer(-1);
        if (hovered != -1) { hovered = -1; paint(); }
      }
      if (event.type == ButtonPress) {
        if (event.xbutton.button == Button3) menu(hit(event.xbutton.x, event.xbutton.y));
        if (event.xbutton.button == Button1) { pressed = hit(event.xbutton.x, event.xbutton.y); paint(); }
        if (event.xbutton.button == Button4 || event.xbutton.button == Button5) {
          const bool forward = event.xbutton.button == Button5;
          // 网格的滚轮一格滚一行，不看「滚轮翻页」开关（macOS 的网格滚轮同样与它无关）；普通候选窗照旧按开关翻页。
          if (content.grid_columns > 0) {
            const auto scroll = content.grid_scroll;
            if (scroll) scroll(!forward);
          } else if (content.wheel_paging) {
            const auto page = content.page;
            if (page && (forward ? content.next : content.previous)) page(forward);
          }
        }
      }
      if (event.type == ButtonRelease && event.xbutton.button == Button1) {
        const int item = pressed;
        pressed = -1;
        paint();
        if (item == -1 || item != hit(event.xbutton.x, event.xbutton.y)) continue;
        // 回调可能在同一栈上更新 content，先复制函数对象，避免调用中析构它自己。
        const auto select = content.select;
        const auto page = content.page;
        const auto expand = content.expand;
        const bool grid = content.grid_columns > 0;
        if (item >= 0 && select) select(item);
        else if ((item == kNativeCanvasHitPrevious || item == kNativeCanvasHitNext) && page) page(item == kNativeCanvasHitNext);
        else if (item == kNativeCanvasHitExpand && expand) expand(!grid);
      }
    }
  }
  WaveOverlayWorkArea workArea() const {
    const bool randr = x11_overlay_randr_monitors(display);
    auto work = x11_overlay_monitor(display, randr).work;
    if (!randr) return work;
    const auto root = RootWindow(display, DefaultScreen(display));
    int count = 0;
    if (auto *monitors = XRRGetMonitors(display, root, True, &count)) {
      // 跨显示器的编辑器以光标所在屏幕定位，不能取整个活动窗口的中心。
      for (int at = 0; at < count; ++at) {
        const auto &monitor = monitors[at];
        if (cursor_x < monitor.x || cursor_y < monitor.y ||
            cursor_x >= monitor.x + monitor.width || cursor_y >= monitor.y + monitor.height) continue;
        work = wave_overlay_monitor_work({monitor.x, monitor.y, monitor.width, monitor.height},
            x11_placement::desktop_work_areas(display, root));
        break;
      }
      XRRFreeMonitors(monitors);
    }
    return work;
  }
  bool steadyWidth() const {
    return assets && assets->chrome && content.horizontal && !assets->fit_content && !content.menu && content.grid_columns == 0;
  }
  bool measure() {
    const auto work = workArea();
    auto *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
    auto *cr = cairo_create(surface);
    const bool steady = steadyWidth();
    content.minimum_width = steady && visible ? widest : 0;
    double limit = std::min(work.width, 8192) / scale;
    double previous_width = std::numeric_limits<double>::infinity();
    // 外留白和挂件也占窗口宽度；收紧面板后重新排字，不能在提交窗口时裁切。记住的最宽卡片在换到更窄的屏幕后放不下时先放弃它。
    for (int attempt = 0; attempt < 8; ++attempt) {
      layout = measure_native_canvas(cr, *assets, content, limit);
      inset = native_canvas_shadow_insets(layout, composited);
      const double wanted = std::ceil((layout.width + inset.left + inset.right) * scale);
      if (wanted <= std::min(work.width, 8192)) break;
      if (content.minimum_width > 0) { content.minimum_width = 0; continue; }
      if (wanted >= previous_width) break;
      previous_width = wanted;
      limit = layout.panel.width - (wanted - std::min(work.width, 8192)) / scale - 1;
      if (limit <= 0) break;
    }
    cairo_destroy(cr); cairo_surface_destroy(surface);
    const double wanted_width = std::ceil((layout.width + inset.left + inset.right) * scale);
    const double wanted_height = std::ceil((layout.height + inset.top + inset.bottom) * scale);
    if (wanted_width <= 0 || wanted_height <= 0 || wanted_width > work.width ||
        wanted_height > work.height || wanted_width > 8192 || wanted_height > 8192) return false;
    width = static_cast<int>(wanted_width); height = static_cast<int>(wanted_height);
    measured_work_width = work.width; measured_work_height = work.height;
    const NativeCanvasRect card{inset.left + layout.panel.x, inset.top + layout.panel.y, layout.panel.width, layout.panel.height};
    const std::array<double, 8> key{static_cast<double>(width), static_cast<double>(height), card.x, card.y, card.width,
                                    card.height, layout.metrics.card_radius, scale};
    if (inset.bottom <= 0) shadow.reset();
    else if (!shadow || key != shadow_key) shadow = native_canvas_card_shadow(width, height, card, layout.metrics.card_radius, scale);
    shadow_key = key;
    if (steady) widest = shared::WidestCandidateWidth(widest, layout.width, false, visible);
    return true;
  }
  void menu(int at) {
    if (at < 0 || !content.actions) return;
    auto actions = content.actions(at);
    if (actions.empty()) return;
    const auto original = content;
    NativeCanvasContent menu_content;
    menu_content.font = content.font;
    menu_content.menu = true;
    menu_content.preedit = "候选操作";
    for (const auto &action : actions) menu_content.items.push_back({{}, action.text});
    menu_content.items.push_back({{}, "返回候选"});
    menu_content.select = [this, actions, original](int index) {
      if (index >= 0 && static_cast<std::size_t>(index) < actions.size()) {
        actions[index].invoke();
      } else {
        content = original;
        pressed = hovered = -1;
        if (measure()) { shape(); place(); paint(); }
      }
    };
    content = std::move(menu_content);
    if (!measure()) { content = original; measure(); return; }
    pressed = hovered = -1;
    shape(); place(); paint();
  }
  bool paused() const {
    if (!dpms || !display) return false;
    CARD16 level = DPMSModeOn;
    BOOL enabled = False;
    return DPMSInfo(display, &level, &enabled) && enabled && level != DPMSModeOn;
  }
  // 这一帧的动效状态；退场播完时返回 false，调用方撤下窗口。
  bool motionFrame(std::uint64_t now, NativeCanvasMotion &motion, shared::CardMotionFrame &card, bool &active) {
    active = false;
    const auto &skin = *assets;
    const double duration = shared::MotionSeconds(skin.duration_ms);
    const auto seconds = [now](std::uint64_t start) { return (now - std::min(now, start)) / 1e6; };
    if (exiting) {
      const double t = seconds(exit_start_us) / duration;
      if (t >= 1) return false;
      card = shared::ExitFrame(skin.exit, t, exit_from);
      active = true;
    } else if (entering) {
      const double elapsed = seconds(enter_start_us);
      card = shared::EnterFrame(skin.enter, elapsed / duration);
      double end = duration;
      if (shared::MotionStaggersRows(skin.enter)) {
        for (std::size_t at = 0; at < content.items.size(); ++at) {
          const double delay = shared::MotionRowDelay(at, skin.stagger_ms);
          const double t = (elapsed - delay) / duration;
          motion.rows.push_back(t <= 0 ? shared::CardMotionFrame{0, 1, -shared::kMotionRowRevealDistance}
                                       : shared::RowRevealFrame(std::min(1.0, t)));
          end = std::max(end, delay + duration);
        }
      }
      if (elapsed >= end) { entering = false; card = {}; motion.rows.clear(); }
      else active = true;
    }
    if (selecting) {
      const double t = seconds(selection_start_us) / duration;
      if (t >= 1) selecting = false;
      else { motion.selection = shared::SelectionFrame(skin.selection, t); active = true; }
    }
    if (!reduce_motion && skin.band.image && skin.band.motion != shared::DecorationMotionKind::none &&
        skin.band.period_ms > 0 && layout.band.width > 0 && !content.menu) {
      const double phase = seconds(decoration_anchor_us) * 1000 / skin.band.period_ms;
      motion.decoration = shared::DecorationMotionAt(skin.band.motion, phase, layout.band.width, layout.band.height,
                                                     shared::kDecorationMotionMargin);
    }
    return true;
  }
  void scheduleAnimation(std::uint64_t elapsed_ms, bool short_motion) {
    std::optional<std::uint64_t> next;
    const auto inspect = [&](const std::shared_ptr<const NativeCanvasAnimation> &animation) {
      if (!animation) return;
      const auto sample = sample_native_canvas_animation(*animation, elapsed_ms);
      if (sample.next_ms && (!next || *sample.next_ms < *next)) next = sample.next_ms;
    };
    // 减弱动态效果时帧动画停在首帧；屏幕熄灭时整个画布不再唤醒，下一次重画（按键、换肤）接着播。
    if (!content.menu && !reduce_motion) {
      inspect(assets->panel_animation);
      for (const auto &decoration : assets->decorations) inspect(decoration.animation);
      inspect(assets->band.image);
    }
    if (short_motion) next = std::min(next.value_or(kMotionFrameMs), kMotionFrameMs);
    else if (!reduce_motion && !content.menu && assets->band.motion != shared::DecorationMotionKind::none &&
             assets->band.period_ms > 0 && layout.band.width > 0)
      next = std::min(next.value_or(kIdleMotionFrameMs), kIdleMotionFrameMs);
    if (!next || paused()) {
      if (animation_timer) animation_timer->setEnabled(false);
      return;
    }
    const auto deadline = fcitx::now(CLOCK_MONOTONIC) + *next * 1000;
    if (!animation_timer) {
      animation_timer = loop.addTimeEvent(CLOCK_MONOTONIC, deadline, 0,
          [this](fcitx::EventSourceTime *, std::uint64_t) {
            paint();
            return true;
          });
    } else {
      animation_timer->setTime(deadline);
      animation_timer->setOneShot();
    }
  }
  void paint() {
    if ((!visible && !exiting) || !assets) return;
    const auto now = fcitx::now(CLOCK_MONOTONIC);
    const auto elapsed_ms = reduce_motion ? 0 : (now - std::min(now, animation_started_us)) / 1000;
    NativeCanvasMotion motion;
    shared::CardMotionFrame card;
    bool short_motion = false;
    if (!motionFrame(now, motion, card, short_motion)) { finishExit(); return; }
    auto *buffer = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width, height);
    auto *cr = cairo_create(buffer);
    cairo_scale(cr, scale, scale);
    const bool transformed = card.opacity < 1 || card.scale != 1 || card.lift != 0;
    if (transformed) cairo_push_group(cr);
    // 投影图是设备像素，随卡片一起淡入、缩放；卡片按逻辑像素画在投影边距里面。
    if (shadow) {
      cairo_save(cr);
      cairo_identity_matrix(cr);
      cairo_set_source_surface(cr, shadow.get(), 0, 0);
      cairo_paint(cr);
      cairo_restore(cr);
    }
    cairo_save(cr);
    cairo_translate(cr, inset.left, inset.top);
    paint_native_canvas(cr, *assets, content, layout, palette, elapsed_ms,
                        exiting ? NativeCanvasPointer{} : NativeCanvasPointer{hovered, pressed}, motion);
    cairo_restore(cr);
    if (transformed) {
      // 卡片动效以卡片中心为轴缩放、在屏幕方向上抬起（Linux 的 y 朝下，抬起是负方向），整张淡入淡出。
      auto *group = cairo_pop_group(cr);
      const auto &panel = layout.panel;
      const double cx = inset.left + panel.x + panel.width / 2, cy = inset.top + panel.y + panel.height / 2;
      cairo_save(cr);
      cairo_translate(cr, cx, cy - card.lift);
      cairo_scale(cr, card.scale, card.scale);
      cairo_translate(cr, -cx, -cy);
      cairo_set_source(cr, group);
      cairo_paint_with_alpha(cr, std::clamp(card.opacity, 0.0, 1.0));
      cairo_restore(cr);
      cairo_pattern_destroy(group);
    }
    if (!composited) {
      cairo_surface_flush(buffer);
      const int stride = cairo_image_surface_get_stride(buffer);
      const auto *pixels = cairo_image_surface_get_data(buffer);
      const auto row_bytes = static_cast<std::size_t>((width + 7) / 8);
      std::vector<char> bits(row_bytes * static_cast<std::size_t>(height), 0);
      for (int y = 0; y < height; ++y) for (int x = 0; x < width; ++x) {
        std::uint32_t pixel = 0;
        std::memcpy(&pixel, pixels + y * stride + x * 4, 4);
        if (pixel >> 24) bits[static_cast<std::size_t>(y) * row_bytes + x / 8] |= static_cast<char>(1u << (x % 8));
      }
      const auto mask = XCreateBitmapFromData(display, window, bits.data(), static_cast<unsigned>(width), static_cast<unsigned>(height));
      XShapeCombineMask(display, window, ShapeBounding, 0, 0, mask, ShapeSet);
      XFreePixmap(display, mask);
      // 无合成器只能提供二值透明；半透明像素合在主题底色上，不能落成黑边。
      cairo_set_operator(cr, CAIRO_OPERATOR_DEST_OVER);
      native_canvas_color(cr, palette, "surface", "#f8f8f8");
      cairo_paint(cr);
    }
    cairo_destroy(cr);
    auto *surface = cairo_xlib_surface_create(display, window, visual, width, height);
    cr = cairo_create(surface);
    cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
    cairo_set_source_surface(cr, buffer, 0, 0);
    cairo_paint(cr);
    cairo_destroy(cr);
    cairo_surface_destroy(surface); cairo_surface_destroy(buffer);
    XFlush(display);
    scheduleAnimation(elapsed_ms, short_motion);
  }
  void shape(bool clickable = true) {
    const auto region = XCreateRegion();
    const auto add = [&](const NativeCanvasRect &box) {
      if (box.width <= 0 || box.height <= 0) return;
      XRectangle rectangle{static_cast<short>(std::floor((inset.left + box.x) * scale)),
          static_cast<short>(std::floor((inset.top + box.y) * scale)),
          static_cast<unsigned short>(std::ceil(box.width * scale)), static_cast<unsigned short>(std::ceil(box.height * scale))};
      XUnionRectWithRegion(&rectangle, region, region);
    };
    if (clickable) {
      for (const auto &box : layout.items) add(box);
      // 禁用箭头仍占输入区域，点击不应穿透到编辑器或下面的窗口。
      if (layout.chrome ? layout.page_controls && content.grid_columns == 0 : layout.pager.visible) { add(layout.previous); add(layout.next); }
      add(layout.expand);
    }
    // 挂件和留白不吞鼠标；候选与箭头才接收点击。退场中的窗口一点也不接收。
    XShapeCombineRegion(display, window, ShapeInput, 0, 0, region, ShapeSet);
    XDestroyRegion(region);
  }
  void place() {
    if (!display || !window || !assets) return;
    const auto work = workArea();
    // 卡片外的留白：顶上是装饰带或外留白，左、下是 canvas 的外留白，再加上投影边距；翻不翻到光标上方按记住的最高一页定。
    const double card_top = inset.top + layout.panel.y, card_left = inset.left + layout.panel.x;
    const double card_bottom = std::max(0.0, layout.height + inset.bottom - layout.panel.y - layout.panel.height);
    const auto [x, y] = native_canvas_window_origin(scale, cursor_x, cursor_y, cursor_height, width, height,
        work.x, work.y, work.width, work.height, content.horizontal ? 0 : tallest / scale, card_top, card_left, card_bottom);
    XMoveResizeWindow(display, window, x, y, static_cast<unsigned>(width), static_cast<unsigned>(height));
    XFlush(display);
  }
  void finishExit() {
    exiting = false;
    if (display && window) {
      XUnmapWindow(display, window);
      XFlush(display);
    }
    if (animation_timer) animation_timer->setEnabled(false);
    content = {};
  }
};

NativeCanvasX11::NativeCanvasX11(fcitx::EventLoop &loop) : impl_(std::make_unique<Impl>(loop)) {}
NativeCanvasX11::~NativeCanvasX11() = default;
bool NativeCanvasX11::show(const std::string &display, std::shared_ptr<NativeCanvasAssets> assets,
                           NativeCanvasContent content, nlohmann::json palette,
                           int cursor_x, int cursor_y, int cursor_height) {
  auto &state = *impl_;
  if (!assets || content.items.empty() || !state.open(display)) { hide(); return false; }
  const auto now = fcitx::now(CLOCK_MONOTONIC);
  const bool was_visible = state.visible, was_exiting = state.exiting;
  // 正在退场时重新显示：撤掉退场，以完整不透明度接着显示，不再播进场。
  state.exiting = false;
  state.pressed = state.hovered = -1;
  if (!state.visible || state.assets != assets) state.animation_started_us = now;
  const bool new_skin = state.assets != assets;
  state.assets = std::move(assets); state.content = std::move(content); state.palette = std::move(palette);
  state.cursor_x = cursor_x; state.cursor_y = cursor_y; state.cursor_height = cursor_height;
  state.scale = x11_overlay_scale(state.display);
  if (!was_visible) state.tallest = state.widest = 0;
  if (!state.measure()) { hide(); return false; }
  const auto &skin = *state.assets;
  const bool burst = state.last_hide_us > 0 && now - state.last_hide_us < static_cast<std::uint64_t>(shared::kMotionTypingBurstSeconds * 1e6);
  // 停下来之后再出现，装饰图从原位开始动；同一段连续输入里收起又出现，接着原来的相位。
  if (!was_visible && (state.decoration_anchor_us == 0 || !burst)) state.decoration_anchor_us = now;
  // 进场只在候选窗从隐藏到出现、没有在退场、不是连续输入、皮肤写了进场预设、桌面没有减弱动态效果时播；没有合成器时窗口只有二值透明，淡入和缩放都画不出来。
  if (!was_visible) {
    state.entering = !was_exiting && !burst && !state.reduce_motion && state.composited &&
        skin.enter != shared::MotionPreset::none && skin.duration_ms > 0;
    state.enter_start_us = now;
  } else if (new_skin) {
    state.entering = false;
  }
  // 选中装饰：同一段组字、同一页里高亮换了一行。
  const int highlighted = state.content.selected;
  const auto &context = state.content.selection_context;
  if (was_visible && !state.reduce_motion && skin.selection != shared::MotionPreset::none && skin.duration_ms > 0 &&
      context == state.highlight_context && state.highlighted >= 0 && highlighted >= 0 && highlighted != state.highlighted &&
      state.content.grid_columns == 0) {
    state.selecting = true;
    state.selection_start_us = now;
  } else if (highlighted != state.highlighted || context != state.highlight_context) {
    state.selecting = false;
  }
  state.highlighted = highlighted;
  state.highlight_context = context;
  if (!state.content.horizontal) state.tallest = shared::TallestCandidateHeight(state.tallest, state.height, true, was_visible);
  state.place();
  state.shape();
  XMapRaised(state.display, state.window);
  state.visible = true;
  state.paint();
  return true;
}
bool NativeCanvasX11::move(int x, int y, int height) {
  auto &state = *impl_;
  if (!state.visible) return true;
  state.cursor_x = x; state.cursor_y = y; state.cursor_height = height;
  const auto work = state.workArea();
  const bool resized = work.width != state.measured_work_width || work.height != state.measured_work_height;
  if (resized) {
    // 单独移动光标也可能换到更窄的屏幕；重排后旧指针格位失效。
    state.pressed = state.hovered = -1;
    if (!state.measure()) { hide(); return false; }
    state.shape();
  }
  state.place();
  if (resized) state.paint();
  return true;
}
void NativeCanvasX11::hide(bool animate) {
  auto &state = *impl_;
  state.pressed = state.hovered = -1;
  const bool was_visible = state.visible;
  const auto now = fcitx::now(CLOCK_MONOTONIC);
  // 进场播到一半就收起时，退场从卡片此刻的样子开始，不先跳回完整显示。
  shared::CardMotionFrame current;
  if (was_visible && state.entering && state.assets) {
    NativeCanvasMotion motion;
    bool active = false;
    state.motionFrame(now, motion, current, active);
  }
  state.visible = false;
  state.entering = state.selecting = false;
  state.highlighted = -1;
  state.highlight_context.clear();
  if (was_visible) state.last_hide_us = now;
  if (state.display && state.window && state.hand_shown) {
    XUndefineCursor(state.display, state.window);
    state.hand_shown = false;
  }
  // 退场从卡片此刻的样子开始；窗口留在屏幕上播完，期间不接收鼠标。
  if (animate && was_visible && state.assets && state.display && state.window && state.composited && !state.reduce_motion &&
      state.assets->exit != shared::MotionPreset::none && state.assets->duration_ms > 0) {
    state.exit_from = current;
    state.exiting = true;
    state.exit_start_us = now;
    state.shape(false);
    state.paint();
    return;
  }
  // 已经收起时再普通地收一次（上屏后又一个没有候选的键）不打断正在播的退场；失焦这类立即收起要把退场中的窗口一起撤下。
  if (animate && !was_visible && state.exiting) return;
  const bool was_exiting = state.exiting;
  state.exiting = false;
  if ((was_visible || was_exiting) && state.display && state.window) {
    XUnmapWindow(state.display, state.window);
    XFlush(state.display);
  }
  if (state.animation_timer) state.animation_timer->setEnabled(false);
  state.content = {};
}
void NativeCanvasX11::setReduceMotion(bool reduce) {
  auto &state = *impl_;
  if (state.reduce_motion == reduce) return;
  state.reduce_motion = reduce;
  if (reduce) {
    state.entering = state.selecting = false;
    if (state.exiting) state.finishExit();
  }
  if (state.visible) state.paint();
}

} // msime::linux_host 命名空间
