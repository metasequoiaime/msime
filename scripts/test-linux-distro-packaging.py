#!/usr/bin/env python3
"""核对发行版源码包（platforms/linux/packaging/rpm/msime.spec 与 platforms/linux/packaging/debian/rules）没有和发布页安装包的构建脱节。

发布页的 .deb/.rpm 由 platforms/linux/package-container.sh 构建；RPM 规格与 debian/rules 照搬它的步骤，但不会跟着它自动变。这里比对三处：

- 传给 CMake 的 -DMSIME_* 选项集合相同。package-container.sh 新增一个选项（例如再随包带一类数据）而两个源码包没跟上时，它们会悄悄打出缺东西的包。
- 用 cargo build 构建的包与目标（-p 与 --bin）相同。
- render-sources.py 渲染出的版本、发布号和更新日志能被正确改写。

只用标准库，不需要构建环境。
"""
from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTAINER = ROOT / "platforms/linux/package-container.sh"
SPEC = ROOT / "platforms/linux/packaging/rpm/msime.spec"
RULES = ROOT / "platforms/linux/packaging/debian/rules"
RENDER = ROOT / "platforms/linux/packaging/rpm/render-sources.py"


def cmake_options(text: str) -> set[str]:
    return set(re.findall(r"-D(MSIME_[A-Z0-9_]+)", text))


def cargo_targets(text: str) -> set[str]:
    targets = set()
    for line in text.splitlines():
        if "cargo build" not in line:
            continue
        package = re.search(r"-p\s+(\S+)", line)
        binary = re.search(r"--bin\s+(\S+)", line)
        if package:
            targets.add(f"{package.group(1)}:{binary.group(1) if binary else ''}")
    return targets


def main() -> int:
    failures = []
    reference = CONTAINER.read_text(encoding="utf-8")
    expected_options = cmake_options(reference)
    expected_targets = cargo_targets(reference)
    for path in (SPEC, RULES):
        text = path.read_text(encoding="utf-8")
        name = path.relative_to(ROOT)
        options = cmake_options(text)
        if options != expected_options:
            failures.append(f"{name}: CMake options differ from package-container.sh; missing {sorted(expected_options - options)}, extra {sorted(options - expected_options)}")
        targets = cargo_targets(text)
        if targets != expected_targets:
            failures.append(f"{name}: cargo build targets differ from package-container.sh; missing {sorted(expected_targets - targets)}, extra {sorted(targets - expected_targets)}")

    with tempfile.TemporaryDirectory() as scratch:
        spec_out = Path(scratch) / "msime.spec"
        changelog_out = Path(scratch) / "changelog"
        subprocess.run([sys.executable, str(RENDER), "--version", "12.34.56", "--rpm-release", "2",
                        "--debian-revision", "1~ppa1", "--debian-distribution", "resolute", "--date", "2026-01-02",
                        "--spec-out", str(spec_out), "--changelog-out", str(changelog_out)], check=True)
        spec = spec_out.read_text(encoding="utf-8")
        for needle in ("\nVersion:        12.34.56\n", "\nRelease:        2%{?dist}\n",
                       "\n%changelog\n* Fri Jan 02 2026 Metasequoia IME <metasequoiaime@gmail.com> - 12.34.56-2\n"):
            if needle not in spec:
                failures.append(f"render-sources.py: rendered spec lacks {needle.strip()!r}")
        changelog = changelog_out.read_text(encoding="utf-8")
        if not changelog.startswith("msime (12.34.56-1~ppa1) resolute; urgency=medium\n"):
            failures.append(f"render-sources.py: unexpected changelog header {changelog.splitlines()[0]!r}")
        if "\n -- Metasequoia IME <metasequoiaime@gmail.com>  Fri, 02 Jan 2026 00:00:00 +0000\n" not in changelog:
            failures.append("render-sources.py: changelog trailer is not in Debian format")

    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    if not failures:
        print(f"ok: {len(expected_options)} CMake options and {len(expected_targets)} cargo targets match package-container.sh")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
