#!/usr/bin/env python3
"""编译真实偏好监听线程，区分显式唤醒与正常轮询并验证停止。"""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
compiler = os.environ.get("CXX") or shutil.which("clang++") or shutil.which("g++")
if not compiler:
    print("skipped: C++ compiler is required for preference monitor wake regression")
    raise SystemExit(0)

# 只替换输入队列与存储依赖；条件变量、停止和发布循环使用未经改写的生产源码。
fixture = r'''
#pragma once
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <functional>
#include <future>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <thread>
namespace msime::windows {
inline std::atomic<unsigned> synthetic_revision{1}, reads{0};
class PreferenceSnapshot {
public:
  static std::optional<PreferenceSnapshot> try_load(const std::string &) {
    const PreferenceSnapshot value(synthetic_revision.load());
    // 先固定读取结果，再通知测试线程，避免新修订混进正在完成的旧读取。
    reads.fetch_add(1);
    return value;
  }
  unsigned revision() const { return revision_; }
  const std::string &serialized() const { return serialized_; }
private:
  explicit PreferenceSnapshot(unsigned revision)
      : revision_(revision), serialized_(std::to_string(revision)) {}
  unsigned revision_;
  std::string serialized_;
};
enum class InputTaskStatus { Completed };
class InputState {
public:
  PreferenceSnapshot publish_preferences_at_least(const PreferenceSnapshot &value) { return value; }
};
class InputQueue {
public:
  struct Stats { bool accepting = true; };
  bool on_worker_thread() const { return false; }
  Stats stats() const { return {}; }
  std::optional<std::future<InputTaskStatus>> submit(
      std::function<void(InputState &)> task) {
    InputState state;
    task(state);
    std::promise<InputTaskStatus> done;
    auto result = done.get_future();
    done.set_value(InputTaskStatus::Completed);
    return result;
  }
};
}
'''

probe = r'''
#include "PreferenceMonitor.h"
#include <iostream>
using namespace msime::windows;
template<class F> bool until(F ready, std::chrono::milliseconds timeout) {
  const auto end = std::chrono::steady_clock::now() + timeout;
  while (!ready()) {
    if (std::chrono::steady_clock::now() >= end) return false;
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
  return true;
}
int main(int argc, char **argv) {
  if (argc != 2) return 2;
  InputQueue input;
  std::atomic<unsigned> published{0};
  PreferenceMonitor monitor(input, argv[1], std::chrono::seconds(5),
      [&](const PreferenceSnapshot &value) { published.store(value.revision()); });
  if (!until([&] { return published.load() == 1 && reads.load() >= 2; },
             std::chrono::seconds(10))) return 3;
  synthetic_revision.store(2);
  monitor.wake();
  const bool prompt = until([&] { return published.load() == 2; },
                            std::chrono::seconds(1));
  const auto stop_at = std::chrono::steady_clock::now();
  monitor.stop();
  monitor.wake();
  monitor.stop();
  const bool stopped = monitor.status() == PreferenceMonitorStatus::Stopped &&
      std::chrono::steady_clock::now() - stop_at < std::chrono::seconds(1);
  std::cout << "preference monitor explicit wake: " << (prompt ? "passed" : "FAILED")
            << "; stop and late wake: " << (stopped ? "passed" : "FAILED") << '\n';
  return prompt && stopped ? 0 : 1;
}
'''

with tempfile.TemporaryDirectory(prefix="msime-preference-wake-") as temporary:
    path = Path(temporary)
    source = ROOT / "platforms/windows/src/system"
    for name in ("PreferenceMonitor.h", "PreferenceMonitor.cpp"):
        shutil.copyfile(source / name, path / name)
    (path / "InputQueue.h").write_text(fixture)
    (path / "probe.cpp").write_text(probe)
    executable = path / ("probe.exe" if os.name == "nt" else "probe")
    subprocess.run([compiler, "-std=c++17", "-pthread", "-Wall", "-Wextra", "-Werror",
                    str(path / "PreferenceMonitor.cpp"), str(path / "probe.cpp"),
                    "-o", str(executable)], check=True)
    subprocess.run([str(executable), str(path)], check=True, timeout=20)
