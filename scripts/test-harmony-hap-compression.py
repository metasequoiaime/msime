#!/usr/bin/env python3
"""HAP 资源压缩脚本的契约测试。"""

from __future__ import annotations

import importlib.util
import tempfile
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("compress_harmony_hap", ROOT / "scripts/compress_harmony_hap.py")
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="msime-harmony-hap-") as directory:
        root = Path(directory)
        source = root / "input.hap"
        destination = root / "output.hap"
        payload = (b"resource-data-" * 1024) + b"\n"
        with zipfile.ZipFile(source, "w", compression=zipfile.ZIP_STORED) as archive:
            archive.comment = b"preserve this comment"
            archive.writestr("resources/resfile/engine/example.db", payload)
            archive.writestr("libs/arm64-v8a/libexample.so", payload)
            archive.writestr("resources.index", payload)
            archive.writestr("ets/modules.abc", payload)

        MODULE.compress_resources(source, destination)
        with zipfile.ZipFile(destination) as archive:
            assert archive.comment == b"preserve this comment"
            assert archive.read("resources/resfile/engine/example.db") == payload
            assert archive.read("libs/arm64-v8a/libexample.so") == payload
            assert archive.read("resources.index") == payload
            assert archive.read("ets/modules.abc") == payload
            assert archive.getinfo("resources/resfile/engine/example.db").compress_type == zipfile.ZIP_DEFLATED
            for name in ("libs/arm64-v8a/libexample.so", "resources.index", "ets/modules.abc"):
                assert archive.getinfo(name).compress_type == zipfile.ZIP_STORED
            assert archive.testzip() is None

    print("Harmony HAP compression contract passed")


if __name__ == "__main__":
    main()
