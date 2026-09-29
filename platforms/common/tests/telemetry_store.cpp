#include "../Telemetry.cpp"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <iterator>

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-telemetry-" + msime::telemetry::id());
  const auto outside = root / "outside.json";
  const auto path = root / "state" / "telemetry.json";
  std::filesystem::create_directories(path.parent_path());
  std::ofstream(outside) << "keep";
  assert(msime::telemetry::write_queue(
      path, nlohmann::json::array({{{"id", "synthetic"}}})));
  std::ifstream saved(path);
  assert(std::string(std::istreambuf_iterator<char>(saved), {}) ==
         "[{\"id\":\"synthetic\"}]");

  const auto linked = root / "state" / "linked.json";
  std::filesystem::create_symlink(outside, linked);
  assert(!msime::telemetry::write_queue(
      linked, nlohmann::json::array({{{"id", "changed"}}})));
  std::ifstream untouched(outside);
  assert(std::string(std::istreambuf_iterator<char>(untouched), {}) == "keep");
  std::filesystem::remove_all(root);
}
