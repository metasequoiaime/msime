#!/usr/bin/env python3
"""把 msime-dictionary 的符号名称表同步成仓库里的副本。

符号混输的名称表（sources/symbols/symbol-names.tsv）和 LaTeX 命令名表（sources/symbols/latex-symbols.tsv）
是本项目自写的数据，唯一的编辑入口在 msime-dictionary。resources/symbol-tables.lock.json 固定词库仓提交
和每个文件的大小与 SHA-256，并写明副本的位置：引擎用 include_str! 编进 crates/engine/src/local/symbol_candidates.tsv，
官方短语表的生成器读 plugins/phrase_table/latex-symbols/symbols.tsv。副本提交进仓库，编译不联网，Linux 发行版的
离线打包不需要额外的输入。

不带参数时按锁下载（缓存在 target/symbol-tables-cache/<commit>/，下载先限长再校验）并逐字节写入副本。
--check 不联网，只核对每个副本的大小和 SHA-256 等于锁；设了 MSIME_FETCH=1 时再下载一次，确认锁定的提交
确实提供这些字节。

usage: sync_symbol_tables.py [--check]
"""
import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.dont_write_bytecode = True  # 导入 scripts/ 下的脚本时不留下 __pycache__
from fetch_wordbooks import fetch  # noqa: E402  （按锁下载并校验长度和 sha256）

LOCK = ROOT / "resources/symbol-tables.lock.json"
REPOSITORY = "https://github.com/metasequoiaime/msime-dictionary"


def read_lock() -> dict:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    if (lock.get("version") != 1 or lock.get("repository") != REPOSITORY
            or not re.fullmatch(r"[0-9a-f]{40}", lock.get("commit", ""))):
        raise ValueError("符号表锁必须固定 msime-dictionary 的完整 Git 提交")
    copies = set()
    for entry in lock["files"]:
        if (not re.fullmatch(r"sources/symbols/[a-z][a-z0-9-]*\.tsv", entry["path"])
                or entry["copy"] in copies
                or type(entry["size"]) is not int or not 0 < entry["size"] <= 1_000_000
                or not re.fullmatch(r"[0-9a-f]{64}", entry["sha256"])):
            raise ValueError(f"无效的符号表记录：{entry['path']}")
        copies.add(entry["copy"])
    return lock


def matches(path: Path, entry: dict) -> bool:
    return (path.is_file() and path.stat().st_size == entry["size"]
            and hashlib.sha256(path.read_bytes()).hexdigest() == entry["sha256"])


def download(lock: dict, entry: dict) -> Path:
    artifact = {
        "name": Path(entry["path"]).name,
        "url": f"https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/{lock['commit']}/{entry['path']}",
        "size": entry["size"],
        "sha256": entry["sha256"],
    }
    return fetch(artifact, ROOT / "target/symbol-tables-cache" / lock["commit"])


def check(lock: dict) -> list[str]:
    """副本与锁不一致的说明，一致时为空。"""
    return [
        f"{entry['copy']} 与锁不一致：改符号表要改 msime-dictionary 的 {entry['path']}，"
        "再更新 resources/symbol-tables.lock.json 并运行 scripts/sync_symbol_tables.py"
        for entry in lock["files"]
        if not matches(ROOT / entry["copy"], entry)
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    lock = read_lock()
    if arguments.check:
        problems = check(lock)
        for problem in problems:
            print(problem, file=sys.stderr)
        if problems:
            return 1
        if os.environ.get("MSIME_FETCH") == "1":
            for entry in lock["files"]:
                download(lock, entry)
        return 0
    for entry in lock["files"]:
        (ROOT / entry["copy"]).write_bytes(download(lock, entry).read_bytes())
    return 0


if __name__ == "__main__":
    sys.exit(main())
