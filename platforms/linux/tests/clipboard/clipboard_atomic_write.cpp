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
  const auto external_store = root / "external-history.json";
  std::ofstream(external_store) << "[\"outside\"]";
  const auto linked_store = directory / "linked-history.json";
  std::filesystem::create_symlink(external_store, linked_store);
  assert(!msime::linux_host::read_clipboard_file(linked_store, 1024));
  const auto loaded = msime::linux_host::read_clipboard_file(file, 1024);
  assert(loaded && *loaded == "[\"synthetic\"]");

  const auto external_directory = root / "external-state";
  std::filesystem::create_directories(external_directory / "nested");
  const auto linked_directory = root / "linked-state";
  std::filesystem::create_directory_symlink(external_directory, linked_directory);
  // 以 root 身份运行时（Linux 容器里就是这样），root 自己不对外开放的目录里的链接会被当成受信任的系统链接（见 `src/core/SafePath.h`）；把目录改成其他人可写，这条链接就成了任何人都可能放进去的链接。
  std::filesystem::permissions(root, std::filesystem::perms::others_write, std::filesystem::perm_options::add);
  const auto linked_history = linked_directory / "nested" / "history.json";
  std::ofstream(external_directory / "nested" / "history.json") << "keep";
  assert(!msime::linux_host::clipboard_directory_is_safe(linked_history.parent_path()));
  assert(!msime::linux_host::write_clipboard_file_atomically(linked_history, "[\"synthetic\"]"));
  assert(!msime::linux_host::read_clipboard_file(linked_history, 1024));
  assert(msime::linux_host::open_clipboard_lock(linked_history) < 0);
  assert(!msime::linux_host::remove_clipboard_file(linked_history));
  assert(std::filesystem::exists(external_directory / "nested" / "history.json"));
  const auto removable = directory / "clear.json";
  std::ofstream(removable) << "remove";
  assert(msime::linux_host::remove_clipboard_file(removable));
  assert(!std::filesystem::exists(removable));
  std::filesystem::remove_all(root);
}
