#include "HandwritingWindow.h"
#include "ClipboardWindow.h"
#include "CloudClipboardWindow.h"
#include "VocabularyWindow.h"
#include "CloudDictionaryWindow.h"
#include "EmojiWindow.h"
#include "KeyboardWindow.h"
#include "VoiceWindow.h"
#include "../../compat/WindowsDpi.h"
#include "../../common/SurfaceRouteArgument.h"

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR, int show) {
  const auto requested = msime::windows::requested_surface_route("handwriting");
  if (!requested) return 2;
  const auto& route = *requested;
  if (route != "handwriting" && route != "clipboard" && route != "cloud-clipboard" && route != "vocabulary" && route != "cloud-dictionary" && route != "emoji" && route != "keyboard" && route != "voice") return 2;
  msime::windows::compat::process_context(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
  if (route == "voice") {
    msime::windows::win7::VoiceWindow window;
    if (!window.create(instance)) return 1;
    return window.run(show);
  }
  if (route == "keyboard") {
    msime::windows::win7::KeyboardWindow window;
    if (!window.create(instance)) return 1;
    return window.run(show);
  }
  if (route == "emoji") {
    msime::windows::win7::EmojiWindow window;
    if (!window.create(instance)) return 1;
    return window.run(show);
  }
  if (route == "cloud-dictionary") {
    msime::windows::win7::CloudDictionaryWindow window;
    if (!window.create(instance)) return 1;
    return window.run(show);
  }
  if (route == "vocabulary") {
    msime::windows::win7::VocabularyWindow window;
    if (!window.create(instance)) return 1;
    return window.run(show);
  }
  if (route == "cloud-clipboard") {
    msime::windows::win7::CloudClipboardWindow window;
    if (!window.create(instance)) return 1;
    return window.run(show);
  }
  if (route == "clipboard") {
    msime::windows::win7::ClipboardWindow window;
    if (!window.create(instance)) return 1;
    return window.run(show);
  }
  msime::windows::win7::HandwritingWindow window;
  if (!window.create(instance)) return 1;
  return window.run(show);
}
