#!/usr/bin/env python3
"""Keep the single-character reading ``昍 xuan`` in generated dictionaries.

The source data contains this character, but the locked Engine whitelist omitted it.  That
omission affects both quanpin and the generated Xiaohe shuangpin tables because those tables are
built from the same single-character input.
"""

from pathlib import Path


def apply(root: Path) -> None:
    path = root / "dictionary/cn/SingleCharWhitelist.txt"
    text = path.read_text(encoding="utf-8")
    if any(line == "昍" for line in text.splitlines()):
        return
    anchor = "昊\n"
    if text.count(anchor) != 1:
        raise RuntimeError(f"Engine overlay expected one whitelist anchor in {path}")
    path.write_text(text.replace(anchor, anchor + "昍\n", 1), encoding="utf-8")


if __name__ == "__main__":
    import sys

    apply(Path(sys.argv[1]))
