#!/usr/bin/env python3
"""实际编译辅助进程的配置选择函数，拒绝用默认目录替代非法同意来源。"""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parent.parent
compiler = shutil.which(os.environ.get("CXX", "c++"))
if compiler is None:
    print("skipped: C++ compiler not installed")
    raise SystemExit(0)

includes = ["-I" + str(ROOT), "-I/opt/homebrew/include", "-I/usr/local/include"]
probe = subprocess.run(
    [compiler, "-std=c++17", *includes, "-x", "c++", "-fsyntax-only", "-"],
    input="#include <nlohmann/json.hpp>\n", text=True, capture_output=True,
)
if probe.returncode:
    print("skipped: nlohmann/json.hpp not installed")
    raise SystemExit(0)

# Win32 文件句柄读取由原生门禁负责；这里保留整个真实选择函数，仅替代读取接口。
header = (ROOT / "platforms/windows/common/AuxiliaryCrashCapture.h").read_text()
body = header[header.index("namespace msime::windows {"):]
source = r'''
#include <cassert>
#include <fstream>
#include <nlohmann/json.hpp>
#include "platforms/common/Telemetry.h"
#define MSIME_EDITION_USER_DATA_DIRECTORY "synthetic-edition"
namespace msime::telemetry {
std::filesystem::path default_directory() { return "/synthetic/telemetry"; }
}
namespace msime::windows {
std::optional<std::string> raw_options;
std::optional<std::string> read_private_file(const std::filesystem::path &, size_t) {
  return raw_options;
}
}
''' + body + r'''
int main(int argc, char **argv) {
  assert(argc == 2);
  const std::filesystem::path state(argv[1]);
  auto read = [&] { return msime::windows::auxiliary_crash_host("prepare", "0.0.0", state); };
  assert(read()->preferences_directory == state);
  std::ofstream(state / "runtime-options.json") << "synthetic";
  for (const auto &document : {"null", "{", "[]", "{\"preferences_directory\":false}",
                              "{\"preferences_directory\":\"\"}",
                              "{\"preferences_directory\":\"relative\"}",
                              "{\"preferences_directory\":\"../other\"}"}) {
    msime::windows::raw_options = document;
    assert(!read());
  }
  msime::windows::raw_options = "{}";
  assert(read()->preferences_directory == state);
  const auto redirected = state / "synthetic-preferences";
  msime::windows::raw_options = nlohmann::json{{"preferences_directory", redirected.string()}}.dump();
  assert(read()->preferences_directory == redirected);
  msime::windows::raw_options.reset();
  assert(!read());
}
'''
with tempfile.TemporaryDirectory() as temporary:
    root = Path(temporary)
    cpp, binary = root / "test.cpp", root / "test"
    cpp.write_text(source)
    subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", *includes,
                    str(cpp), "-o", str(binary)], check=True)
    subprocess.run([str(binary), str(root)], check=True)
print("Windows auxiliary crash consent: invalid redirects rejected; defaults and absolute redirects preserved")
