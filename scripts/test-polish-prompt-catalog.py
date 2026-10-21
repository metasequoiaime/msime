#!/usr/bin/env python3
"""检查各语言消费的内置润色正文和 Windows 展示目录相同，不读取用户配置。"""
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

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
