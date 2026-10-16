#!/usr/bin/env python3
"""邮箱后缀四处一致：引擎网址模式的 `EMAIL_SUFFIXES`、Android 的 `EmailSuffixPolicy.SUFFIXES`、iOS 键盘的后缀表和 HarmonyOS 的 `EMAIL_SUFFIXES` 是同一张表、同一个顺序。

引擎在中文组字里打到 `xxx@` 时给出后缀候选；Android、iOS 和 HarmonyOS 的宿主在输入框里（密码框除外）光标前打到 `xxx@` 时自己补全后缀，Android 的符号面板「网络」分类也列出这份表。四份各自都能编译、能出候选，某一处加了、删了或挪了一个后缀，各处给的后缀就悄悄不一样了，只有把它们放在一起比才看得出来。以引擎的表为准，逐份比较内容和顺序，哪一份不同就报哪一份。
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# 每一份：名字、文件、声明后缀表的字面量（第一组是方括号或括号里的全部条目）。第一份是基准。
SOURCES = [
    (
        "引擎的 EMAIL_SUFFIXES",
        ROOT / "crates/engine/src/local/url.rs",
        r"pub const EMAIL_SUFFIXES: \[&str; \d+\] = \[([^\]]*)\];",
    ),
    (
        "Android 的 EmailSuffixPolicy.SUFFIXES",
        ROOT / "platforms/android/java/app/msime/android/policy/EmailSuffixPolicy.java",
        r"List<String> SUFFIXES = List\.of\(([^)]*)\);",
    ),
    (
        "iOS 的 EmailSuffixPolicy.suffixes",
        ROOT / "platforms/ios/KeyboardExtension/Sources/input/EmailSuffixPolicy.swift",
        r"static let suffixes(?:: \[String\])? = \[([^\]]*)\]",
    ),
    (
        "HarmonyOS 的 EMAIL_SUFFIXES",
        ROOT / "platforms/harmony/entry/src/main/ets/keyboard/input/EmailSuffixPolicy.ts",
        r"export const EMAIL_SUFFIXES(?:: string\[\])? = \[([^\]]*)\];",
    ),
]


def suffixes(path: pathlib.Path, pattern: str, findings: list[str]) -> list[str] | None:
    relative = path.relative_to(ROOT)
    if not path.is_file():
        findings.append(f"找不到 {relative}")
        return None
    declaration = re.search(pattern, path.read_text(encoding="utf-8"))
    if not declaration:
        findings.append(f"{relative} 不再以字面量声明邮箱后缀表")
        return None
    found = re.findall(r'"(@[^"]+)"', declaration.group(1))
    if not found:
        findings.append(f"{relative} 的邮箱后缀表是空的")
        return None
    return found


def difference(reference: list[str], other: list[str]) -> str:
    """说清楚一份表和基准差在哪：少了哪些、多了哪些；条目相同时是第几个起顺序不同。"""
    missing = [item for item in reference if item not in other]
    extra = [item for item in other if item not in reference]
    parts = []
    if missing:
        parts.append(f"少了 {missing}")
    if extra:
        parts.append(f"多了 {extra}")
    if len(other) != len(set(other)):
        parts.append("有重复的条目")
    if not parts:
        index = next(i for i, (a, b) in enumerate(zip(reference, other)) if a != b)
        parts.append(f"第 {index + 1} 个起顺序不同（基准是 {reference[index]}，这里是 {other[index]}）")
    return "，".join(parts)


def main() -> int:
    findings: list[str] = []
    tables = [(name, suffixes(path, pattern, findings)) for name, path, pattern in SOURCES]
    reference_name, reference = tables[0]
    if reference is not None:
        for name, table in tables[1:]:
            if table is not None and table != reference:
                findings.append(f"{name} 与{reference_name} 不同：{difference(reference, table)}")
    if findings:
        for finding in findings:
            print(f"FAIL {finding}", file=sys.stderr)
        return 1
    names = "、".join(name.split(" ", 1)[0].removesuffix("的") for name, _, _ in SOURCES)
    print(f"邮箱后缀：{names} 一致（{len(reference)} 个）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
