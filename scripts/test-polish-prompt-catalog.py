#!/usr/bin/env python3
"""检查各语言消费的内置润色正文和原生展示目录相同，不读取用户配置。"""
import base64
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "packages/ui/src/voice/polish-prompt-catalog.json"
PRESETS = json.loads(CATALOG.read_text(encoding="utf-8"))
expected = {preset["id"]: preset["prompt"] for preset in PRESETS}
assert len(expected) == len(PRESETS), "润色方案 id 重复"
assert all(preset["title"] and "<asr_text>" in preset["prompt"] for preset in PRESETS)

spec = importlib.util.spec_from_file_location(
    "generated_polish_prompts", ROOT / "platforms/linux/scripts/msime_voice_polish_prompts.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
assert module.PROMPTS == expected, "Linux 润色正文与共享目录不同"

compiler = shutil.which("c++") or shutil.which("g++") or shutil.which("clang++")
if not compiler:
    print("skipped: C++ 润色目录检查需要 C++17 编译器；Python 目录检查已通过")
else:
    with tempfile.TemporaryDirectory(prefix="msime-polish-catalog-") as directory:
        source = Path(directory) / "prompt.cpp"
        program = Path(directory) / "prompt"
        source.write_text('''#include "shared/voice/PolishPrompt.h"
#include <iostream>
int main(int argc, char **argv) {
  if (argc == 3 && std::string_view(argv[2]) == "title") {
    for (const auto &preset : msime::windows::kPolishPromptPresets) {
      if (preset.id == argv[1]) { std::cout << preset.title; return 0; }
    }
    return 3;
  }
  if (argc != 2) return 2;
  msime::windows::PolishPromptSlots slots;
  slots.id = argv[1];
  std::cout << msime::windows::polish_prompt_for(slots);
}
''', encoding="utf-8")
        subprocess.run([compiler, "-std=c++17", "-I", str(ROOT), str(source), "-o", str(program)],
                       check=True, timeout=60)
        for identifier, prompt in expected.items():
            result = subprocess.check_output([str(program), identifier], timeout=10).decode("utf-8")
            assert result == prompt, f"C++ 润色正文不同：{identifier}"
        for preset in PRESETS:
            title = subprocess.check_output([str(program), preset["id"], "title"], timeout=10).decode("utf-8")
            assert title == preset["title"], f"C++ 润色名称不同：{preset['id']}"
        source.write_text('''#include "platforms/windows/settings/VoiceSettings.h"
#include "shared/voice/PolishPromptWideCatalog.h"
#include <cstdint>
#include <iostream>
int main() {
  for (const auto &preset : msime::settings::voice::polish_presets) {
    for (const auto value : {preset.id, preset.name, preset.prompt}) {
      for (const auto character : value) std::cout << static_cast<std::uint32_t>(character) << ' ';
      std::cout << '\\n';
    }
  }
}
''', encoding="utf-8")
        subprocess.run([compiler, "-std=c++17", "-I", str(ROOT), str(source), "-o", str(program)],
                       check=True, timeout=60)
        lines = subprocess.check_output([str(program)], timeout=10).decode("ascii").splitlines()
        values = ["".join(chr(int(value)) for value in line.split())
                  .encode("utf-16-le", "surrogatepass").decode("utf-16-le") for line in lines]
        projected = [dict(zip(("id", "title", "prompt"), values[index:index + 3]))
                     for index in range(0, len(values), 3)]
        assert projected == PRESETS, "Windows 设置的名称、顺序或正文与共享目录不同"
    print("polish prompt catalog: 请求正文、Python、UI 与 Windows 设置目录一致")

if sys.platform != "darwin" or not compiler:
    print("skipped: macOS 润色展示目录检查需要 Apple 宿主与 C++ 编译器")
else:
    with tempfile.TemporaryDirectory(prefix="msime-polish-macos-") as directory:
        source = Path(directory) / "catalog.mm"
        program = Path(directory) / "catalog"
        source.write_text('''#import "platforms/macos/src/voice/VoiceSettings.h"
#include <cstdio>
int main() {
  @autoreleasepool {
    NSData *data = [NSJSONSerialization dataWithJSONObject:@{
      @"ids": MSIMEPolishPromptIdentifiers(), @"titles": MSIMEPolishPromptTitles()
    } options:0 error:nil];
    if (!data) return 1;
    return std::fwrite(data.bytes, 1, data.length, stdout) == data.length ? 0 : 2;
  }
}
''', encoding="utf-8")
        subprocess.run([compiler, "-std=c++17", "-fobjc-arc", "-Wall", "-Wextra", "-Werror",
                        "-I", str(ROOT), str(source), "-framework", "AppKit", "-o", str(program)],
                       check=True, timeout=60)
        projected = json.loads(subprocess.check_output([str(program)], timeout=10))
        assert projected == {
            "ids": [preset["id"] for preset in PRESETS] + [f"custom_{index}" for index in range(1, 4)],
            "titles": [preset["title"] for preset in PRESETS] + [f"自定义{title}" for title in "一二三"],
        }, "macOS 润色方案 id、名称或顺序与共享目录不同"
    print("polish prompt catalog: macOS 实际选择表与共享目录一致，自定义槽位保留")

swiftc = shutil.which("swiftc")
if not swiftc:
    print("skipped: iOS 润色展示目录检查需要 Swift 编译器")
else:
    with tempfile.TemporaryDirectory(prefix="msime-polish-swift-") as directory:
        source = Path(directory) / "main.swift"
        program = Path(directory) / "catalog"
        # 只用标准库输出码点，避免目录探针为序列化结果冷编译 Foundation；控制字符也能无损比较。
        source.write_text('''for preset in VoicePolishPresetCatalog.builtIns {
  for value in [preset.id, preset.title] {
    print(value.unicodeScalars.map { String($0.value) }.joined(separator: " "))
  }
}
''', encoding="utf-8")
        # 保留版本和阶段命令，下一次超时才能区分前端编译与链接；失败也记录实际耗时。
        swift_compile_timeout = 120
        print(f"polish prompt catalog: Swift 编译开始（{swiftc}，限时 {swift_compile_timeout} 秒）", flush=True)
        started = time.monotonic()
        try:
            subprocess.run([swiftc, "-v", str(ROOT / "platforms/ios/App/Services/VoicePolishPresetCatalog.swift"),
                            str(source), "-o", str(program)], check=True, timeout=swift_compile_timeout)
        finally:
            print(f"polish prompt catalog: Swift 编译用时 {time.monotonic() - started:.3f} 秒", flush=True)
        lines = subprocess.check_output([str(program)], timeout=10).decode("ascii").splitlines()
        values = ["".join(chr(int(value)) for value in line.split()) for line in lines]
        projected = [dict(zip(("id", "title"), values[index:index + 2]))
                     for index in range(0, len(values), 2)]
        assert projected == [{"id": preset["id"], "title": preset["title"]} for preset in PRESETS], \
            "iOS 润色方案 id、名称或顺序与共享目录不同"
    print("polish prompt catalog: iOS 展示目录与共享目录一致")

javac = shutil.which("javac")
java = shutil.which("java")
jdk_version = subprocess.run([javac, "-version"], capture_output=True, text=True, timeout=10) if javac else None
version = re.search(r"javac (\d+)", jdk_version.stdout + jdk_version.stderr) if jdk_version else None
if not java or not version or int(version[1]) < 17:
    print("skipped: Android 润色展示目录检查需要 JDK 17")
else:
    with tempfile.TemporaryDirectory(prefix="msime-polish-java-") as directory:
        source = Path(directory) / "CatalogCheck.java"
        source.write_text('''package app.msime.android;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
public final class CatalogCheck {
  public static void main(String[] args) {
    String[] ids = VoicePolishPresetCatalog.ids();
    String[] titles = VoicePolishPresetCatalog.titles();
    ids[0] = "synthetic-id";
    titles[0] = "synthetic-title";
    if (VoicePolishPresetCatalog.ids()[0].equals(ids[0])
        || VoicePolishPresetCatalog.titles()[0].equals(titles[0])) {
      throw new AssertionError("调用方不能修改共享生成目录");
    }
    for (String id : VoicePolishSettingsPolicy.PROMPTS) {
      for (String value : new String[] {id, VoicePolishSettingsPolicy.promptLabel(id)}) {
        System.out.println(Base64.getEncoder().encodeToString(value.getBytes(StandardCharsets.UTF_8)));
      }
    }
  }
}
''', encoding="utf-8")
        voice = ROOT / "platforms/android/java/app/msime/android/voice"
        subprocess.run([javac, "--release", "17", "-encoding", "UTF-8", "-Xlint:all", "-Werror",
                        "-d", directory, str(voice / "VoicePolishPresetCatalog.java"),
                        str(voice / "VoicePolishSettingsPolicy.java"), str(source)],
                       check=True, timeout=60)
        lines = subprocess.check_output([java, "-cp", directory, "app.msime.android.CatalogCheck"],
                                        timeout=10).decode("ascii").splitlines()
        values = [base64.b64decode(line).decode("utf-8") for line in lines]
        projected = [dict(zip(("id", "title"), values[index:index + 2]))
                     for index in range(0, len(values), 2)]
        custom = [{"id": f"custom_{index}", "title": f"自定义{title}"}
                  for index, title in enumerate("一二三", start=1)]
        assert projected == [{"id": preset["id"], "title": preset["title"]}
                             for preset in PRESETS] + custom, \
            "Android 润色方案 id、名称或顺序与共享目录不同"
    print("polish prompt catalog: Android 实际选择表与共享目录一致，自定义槽位保留")
