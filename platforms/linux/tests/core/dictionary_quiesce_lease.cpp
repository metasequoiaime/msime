#include "../src/core/DictionaryQuiesceLease.h"

#include <nlohmann/json.hpp>

#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <unistd.h>

int main() {
  using msime::linux_host::dictionary_quiesced;
  using msime::linux_host::preference_save_held;

  // The lease itself is covered by platforms/common/tests/dictionary_quiesce_lease.cpp; this is what the Linux hosts add on top of it.
  char pattern[] = "/tmp/msime-quiesce-XXXXXX";
  const std::filesystem::path root = mkdtemp(pattern);

  // A data directory move holds the lease on the old user directory for the whole copy, in the form the settings window stages and renames into place. A host still configured for the old directory stays off it; one that has read the rewritten locator opens on the copy, which carries no lease.
  const auto old_user = root / "old" / "user";
  const auto new_user = root / "new" / "user";
  std::filesystem::create_directories(old_user);
  std::filesystem::create_directories(new_user);
  {
    std::ofstream(old_user / ".msime-dictionary-quiesce") << "1030000\n";
  }
  assert(dictionary_quiesced(old_user.string(), 1000000));
  assert(dictionary_quiesced(old_user.string() + "/", 1000000));
  assert(!dictionary_quiesced(new_user.string(), 1000000));
  // Menu preference saves into the old root wait while it is copied; saves into the copy go ahead.
  assert(preference_save_held(old_user.string(), 1000000));
  assert(!preference_save_held(new_user.string(), 1000000));
  assert(!preference_save_held("", 1000000));
  assert(!preference_save_held("relative", 1000000));
  // A lease still being staged is not the lease; the rename is what raises it.
  std::filesystem::remove(old_user / ".msime-dictionary-quiesce");
  {
    std::ofstream(old_user / ".msime-dictionary-quiesce.4242") << "1030000\n";
  }
  assert(!dictionary_quiesced(old_user.string(), 1000000));
  // Once the move has taken the old user directory away there is no lease to read; the host's session open fails there instead, so it cannot start an empty library at the old path.
  std::filesystem::remove_all(root / "old");
  assert(!dictionary_quiesced(old_user.string(), 1000000));
  assert(!std::filesystem::exists(old_user));
  // With the lease gone along with the old user directory, a host still configured for it must not put a preferences file back into the old root.
  assert(preference_save_held(old_user.string(), 1000000));
  assert(!preference_save_held(new_user.string(), 1000000));
  std::filesystem::remove_all(root);

  // 建会话撞上设置页持着的独占锁是“词库正在维护”，宿主把按键交给应用；其余失败仍按运行配置有错处理。
  using msime::linux_host::dictionary_maintenance_busy;
  using Json = nlohmann::json;
  assert(dictionary_maintenance_busy(Json::parse(R"({"ok":false,"error":"dictionary maintenance busy"})")));
  assert(!dictionary_maintenance_busy(Json::parse(R"({"ok":false,"error":"dictionary access unavailable"})")));
  assert(!dictionary_maintenance_busy(Json::parse(R"({"ok":false,"error":"invalid options buffer"})")));
  assert(!dictionary_maintenance_busy(Json::parse(R"({"ok":true,"value":{"session":1}})")));
  assert(!dictionary_maintenance_busy(Json::parse(R"({"ok":"false","error":"dictionary maintenance busy"})")));
  assert(!dictionary_maintenance_busy(Json::parse(R"({"ok":false,"error":["dictionary maintenance busy"]})")));
  assert(!dictionary_maintenance_busy(Json::parse(R"({"ok":false})")));
  assert(!dictionary_maintenance_busy(Json::parse(R"("dictionary maintenance busy")")));
  return 0;
}
