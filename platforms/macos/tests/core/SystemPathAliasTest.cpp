#include "../../src/core/SystemPathAlias.h"

#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <string>
#include <unistd.h>

using msime::mac::IsTrustedSystemAlias;
using msime::mac::IsTrustedSystemAliasTarget;
using msime::mac::StoragePathIsSafe;

int main() {
  // 只有 `/tmp`、`/var` 指向各自的 `/private` 目录时才受信任，相对目标按链接所在目录解析，名字对上而目标不对的不算。
  assert(IsTrustedSystemAliasTarget("/tmp", "private/tmp"));
  assert(IsTrustedSystemAliasTarget("/tmp", "/private/tmp"));
  assert(IsTrustedSystemAliasTarget("/tmp", "/private/tmp/"));
  assert(IsTrustedSystemAliasTarget("/var", "/private/var"));
  assert(IsTrustedSystemAliasTarget("/var", "../private/var"));
  assert(!IsTrustedSystemAliasTarget("/tmp", "/private/var"));
  assert(!IsTrustedSystemAliasTarget("/var", "/private/tmp"));
  assert(!IsTrustedSystemAliasTarget("/tmp", "/Users/synthetic/outside"));
  assert(!IsTrustedSystemAliasTarget("/tmp", "/private/tmp/work"));
  assert(!IsTrustedSystemAliasTarget("/tmp/work", "/private/tmp/work"));
  assert(!IsTrustedSystemAliasTarget("/etc", "/private/etc"));
  assert(!IsTrustedSystemAliasTarget("/private/tmp", "/private/tmp"));

  // 本机真实的系统别名。
  assert(IsTrustedSystemAlias("/tmp"));
  assert(IsTrustedSystemAlias("/var"));
  assert(!IsTrustedSystemAlias("/private/tmp"));
  assert(!IsTrustedSystemAlias("/etc"));

  char pattern[] = "/tmp/msime-system-path-alias-XXXXXX";
  assert(mkdtemp(pattern) != nullptr);
  const std::filesystem::path root(pattern);
  const std::filesystem::path real = root / "real";
  const std::filesystem::path outside = root / "outside";
  std::filesystem::create_directories(real / "nested");
  std::filesystem::create_directories(outside);

  // 经过 `/tmp` 别名一次的真实目录可以接受，还不存在的层也可以。
  assert(StoragePathIsSafe(real / "nested", true));
  assert(StoragePathIsSafe(real / "missing" / "deeper", true));
  assert(StoragePathIsSafe(std::filesystem::path("/private") / root.relative_path() / "real", true));

  // 临时目录里放进去的链接，不论在中间还是最后一层，都要拒绝。
  const std::filesystem::path planted = root / "planted";
  std::filesystem::create_directory_symlink(outside, planted);
  assert(!StoragePathIsSafe(planted / "child", true));
  assert(!StoragePathIsSafe(planted / "child", false));
  assert(!StoragePathIsSafe(planted, true));
  assert(!StoragePathIsSafe(planted, false));

  // 名字叫 `var`、目标也是 `/private/var` 的链接不在根下，不算系统别名。
  const std::filesystem::path fakeVar = root / "var";
  std::filesystem::create_directory_symlink("/private/var", fakeVar);
  assert(!IsTrustedSystemAlias(fakeVar));
  assert(!StoragePathIsSafe(fakeVar / "folders", true));

  // 系统别名本身作为最后一层也要拒绝。
  assert(!StoragePathIsSafe("/tmp", true));
  assert(!StoragePathIsSafe("/var", false));
  assert(StoragePathIsSafe("/private/tmp", true));

  // 已存在的层不是目录时，按调用方的要求处理；相对路径和 `..` 一律拒绝。
  const std::filesystem::path file = root / "file";
  std::ofstream(file) << "synthetic";
  assert(!StoragePathIsSafe(file, true));
  assert(StoragePathIsSafe(file, false));
  assert(!StoragePathIsSafe(file / "child", false));
  assert(!StoragePathIsSafe("relative/path", true));
  assert(!StoragePathIsSafe(real / ".." / "real", true));

  std::filesystem::remove_all(root);
  return 0;
}
