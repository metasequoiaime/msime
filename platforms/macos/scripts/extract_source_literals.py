#!/usr/bin/env python3
"""从来源 MSIME-Apple 抽取含汉字的中文字符串字面量，供《macos-parity.md》的文案轴复跑。

口径（此前文档里的 623 / 643 来自一次没有记录、无法复跑的过滤，本脚本把口径固定下来）：

- 范围：`platforms/macos/src` 下的 `.h/.m/.mm/.cpp/.swift`，排除 `*Tests*`；
- 只取单行字符串字面量（`@"..."` 或 `"..."`）。跨行的原始 C 字符串是代码片段而不是文案，
  单字符字面量（`@"中"`、`@"英"` 这类简繁开关用的字符）与长错误消息也照收——它们确实是
  用户可能看到的文本，收不收属于口径选择，这里选「收」，所以数字比旧文档大；
- 按字面量内容去重，同时报出现次数。

用法：extract_source_literals.py <来源仓库根>
"""

import re
import sys
from pathlib import Path

HAN = re.compile(r"[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]")
LITERAL = re.compile(r'@?"((?:[^"\\\n]|\\.)*)"')


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: extract_source_literals.py <source root>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])
    source = root / "platforms/macos/src"
    if not source.is_dir():
        print(f"no platforms/macos/src under {root}", file=sys.stderr)
        return 1

    seen: dict[str, int] = {}
    for path in sorted(source.rglob("*")):
        if not path.is_file() or path.suffix not in {".h", ".m", ".mm", ".cpp", ".swift"}:
            continue
        if "Tests" in path.parts or path.name.endswith("Tests" + path.suffix):
            continue
        for match in LITERAL.finditer(path.read_text(encoding="utf-8", errors="ignore")):
            literal = match.group(1)
            if HAN.search(literal):
                seen[literal] = seen.get(literal, 0) + 1

    print(f"含汉字的中文字面量（去重后）: {len(seen)}")
    print(f"含汉字的中文字面量（出现次数）: {sum(seen.values())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
