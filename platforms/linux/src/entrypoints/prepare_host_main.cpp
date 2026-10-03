#include "msime_client.h"
#include "../core/LinuxEdition.h"
#include "../core/PreparePaths.h"
#include "../core/PrepareState.h"
#include "../core/RuntimeOptionsRefresh.h"

#include <cerrno>
#include <cstdlib>
#include <filesystem>
#include <fcntl.h>
#include <iostream>
#include <memory>
#include <nlohmann/json.hpp>
#include <stdexcept>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>

namespace {
struct Descriptor {
  int value;
  explicit Descriptor(int descriptor) : value(descriptor) {}
  ~Descriptor() { if (value >= 0) close(value); }
  Descriptor(const Descriptor &) = delete;
  Descriptor &operator=(const Descriptor &) = delete;
};

bool publish(const std::filesystem::path &state, const std::string &document) {
  Descriptor directory(open(state.c_str(), O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW));
  if (directory.value < 0) return false;
  auto pattern = (state / ".runtime-options-XXXXXX").string();
  std::vector<char> name(pattern.begin(), pattern.end());
  name.push_back('\0');
  Descriptor file(mkstemp(name.data()));
  if (file.value < 0) return false;
  const auto temporary = std::filesystem::path(name.data()).filename().string();
  bool success = false;
  size_t offset = 0;
  while (offset < document.size()) {
    const auto count = write(file.value, document.data() + offset, document.size() - offset);
    if (count < 0 && errno == EINTR) continue;
    if (count <= 0) break;
    offset += static_cast<size_t>(count);
  }
  if (offset == document.size() && fsync(file.value) == 0) {
    // Publishing via a hard link is atomic and never replaces an existing file.
    success = linkat(directory.value, temporary.c_str(), directory.value,
                     "runtime-options.json", 0) == 0;
  }
  const bool removed = unlinkat(directory.value, temporary.c_str(), 0) == 0;
  return fsync(directory.value) == 0 && success && removed;
}

bool installer_account_state(const std::filesystem::path &state) {
  struct stat directory {};
  if (lstat(state.c_str(), &directory) != 0 || !S_ISDIR(directory.st_mode) ||
      directory.st_uid != geteuid() || (directory.st_mode & 0077) != 0) {
    return false;
  }
  bool found = false;
  std::error_code error;
  for (const auto &entry : std::filesystem::directory_iterator(state, error)) {
    if (error || entry.is_symlink(error) || error ||
        (entry.path().filename() != "anonymous-account.json" &&
         entry.path().filename() != "anonymous-session.json") ||
        !entry.is_regular_file(error) || error) {
      return false;
    }
    struct stat file {};
    if (lstat(entry.path().c_str(), &file) != 0 || file.st_uid != geteuid() ||
        (file.st_mode & 0077) != 0) {
      return false;
    }
    found = true;
  }
  return !error && found;
}

using Owned = std::unique_ptr<char, decltype(&msime_client_string_free)>;

nlohmann::json value_of(Owned raw) {
  if (!raw) throw std::runtime_error("host call failed");
  auto result = nlohmann::json::parse(raw.get());
  if (!result.value("ok", false)) throw std::runtime_error("host call failed");
  return result.at("value");
}

// msime-linux-setup --update runs this once it has dictionaries matching the installed lock (staged in a new directory beside the recorded one, named in a copy of the options it publishes only after this succeeds) and the quiesce lease has closed both hosts' sessions: the same refresh the input method hosts run at startup, so the new generation is prepared and the user dictionary replayed into it the way the Windows installer replays it after an upgrade, without waiting for the next host start. Exit 3 is the one failure setup can explain itself: the recorded dictionaries still do not match this version.
int refresh(const std::filesystem::path &options) {
  if (!options.is_absolute()) {
    std::cerr << "The runtime options path must be absolute\n";
    return 2;
  }
  try {
    umask(0077);
    const bool rewritten = msime::linux_host::refresh_runtime_options(options);
    std::cout << (rewritten ? "refreshed" : "current") << '\n';
    return std::cout ? 0 : 1;
  } catch (const msime::linux_host::DictionaryOutdated &) {
    std::cerr << "The recorded dictionaries do not match this version; runtime options were left unchanged\n";
    return 3;
  } catch (...) {
    // Host diagnostics can contain private paths; do not forward them.
    std::cerr << "Dictionary refresh failed; runtime options were left unchanged\n";
    return 1;
  }
}

// The Windows installer asks on a first install whether cloud candidates may run, since they are the one network feature active without any token; declining writes the preference before the input method first starts.
void disable_cloud_candidates(const std::filesystem::path &state, nlohmann::json &options) {
  const auto directory = state.string();
  auto snapshot = value_of(Owned(
      msime_client_load_preferences(reinterpret_cast<const uint8_t *>(directory.data()), directory.size()),
      msime_client_string_free));
  snapshot.at("preferences")["cloud_candidates"] = false;
  const auto revision = snapshot.at("revision").get<uint64_t>();
  const auto document = snapshot.dump();
  const auto saved = value_of(Owned(
      msime_client_save_preferences(reinterpret_cast<const uint8_t *>(directory.data()), directory.size(), revision,
                                    reinterpret_cast<const uint8_t *>(document.data()), document.size()),
      msime_client_string_free));
  options["preferences"] = saved.at("preferences");
}
} // namespace

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-prepare [--no-cloud-candidates] <absolute-resource-directory> <absolute-new-state-directory>\n"
                 "       msime-linux-prepare [--no-cloud-candidates] --installed <absolute-new-state-directory>\n"
                 "       msime-linux-prepare --refresh <absolute-runtime-options.json>\n"
                 "The state directory must not exist (except for installer-created anonymous account files); its parent must exist.\n"
                 "--installed uses the resource bundle installed beside this executable.\n"
                 "--no-cloud-candidates turns cloud candidates off in the new preferences.\n"
                 "Prints the new runtime-options.json path on success.\n"
                 "--refresh moves existing runtime options to the installed dictionary generation, replaying the\n"
                 "user dictionary; prints \"refreshed\" or \"current\", exits 3 when the recorded dictionaries are outdated.\n";
    return 0;
  }
  if (argc == 3 && std::string(argv[1]) == "--refresh") return refresh(argv[2]);
  const bool no_cloud = argc > 1 && std::string(argv[1]) == "--no-cloud-candidates";
  if (no_cloud) {
    --argc;
    ++argv;
  }
  if (argc != 3) {
    std::cerr << "Usage: msime-linux-prepare [--no-cloud-candidates] <absolute-resource-directory> <absolute-new-state-directory>\n"
                 "       msime-linux-prepare [--no-cloud-candidates] --installed <absolute-new-state-directory>\n"
                 "       msime-linux-prepare --refresh <absolute-runtime-options.json>\n";
    return 2;
  }
  try {
    const bool installed = std::string(argv[1]) == "--installed";
    std::filesystem::path resources;
    const std::filesystem::path requested_state(argv[2]);
    if (installed) {
      std::error_code error;
      const auto executable = std::filesystem::read_symlink("/proc/self/exe", error);
      if (error || !executable.is_absolute()) {
        std::cerr << "Cannot locate the installed executable resource bundle\n";
        return 1;
      }
      const auto discovered = msime_linux::installed_resource_directory(executable);
      if (discovered.empty()) {
        std::cerr << "Installed Engine resources were not found; provide a packaged resource bundle\n";
        return 1;
      }
      resources = discovered;
    } else {
      resources = argv[1];
    }
    if (!resources.is_absolute() || !requested_state.is_absolute() ||
        !std::filesystem::is_directory(resources)) {
      std::cerr << "Resource and new state directories must use absolute paths\n";
      return 2;
    }
    const auto state = requested_state.lexically_normal();
    auto bootstrap = nlohmann::json({{"resources", std::filesystem::canonical(resources).string()},
                                     {"state_root", state.string()}});
    // 不是 full 的版本把版本 id 交给宿主库：它按本版本的资源锁校验词库，在 HostOptions 里记下版本，并写下本版本的默认偏好。full 不写，请求与引入版本之前相同。
    if (!MSIME_EDITION_IS_FULL) bootstrap["edition"] = MSIME_EDITION_ID;
    const auto request = bootstrap.dump();
    if (request.size() > 16384) return 2;
    umask(0077);
    if (!msime_linux::state_directory_path_is_safe(state) ||
        (mkdir(state.c_str(), 0700) != 0 && (errno != EEXIST || !installer_account_state(state)))) {
      std::cerr << "Cannot create a fresh state directory; existing state is never replaced\n";
      return 1;
    }
    std::unique_ptr<char, decltype(&msime_client_string_free)> raw(
        msime_client_prepare_host(reinterpret_cast<const uint8_t *>(request.data()), request.size()),
        msime_client_string_free);
    if (!raw) throw std::runtime_error("prepare failed");
    const auto result = nlohmann::json::parse(raw.get());
    if (!result.value("ok", false) || !result.at("value").is_object()) {
      std::cerr << "State preparation failed; check the pinned resources and use a fresh directory to retry\n";
      return 1;
    }
    auto options = result.at("value");
    if (no_cloud) {
      try {
        disable_cloud_candidates(state, options);
      } catch (...) {
        std::cerr << "Cannot record the cloud candidate choice; nothing was published, use a fresh directory to retry\n";
        return 1;
      }
    }
    if (!publish(state, options.dump(2) + "\n")) {
      std::cerr << "Cannot publish runtime configuration; prepared data has been retained\n";
      return 1;
    }
    std::cout << (state / "runtime-options.json").string() << '\n';
    return std::cout ? 0 : 1;
  } catch (...) {
    // Host diagnostics can contain private paths; do not forward them.
    std::cerr << "State preparation failed; existing and partially prepared data has been retained\n";
    return 1;
  }
}
