#!/usr/bin/env python3
"""从来源 MSIME-Apple 的 `platforms/macos/tests` 抽取 require / Require 断言。

文档《macos-assertion-audit.md》原先用一条 grep 计数：

    grep -rhoE 'require\\([^,]*,\\s*"[^"]+"' *.mm *.cpp | wc -l   # 93

那条命令有三处缺陷，合起来只覆盖约一成断言：

1. 只认小写 `require(`，来源大量文件用大写 `Require(bool, const char*)`；
2. `[^,]*` 遇到首参里的逗号（`require(foo(x, y), "...")`、`require(a || b, "...")`）就失配；
3. 不跨行、也不看 `.swift`（`BackendAccountTests.swift` 用的是抛 `Failure` 的
   `require(_ value: Bool)`，本来就没有消息文本）。

本脚本按括号配对切分调用，所以首参里的逗号、括号与字符串都不会打断匹配，并覆盖
`.mm` / `.cpp` / `.swift`。计数分三档：

- 断言总数：所有 `require(` / `Require(` 出现次数；
- 带字符串消息：第二个参数是 `@"..."` / `"..."` 字面量的条数（旧命令想数的就是这一档）；
- 无消息：单参断言、以 `error.localizedDescription.UTF8String` 之类表达式作消息的断言，
  以及函数定义本身。这些没有可比对的文本，只能逐条读代码核对。

用法：extract_source_assertions.py <来源仓库根或 platforms/macos/tests 路径>
"""

import re
import sys
from pathlib import Path


def calls(text: str):
    """产出 (宏名, 参数原文) 列表。"""
    for match in re.finditer(r"\b([Rr]equire)\s*\(", text):
        index = match.end()
        depth = 1
        start = index
        while index < len(text) and depth:
            character = text[index]
            if character == '"':
                index += 1
                while index < len(text) and text[index] != '"':
                    if text[index] == "\\":
                        index += 1
                    index += 1
            elif character in "([{":
                depth += 1
            elif character in ")]}":
                depth -= 1
                if depth == 0:
                    break
            index += 1
        yield match.group(1), text[start:index]


def split_top_level(arguments: str) -> list[str]:
    """按顶层逗号切分参数，跳过字符串与括号内部。"""
    parts: list[str] = []
    depth = 0
    current: list[str] = []
    index = 0
    while index < len(arguments):
        character = arguments[index]
        if character == '"':
            current.append(character)
            index += 1
            while index < len(arguments) and arguments[index] != '"':
                current.append(arguments[index])
                if arguments[index] == "\\":
                    index += 1
                    if index < len(arguments):
                        current.append(arguments[index])
                index += 1
            if index < len(arguments):
                current.append(arguments[index])
        elif character in "([{":
            depth += 1
            current.append(character)
        elif character in ")]}":
            depth -= 1
            current.append(character)
        elif character == "," and depth == 0:
            parts.append("".join(current))
            current = []
        else:
            current.append(character)
        index += 1
    if current:
        parts.append("".join(current))
    return parts


def message_of(arguments: str) -> str | None:
    """第二个参数若为字符串字面量则返回其内容，否则 None。"""
    parts = split_top_level(arguments)
    if len(parts) < 2:
        return None
    match = re.fullmatch(r'@?"((?:[^"\\]|\\.)*)"', parts[1].strip())
    return match.group(1) if match else None


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: extract_source_assertions.py <source root>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])
    tests = root / "platforms/macos/tests"
    if not tests.is_dir():
        tests = root
    if not tests.is_dir():
        print(f"no tests directory under {root}", file=sys.stderr)
        return 1

    total = with_message = 0
    messages: set[str] = set()
    per_file: list[tuple[str, int, int]] = []
    for path in sorted(tests.rglob("*")):
        if not path.is_file() or path.suffix not in {".mm", ".cpp", ".swift"}:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        count = matched = 0
        for _, arguments in calls(text):
            count += 1
            message = message_of(arguments)
            if message is not None:
                matched += 1
                messages.add(message)
        if count:
            per_file.append((str(path.relative_to(tests)), count, matched))
        total += count
        with_message += matched

    for relative, count, matched in per_file:
        print(f"{count:5d} 断言 / {matched:5d} 带消息  {relative}")
    print(f"断言总数: {total}")
    print(f"带字符串消息: {with_message}")
    print(f"无消息或非字符串消息: {total - with_message}")
    print(f"去重后的消息条数: {len(messages)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
