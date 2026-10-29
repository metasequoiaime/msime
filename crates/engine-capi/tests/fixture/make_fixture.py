#!/usr/bin/env python3
"""生成 engine-capi 的 C 示例和 CI 作业打字用的最小资源目录。

  make_fixture.py RESOURCES_DIR

按本目录的 .sql 在 RESOURCES_DIR 里建 msime-pinyin.db 和 msime-english.db，已有的先删掉。只用标准库。
"""
from __future__ import annotations

import sqlite3
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    resources = Path(sys.argv[1])
    resources.mkdir(parents=True, exist_ok=True)
    for name in ("msime-pinyin", "msime-english"):
        target = resources / f"{name}.db"
        target.unlink(missing_ok=True)
        connection = sqlite3.connect(target)
        try:
            connection.executescript((HERE / f"{name}.sql").read_text(encoding="utf-8"))
            connection.commit()
        finally:
            connection.close()


if __name__ == "__main__":
    main()
