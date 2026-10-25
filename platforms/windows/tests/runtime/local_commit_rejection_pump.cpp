#include "../core/TestHostOptions.h"
#include <chrono>
#include <filesystem>
#include <iostream>
#include <stdexcept>

void local_commit_rejection_pump_tests(const std::string &options);
int main() {
  try {
    const auto root = std::filesystem::temp_directory_path() /
        ("msime-local-commit-rejection-pump-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    struct Cleanup {
      std::filesystem::path root;
      ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
    } cleanup{root};
    const auto options = test_host_options(root);
    local_commit_rejection_pump_tests(options.dump());
    std::cout << "Rejected local commit keeps the input queue running\n";
    return 0;
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
