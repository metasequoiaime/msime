#!/usr/bin/env python3
"""压缩 HAP 中的资源条目，保留代码和原生库的可直接加载布局。"""

from __future__ import annotations

import argparse
import copy
import tempfile
import zipfile
from pathlib import Path


RESOURCE_PREFIX = "resources/"
MIN_RESOURCE_SIZE = 1024


def compress_resources(source: Path, destination: Path) -> tuple[int, int]:
    """复制 HAP 并压缩原本未压缩的资源文件。

    原生库、Ark 字节码、资源索引和其他条目保留原压缩方式。鸿蒙的包加载器可以解压
    DEFLATE 资源，而原生加载器仍会拿到它所要求的未压缩共享库。
    """
    if source.resolve() == destination.resolve():
        raise ValueError("source and destination must be different files")
    destination.parent.mkdir(parents=True, exist_ok=True)
    resource_bytes = 0
    temporary = tempfile.NamedTemporaryFile(
        dir=destination.parent, prefix=f".{destination.name}.", suffix=".tmp", delete=False
    )
    temporary_path = Path(temporary.name)
    temporary.close()
    try:
        with zipfile.ZipFile(source, "r") as archive, zipfile.ZipFile(
            temporary_path, "w", allowZip64=True
        ) as output:
            output.comment = archive.comment
            for original in archive.infolist():
                entry = copy.copy(original)
                data = archive.read(original)
                should_compress = (
                    original.filename.startswith(RESOURCE_PREFIX)
                    and not original.is_dir()
                    and original.compress_type == zipfile.ZIP_STORED
                    and len(data) >= MIN_RESOURCE_SIZE
                )
                if should_compress:
                    entry.compress_type = zipfile.ZIP_DEFLATED
                    resource_bytes += len(data)
                output.writestr(
                    entry,
                    data,
                    compress_type=entry.compress_type,
                    compresslevel=9 if should_compress else None,
                )
        temporary_path.replace(destination)
    except BaseException:
        temporary_path.unlink(missing_ok=True)
        raise
    return resource_bytes, destination.stat().st_size


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="hvigor 生成的 HAP")
    parser.add_argument("destination", type=Path, help="压缩后的 HAP")
    arguments = parser.parse_args()
    resource_bytes, size = compress_resources(arguments.source, arguments.destination)
    print(f"已压缩 HAP 资源 {resource_bytes} 字节；输出 {size} 字节")


if __name__ == "__main__":
    main()
