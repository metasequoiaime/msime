#!/usr/bin/env python3
"""检查普通 TypeScript 编译器不强制执行的 ArkTS 语法子集。

`tests/run.sh` 只编译移植的 `.ts` 逻辑，应用的 `.ets` 由 `hvigorw assembleHap`
调用真正的 ArkTS 编译器。逻辑测试、设置页构建和 quick 通过，仍可能打不出 HAP；
此前多批合入的 PR 就留下过这类错误。此脚本只检查无需 SDK 即可识别的窄写法：

  arkts-no-any-unknown           类型位置的 `any` 与 `unknown`。
  arkts-no-aliases-by-index      类型位置的 `T["member"]`。
  arkts-no-spread                以对象展开开头的字面量 `{ ...other }`。
  arkts-limited-throw            catch 内直接重抛无类型绑定，未写 `as Error`。
  arkts-no-obj-literals-as-types 参数、返回值和接口成员中的对象字面量类型。

只扫描应用的 `.ets`，旁边的 `.ts` 仍按普通 TypeScript 检查；把规则扩大到 `.ts`
曾误报编译器接受的数百处写法。接口体只有类型声明，所以能检查其中的 `key: {`；
在接口之外，这也可能是合法的对象值，不能全文件一律禁止。

`arkts-no-untyped-obj-literals` 是否违规取决于接收方类型，纯文本规则无法可靠判断。
漏 import、只读数组赋给可变状态等类型兼容问题也留给真实编译器。此检查用于提前
拦截已知语法错误，不替代带原生库、资源、依赖和 SDK 的 HAP 构建。
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
# Only the application's own sources. `tests/` is compiled by tsc on purpose and is not shipped.
SOURCES = ROOT / "platforms/harmony/entry/src/main/ets"


def strip_noise(text: str) -> str:
    """Blank out comments and string bodies so their contents cannot look like code."""
    out = []
    index = 0
    length = len(text)
    while index < length:
        pair = text[index : index + 2]
        if pair == "//":
            end = text.find("\n", index)
            end = length if end < 0 else end
            out.append(" " * (end - index))
            index = end
        elif pair == "/*":
            end = text.find("*/", index + 2)
            end = length if end < 0 else end + 2
            # Keep newlines so reported line numbers stay right.
            out.append("".join(c if c == "\n" else " " for c in text[index:end]))
            index = end
        elif text[index] in "\"'`":
            quote = text[index]
            end = index + 1
            while end < length and text[end] != quote:
                if text[end] == "\\":
                    end += 1
                end += 1
            end = min(end + 1, length)
            out.append("".join(c if c == "\n" else " " for c in text[index:end]))
            index = end
        else:
            out.append(text[index])
            index += 1
    return "".join(out)


# `unknown`/`any` in a type position: after a colon, an `as`, inside a generic, or in a union.
# Requiring the prefix is what keeps a variable named `any` from being reported as a type — the
# first version of this did exactly that, on code the real compiler had just accepted.
BANNED_TYPE = re.compile(r"(?:[:<|&,]|\bas\b)\s*(unknown|any)(?![A-Za-z0-9_$])")
# `Something["member"]` in a type position: after `:`, `as`, `<`, or an arrow return.
INDEXED_TYPE = re.compile(r"(?::\s*|\bas\s+|<)\s*[A-Z][A-Za-z0-9_]*\s*\[\s*['\"]")
# An object literal whose first member is a spread. A spread later in the literal looks the same as
# one in an argument list, so only the leading form is read; it is the one copying code writes.
OBJECT_SPREAD = re.compile(r"\{\s*\.\.\.")
CATCH = re.compile(r"\bcatch\s*\(\s*([A-Za-z_$][A-Za-z0-9_$]*)\s*\)\s*\{")
# An object literal type on a parameter or a return. `key: {` elsewhere is just as often a value
# inside an object literal, which ArkTS accepts, so those positions are left to the compiler.
LITERAL_PARAMETER_TYPE = re.compile(r"\(\s*[A-Za-z_$][A-Za-z0-9_$]*\s*\??\s*:\s*\{\s*[A-Za-z_$][A-Za-z0-9_$]*\s*\??\s*:")
LITERAL_RETURN_TYPE = re.compile(r"\)\s*:\s*\{\s*[A-Za-z_$][A-Za-z0-9_$]*\s*\??\s*:")
# 接口体只有类型声明；在这个范围内，成员后的 `{` 不会是对象值。
INTERFACE = re.compile(r"\binterface\s+[A-Za-z_$][A-Za-z0-9_$]*\b[^{};]*\{")
LITERAL_MEMBER_TYPE = re.compile(r"\b[A-Za-z_$][A-Za-z0-9_$]*\s*\??\s*:\s*\{")


def block_end(text: str, open_brace: int) -> int:
    """Index just past the brace that closes the one at `open_brace`."""
    depth = 0
    for index in range(open_brace, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    return len(text)


def line_of(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def findings(path: pathlib.Path) -> list[tuple[int, str]]:
    text = strip_noise(path.read_text(encoding="utf-8"))
    found: list[tuple[int, str]] = []
    for number, line in enumerate(text.splitlines(), start=1):
        for match in BANNED_TYPE.finditer(line):
            found.append((number, f'"{match.group(1)}" is not a type in ArkTS (arkts-no-any-unknown)'))
        if INDEXED_TYPE.search(line):
            found.append((number, "indexed access type (arkts-no-aliases-by-index)"))
    for match in OBJECT_SPREAD.finditer(text):
        found.append((line_of(text, match.start()), "object spread (arkts-no-spread)"))
    for pattern in (LITERAL_PARAMETER_TYPE, LITERAL_RETURN_TYPE):
        for match in pattern.finditer(text):
            found.append((
                line_of(text, match.start()),
                "object literal used as a type (arkts-no-obj-literals-as-types)",
            ))
    for interface in INTERFACE.finditer(text):
        start = interface.end()
        body = text[start : block_end(text, start - 1) - 1]
        for member in LITERAL_MEMBER_TYPE.finditer(body):
            found.append((
                line_of(text, start + member.start()),
                "object literal used as a type (arkts-no-obj-literals-as-types)",
            ))
    for match in CATCH.finditer(text):
        name = re.escape(match.group(1))
        start = match.end() - 1
        body = text[start : block_end(text, start)]
        for rethrow in re.finditer(rf"\bthrow\s+{name}\s*;", body):
            found.append((
                line_of(text, start + rethrow.start()),
                f"untyped catch binding rethrown; write `throw {match.group(1)} as Error` (arkts-limited-throw)",
            ))
    return sorted(set(found))


def main() -> int:
    if not SOURCES.is_dir():
        print(f"harmony ArkTS subset: {SOURCES} is missing", file=sys.stderr)
        return 1
    files = sorted(SOURCES.rglob("*.ets"))
    if not files:
        print("harmony ArkTS subset: no sources found; the extraction is wrong", file=sys.stderr)
        return 1
    problems = 0
    for path in files:
        for number, reason in findings(path):
            print(f"{path.relative_to(ROOT)}:{number}: {reason}", file=sys.stderr)
            problems += 1
    if problems:
        print(
            f"harmony ArkTS subset: {problems} construction(s) the ArkTS compiler rejects; "
            "tsc accepts them and the HAP will not build",
            file=sys.stderr,
        )
        return 1
    print(f"harmony ArkTS subset: {len(files)} host sources pass the checked rules; HAP compilation is still required")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
