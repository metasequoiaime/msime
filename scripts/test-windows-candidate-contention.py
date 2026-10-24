#!/usr/bin/env python3
"""执行候选窗生产读取和隐藏分支，核对锁争用与焦点切换。"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

HARNESS = r'''
#include "platforms/windows/src/system/FocusGate.h"
#include <functional>
#include <future>
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
struct CandidatePresentation {
  FocusLease lease;
  uint64_t generation = 1;
  bool visible = true;
  bool composition_ended = false;
  std::string preedit = "mo'xing";
};
struct DpiScope {};
struct Painting { explicit Painting(int) {} bool dc = true; };
struct CandidateWindow {
  std::function<std::optional<CandidatePresentation>()> reader_;
  std::function<bool(const FocusLease &)> lease_current_;
  std::optional<CandidatePresentation> shown_, exit_value_, observed_;
  bool finishing_animation_ = false;
  int window_ = 1, hides = 0, immediate_hides = 0;
  void hide() { ++hides; }
  void hide_immediately() {
    ++immediate_hides;
    shown_.reset();
    finishing_animation_ = false;
    exit_value_.reset();
  }
  bool exit_lease_lost() const;
  std::optional<CandidatePresentation> read_presentation() const;
  void hide_for(const std::optional<CandidatePresentation> &value);
  void reposition();
  void paint();
};

// 生产读取与隐藏方法
// 生产定位读取分支
// 生产绘制读取分支

void require(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}

int main() {
  FocusGate gate;
  const PipeTicket ticket{42, {1, 2, 3}};
  const auto change = gate.begin(ticket, 7);
  require(change.has_value(), "synthetic focus activation failed");
  const auto lease = change->pending;
  require(gate.acknowledge(lease, [] { return true; }), "synthetic focus acknowledgement failed");
  std::optional<CandidatePresentation> live = CandidatePresentation{lease};
  bool stopped = false;
  CandidateWindow window;
  window.shown_ = live;
  window.lease_current_ = [&](const FocusLease &owner) { return !stopped && gate.still_current(owner); };
  window.reader_ = [&] {
    std::optional<CandidatePresentation> result;
    gate.try_with_active(lease, [&] { result = live; });
    return result;
  };

  std::promise<void> held, release;
  auto ready = held.get_future();
  auto resume = release.get_future();
  auto holder = std::async(std::launch::async, [&] {
    gate.with_active(lease, [&] { held.set_value(); resume.wait(); });
  });
  ready.wait();
  bool retained = true;
  for (int frame = 0; frame != 3; ++frame) {
    window.observed_.reset();
    window.reposition();
    retained = retained && window.observed_ && window.observed_->generation == 1;
    window.observed_.reset();
    window.paint();
    retained = retained && window.observed_ && window.observed_->preedit == "mo'xing";
  }
  release.set_value();
  holder.get();
  require(retained && window.hides == 0 && window.immediate_hides == 0,
          "#9430: a busy focus gate hides the live candidate window");

  live->generation = 2;
  live->preedit = "xin'mo'xing";
  window.paint();
  require(window.observed_ && window.observed_->generation == 2 &&
              window.observed_->preedit == "xin'mo'xing",
          "a fresh snapshot must replace the retained frame");
  live->visible = false;
  window.paint();
  require(window.immediate_hides == 1, "an explicit hidden snapshot must not revive old candidates");
  window.shown_ = CandidatePresentation{lease};
  live->composition_ended = true;
  window.reposition();
  require(window.hides == 1, "a confirmed composition end must still use the normal exit");

  live.reset();
  window.finishing_animation_ = true;
  window.exit_value_ = window.shown_;
  window.reposition();
  window.observed_.reset();
  window.paint();
  require(window.observed_ && window.hides == 1 && window.immediate_hides == 1,
          "contention must preserve an already running normal exit");
  live = CandidatePresentation{lease};
  live->visible = false;
  window.observed_.reset();
  window.paint();
  require(!window.observed_ && window.immediate_hides == 2,
          "an explicit hidden snapshot must interrupt an old exit frame");
  live.reset();

  const auto next = gate.begin(PipeTicket{43, {4, 5, 6}}, 8);
  require(next.has_value(), "second synthetic activation failed");
  window.reposition();
  require(window.immediate_hides == 3, "pending focus changes must immediately hide the old owner");

  window.shown_ = CandidatePresentation{lease};
  window.lease_current_ = {};
  window.reposition();
  require(window.immediate_hides == 4, "an absent lease check cannot authorize a retained frame");
  window.shown_ = CandidatePresentation{lease};
  window.exit_value_ = window.shown_;
  window.finishing_animation_ = true;
  window.observed_.reset();
  window.paint();
  require(!window.observed_ && window.immediate_hides == 5,
          "an absent lease check cannot authorize an exit frame");
  window.lease_current_ = [&](const FocusLease &) { return !stopped; };
  window.shown_ = CandidatePresentation{lease};
  stopped = true;
  window.paint();
  require(window.immediate_hides == 6, "stopping must not keep the old candidate frame");
  std::cout << "candidate contention: live frames retained; hidden and stale owners suppressed\n";
}
'''


def method(source: str, signature: str) -> str:
    begin = source.index(signature)
    return source[begin:source.index("\n}", begin) + 2]


def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
    if not compiler:
        print("skipped: no C++ compiler for candidate contention regression")
        return 0
    source = (ROOT / "platforms/windows/src/candidate/CandidateWindow.cpp").read_text(encoding="utf-8")
    helpers = [method(source, "bool CandidateWindow::exit_lease_lost() const"),
               method(source, "void CandidateWindow::hide_for(")]
    if "CandidateWindow::read_presentation() const" in source:
        helpers.append(method(source, "std::optional<CandidatePresentation> CandidateWindow::read_presentation() const"))
    reposition = method(source, "void CandidateWindow::reposition()")
    reposition = reposition[:reposition.index("  // Server 那边")] + "  observed_ = value;\n}"
    paint = method(source, "void CandidateWindow::paint()")
    paint = paint[:paint.index("  if (value->candidates.size() > 9)")] + "  observed_ = value;\n}"
    harness = HARNESS.replace("// 生产读取与隐藏方法", "\n".join(helpers))
    harness = harness.replace("// 生产定位读取分支", reposition).replace("// 生产绘制读取分支", paint)
    with tempfile.TemporaryDirectory(prefix="msime-candidate-contention-") as temporary:
        directory = pathlib.Path(temporary)
        cpp, binary = directory / "contention.cpp", directory / "contention"
        cpp.write_text(harness, encoding="utf-8")
        built = subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror",
                                "-Wno-unused-variable", "-pthread", "-I", str(ROOT),
                                "-I", str(ROOT / "platforms/windows/src/ipc"),
                                str(cpp), "-o", str(binary)], check=False)
        if built.returncode:
            return built.returncode
        return subprocess.run([str(binary)], check=False, timeout=30).returncode


if __name__ == "__main__":
    raise SystemExit(main())
