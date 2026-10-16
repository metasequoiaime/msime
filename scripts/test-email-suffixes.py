#!/usr/bin/env python3
"""邮箱后缀两处一致：Android 的 `EmailSuffixPolicy.SUFFIXES` 与引擎网址模式的 `EMAIL_SUFFIXES` 是同一张表、同一个顺序。

Android 在邮箱输入框里由宿主自己补全后缀，符号面板「网络」分类也列出这份表；其他平台在中文组字里打到 `xxx@` 时由引擎给出后缀候选。两份各自都能编译、能出候选，某一边加了、删了或挪了一个后缀，两处给的后缀就悄悄不一样了，只有把两份放在一起比才看得出来。
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ENGINE = ROOT / "crates/engine/src/local/url.rs"
ANDROID = ROOT / "platforms/android/java/app/msime/android/policy/EmailSuffixPolicy.java"


def suffixes(path: pathlib.Path, pattern: str, findings: list[str]) -> list[str] | None:
    declaration = re.search(pattern, path.read_text(encoding="utf-8"), re.S)
    if not declaration:
        findings.append(f"{path.relative_to(ROOT)} 不再以字面量声明邮箱后缀表")
        return None
    return re.findall(r'"(@[^"]+)"', declaration.group(1))


def main() -> int:
    findings: list[str] = []
    engine = suffixes(ENGINE, r"pub const EMAIL_SUFFIXES: \[&str; \d+\] = \[(.*?)\];", findings)
    android = suffixes(ANDROID, r"List<String> SUFFIXES = List\.of\((.*?)\);", findings)
    if engine is not None and android is not None and engine != android:
        findings.append(f"引擎的 EMAIL_SUFFIXES {engine} 与 Android 的 SUFFIXES {android} 不同")
    if engine is not None and not engine:
        findings.append("引擎的 EMAIL_SUFFIXES 是空的")
    if findings:
        for finding in findings:
            print(f"FAIL {finding}", file=sys.stderr)
        return 1
    print(f"邮箱后缀：引擎与 Android 一致（{len(engine)} 个）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
