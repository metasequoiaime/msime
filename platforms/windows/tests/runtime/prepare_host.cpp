#include "PrepareHost.h"
#include <chrono>
#include <iostream>

int main() {
  namespace fs = std::filesystem;
  const auto root = fs::temp_directory_path() /
      ("msime-prepare-test-" + std::to_string(
          std::chrono::steady_clock::now().time_since_epoch().count()));
  if (!fs::create_directory(root)) return 1;
  try {
    const auto resources = root / "synthetic resources";
    fs::create_directory(resources);
    int calls = 0;
    auto host = [&](const std::string &request) {
      ++calls;
      const auto options = nlohmann::json::parse(request);
      if (options.at("resources") != fs::canonical(resources).u8string())
        throw std::runtime_error("Resource path mismatch");
      return nlohmann::json{{"ok", true}, {"value", {
          {"resources", options.at("resources")},
          {"preferences_directory", options.at("state_root")}}}}.dump();
    };
    auto reject = [](const auto &action) {
      bool rejected = false;
      try { action(); } catch (...) { rejected = true; }
      if (!rejected) throw std::runtime_error("Expected refusal");
    };
    const auto state = root / "new state";
    const auto path = msime::windows::prepare_host_state(resources, state, host);
    std::ifstream input(path);
    const auto saved = nlohmann::json::parse(input);
    if (saved.at("preferences_directory") != state.u8string() || calls != 1 ||
        fs::exists(state / ".runtime-options-prepared"))
      throw std::runtime_error("Publication mismatch");
#ifdef _WIN32
    // The Server reads it through the single-link check; a published hard link would fail here.
    if (!msime::windows::read_private_file(path, 16384))
      throw std::runtime_error("Published configuration unreadable");
#endif
    reject([&] { msime::windows::prepare_host_state(resources, state, host); });
    reject([&] { msime::windows::prepare_host_state("relative", root / "bad", host); });
    reject([&] { msime::windows::prepare_host_state(resources, root / "missing" / "child", host); });
    if (calls != 1) throw std::runtime_error("Invalid request reached host");

    // 临时配置文件必须独占创建，不能覆盖预先存在的文件或跟随符号链接。
    const auto staged_state = root / "staged-state";
    fs::create_directory(staged_state);
    const auto staged_target = root / "staged-target";
    std::ofstream(staged_target) << "keep";
    const auto staged = staged_state / ".runtime-options-prepared";
#ifdef _WIN32
    std::ofstream(staged) << "keep";
#else
    fs::create_symlink(staged_target, staged);
#endif
    reject([&] { msime::windows::prepare_host_state_in_directory(resources, staged_state, host); });
    std::ifstream preserved(staged_target);
    std::string preserved_value;
    preserved >> preserved_value;
    if (preserved_value != "keep" || fs::exists(staged_state / "runtime-options.json"))
      throw std::runtime_error("Followed or replaced staged configuration");

    for (const std::string response : {"{", "{\"ok\":false}",
                                      "{\"ok\":true,\"value\":null}"}) {
      const auto failed = root / ("failure-" + std::to_string(++calls));
      reject([&] { msime::windows::prepare_host_state(resources, failed,
          [&](const std::string &) { return response; }); });
      if (!fs::is_directory(failed) || fs::exists(failed / "runtime-options.json"))
        throw std::runtime_error("Failure retention mismatch");
    }
    const auto raced = root / "raced";
    reject([&] { msime::windows::prepare_host_state(resources, raced,
        [&](const std::string &request) {
          std::ofstream(raced / "runtime-options.json") << "sentinel";
          return host(request);
        }); });
    std::ifstream sentinel(raced / "runtime-options.json");
    std::string value;
    sentinel >> value;
    if (value != "sentinel") throw std::runtime_error("Replaced concurrent state");
    sentinel.close();
    preserved.close();
    input.close();
    fs::remove_all(root);
    std::cout << "Windows preparation orchestration passed\n";
    return 0;
  } catch (...) {
    fs::remove_all(root);
    std::cerr << "Windows preparation orchestration failed\n";
    return 1;
  }
}
