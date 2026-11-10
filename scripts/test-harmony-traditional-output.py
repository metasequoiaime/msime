#!/usr/bin/env python3
"""检查鸿蒙中文输出是否接入共享的 OpenCC 转换器。

其他宿主通过 `msime_client_simplified_to_traditional` 使用 Rust 编译的短语级 OpenCC 表，
因此「头发」变成「頭髮」、「发展」变成「發展」。ICU 的 `Simplified-Traditional` 转写器
逐字转换，无法区分这两个「发」，会让鸿蒙键盘提交「頭發」。注音简体输出还通过
`msime_client_traditional_to_simplified` 走反向表。Node 套件看不到会话传给
`ChineseOutputPolicy` 的具体转换器，设备外也无法调用 native 函数，因此这里固定从 C 头文件
到调用点的接线。
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADER = ROOT / "crates/host-api/include/msime_client.h"
NATIVE = ROOT / "platforms/harmony/native/client_napi.cpp"
TYPES = ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts"
SESSION = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardSession.ets"
VIEW = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"
SOURCES = ROOT / "platforms/harmony/entry/src/main/ets"
POLICY = SOURCES / "keyboard/input/ChineseOutputPolicy.ts"


def body(text: str, opening: str, closing: str) -> str:
    """The definition that starts at `opening`, or nothing when it is gone."""
    start = text.find(opening)
    if start < 0:
        return ""
    end = text.find(closing, start)
    return text[start:] if end < 0 else text[start:end]


def main() -> int:
    header = HEADER.read_text(encoding="utf-8")
    native = NATIVE.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    session = SESSION.read_text(encoding="utf-8")
    view = VIEW.read_text(encoding="utf-8")
    policy = POLICY.read_text(encoding="utf-8")

    binding = body(native, "static napi_value SimplifiedToTraditional(", "\n}\n")
    reverse_binding = body(native, "static napi_value TraditionalToSimplified(", "\n}\n")
    convert = body(session, "  private asTraditional(text: string, source: number = -1): string {", "\n  }\n")

    required = {
        "C ABI declaration": "char *msime_client_simplified_to_traditional(const uint8_t *text, size_t length);"
        in header,
        "NAPI call": "msime_client_simplified_to_traditional(" in binding,
        # `response` frees the Rust string after copying it into an ArkTS string.
        "NAPI result released": "return response(env, converted);" in binding,
        "NAPI null for a refused input": "napi_get_null(" in binding,
        "NAPI export": 'ENTRY("simplifiedToTraditional", SimplifiedToTraditional)' in native,
        "ArkTS declaration": "export const simplifiedToTraditional: (text: string) => string | null;" in types,
        "session converts natively": "client.simplifiedToTraditional(value)" in convert,
        "session keeps the output policy": "ChineseOutputPolicy.candidateOutput(text, this.traditional," in convert
        and "ChineseOutputPolicy.applies(this.englishMode, this.scheme, this.localMode)" in convert
        and re.search(r"localMode\),\s*source,", convert) is not None,
        "reverse C ABI declaration": "char *msime_client_traditional_to_simplified(const uint8_t *text, size_t length);" in header,
        "reverse NAPI call": "msime_client_traditional_to_simplified(" in reverse_binding,
        "reverse NAPI result released": "return response(env, converted);" in reverse_binding,
        "reverse NAPI export": 'ENTRY("traditionalToSimplified", TraditionalToSimplified)' in native,
        "reverse ArkTS declaration": "export const traditionalToSimplified: (text: string) => string | null;" in types,
        "session uses reverse converter": "client.traditionalToSimplified(value)" in session,
        "policy has Zhuyin switch": "switchFollowsZhuyin" in policy and "toggledZhuyinSimplified" in policy,
        # 候选栏、平面列表和展开面板都携带来源，防止整行再次转换多语言候选。
        "candidate display converts": session.count("text: this.asTraditional(candidate.text, candidate.source)") == 3,
        "multilingual candidate keeps segment conversion":
        "ChineseOutputPolicy.output(text, traditional, applies && source !== 18, converter)" in policy,
        "commit keeps runtime conversion boundary":
        "value.commit_context?.script_conversion === false" in session
        and "!this.characterSetFollowsZhuyin() ? commit : this.asTraditional(commit)" in session,
        # 开关在存储接受新值之后才翻转，随即重发候选，所以被拒绝的写入不改变输出。粤拼下开关走前面的粤拼分支并在这次写入之前返回，那一支的行为由 keyboard-logic.test.ts 的 `toggledCantoneseSimplified` 用例检查。
        "switch republishes the candidates": "this.publishCurrentCandidates();" in session
        and "this.zhuyinSimplified = ChineseOutputPolicy.toggledZhuyinSimplified" in session,
        # 键盘的「繁」按键必须走同一个会话开关，不能绕过会话直接写偏好。
        "keyboard tile uses the session switch": "this.traditional = KeyboardSession.shared.toggleCharacterSet();" in view,
    }
    problems = [name for name, present in required.items() if not present]

    for path in sorted(SOURCES.rglob("*")):
        if path.suffix not in (".ets", ".ts") or not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        if re.search(r"Transliterator|Simplified-Traditional", text):
            problems.append(f"ICU transliteration in {path.relative_to(ROOT)}")

    if problems:
        print("harmony traditional output: missing " + ", ".join(problems), file=sys.stderr)
        return 1
    print("harmony Chinese output: candidates and commits use the shared OpenCC s2t/t2s tables")
    return 0


if __name__ == "__main__":
    sys.exit(main())
