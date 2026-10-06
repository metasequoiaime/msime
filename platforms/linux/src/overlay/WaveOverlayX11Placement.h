#pragma once

#include "WaveOverlayPlacement.h"

#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xresource.h>
#include <X11/extensions/Xrandr.h>

#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <locale>
#include <optional>
#include <sstream>
#include <string>
#include <vector>

namespace msime::linux_host {

// The X11 queries behind WaveOverlayPlacement.h, shared by every override-redirect overlay (the voice bar and the mode badge) so they land on the same monitor, work area and scale.
namespace x11_placement {

inline std::vector<std::uint32_t> cardinal_property(Display *display,
                                                    Window window,
                                                    Atom property,
                                                    Atom type = XA_CARDINAL) {
  Atom actual_type = None;
  int actual_format = 0;
  unsigned long item_count = 0;
  unsigned long bytes_after = 0;
  unsigned char *raw = nullptr;
  const auto status = XGetWindowProperty(
      display, window, property, 0, 4096, False, type, &actual_type,
      &actual_format, &item_count, &bytes_after, &raw);
  if (status != Success || actual_type != type || actual_format != 32 || !raw) {
    if (raw)
      XFree(raw);
    return {};
  }
  const auto *values = reinterpret_cast<const unsigned long *>(raw);
  std::vector<std::uint32_t> result;
  result.reserve(item_count);
  for (unsigned long index = 0; index < item_count; ++index)
    result.push_back(static_cast<std::uint32_t>(values[index]));
  XFree(raw);
  return result;
}

inline std::optional<double> parse_number(const char *text) {
  if (!text || !*text)
    return std::nullopt;
  // The host process may run under a locale whose decimal separator is a comma; X resources and GDK_SCALE always use a period.
  std::istringstream stream(text);
  stream.imbue(std::locale::classic());
  double value = 0.0;
  if (!(stream >> value) || value <= 0.0)
    return std::nullopt;
  return value;
}

// Xft.dpi from the RESOURCE_MANAGER property, read from the root window each time because the copy Xlib takes at XOpenDisplay goes stale on this long-lived connection when the desktop changes its scale.
inline std::optional<double> xft_dpi(Display *display, Window root) {
  std::string resources;
  Atom actual_type = None;
  int actual_format = 0;
  unsigned long item_count = 0;
  unsigned long bytes_after = 0;
  unsigned char *raw = nullptr;
  if (XGetWindowProperty(display, root, XA_RESOURCE_MANAGER, 0, 1 << 16, False,
                         XA_STRING, &actual_type, &actual_format, &item_count,
                         &bytes_after, &raw) == Success &&
      actual_type == XA_STRING && actual_format == 8 && raw)
    resources.assign(reinterpret_cast<const char *>(raw), item_count);
  if (raw)
    XFree(raw);
  if (resources.empty()) {
    if (const auto *initial = XResourceManagerString(display))
      resources = initial;
  }
  if (resources.empty())
    return std::nullopt;
  XrmInitialize();
  const auto database = XrmGetStringDatabase(resources.c_str());
  if (!database)
    return std::nullopt;
  char *type = nullptr;
  XrmValue value{};
  std::optional<double> dpi;
  if (XrmGetResource(database, "Xft.dpi", "Xft.Dpi", &type, &value) &&
      value.addr)
    dpi = parse_number(value.addr);
  XrmDestroyDatabase(database);
  return dpi;
}

inline std::vector<WaveOverlayWorkArea>
work_area_rectangles(const std::vector<std::uint32_t> &values,
                     std::size_t first, std::size_t count) {
  std::vector<WaveOverlayWorkArea> areas;
  areas.reserve(count);
  for (std::size_t index = first; index < first + count; ++index) {
    const auto offset = index * 4;
    if (offset + 4 > values.size())
      break;
    const WaveOverlayWorkArea area{
        static_cast<std::int32_t>(values[offset]),
        static_cast<std::int32_t>(values[offset + 1]),
        static_cast<std::int32_t>(values[offset + 2]),
        static_cast<std::int32_t>(values[offset + 3])};
    if (area.width > 0 && area.height > 0)
      areas.push_back(area);
  }
  return areas;
}

// The current desktop's work areas: Mutter's per-monitor _GTK_WORKAREAS_D<n> when it publishes one, otherwise the single EWMH _NET_WORKAREA rectangle.
inline std::vector<WaveOverlayWorkArea> desktop_work_areas(Display *display,
                                                           Window root) {
  std::size_t desktop = 0;
  const auto desktop_atom = XInternAtom(display, "_NET_CURRENT_DESKTOP", True);
  if (desktop_atom != None) {
    const auto current = cardinal_property(display, root, desktop_atom);
    if (!current.empty())
      desktop = current.front();
  }
  const auto gtk_name = "_GTK_WORKAREAS_D" + std::to_string(desktop);
  const auto gtk_atom = XInternAtom(display, gtk_name.c_str(), True);
  if (gtk_atom != None) {
    const auto values = cardinal_property(display, root, gtk_atom);
    auto areas = work_area_rectangles(values, 0, values.size() / 4);
    if (!areas.empty())
      return areas;
  }
  const auto workarea_atom = XInternAtom(display, "_NET_WORKAREA", True);
  if (workarea_atom == None)
    return {};
  const auto values = cardinal_property(display, root, workarea_atom);
  const auto desktop_count = values.size() / 4;
  if (desktop >= desktop_count)
    desktop = 0;
  return work_area_rectangles(values, desktop, 1);
}

inline int ignore_x_error(Display *, XErrorEvent *) { return 0; }

// Centre of the EWMH active window in root coordinates. The window can be destroyed between reading _NET_ACTIVE_WINDOW and querying it, and Xlib's default handler exits the process on the resulting BadWindow, so the queries run under a handler that ignores errors.
inline std::optional<WaveOverlayPosition> active_window_center(Display *display,
                                                               Window root) {
  const auto active_atom = XInternAtom(display, "_NET_ACTIVE_WINDOW", True);
  if (active_atom == None)
    return std::nullopt;
  const auto values = cardinal_property(display, root, active_atom, XA_WINDOW);
  if (values.empty() || values.front() == None)
    return std::nullopt;
  const Window active = values.front();
  XSync(display, False);
  const auto previous = XSetErrorHandler(ignore_x_error);
  std::optional<WaveOverlayPosition> center;
  XWindowAttributes attributes{};
  int x = 0;
  int y = 0;
  Window child = 0;
  if (XGetWindowAttributes(display, active, &attributes) &&
      attributes.map_state == IsViewable &&
      XTranslateCoordinates(display, active, root, attributes.width / 2,
                            attributes.height / 2, &x, &y, &child))
    center = WaveOverlayPosition{x, y};
  XSync(display, False);
  XSetErrorHandler(previous);
  return center;
}

inline std::optional<WaveOverlayPosition> pointer_position(Display *display,
                                                           Window root) {
  Window root_return = 0;
  Window child = 0;
  int root_x = 0;
  int root_y = 0;
  int window_x = 0;
  int window_y = 0;
  unsigned int mask = 0;
  if (!XQueryPointer(display, root, &root_return, &child, &root_x, &root_y,
                     &window_x, &window_y, &mask))
    return std::nullopt;
  return WaveOverlayPosition{root_x, root_y};
}

} // namespace x11_placement

// RandR 1.5 monitor enumeration; without it an overlay is placed on the EWMH work area or the root window.
inline bool x11_overlay_randr_monitors(Display *display) {
  int event_base = 0;
  int error_base = 0;
  int major = 0;
  int minor = 0;
  return XRRQueryExtension(display, &event_base, &error_base) &&
         XRRQueryVersion(display, &major, &minor) &&
         (major > 1 || (major == 1 && minor >= 5));
}

// The desktop scale, read fresh on every call so a changed Xft.dpi applies on the next show.
inline double x11_overlay_scale(Display *display) {
  const auto root = RootWindow(display, DefaultScreen(display));
  return wave_overlay_scale(
      x11_placement::xft_dpi(display, root),
      x11_placement::parse_number(std::getenv("GDK_SCALE")));
}

// The monitor an overlay belongs on: the one holding the focused window's centre, else the one under the pointer, else the primary, with its work area. Without RandR 1.5 the whole EWMH work area, or the root window, stands in for the monitor, which keeps its multi-head negative coordinates.
inline WaveOverlayMonitor x11_overlay_monitor(Display *display,
                                              bool randr_monitors) {
  const auto screen = DefaultScreen(display);
  const auto root = RootWindow(display, screen);
  const auto work_areas = x11_placement::desktop_work_areas(display, root);
  const WaveOverlayWorkArea root_area{0, 0, DisplayWidth(display, screen),
                                      DisplayHeight(display, screen)};
  const auto fallback = work_areas.empty() ? root_area : work_areas.front();
  WaveOverlayMonitor target{fallback, fallback};
  if (!randr_monitors)
    return target;
  std::vector<WaveOverlayMonitor> monitors;
  std::optional<std::size_t> primary;
  int count = 0;
  if (auto *infos = XRRGetMonitors(display, root, True, &count)) {
    monitors.reserve(static_cast<std::size_t>(count));
    for (int index = 0; index < count; ++index) {
      const WaveOverlayWorkArea full{infos[index].x, infos[index].y,
                                     infos[index].width, infos[index].height};
      if (full.width <= 0 || full.height <= 0)
        continue;
      if (infos[index].primary)
        primary = monitors.size();
      monitors.push_back({full, full});
    }
    XRRFreeMonitors(infos);
  }
  std::optional<WaveOverlayPosition> focus;
  std::optional<WaveOverlayPosition> pointer;
  // A single monitor needs no focus or pointer round-trips. With several, the pointer is read even when there is an active window, because that window's centre can lie outside every monitor (dragged partly off-screen, or in the dead zone beside a shorter monitor) and the pick then falls through to the pointer.
  if (monitors.size() > 1) {
    focus = x11_placement::active_window_center(display, root);
    pointer = x11_placement::pointer_position(display, root);
  }
  if (const auto index =
          wave_overlay_pick_monitor(monitors, focus, pointer, primary)) {
    target.full = monitors[*index].full;
    target.work = wave_overlay_monitor_work(target.full, work_areas);
  }
  return target;
}

} // namespace msime::linux_host
