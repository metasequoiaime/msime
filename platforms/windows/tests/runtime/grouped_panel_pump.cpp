#include "../core/TestHostOptions.h"
#include <chrono>
#include <filesystem>
#include <iostream>
#include <stdexcept>

void grouped_panel_pump_tests(const std::string &options);
int main() {
  try {
    const auto root = std::filesystem::temp_directory_path() /
        ("msime-grouped-panel-pump-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    struct Cleanup {
      std::filesystem::path root;
      ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
    } cleanup{root};
    const auto options = test_host_options(root);
    grouped_panel_pump_tests(options.dump());
    std::cout << "Grouped panel key negotiation and main-pump checks passed\n";
    return 0;
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
