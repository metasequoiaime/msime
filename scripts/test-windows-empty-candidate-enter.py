#!/usr/bin/env python3
"""编译生产组字功能键分支，验证空候选回车仍提交原文。"""
from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


def block(source: str, start: str) -> str:
    offset = source.index(start)
    opening = source.index("{", offset)
    depth = 1
    end = opening + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[offset:end]


HARNESS = r'''
#include <cstdio>
#include <initializer_list>
using BOOL = int;
using UINT = unsigned;
constexpr BOOL TRUE = 1, FALSE = 0;
@@ENUMS@@
struct _KEYSTROKE_STATE {
    KEYSTROKE_CATEGORY Category = CATEGORY_NONE;
    KEYSTROKE_FUNCTION Function = FUNCTION_NONE;
};
// 候选模式值只在分类分支里比较；原列表模式由生产函数更早的分支处理。
enum CANDIDATE_MODE { CANDIDATE_NONE, CANDIDATE_INCREMENTAL, CANDIDATE_ORIGINAL };
@@KEYS@@
bool shift = false;
short GetAsyncKeyState(UINT key) { return key == VK_SHIFT && shift ? short(0x8000) : 0; }
BOOL classify(UINT uCode, BOOL fComposing, CANDIDATE_MODE candidateMode, _KEYSTROKE_STATE *pKeyState) {
@@PRODUCTION@@
    return FALSE;
}
int checks = 0, failures = 0;
void expect(bool value, const char *description) {
    ++checks;
    if (!value) { ++failures; std::fprintf(stderr, "FAIL %s\n", description); }
}
void route(UINT key, CANDIDATE_MODE mode, KEYSTROKE_CATEGORY category,
           KEYSTROKE_FUNCTION function, const char *description) {
    _KEYSTROKE_STATE result;
    expect(classify(key, TRUE, mode, &result) && result.Category == category && result.Function == function,
           description);
}
int main() {
    for (auto mode : {CANDIDATE_NONE, CANDIDATE_INCREMENTAL}) {
        route(VK_RETURN, mode, CATEGORY_CANDIDATE, FUNCTION_FINALIZE_CANDIDATELISTForVKReturn,
              mode == CANDIDATE_NONE ? "空候选的回车必须进入原文提交处理器" : "有候选的回车仍提交原文");
        expect(classify(VK_RETURN, TRUE, mode, nullptr), "只探测回车时也必须报告接管");
        _KEYSTROKE_STATE untouched;
        expect(!classify(VK_RETURN, FALSE, mode, &untouched)
               && untouched.Category == CATEGORY_NONE && untouched.Function == FUNCTION_NONE,
               "没有组字时不接管应用回车");
        route(VK_BACK, mode, CATEGORY_COMPOSING, FUNCTION_BACKSPACE, "退格仍编辑原组字");
        route(VK_DELETE, mode, CATEGORY_COMPOSING, FUNCTION_DELETE, "Delete仍编辑原组字");
        route(VK_ESCAPE, mode, mode == CANDIDATE_NONE ? CATEGORY_COMPOSING : CATEGORY_CANDIDATE,
              FUNCTION_CANCEL, "Esc仍走原取消路径");
        route(VK_SPACE, mode, mode == CANDIDATE_NONE ? CATEGORY_COMPOSING : CATEGORY_CANDIDATE,
              FUNCTION_CONVERT, "空格仍尝试转换，不改成回车原文提交");
        for (bool held : {false, true}) {
            shift = held;
            route(VK_TAB, mode, CATEGORY_COMPOSING, held ? FUNCTION_MOVE_PAGE_UP : FUNCTION_MOVE_PAGE_DOWN,
                  "Tab与Shift+Tab保留原翻页规则");
        }
    }
    route(VK_LEFT, CANDIDATE_NONE, CATEGORY_COMPOSING, FUNCTION_MOVE_LEFT, "空候选左右键仍编辑光标");
    _KEYSTROKE_STATE incremental;
    expect(!classify(VK_LEFT, TRUE, CANDIDATE_INCREMENTAL, &incremental)
           && incremental.Category == CATEGORY_INVOKE_COMPOSITION_EDIT_SESSION
           && incremental.Function == FUNCTION_CANCEL, "有候选左右键仍保留原交还应用规则");
    std::printf("Windows空候选回车：%d项检查，%d项失败\n", checks, failures);
    return failures != 0;
}
'''


def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++")
    if compiler is None:
        print("skipped: 空候选回车行为回归需要C++编译器")
        return 0
    engine = (ROOT / "platforms/windows/tsf/Composition/CompositionProcessorEngine.cpp").read_text()
    start = engine.index("// System pre-defined keystroke", engine.index("BOOL CCompositionProcessorEngine::IsVirtualKeyNeed("))
    production = block(engine[start:], "if (fComposing)")
    types = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIMEBaseStructure.h").read_text()
    enums = "\n".join(block(types, "enum " + name) + ";" for name in ("KEYSTROKE_CATEGORY", "KEYSTROKE_FUNCTION"))
    # 虚拟键的具体数值不影响这段生产分支，给每种键独立值即可执行所有真实switch分支。
    keys = "\n".join(f"constexpr UINT {name} = {index + 1};" for index, name in enumerate(sorted(set(re.findall(r"\bVK_[A-Z_]+\b", production)))))
    harness = HARNESS.replace("@@ENUMS@@", enums).replace("@@KEYS@@", keys).replace("@@PRODUCTION@@", production)
    with tempfile.TemporaryDirectory(prefix="msime-empty-candidate-enter-") as temporary:
        directory = pathlib.Path(temporary)
        source = directory / "test.cpp"
        executable = directory / ("test.exe" if os.name == "nt" else "test")
        source.write_text(harness)
        subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", str(source), "-o", str(executable)], check=True)
        return subprocess.run([str(executable)], check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
