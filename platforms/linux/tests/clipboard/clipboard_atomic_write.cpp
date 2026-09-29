#include "../../src/clipboard/ClipboardAtomicWrite.h"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <iterator>

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-clipboard-atomic-" + std::to_string(::getpid()));
  const auto outside = root / "outside.txt";
  const auto directory = root / "state";
  const auto file = directory / "history.json";
  std::filesystem::remove_all(root);
  std::filesystem::create_directories(directory);
  std::ofstream(outside) << "keep";
  std::filesystem::create_symlink(
      outside, directory / (".tmp." + std::to_string(::getpid())));
  assert(msime::linux_host::write_clipboard_file_atomically(file, "[\"synthetic\"]"));
  std::ifstream input(outside);
  assert(std::string(std::istreambuf_iterator<char>(input), {}) == "keep");
  std::ifstream saved(file);
  assert(std::string(std::istreambuf_iterator<char>(saved), {}) == "[\"synthetic\"]");
  std::filesystem::remove_all(root);
}
