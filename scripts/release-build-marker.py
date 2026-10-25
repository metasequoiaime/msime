#!/usr/bin/env python3
"""打印追加在 Windows、Linux、macOS 发布正文末尾的隐藏构建标记。

应用内检查更新（crates/client-services/src/update_check.rs）按（版本号，发布 workflow 的运行 ID）比较新旧：同一版本号会有好几次构建（develop 的 beta、之后 main 的正式版、重跑同一版本），包内版本号始终是 X.Y.Z，只有运行 ID 分得开。正式版和测试版的运行 ID 就记在这行标记里，程序自己的运行 ID 是构建时的 MSIME_BUILD_RUN，两者都取 github.run_id。

用法：release-build-marker.py <发布名> <源码提交> <运行 ID>
发布名是 release-*.yml 的 release 输出：main 上是 X.Y.Z，develop 上是 X.Y.Z-beta，其他分支是 X.Y.Z-alpha。
"""

import json
import re
import sys


RELEASE = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+(-beta|-alpha)?")
SOURCE = re.compile(r"[0-9a-f]{40}")
RUN = re.compile(r"[1-9][0-9]{0,18}")


def marker(release, source, run):
    match = RELEASE.fullmatch(release)
    if not match:
        raise ValueError(f"发布名不是 X.Y.Z、X.Y.Z-beta 或 X.Y.Z-alpha：{release!r}")
    if not SOURCE.fullmatch(source):
        raise ValueError(f"源码提交不是 40 位小写十六进制：{source!r}")
    if not RUN.fullmatch(run):
        raise ValueError(f"运行 ID 不是正整数：{run!r}")
    channel = {None: "stable", "-beta": "beta", "-alpha": "alpha"}[match.group(1)]
    payload = json.dumps({"run": int(run), "source": source, "channel": channel}, separators=(",", ":"))
    return f"<!-- msime-build {payload} -->"


def main():
    if len(sys.argv) != 4:
        print("usage: release-build-marker.py <release> <source-sha> <run-id>", file=sys.stderr)
        raise SystemExit(2)
    try:
        line = marker(*sys.argv[1:])
    except ValueError as error:
        print(f"release-build-marker.py: {error}", file=sys.stderr)
        raise SystemExit(1)
    # 前面空一行，免得标记贴在上一段的 Markdown 后面。
    print()
    print(line)


if __name__ == "__main__":
    main()
