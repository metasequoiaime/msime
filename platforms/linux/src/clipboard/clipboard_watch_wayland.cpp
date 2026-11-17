#include "WaylandClipboardBridge.h"
#include "../core/AuxiliaryCrashCapture.h"

int main() {
  msime::linux_host::AuxiliaryCrashCapture capture("clipboard-watch-wayland");
  return msime_run_wayland_clipboard_bridge();
}
