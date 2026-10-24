#!/usr/bin/env python3
"""核对按住录音目录的生成物与 Windows 真实消费者。"""

import fcntl
import re
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WINDOWS_SOURCE = ROOT / "platforms/windows/settings/VoiceSettings.h"


def run_cargo_check(cargo: str) -> None:
    lock_path = Path(tempfile.gettempdir()) / "msime-contract-cargo.lock"
    with lock_path.open("a+") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        subprocess.run(
            [cargo, "run", "--quiet", "--locked", "-p", "msime-client-providers", "--example",
             "export_voice_hold_key_catalog", "--", "--check"],
            cwd=ROOT, check=True, timeout=120,
        )


def check_windows_consumer(source: str) -> None:
    compiler = shutil.which("c++") or shutil.which("clang++") or shutil.which("g++")
    if compiler is None:
        print("skipped: 没有 C++ 编译器，跳过按住录音目录消费者运行检查")
        return
    with tempfile.TemporaryDirectory(prefix="msime-voice-hold-") as directory:
        temporary = Path(directory)
        catalog = temporary / "catalog.h"
        catalog.write_text(
            "#pragma once\n#include <array>\n#include <string_view>\n"
            "namespace msime::voice {\n"
            "struct VoiceHoldKeyCatalogEntry { std::wstring_view key; std::wstring_view label; bool fallback; };\n"
            "inline constexpr std::array<VoiceHoldKeyCatalogEntry, 3> kVoiceHoldKeys = {{\n"
            '  {L"hotkey_ctrl_win", L"合成首项", true},\n'
            '  {L"hotkey_ralt", L"合成次项", false},\n'
            '  {L"hotkey_rctrl_ralt", L"合成末项", false},\n'
            "}};\n}\n",
            encoding="utf-8",
        )

        # 只替换静态元数据；选择与补丁仍编译生产头文件里的真实实现。
        def include(match: re.Match[str]) -> str:
            relative = match.group(1)
            path = catalog if relative.endswith("/VoiceHoldKeyCatalog.h") else (WINDOWS_SOURCE.parent / relative).resolve()
            return f'#include "{path.as_posix()}"'

        header = temporary / "VoiceSettings.h"
        header.write_text(re.sub(r'#include "([^"]+)"', include, source), encoding="utf-8")
        probe = temporary / "probe.cpp"
        probe.write_text(
            '#include "VoiceSettings.h"\n'
            "int main() {\n  using namespace msime::settings::voice;\n"
            '  if (hold_keys[0].label != L"合成首项") return 1;\n'
            '  if (selected_hold_key({}) != L"hotkey_ctrl_win") return 2;\n'
            '  if (selected_hold_key({{L"hotkey_ctrl_win", false}}) != no_hold_key) return 3;\n'
            '  if (selected_hold_key({{L"hotkey_ralt", true}, {L"hotkey_ctrl_win", true}}) != L"hotkey_ctrl_win") return 4;\n'
            '  const auto patch = hold_key_patch(L"hotkey_ralt");\n'
            '  if (patch[0].first != L"hotkey_ctrl_win" || patch[0].second || !patch[1].second || patch[2].second) return 5;\n'
            "  return 0;\n}\n",
            encoding="utf-8",
        )
        executable = temporary / "probe"
        subprocess.run([compiler, "-std=c++17", str(probe), "-o", str(executable)], check=True, timeout=120)
        subprocess.run([str(executable)], check=True, timeout=30)


def main() -> None:
    cargo = shutil.which("cargo")
    if cargo is None:
        print("skipped: 没有 Cargo，跳过按住录音目录生成物漂移检查")
    else:
        run_cargo_check(cargo)
    check_windows_consumer(WINDOWS_SOURCE.read_text(encoding="utf-8"))
    print("按住录音共享目录及 Windows 消费者通过")


if __name__ == "__main__":
    main()
