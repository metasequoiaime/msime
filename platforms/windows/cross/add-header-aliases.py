#!/usr/bin/env python3
"""一次扫描创建 MinGW 头文件的首字母大写别名，不启动逐文件子进程。"""

import os
from pathlib import Path
import sys


def add_header_aliases(directory: Path) -> int:
    if not directory.is_dir():
        return 0
    created = 0
    with os.scandir(directory) as entries:
        for entry in entries:
            name = entry.name
            if not ("a" <= name[0] <= "z" and name.endswith(".h")):
                continue
            if not os.path.exists(entry.path):
                continue
            alias = directory / (name[0].upper() + name[1:])
            if not os.path.lexists(alias):
                os.symlink(name, alias)
                created += 1
    return created


def main(arguments: list[str]) -> int:
    if not arguments:
        print("usage: add-header-aliases.py <directory> [directory ...]", file=sys.stderr)
        return 2
    created = sum(add_header_aliases(Path(directory)) for directory in arguments)
    print(f"MinGW 头文件别名：新增 {created} 项")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
