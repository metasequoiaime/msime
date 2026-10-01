// The reporter wrapper against the real Host API: sessions, the sigaction crash path, the terminate path, chaining to a crash handler the process had before, and the switch. Linux only (fork and signals); nothing here reaches the network.
#include "Telemetry.h"
#include "msime_client.h"

#include <nlohmann/json.hpp>
#include <cassert>
#include <csignal>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <memory>
#include <random>
#include <stdexcept>
#include <string>
#include <sys/wait.h>
#include <unistd.h>
#include <vector>

namespace {
namespace fs = std::filesystem;

nlohmann::json queue(const fs::path &directory) {
  std::ifstream file(directory / "telemetry.json");
  if (!file)
    return nlohmann::json::array();
  return nlohmann::json::parse(std::string(std::istreambuf_iterator<char>(file), {}));
}

std::vector<nlohmann::json> of_kind(const fs::path &directory, const std::string &kind) {
  std::vector<nlohmann::json> found;
  for (const auto &event : queue(directory))
    if (event.value("kind", "") == kind)
      found.push_back(event);
  return found;
}

msime::telemetry::Host host(const fs::path &directory) {
  return {"linux", "1.2.3", directory, true, {}};
}

// Runs body in a child that is expected to die; returns the raw wait status.
template <class Body> int crash_child(Body body) {
  const pid_t child = fork();
  assert(child >= 0);
  if (child == 0) {
    body();
    _exit(0);
  }
  int status = 0;
  assert(waitpid(child, &status, 0) == child);
  return status;
}

[[noreturn]] void synthetic_terminate() {
  std::set_terminate([] {
    msime::telemetry::record_terminate();
    std::abort();
  });
  throw std::runtime_error("synthetic failure\nsecond line");
}

void previous_handler(int) { _exit(42); }
} // namespace

