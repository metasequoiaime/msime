#include "HandwritingWindow.h"
#include "PanelContext.h"
#include "../../../common/Telemetry.h"
#include <exception>
#include <cstdlib>
#include "ClipboardWindow.h"
#include "CloudClipboardWindow.h"
#include "VocabularyWindow.h"
#include "CloudDictionaryWindow.h"
#include "DictionarySyncWindow.h"
#include "EmojiWindow.h"
#include "KeyboardWindow.h"
#include "VoiceWindow.h"
#include "../../compat/WindowsDpi.h"
#include "../../common/SurfaceRouteArgument.h"

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR, int show) {
  const auto requested = msime::windows::requested_surface_route("handwriting");
  if (!requested) return 2;
  const auto& route = *requested;
  if (route != "handwriting" && route != "clipboard" && route != "cloud-clipboard" && route != "vocabulary" && route != "cloud-dictionary" && route != "dictionary-sync" && route != "emoji" && route != "keyboard" && route != "voice") return 2;
  auto reporting_directory = msime::telemetry::default_directory();
  if (!reporting_directory.empty()) reporting_directory = reporting_directory.parent_path() / MSIME_EDITION_USER_DATA_DIRECTORY;
  msime::telemetry::install_crash_handlers();
  msime::telemetry::begin({"windows", MSIME_WINDOWS_VERSION, reporting_directory, std::nullopt,
      msime::windows::win7::panel_context().directory, "panels-win7", true});
  msime::telemetry::ConsentObserver reporting_observer;
  std::set_terminate([] { msime::telemetry::record_terminate(); std::abort(); });
  msime::telemetry::start_flushing();
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
  if (route == "dictionary-sync") {
    msime::windows::win7::DictionarySyncWindow window;
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
