#include "FirstRun.h"
#include <chrono>
#include <iostream>

int main() {
  namespace fs = std::filesystem;
  const auto root = fs::temp_directory_path() /
      ("msime-first-run-" + std::to_string(
          std::chrono::steady_clock::now().time_since_epoch().count()));
  if (!fs::create_directory(root)) return 1;
  try {
    const auto executable = root / "用户目录" / "installed server";
    fs::create_directories(executable / "resources");
    const auto state = root / "用户目录" / "new user state";
    int calls = 0;
    auto host = [&](const std::string &request) {
      ++calls;
      const auto options = nlohmann::json::parse(request);
      if (options.at("resources") != fs::canonical(executable / "resources").u8string())
        throw std::runtime_error("Incorrect packaged resource path");
      if (options.at("state_root") != state.u8string())
        throw std::runtime_error("Incorrect Unicode state path");
      return nlohmann::json{{"ok", true}, {"value", options}}.dump();
    };
    if (!msime::windows::prepare_first_run(executable, state, host) || calls != 1 ||
        !fs::is_regular_file(state / "runtime-options.json"))
      throw std::runtime_error("First launch did not publish configuration");
    if (msime::windows::prepare_first_run(executable, state, host) || calls != 1)
      throw std::runtime_error("Second launch prepared existing state");
    fs::remove(state / "runtime-options.json");
    if (msime::windows::prepare_first_run(executable, state, host) || calls != 1)
      throw std::runtime_error("Incomplete state was rebuilt");
    // 别的版本的标记文件名不同（本版本的加上它的名字后缀），不能让本版本接管那个目录。
    const auto foreign = state / (std::wstring(msime::windows::kDataDirectoryMarker) + L".foreign");
    std::ofstream(foreign) << "synthetic foreign ownership";
    if (msime::windows::prepare_first_run(executable, state, host) || calls != 1)
      throw std::runtime_error("Another edition's state was prepared");
    fs::remove(foreign);
    std::ofstream(state / msime::windows::kDataDirectoryMarker) << "synthetic ownership";
    if (!msime::windows::prepare_first_run(executable, state, host) || calls != 2 ||
        !fs::is_regular_file(state / "runtime-options.json"))
      throw std::runtime_error("Installer-owned state was not prepared");
    fs::remove(state / "runtime-options.json");
    if (!msime::windows::prepare_first_run(executable, state, host) || calls != 3)
      throw std::runtime_error("Installer-owned state was not recoverable");
    const auto file = root / "state-file";
    std::ofstream(file) << "synthetic sentinel";
    if (msime::windows::prepare_first_run(executable, file, host) || calls != 3)
      throw std::runtime_error("Existing file was rebuilt");
    auto reject = [](const auto &action) {
      bool rejected = false;
      try { action(); } catch (...) { rejected = true; }
      if (!rejected) throw std::runtime_error("Expected refusal");
    };
    reject([&] { msime::windows::prepare_first_run(executable, "relative", host); });
    reject([&] { msime::windows::prepare_first_run(root / "missing", root / "unprepared", host); });
    if (fs::exists(root / "unprepared") || calls != 3)
      throw std::runtime_error("Missing resources mutated state");
    const auto failed = root / "failed";
    reject([&] { msime::windows::prepare_first_run(executable, failed,
        [](const std::string &) { return "{\"ok\":false}"; }); });
    if (msime::windows::prepare_first_run(executable, failed, host) || calls != 3)
      throw std::runtime_error("Failed preparation was retried destructively");
    fs::remove_all(root);
    std::cout << "Production first-run policy passed\n";
    return 0;
  } catch (...) {
    fs::remove_all(root);
    std::cerr << "Production first-run policy failed\n";
    return 1;
  }
}
