// The reporter wrapper against the real Host API: sessions, the sigaction crash path, the terminate path, chaining to a crash handler the process had before, and the switch. Linux only (fork and signals); nothing here reaches the network.
#include "Telemetry.h"
#include "CrashRecorder.h"
#include "HostApiString.h"
#include "msime_client.h"

#include <nlohmann/json.hpp>
#include <cassert>
#include <csignal>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
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

// 同一目录里另一个宿主进程开始了自己的会话（例如同一用户的另一个图形会话）：直接经 Host API 开始，不动本进程包装层的状态。返回那个会话的 id（崩溃记录文件名去掉扩展名）。
std::string foreign_session(const fs::path &directory) {
  const auto body = nlohmann::json{{"directory", directory.string()}, {"platform", "linux"}, {"version", "9.9.9"}, {"enabled", true}}.dump();
  auto response = msime::host_api::own_string(msime_client_telemetry_begin(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
  const auto value = nlohmann::json::parse(response.get()).at("value");
  return fs::path(value.at("crash_record_path").get<std::string>()).stem().string();
}

std::string marker_id(const fs::path &directory) {
  std::ifstream file(directory / "telemetry-session.json");
  if (!file)
    return {};
  return nlohmann::json::parse(std::string(std::istreambuf_iterator<char>(file), {})).value("id", std::string());
}
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
  // raise() 是本进程自己发出的信号，不是执行出错：摘要写明来源。
  assert(crashes[0].at("message").get<std::string>().rfind("SIGSEGV: segmentation fault (raised by this process)\n", 0) == 0);
  const auto stack = crashes[0].value("stack", std::string());
  assert(!stack.empty());
  // Module paths are reduced to file names.
  assert(stack.find('/') == std::string::npos);
  assert(of_kind(directory, "session_crash").size() == 1);
  // Still exactly one active today.
  assert(of_kind(directory, "active").size() == 1);

  // 别的进程用 kill 发来的 SIGSEGV：摘要写出发送者的进程名（这里是测试程序自己，/proc/<pid>/comm 截到 15 个字节），不写 pid。用单独的目录，直接读子进程写下的崩溃记录，不动本进程包装层的会话。
  {
    const auto sent_directory = directory / "sent";
    fs::create_directories(sent_directory);
    int ready[2];
    assert(pipe(ready) == 0);
    const pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
      close(ready[0]);
      msime::telemetry::install_crash_handlers();
      msime::telemetry::begin(host(sent_directory));
      const char byte = 1;
      [[maybe_unused]] const auto written = write(ready[1], &byte, 1);
      for (;;)
        pause();
    }
    close(ready[1]);
    char byte = 0;
    assert(read(ready[0], &byte, 1) == 1);
    close(ready[0]);
    assert(kill(child, SIGSEGV) == 0);
    int killed = 0;
    assert(waitpid(child, &killed, 0) == child);
    assert(WIFSIGNALED(killed) && WTERMSIG(killed) == SIGSEGV);
    std::ifstream comm("/proc/self/comm");
    std::string sender;
    std::getline(comm, sender);
    assert(!sender.empty());
    std::vector<fs::path> records;
    for (const auto &entry : fs::directory_iterator(sent_directory / "telemetry-crashes"))
      records.push_back(entry.path());
    assert(records.size() == 1);
    std::ifstream record(records[0]);
    std::string message;
    std::getline(record, message);
    assert(message.rfind("MSIME-CRASH/1 ", 0) == 0);
    std::getline(record, message);
    assert(message == "SIGSEGV: segmentation fault (sent by " + sender + ")");
  }

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
  assert(crashes[1].at("message").get<std::string>().rfind("std::terminate: std::runtime_error\n", 0) == 0);
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

  // 真实共享偏好与持续授权：读失败暂停，同代次恢复保留会话，短暂撤回必须换身份。
  const auto shared = directory / "shared";
  const auto preferences = directory / "preferences";
  fs::create_directories(shared);
  fs::create_directories(preferences);
  auto save_shared = [&](bool enabled) {
    const auto path = preferences.string();
    auto loaded = msime::host_api::own_string(msime_client_load_preferences(
        reinterpret_cast<const uint8_t *>(path.data()), path.size()));
    auto snapshot = nlohmann::json::parse(loaded.get()).at("value");
    snapshot["preferences"]["usage_reporting"] = enabled;
    const auto body = snapshot.dump();
    auto saved = msime::host_api::own_string(msime_client_save_preferences(
        reinterpret_cast<const uint8_t *>(path.data()), path.size(), snapshot.at("revision").get<uint64_t>(),
        reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    assert(nlohmann::json::parse(saved.get()).value("ok", false));
  };
  auto shared_session = [&] {
    std::ifstream file(shared / "telemetry-session.json");
    return nlohmann::json::parse(file).at("id").get<std::string>();
  };
  assert(msime::telemetry::begin({"linux", "1.2.3", shared, std::nullopt, preferences}));
  assert(of_kind(shared, "active").size() == 1);
  const auto initial_session = shared_session();
  fs::rename(preferences / "preferences.json", preferences / "saved.json");
  { std::ofstream file(preferences / "preferences.json"); file << "{"; }
  msime::telemetry::refresh();
  assert(!msime::crash::is_armed());
  fs::rename(preferences / "saved.json", preferences / "preferences.json");
  msime::telemetry::refresh();
  assert(msime::crash::is_armed() && shared_session() == initial_session);
  save_shared(false);
  save_shared(true);
  msime::telemetry::set_enabled(true);
  assert(msime::crash::is_armed() && shared_session() != initial_session);
  save_shared(false);
  // 旧发布回调带 true 也不能绕过已经关闭的磁盘偏好。
  msime::telemetry::set_enabled(true);
  assert(!msime::crash::is_armed() && queue(shared).empty());
  assert(!msime::telemetry::begin({"linux", "1.2.3", shared, std::nullopt, preferences}));
  assert(queue(shared).empty());

  // 既没有偏好目录也没带开关（例如运行时配置缺 preferences_directory）等同读不出偏好：不开会话、不武装、不入队。
  {
    const auto unconfigured = directory / "unconfigured";
    fs::create_directories(unconfigured);
    assert(!msime::telemetry::begin({"linux", "1.2.3", unconfigured, std::nullopt, {}}));
    msime::telemetry::flush();
    assert(!msime::crash::is_armed());
    assert(queue(unconfigured).empty());
    assert(!fs::exists(unconfigured / "telemetry-session.json"));
  }

  // 同目录里还有别的进程的会话：本进程 terminate 时只为自己的会话写一份记录（随后的 SIGABRT 不再写第二份），下次启动只为它补报一次 session_crash，别的进程的会话不算崩溃。
  const auto concurrent = directory / "concurrent";
  fs::create_directories(concurrent);
  status = crash_child([&] {
    msime::telemetry::install_crash_handlers();
    msime::telemetry::begin(host(concurrent));
    foreign_session(concurrent);
    synthetic_terminate();
  });
  assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
  assert(msime::telemetry::begin(host(concurrent)));
  {
    const auto crashed = of_kind(concurrent, "crash");
    const auto sessions_crashed = of_kind(concurrent, "session_crash");
    assert(crashed.size() == 1);
    assert(crashed[0].at("message").get<std::string>().rfind("std::terminate: std::runtime_error\n", 0) == 0);
    assert(sessions_crashed.size() == 1);
    // 崩溃和 session_crash 属于同一个会话，也就是崩溃的那个进程的会话（版本是它的 1.2.3，不是另一个进程的 9.9.9）。
    assert(crashed[0].at("id").get<std::string>().substr(6) == sessions_crashed[0].at("id").get<std::string>().substr(8));
    assert(sessions_crashed[0].at("version") == "1.2.3");
  }

  // 正常结束只结束自己的会话：别的进程后来写下的标记留着，它之后崩溃仍报 session_crash。
  const auto other = foreign_session(concurrent);
  assert(marker_id(concurrent) == other);
  msime::telemetry::end();
  {
    const auto ended = of_kind(concurrent, "session");
    assert(ended.size() == 1);
    assert(ended[0].at("id") != "session-" + other);
    assert(ended[0].at("version") == "1.2.3");
  }
  assert(marker_id(concurrent) == other);
  {
    std::ofstream record(concurrent / "telemetry-crashes" / (other + ".crash"));
    record << "SIGSEGV: segmentation fault\n";
  }
  assert(msime::telemetry::begin(host(concurrent)));
  {
    const auto sessions_crashed = of_kind(concurrent, "session_crash");
    assert(sessions_crashed.size() == 2);
    assert(sessions_crashed[1].at("id") == "session-" + other);
    assert(sessions_crashed[1].at("version") == "9.9.9");
  }
  msime::telemetry::end();

  // Off from the start: nothing is queued.
  const auto off = directory / "off";
  fs::create_directories(off);
  assert(!msime::telemetry::begin({"linux", "1.2.3", off, false, {}}));
  assert(queue(off).empty());

  fs::remove_all(directory);
  return 0;
}
