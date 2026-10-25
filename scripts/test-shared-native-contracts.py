#!/usr/bin/env python3
"""在宿主上编译并运行共享 C++ 契约，不依赖 Windows SDK。"""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


def main() -> int:
    compiler = next(
        (driver for driver in (os.environ.get("CXX"), "c++", "clang++", "g++")
         if driver and shutil.which(driver)),
        None,
    )
    if compiler is None:
        print("skipped: no host C++ compiler for shared native contracts")
        return 0
    sources = sorted((ROOT / "shared/contracts/tests").glob("*_contract.cpp"))
    if not sources:
        print("skipped: shared native contract sources are not present")
        return 0
    environment = os.environ.copy()
    # 契约在当前主机运行，不继承 macOS 产品的最低系统版本。
    environment.pop("MACOSX_DEPLOYMENT_TARGET", None)
    with tempfile.TemporaryDirectory(prefix="msime-shared-contracts-") as directory:
        for source in sources:
            binary = pathlib.Path(directory) / (source.stem + (".exe" if os.name == "nt" else ""))
            commands = [
                [compiler, "-std=c++17", "-DMSIME_EDITION_FULL=1", "-UNDEBUG",
                 "-Wall", "-Wextra", "-Werror", str(source), "-o", str(binary)],
                [str(binary)],
            ]
            for command in commands:
                try:
                    result = subprocess.run(
                        command, cwd=ROOT, env=environment,
                        stdin=subprocess.DEVNULL, timeout=60, check=False,
                    )
                except subprocess.TimeoutExpired:
                    print(f"FAIL {source.name}: timed out", flush=True)
                    return 1
                if result.returncode:
                    print(f"FAIL {source.name}: exit {result.returncode}", flush=True)
                    return 1
    print(f"shared native contracts: {len(sources)} passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
