#!/usr/bin/env python3
"""核对语音语言目录的共享数据和各平台消费者。"""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from contract_cargo import contract_cargo_lock


ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "packages/ui/src/voice/voice-language-catalog.json"
HEADER = ROOT / "shared/voice/VoiceLanguageCatalog.h"
UI_SOURCE = ROOT / "packages/ui/src/voice/voice-language-options.tsx"
WINDOWS_SOURCE = ROOT / "platforms/windows/settings/VoiceSettings.h"
ANDROID_SOURCE = ROOT / "platforms/android/java/app/msime/android/home/VoicePage.java"
IOS_SOURCE = ROOT / "platforms/ios/App/Services/VoicePolish.swift"
IOS_CATALOG = ROOT / "platforms/ios/App/Services/VoiceLanguageCatalog.swift"

EXPECTED = [
    {
        "id": "zh-cn",
        "provider_value": "zh-cn",
        "provider_title": "中文（普通话）",
        "system_value": "zh-CN",
        "system_title": "中文（普通话）",
        "mobile_value": "zh-cn",
        "mobile_title": "普通话",
    },
    {
        "id": "yue",
        "provider_value": "",
        "provider_title": "",
        "system_value": "",
        "system_title": "",
        "mobile_value": "yue",
        "mobile_title": "粤语",
    },
    {
        "id": "en",
        "provider_value": "en",
        "provider_title": "English",
        "system_value": "en-US",
        "system_title": "English",
        "mobile_value": "en",
        "mobile_title": "英语",
    },
    {
        "id": "ja",
        "provider_value": "ja",
        "provider_title": "日本語",
        "system_value": "ja-JP",
        "system_title": "日本語",
        "mobile_value": "",
        "mobile_title": "日语",
    },
    {
        "id": "auto",
        "provider_value": "auto",
        "provider_title": "自动识别",
        "system_value": "",
        "system_title": "",
        "mobile_value": "auto",
        "mobile_title": "普通话 + 英语",
    },
]


def run_cargo_check(cargo: str) -> None:
    with contract_cargo_lock():
        subprocess.run(
            [cargo, "run", "--quiet", "--locked", "-p", "msime-client-providers",
             "--example", "export_voice_language_catalog", "--", "--check"],
            cwd=ROOT,
            check=True,
            timeout=120,
        )


def main() -> None:
    if not CATALOG.exists() or not HEADER.exists() or not IOS_CATALOG.exists():
        raise AssertionError("语音语言目录生成物尚未接入")
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    if catalog != EXPECTED:
        raise AssertionError(f"语音语言目录数据不符合契约：{catalog!r}")

    cargo = shutil.which("cargo")
    if cargo is None:
        print("skipped: 没有 Cargo，跳过语言目录生成器漂移检查")
    else:
        run_cargo_check(cargo)

    ui = UI_SOURCE.read_text(encoding="utf-8")
    if 'from "./voice-language-catalog.json"' not in ui:
        raise AssertionError("UI 没有读取生成的语言目录")
    if 'value={systemVoice ? "zh-CN" : "zh-cn"}' in ui:
        raise AssertionError("UI 仍保留服务语言值的手写副本")
    if "MOBILE_VOICE_LANGUAGES = [" in ui:
        raise AssertionError("UI 仍保留移动语言表的手写副本")
    if "choice.mobile_value" not in ui or "choice.mobile_title" not in ui:
        raise AssertionError("UI 没有从共享目录投影移动语言选项")

    windows = WINDOWS_SOURCE.read_text(encoding="utf-8")
    if '"../../../shared/voice/VoiceLanguageCatalog.h"' not in windows:
        raise AssertionError("Windows 没有接入生成的语言目录")
    if "inline std::vector<Choice> language_choices(bool system_voice)" not in windows:
        raise AssertionError("Windows 语言选择入口已改变，无法核对消费者")

    compiler = shutil.which("c++") or shutil.which("clang++") or shutil.which("g++")
    if compiler is None:
        print("skipped: 没有 C++ 编译器，跳过 Windows 语言消费者运行检查")
        return
    with tempfile.TemporaryDirectory(prefix="msime-voice-language-") as directory:
        source = Path(directory) / "probe.cpp"
        executable = Path(directory) / "probe"
        source.write_text(
            '#include "platforms/windows/settings/VoiceSettings.h"\n'
            "#include <string>\n"
            "int main() {\n"
            "  using namespace msime::settings::voice;\n"
            "  const auto providers = language_choices(false);\n"
            "  const auto system = language_choices(true);\n"
            '  if (providers.size() != 4 || providers[0].value != L"zh-cn" || '
            'providers[3].value != L"auto") return 1;\n'
            '  if (system.size() != 3 || system[0].value != L"zh-CN" || '
            'system[2].value != L"ja-JP") return 2;\n'
            "  return 0;\n"
            "}\n",
            encoding="utf-8",
        )
        subprocess.run(
            [compiler, "-std=c++17", "-I", str(ROOT), str(source), "-o", str(executable)],
            cwd=ROOT,
            check=True,
            timeout=120,
        )
        subprocess.run([str(executable)], cwd=ROOT, check=True, timeout=30)
    android = ANDROID_SOURCE.read_text(encoding="utf-8")
    if "VoiceLanguageCatalog.mobile()" not in android or "VoiceLanguageCatalog.title(value)" not in android:
        raise AssertionError("Android 没有接入生成的移动语言目录")
    if 'LANGUAGES = {"zh-cn", "yue", "en", "auto"}' in android:
        raise AssertionError("Android 仍保留移动语言表的手写副本")
    ios = IOS_SOURCE.read_text(encoding="utf-8")
    if "static let languages = VoiceLanguageCatalog.mobile" not in ios:
        raise AssertionError("iOS 没有接入生成的移动语言目录")
    swift_catalog = IOS_CATALOG.read_text(encoding="utf-8")
    if 'static let mobile: [(id: String, title: String)]' not in swift_catalog:
        raise AssertionError("iOS 语言目录没有生成移动端投影")
    if '("zh-cn", "普通话")' not in swift_catalog or '("auto", "普通话 + 英语")' not in swift_catalog:
        raise AssertionError("iOS 语言目录缺少稳定的移动端选项")
    print("语音语言目录的 UI、Windows、Android 与 iOS 消费者通过")


if __name__ == "__main__":
    main()
