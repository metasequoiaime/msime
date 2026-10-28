#include "SettingsWindow.h"
#include "../../../common/Telemetry.h"
#include "../../common/StateDirectory.h"
#include <exception>
#include <cstdlib>
#include "../../common/SurfaceRouteArgument.h"
#include "SettingsRoutes.h"
#include "../../compat/WindowsDpi.h"
#include "../../common/GameCompatProfile.h"
#include "../SettingsInstance.h"

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR command, int show) {
  const int game_profile = msime::windows::game_profile::run_if_requested(command);
  if (game_profile >= 0) return game_profile;
  const auto route = msime::windows::requested_surface_route("settings");
  if (!route || !msime::settings::win7::settings_page_for_route(*route)) return 2;
  msime::settings::SettingsInstance instance_guard;
  const auto activation = instance_guard.start(*route);
  if (activation != msime::settings::SettingsInstance::Start::primary)
    return activation == msime::settings::SettingsInstance::Start::forwarded ? 0 : 1;
  msime::settings::win7::start_usage_reporting();
  msime::telemetry::ConsentObserver reporting_observer;
  msime::windows::compat::process_context(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
  msime::settings::win7::SettingsWindow window;
  if (!window.open_route(*route)) return 2;
  if (!window.create(instance)) return 1;
  if (!instance_guard.listen(window.handle(), [&window](std::string_view request) {
        return request == "settings" || window.open_route(request);
      })) return 1;
  return window.run(show);
}
