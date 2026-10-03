#include "../src/core/SafePath.h"

#include <sys/stat.h>
#include <unistd.h>

#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <string>

namespace {

uid_t link_owner(const std::filesystem::path &link) {
  struct stat info {};
  assert(::lstat(link.c_str(), &info) == 0);
  return info.st_uid;
}

} // namespace

int main() {
  using msime::linux_host::is_root_only_link;
  using msime::linux_host::storage_directory_path_is_safe;
  namespace fs = std::filesystem;

  std::string pattern = (fs::temp_directory_path() / "msime-safe-path-XXXXXX").string();
  const fs::path directory = ::mkdtemp(pattern.data());
  assert(!directory.empty());
  const auto canonical = fs::canonical(directory);

  // 真实路径可以接受，包括尚不存在的层级；本该是目录的位置是文件则不行。
  assert(storage_directory_path_is_safe(canonical));
  assert(storage_directory_path_is_safe(canonical / "missing" / "below"));
  std::ofstream(canonical / "file") << "synthetic";
  assert(!storage_directory_path_is_safe(canonical / "file" / "below"));

  // `mkdtemp` 建的目录属于运行测试的用户且不对外开放（0700），所以其中的链接只有属于 root 时才算受信任，也就是测试以 root 身份运行时（Linux 容器里就是这样）。
  const auto target = canonical / "target";
  fs::create_directory(target);
  const auto link = canonical / "home";
  fs::create_directory_symlink(target, link);
  const bool as_root = link_owner(link) == 0;
  assert(as_root == (::geteuid() == 0));
  assert(is_root_only_link(link) == as_root);
  assert(storage_directory_path_is_safe(link / "below") == as_root);
  assert(!is_root_only_link(canonical));

  // 链接本身作为最后一级时永远不放行，第二个受信任的链接也不放行。
  assert(!storage_directory_path_is_safe(link));
  const auto second = target / "second";
  fs::create_directory_symlink(target, second);
  assert(!storage_directory_path_is_safe(link / "second" / "below"));

  // 相对路径不信任任何链接。
  const auto previous = fs::current_path();
  fs::current_path(canonical);
  assert(!storage_directory_path_is_safe(fs::path("home") / "below"));
  fs::current_path(previous);

  // 普通用户拥有的链接要拒绝。以 root 运行时，把链接交给 `nobody` 来构造这种情况。
  const auto user_link = canonical / "user-home";
  fs::create_directory_symlink(target, user_link);
  if (as_root) {
    const int handed = ::lchown(user_link.c_str(), 65534, 65534);
    assert(handed == 0);
  }
  assert(link_owner(user_link) != 0);
  assert(!is_root_only_link(user_link));
  assert(!storage_directory_path_is_safe(user_link / "below"));

  // 目录一旦允许其他人写入，链接就可能已被他们替换。
  fs::permissions(canonical, fs::perms::all);
  assert(!is_root_only_link(link));
  assert(!storage_directory_path_is_safe(link / "below"));

  fs::remove_all(canonical);
  return 0;
}