int main() {
  std::random_device random;
  const auto directory = fs::temp_directory_path() / ("msime-telemetry-test-" + std::to_string(random()) + std::to_string(random()));
  fs::create_directories(directory);

  // Start: today's active, with the anonymous install id.
  assert(msime::telemetry::begin(host(directory)));
  const auto active = of_kind(directory, "active");
  assert(active.size() == 1);
  const auto install_id = active[0].at("install_id").get<std::string>();
  assert(install_id.size() >= 16);
  assert(active[0].at("id").get<std::string>().rfind("active-" + install_id + "-", 0) == 0);
  assert(active[0].at("platform") == "linux" && active[0].at("version") == "1.2.3");

  // A session ended by a signal: the handler writes the record, the next start reports crash and session_crash.
  int status = crash_child([&] {
    msime::telemetry::install_crash_handlers();
    msime::telemetry::begin(host(directory));
    raise(SIGSEGV);
  });
  assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV);
  assert(msime::telemetry::begin(host(directory)));
  auto crashes = of_kind(directory, "crash");
  assert(crashes.size() == 1);
  assert(crashes[0].at("message").get<std::string>().rfind("SIGSEGV: segmentation fault", 0) == 0);
  const auto stack = crashes[0].value("stack", std::string());
  assert(!stack.empty());
  // Module paths are reduced to file names.
  assert(stack.find('/') == std::string::npos);
  assert(of_kind(directory, "session_crash").size() == 1);
  // Still exactly one active today.
  assert(of_kind(directory, "active").size() == 1);

  // A leftover marker alone (the process killed, no record) is no crash.
  status = crash_child([&] {
    msime::telemetry::begin(host(directory));
    raise(SIGKILL);
  });
  assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGKILL);
  assert(msime::telemetry::begin(host(directory)));
  assert(of_kind(directory, "session_crash").size() == 1);
  assert(of_kind(directory, "crash").size() == 1);

  // std::terminate: the exception's type and first line, then abort, whose SIGABRT keeps the first record.
  status = crash_child([&] {
    msime::telemetry::install_crash_handlers();
    msime::telemetry::begin(host(directory));
    synthetic_terminate();
  });
  assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
  assert(msime::telemetry::begin(host(directory)));
  crashes = of_kind(directory, "crash");
  assert(crashes.size() == 2);
  assert(crashes[1].at("message") == "std::terminate: std::runtime_error: synthetic failure");
  assert(of_kind(directory, "session_crash").size() == 2);

  // A crash handler the process already had (Fcitx5's) still runs after the record is written.
  status = crash_child([&] {
    struct sigaction earlier {};
    earlier.sa_handler = previous_handler;
    sigemptyset(&earlier.sa_mask);
    sigaction(SIGSEGV, &earlier, nullptr);
    msime::telemetry::install_crash_handlers();
    msime::telemetry::begin(host(directory));
    raise(SIGSEGV);
  });
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 42);
  assert(msime::telemetry::begin(host(directory)));
  assert(of_kind(directory, "crash").size() == 3);

  // remove_crash_handlers puts back what was there.
  status = crash_child([&] {
    struct sigaction earlier {};
    earlier.sa_handler = previous_handler;
    sigemptyset(&earlier.sa_mask);
    sigaction(SIGBUS, &earlier, nullptr);
    msime::telemetry::install_crash_handlers();
    msime::telemetry::remove_crash_handlers();
    struct sigaction now {};
    sigaction(SIGBUS, nullptr, &now);
    _exit(now.sa_handler == previous_handler ? 7 : 8);
  });
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 7);

  // A JSON error's text can quote the user's own bytes; only its type and id are kept.
  try {
    const auto parsed = nlohmann::json::parse("{\"private words");
    assert(parsed.is_null());
  } catch (...) {
    const auto summary = msime::telemetry::current_exception_summary();
    assert(summary.find("private") == std::string::npos);
    assert(summary.find("parse_error") != std::string::npos);
  }

  // A normal end queues the session.
  msime::telemetry::end();
  assert(of_kind(directory, "session").size() == 1);

  // Turning reporting off clears the queue; the install id stays for when it is turned back on.
  msime::telemetry::begin(host(directory));
  msime::telemetry::set_enabled(false);
  assert(queue(directory).empty());
  assert(!fs::exists(directory / "telemetry-session.json"));
  msime::telemetry::set_enabled(true);
  assert(of_kind(directory, "active").at(0).at("install_id") == install_id);

  // Hosts on the shared preferences: no document yet means the default (on); usage_reporting false in preferences.json is off and clears what an earlier session queued.
  const auto shared = directory / "shared";
  const auto preferences = directory / "preferences";
  fs::create_directories(shared);
  fs::create_directories(preferences);
  assert(msime::telemetry::begin({"linux", "1.2.3", shared, std::nullopt, preferences}));
  assert(of_kind(shared, "active").size() == 1);
  {
    std::unique_ptr<char, decltype(&msime_client_string_free)> defaults(msime_client_default_preferences(), msime_client_string_free);
    auto value = nlohmann::json::parse(defaults.get()).at("value");
    auto snapshot = value.contains("preferences") ? value : nlohmann::json{{"format_version", 1}, {"revision", 0}, {"preferences", value}};
    snapshot["preferences"]["usage_reporting"] = false;
    const auto body = snapshot.dump();
    const auto path = preferences.string();
    std::unique_ptr<char, decltype(&msime_client_string_free)> saved(
        msime_client_save_preferences(reinterpret_cast<const uint8_t *>(path.data()), path.size(), snapshot.value("revision", 0ull),
                                      reinterpret_cast<const uint8_t *>(body.data()), body.size()),
        msime_client_string_free);
    assert(nlohmann::json::parse(saved.get()).value("ok", false));
  }
  assert(!msime::telemetry::begin({"linux", "1.2.3", shared, std::nullopt, preferences}));
  assert(queue(shared).empty());

  // Off from the start: nothing is queued.
  const auto off = directory / "off";
  fs::create_directories(off);
  assert(!msime::telemetry::begin({"linux", "1.2.3", off, false, {}}));
  assert(queue(off).empty());

  fs::remove_all(directory);
  return 0;
}
