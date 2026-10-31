#include "../core/AuxiliaryCrashCapture.h"
#include <X11/Xlib.h>
#include <X11/Xatom.h>
#include <X11/extensions/Xfixes.h>
#include <cstdio>
#include <chrono>
#include <cstring>
#include <poll.h>
#include <string>
#include <algorithm>

namespace {
constexpr size_t kMaxBytes = 12000;
bool read_clipboard(Display *display) {
  const Window window = XCreateSimpleWindow(display, DefaultRootWindow(display), 0, 0, 1, 1, 0, 0, 0);
  XSelectInput(display, window, PropertyChangeMask);
  const Atom clipboard = XInternAtom(display, "CLIPBOARD", False);
  const Atom property = XInternAtom(display, "MSIME_CLIPBOARD_READ", False);
  const Atom utf8 = XInternAtom(display, "UTF8_STRING", False);
  const Atom incr = XInternAtom(display, "INCR", False);
  Atom target = utf8, encoding = None;
  bool incremental = false, complete = false, failed = false;
  std::string text;
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(750);
  XConvertSelection(display, clipboard, target, property, window, CurrentTime);
  XFlush(display);
  while (!complete && !failed && std::chrono::steady_clock::now() < deadline) {
    if (!XPending(display)) {
      const auto remaining = std::chrono::duration_cast<std::chrono::milliseconds>(
          deadline - std::chrono::steady_clock::now()).count();
      if (remaining <= 0) break;
      pollfd connection{ConnectionNumber(display), POLLIN, 0};
      if (poll(&connection, 1, static_cast<int>(remaining)) <= 0) break;
    }
    XEvent event{};
    XNextEvent(display, &event);
    if (!incremental) {
      if (event.type != SelectionNotify || event.xselection.requestor != window ||
          event.xselection.selection != clipboard || event.xselection.target != target) continue;
      if (event.xselection.property == None) {
        if (target == XA_STRING) break;
        target = XA_STRING;
        XConvertSelection(display, clipboard, target, property, window, CurrentTime);
        XFlush(display);
        continue;
      }
      if (event.xselection.property != property) break;
    } else if (event.type != PropertyNotify || event.xproperty.window != window ||
               event.xproperty.atom != property || event.xproperty.state != PropertyNewValue) {
      continue;
    }
    Atom type = None;
    int format = 0;
    unsigned long count = 0, after = 0;
    unsigned char *bytes = nullptr;
    const int status = XGetWindowProperty(display, window, property, 0,
        static_cast<long>((kMaxBytes + 3) / 4), True, AnyPropertyType,
        &type, &format, &count, &after, &bytes);
    if (status != Success) {
      if (bytes) XFree(bytes);
      break;
    }
    if (!incremental && type == incr && format == 32 && count == 1) {
      incremental = true;
      // XGetWindowProperty(True) already deleted the INCR property. Deleting
      // again can acknowledge a chunk the owner has just published.
      XFlush(display);
    } else if (format == 8 && (type == utf8 || type == XA_STRING) &&
               (encoding == None || encoding == type)) {
      encoding = type;
      const size_t copied = std::min(static_cast<size_t>(count), kMaxBytes - text.size());
      if (copied) text.append(reinterpret_cast<char *>(bytes), copied);
      complete = !incremental || count == 0 || text.size() == kMaxBytes;
      XFlush(display);
    } else {
      failed = true;
    }
    if (bytes) XFree(bytes);
  }
  XDestroyWindow(display, window);
  if (!complete || failed || text.empty()) return false;
  if (encoding == XA_STRING) {
    std::string converted;
    for (unsigned char ch : text) {
      if (ch < 128) converted.push_back(static_cast<char>(ch));
      else {
        converted.push_back(static_cast<char>(0xc0 | (ch >> 6)));
        converted.push_back(static_cast<char>(0x80 | (ch & 0x3f)));
      }
      if (converted.size() >= kMaxBytes) break;
    }
    text = converted.substr(0, kMaxBytes);
  }
  return std::fwrite(text.data(), 1, text.size(), stdout) == text.size();
}
} // namespace

// Watch mode emits markers; --read returns bounded bytes for the monitor decoder.
int main(int argc, char **argv) {
  const bool read = argc == 2 && std::strcmp(argv[1], "--read") == 0;
  if (argc != 1 && !read) return 2;
  msime::linux_host::AuxiliaryCrashCapture crash_capture("clipboard-watch-x11");
  Display *display = XOpenDisplay(nullptr);
  if (!display) return 1;
  if (read) {
    const bool ok = read_clipboard(display);
    XCloseDisplay(display);
    return ok ? 0 : 1;
  }
  int event_base = 0, error_base = 0;
  if (!XFixesQueryExtension(display, &event_base, &error_base)) {
    XCloseDisplay(display);
    return 1;
  }
  const Atom clipboard = XInternAtom(display, "CLIPBOARD", False);
  XFixesSelectSelectionInput(display, DefaultRootWindow(display), clipboard,
      XFixesSetSelectionOwnerNotifyMask | XFixesSelectionWindowDestroyNotifyMask |
      XFixesSelectionClientCloseNotifyMask);
  XFlush(display);
  std::puts("");
  std::fflush(stdout);
  for (;;) {
    XEvent event{};
    XNextEvent(display, &event);
    if (event.type == event_base + XFixesSelectionNotify) {
      if (std::puts("") == EOF || std::fflush(stdout) != 0) break;
    }
  }
  XCloseDisplay(display);
  return 0;
}
