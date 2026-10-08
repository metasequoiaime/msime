#include "../src/core/FirstRunGuidance.h"
#include "../src/core/RuntimeOptionsFile.h"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <string>
#include <unistd.h>

using msime::linux_host::locate_runtime_options;
using msime::linux_host::RuntimeOptionsState;

int main() {
  char pattern[] = "/tmp/msime-first-run-guidance-XXXXXX";
  const char *created = mkdtemp(pattern);
  assert(created != nullptr);
  const std::filesystem::path root(created);
  const auto config_home = root / "config";
  const auto home = root / "home";
  const auto user_file = config_home / "msime-client/runtime-options.json";
  const auto system_file = root / "etc/msime-client/runtime-options.json";
  const auto config = config_home.string();
  const auto home_value = home.string();

  // Nothing prepared anywhere: the one state that is first-run rather than a broken configuration.
  auto located = locate_runtime_options(nullptr, config.c_str(), nullptr, system_file);
  assert(located.state == RuntimeOptionsState::NotConfigured);

  // HOME/.config stands in for an unset or empty XDG_CONFIG_HOME, with the same outcome.
  located = locate_runtime_options(nullptr, "", home_value.c_str(), system_file);
  assert(located.state == RuntimeOptionsState::NotConfigured);

  // The system file an administrator installed is a configuration, not first run.
  std::filesystem::create_directories(system_file.parent_path());
  std::ofstream(system_file) << "{}";
  located = locate_runtime_options(nullptr, config.c_str(), nullptr, system_file);
  assert(located.state == RuntimeOptionsState::Found && located.path == system_file);

  // A user file takes precedence over the system one.
  std::filesystem::create_directories(user_file.parent_path());
  std::ofstream(user_file) << "{}";
  located = locate_runtime_options(nullptr, config.c_str(), nullptr, system_file);
  assert(located.state == RuntimeOptionsState::Found && located.path == user_file);

  // A dangling user symlink is a broken configuration: it is reported as found and fails when read, never as first run.
  std::filesystem::remove(user_file);
  std::filesystem::remove(system_file);
  std::filesystem::create_symlink(root / "missing.json", user_file);
  located = locate_runtime_options(nullptr, config.c_str(), nullptr, system_file);
  assert(located.state == RuntimeOptionsState::Found && located.path == user_file);
  std::filesystem::remove(user_file);

  // An explicit override is taken as given even when it is missing, so it fails visibly instead of opening setup for a different file.
  const auto override_file = (root / "elsewhere.json").string();
  located = locate_runtime_options(override_file.c_str(), config.c_str(), nullptr, system_file);
  assert(located.state == RuntimeOptionsState::Found && located.path == override_file);
  assert(locate_runtime_options("relative.json", config.c_str(), nullptr, system_file).state ==
         RuntimeOptionsState::Invalid);
  assert(locate_runtime_options("", config.c_str(), nullptr, system_file).state == RuntimeOptionsState::Invalid);

  // A relative XDG_CONFIG_HOME is invalid rather than silently replaced by HOME, as the addon always behaved.
  assert(locate_runtime_options(nullptr, "relative", home_value.c_str(), system_file).state ==
         RuntimeOptionsState::Invalid);
  assert(locate_runtime_options(nullptr, nullptr, nullptr, system_file).state == RuntimeOptionsState::Invalid);

  // The hint names the entry users find in their application list and the terminal command, matching the .desktop name and msime-linux-setup.
  const std::string hint(msime::linux_host::kFirstRunHint);
  assert(hint.find("「水杉输入法」") != std::string::npos);
  assert(hint.find("msime-linux-setup") != std::string::npos);
  assert(msime::linux_host::kFirstRunGuideProgram == "msime-linux-first-run-guide");

  // Runtime hosts must not follow a replacement symlink when they reload the
  // options document. Keep the regression at the shared reader boundary so
  // both IBus and Fcitx5 exercise the same no-follow contract.
  const auto options_target = root / "options-target.json";
  const auto options_link = root / "options-link.json";
  std::ofstream(options_target) << "{}";
  std::filesystem::create_symlink(options_target, options_link);
  bool rejected = false;
  try {
    (void)msime::linux_host::read_runtime_options(options_link);
  } catch (const std::exception &) {
    rejected = true;
  }
  assert(rejected);

  std::error_code error;
  std::filesystem::remove_all(root, error);
  return 0;
}
