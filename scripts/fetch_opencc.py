#!/usr/bin/env python3
"""准备固定的 OpenCC 编译输入，成功时只向 stdout 输出绝对目录。

数据来自 msime-dictionary 的固定 Git 提交。缓存默认在 target/opencc/<commit>；
MSIME_OPENCC_SOURCE_DIR 可指向离线检出的 sources/opencc/，同样逐文件校验。
MSIME_OPENCC_OFFLINE=1 或 --offline 禁止下载，缺少或损坏的输入直接失败。
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile

import download_retry

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/opencc.lock.json"


def read_lock() -> dict:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    if (lock["version"] != 1
            or lock["repository"] != "https://github.com/metasequoiaime/msime-dictionary"
            or not re.fullmatch(r"[0-9a-f]{40}", lock["commit"])):
        raise ValueError("OpenCC 锁必须固定 msime-dictionary 的完整 Git 提交")
    names = set()
    for entry in lock["files"]:
        path = entry["path"]
        if (not re.fullmatch(r"sources/opencc/[A-Za-z][A-Za-z0-9_.]*", path) or path in names
                or type(entry["size"]) is not int or not 0 < entry["size"] <= 2_000_000
                or not re.fullmatch(r"[0-9a-f]{64}", entry["sha256"])):
            raise ValueError(f"无效的 OpenCC 输入记录：{path}")
        names.add(path)
    return lock


def matches(path: Path, entry: dict) -> bool:
    return (path.is_file() and path.stat().st_size == entry["size"]
            and hashlib.sha256(path.read_bytes()).hexdigest() == entry["sha256"])


def prepare(lock: dict, directory: Path, *, offline: bool, source: bool = False) -> Path:
    """复用已校验缓存；下载先限长再校验，最后原子替换，失败不污染已有文件。"""
    if source:
        if not directory.is_dir():
            raise FileNotFoundError(f"OpenCC 离线源目录不存在：{directory}")
    else:
        directory.mkdir(parents=True, exist_ok=True)
    for entry in lock["files"]:
        target = directory / Path(entry["path"]).name
        if matches(target, entry):
            continue
        if offline or source:
            error = ValueError if target.exists() else FileNotFoundError
            raise error(f"OpenCC 输入缺少或校验失败：{target}；请联网运行 scripts/fetch_opencc.py 准备缓存")
        url = f"https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/{lock['commit']}/{entry['path']}"
        with tempfile.NamedTemporaryFile(dir=directory, delete=False) as handle:
            temporary = Path(handle.name)
        try:
            with download_retry.urlopen(url, timeout=60) as response, temporary.open("wb") as output:
                size = 0
                while chunk := response.read(min(64 * 1024, entry["size"] - size + 1)):
                    size += len(chunk)
                    if size > entry["size"]:
                        raise ValueError(f"OpenCC 下载超过锁定长度：{entry['path']}")
                    output.write(chunk)
            if not matches(temporary, entry):
                raise ValueError(f"OpenCC 下载长度或 SHA-256 不符：{entry['path']}")
            temporary.replace(target)
        finally:
            temporary.unlink(missing_ok=True)
    return directory.resolve()


def ensure_opencc(*, offline: bool = False) -> Path:
    lock = read_lock()
    source = os.environ.get("MSIME_OPENCC_SOURCE_DIR")
    directory = Path(source) if source else ROOT / "target/opencc" / lock["commit"]
    offline = (offline or os.environ.get("MSIME_OPENCC_OFFLINE") == "1"
               or os.environ.get("CARGO_NET_OFFLINE", "").lower() in ("1", "true"))
    directory = prepare(lock, directory, offline=offline, source=bool(source))
    if (directory / "LICENSE").read_bytes() != (ROOT / "resources/licenses/OpenCC-Apache-2.0.txt").read_bytes():
        raise ValueError("随应用分发的 OpenCC 许可证与锁定输入不一致")
    return directory


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true")
    arguments = parser.parse_args()
    try:
        print(ensure_opencc(offline=arguments.offline))
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
