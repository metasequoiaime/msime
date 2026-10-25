#include "HelpcodeRoot.h"

#include <chrono>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>

namespace {
void require(bool value, const char *message) {
  if (!value)
    throw std::runtime_error(message);
}
} // namespace

int main() {
  namespace fs = std::filesystem;
  const auto root = fs::temp_directory_path() /
                    ("msime-helpcode-root-" +
                     std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  if (!fs::create_directory(root))
    return 1;
  struct Cleanup {
    fs::path path;
    ~Cleanup() {
      std::error_code error;
      fs::remove_all(path, error);
    }
  } cleanup{root};
  try {
    using msime::windows::name_builtin_helpcodes;
    const nlohmann::json base = {{"api_version", 1},
                                 {"resources", (root / "server" / "resources").u8string()},
                                 {"preferences_directory", root.u8string()}};
    // A state root without the tables (a development run) leaves the options alone, so the library keeps reading them from the resources directory.
    auto options = base;
    name_builtin_helpcodes(options, root);
    require(options == base, "A missing helpcode directory was named");

    // The installer's layout: DataDir\helpcodes, with DataDir as the root the library reads helpcodes\<table> under.
    fs::create_directory(root / "helpcodes");
    { std::ofstream(root / "helpcodes" / "xiaohe_helpcode.txt") << "杨=mu\n"; }
    name_builtin_helpcodes(options, root);
    require(options.at("helpcode_fallback_resources") == root.u8string(),
            "The staged helpcode root was not named");
    auto expected = base;
    expected["helpcode_fallback_resources"] = root.u8string();
    require(options == expected, "Another key changed");

    // A root already in the options wins (prepare_host writes one for a downloaded dictionary generation); a relative root and a document that is not an object are left alone.
    auto named = base;
    named["helpcode_fallback_resources"] = "C:\\elsewhere";
    name_builtin_helpcodes(named, root);
    require(named.at("helpcode_fallback_resources") == "C:\\elsewhere", "An explicit root was replaced");
    auto relative = base;
    name_builtin_helpcodes(relative, fs::path("state"));
    require(!relative.contains("helpcode_fallback_resources"), "A relative state root was named");
    nlohmann::json not_object = nlohmann::json::array();
    name_builtin_helpcodes(not_object, root);
    require(not_object.is_array() && not_object.empty(), "A non-object document changed");

    // A plain file of that name is not the helpcode directory.
    const auto other = root / "other";
    fs::create_directory(other);
    { std::ofstream(other / "helpcodes") << "not a directory"; }
    auto file = base;
    name_builtin_helpcodes(file, other);
    require(!file.contains("helpcode_fallback_resources"), "A file was named as the helpcode directory");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "Windows built-in helpcode root checks passed\n";
  return 0;
}
