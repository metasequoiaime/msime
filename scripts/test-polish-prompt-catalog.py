#!/usr/bin/env python3
"""检查三种语言消费的内置润色正文相同，不读取用户配置。"""
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
    print("polish prompt catalog: C++、Python 和 UI 静态目录正文一致")
