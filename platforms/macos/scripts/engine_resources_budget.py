#!/usr/bin/env python3
"""核对 macOS 发布包里 EngineResources 的体积没有超出本版本锁定的核心词库预算。

预算是本版本资源锁（full 用 resources/desktop-dictionary.lock.json，其他版本用 resources/editions/<id>.lock.json）里实际随包的条目，加上 resources/helpcodes 的辅助码表和许可声明，每个文件按 4 KiB 向上取整，再留 4 MiB 给目录和其他小文件。被量的一侧用同一个规则：每个文件的长度（st_size）按 4 KiB 向上取整再相加，而不是 du 报的已分配块数。

为什么不用 du：du 量的是文件系统给文件分配了多少块，不是包里装了什么。macOS 0.57.0 正式版的发布工作流（run 41438608904）里，五笔版的应用在打包前用 du 量是在预算内的，同一个应用打进 pkg、再用 pkgutil --expand-full 展开后，du 却多量出 18852 KiB，而 verify_resources 刚确认过目录里恰好是锁定的那些文件、长度和 SHA-256 都对，前一天同样内容的 0.56.2-beta 五笔包展开后也正好是 194804 KiB。分配块数会随写入方式和时机变，拿它跟按长度算的预算比，会把同一份内容时而放行、时而拒绝。按长度量，多出来的文件或变大的文件照样超限，结果只取决于包里装了什么。
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
# 与 stage-resources.sh 暂存进 EngineResources/helpcodes 的来源文件一一对应（ENGINE-NOTICE.md 暂存为 NOTICE.md，NOTICE.md 暂存为 NOTICE-jiajia.md）。
HELPCODE_SOURCES = (
    "helpcode.txt", "zrm_helpcode_big_unique.txt", "shouyou2_0_helpcode.txt", "shouyouplus_helpcode.txt",
    "xiaohe_helpcode.txt", "jiajia_helpcode.txt", "wubi86_helpcode.txt", "ENGINE-NOTICE.md", "NOTICE.md", "NOTICE-wubi86.md",
)
MARGIN_KIB = 4096


def rounded_kib(size: int) -> int:
    """按 4 KiB 文件分配粒度向上取整，单位 KiB。"""
    return (size + 4095) // 4096 * 4


def lock_path(root: Path, edition: str) -> Path:
    return root / ("resources/desktop-dictionary.lock.json" if edition == "full" else f"resources/editions/{edition}.lock.json")


def budget_kib(root: Path, edition: str, staged: Path) -> int:
    """本版本的预算：锁里在 staged 中实际存在的条目（按需省略的不占预算）、辅助码表与 4 MiB 余量。"""
    artifacts = json.loads(lock_path(root, edition).read_text(encoding="utf-8"))["artifacts"]
    locked = sum(rounded_kib(artifact["size"]) for artifact in artifacts if (staged / artifact["name"]).is_file())
    helpcodes = root / "resources/helpcodes"
    helpcode = sum(rounded_kib(path.stat().st_size) for name in HELPCODE_SOURCES if (path := helpcodes / name).is_file())
    return locked + helpcode + MARGIN_KIB


def entries(staged: Path) -> list[tuple[str, os.stat_result]]:
    """staged 下除目录外的每一项（不跟随符号链接），按相对路径排序。"""
    found = []
    for directory, _, names in os.walk(staged):
        for name in names:
            path = Path(directory, name)
            found.append((str(path.relative_to(staged)), path.lstat()))
    return sorted(found)


def content_kib(staged: Path) -> int:
    """staged 里文件长度按 4 KiB 向上取整的总和，与 budget_kib 同一个规则。"""
    return sum(rounded_kib(status.st_size) for _, status in entries(staged))


def check(root: Path, edition: str, staged: Path) -> int:
    content, budget = content_kib(staged), budget_kib(root, edition, staged)
    print(f"EngineResources ({edition}): {content} KiB of the {budget} KiB locked core-dictionary budget")
    if content <= budget:
        return 0
    print(f"EngineResources is {content} KiB, over the {budget} KiB locked core-dictionary budget: {staged}", file=sys.stderr)
    # 超限时列出每个文件的长度和已分配块数，不用再去 CI 机器上找是哪个文件、是内容还是分配。
    for name, status in entries(staged):
        print(f"  {name}: {status.st_size} bytes, {rounded_kib(status.st_size)} KiB counted, {status.st_blocks // 2} KiB allocated", file=sys.stderr)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--edition", required=True, help="shared/contracts/editions.json 里的版本 id")
    parser.add_argument("engine_resources", type=Path, help="暂存好的或包里的 EngineResources 目录")
    arguments = parser.parse_args()
    if not arguments.engine_resources.is_dir():
        parser.error(f"{arguments.engine_resources} is not a directory")
    return check(REPO_ROOT, arguments.edition, arguments.engine_resources)


if __name__ == "__main__":
    sys.exit(main())
