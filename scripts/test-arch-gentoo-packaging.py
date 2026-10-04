#!/usr/bin/env python3
"""Arch（AUR）与 Gentoo 的打包定义与仓库其余部分保持一致。

这些定义不在拉取请求门禁里构建（构建要 archlinux / gentoo 容器，见各自的 check-in-container.sh），所以这里只核对离线就能看出的漂移：
- 两个 PKGBUILD 与两份 ebuild 传给 CMake 的 MSIME_* 选项在 platforms/linux/CMakeLists.txt 里都有定义，调用的 fetch 脚本都存在；
- 安装脚本与 ebuild 里停用、重启的用户单元与 CMakeLists.txt 的 MSIME_USER_UNITS 一致，msime 与 msime-bin 的安装脚本逐字相同；
- live ebuild 的 RUST_MIN_VER 等于 rust-toolchain.toml 钉住的版本，版本 ebuild 模板能完整渲染；
- 提交了 .SRCINFO 时，它的版本与校验值和 PKGBUILD 一致。
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PACKAGING = ROOT / "platforms" / "linux" / "packaging"
ARCH = PACKAGING / "arch"
GENTOO = PACKAGING / "gentoo"
EBUILD_DIR = GENTOO / "app-i18n" / "msime"
CMAKE = ROOT / "platforms" / "linux" / "CMakeLists.txt"
EDITIONS = ROOT / "shared" / "contracts" / "editions.json"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def cmake_units() -> tuple[list[str], list[str]]:
    text = CMAKE.read_text(encoding="utf-8")
    match = re.search(r"set\(MSIME_USER_UNITS\s+([^)]*)\)", text)
    if not match:
        sys.exit(f"{CMAKE}: no MSIME_USER_UNITS")
    # 单元名以版本的包名开头（cmake/Edition.cmake 的 MSIME_EDITION_PACKAGE）；这些包只打 full，full 的包名取自版本表。
    package = full_linux_package()
    units = [unit.replace("${MSIME_EDITION_PACKAGE}", package) for unit in match.group(1).split()]
    return units, [unit for unit in units if unit.endswith(".service")]


def full_linux_package() -> str:
    table = json.loads(EDITIONS.read_text(encoding="utf-8"))
    for entry in table["editions"]:
        if entry["id"] == "full":
            return entry["platforms"]["linux"]["package"]
    sys.exit(f"{EDITIONS}: no full edition")


def cmake_cache_options() -> set[str]:
    text = CMAKE.read_text(encoding="utf-8")
    linux = ROOT / "platforms" / "linux"
    for extra in [*linux.glob("**/CMakeLists.txt"), *linux.glob("cmake/*.cmake")]:
        text += extra.read_text(encoding="utf-8")
    names = set(re.findall(r"(?:set|option)\((MSIME_[A-Z0-9_]+)\b", text))
    return names | {"BUILD_TESTING", "CMAKE_BUILD_TYPE", "CMAKE_INSTALL_PREFIX"}


def quoted_assignment(text: str, name: str) -> str | None:
    match = re.search(rf"^{name}=['\"]([^'\"]*)['\"]", text, re.MULTILINE)
    return match.group(1) if match else None


def main() -> None:
    units, services = cmake_units()
    options = cmake_cache_options()
    for path in [ARCH / "msime" / "PKGBUILD", EBUILD_DIR / "msime-9999.ebuild", EBUILD_DIR / "msime.ebuild.in"]:
        text = path.read_text(encoding="utf-8")
        for option in sorted(set(re.findall(r"-D([A-Z][A-Z0-9_]+)=", text))):
            check(option in options, f"{path.relative_to(ROOT)} passes -D{option}, which platforms/linux/CMakeLists.txt does not define")
        for script in sorted(set(re.findall(r"scripts/(fetch_[a-z_]+\.py)", text))):
            check((ROOT / "scripts" / script).is_file(), f"{path.relative_to(ROOT)} runs scripts/{script}, which does not exist")
        check("collect-notices.py" in text, f"{path.relative_to(ROOT)} does not collect the third-party notices packaging requires")

    # 用户单元：安装脚本与 ebuild 各自写了一份，与 CMake 的列表对照。
    install_scripts = [ARCH / "msime" / "msime.install", ARCH / "msime-bin" / "msime.install"]
    check(install_scripts[0].read_bytes() == install_scripts[1].read_bytes(), "arch/msime/msime.install and arch/msime-bin/msime.install differ")
    for path in [install_scripts[0], EBUILD_DIR / "msime-9999.ebuild", EBUILD_DIR / "msime.ebuild.in"]:
        text = path.read_text(encoding="utf-8")
        check(quoted_assignment(text, "_msime_units") == " ".join(units), f"{path.relative_to(ROOT)}: _msime_units is not MSIME_USER_UNITS ({' '.join(units)})")
        check(quoted_assignment(text, "_msime_services") == " ".join(services), f"{path.relative_to(ROOT)}: _msime_services is not the services of MSIME_USER_UNITS ({' '.join(services)})")

    # Rust：live ebuild 与渲染时取的都是 rust-toolchain.toml。
    channel = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
    live = (EBUILD_DIR / "msime-9999.ebuild").read_text(encoding="utf-8")
    check(quoted_assignment(live, "RUST_MIN_VER") == channel, f"msime-9999.ebuild RUST_MIN_VER is not rust-toolchain.toml's {channel}")

    # 模板渲染：不跑 pycargoebuild，只看占位符都被填上、版本 ebuild 能被 bash 解析。
    with tempfile.TemporaryDirectory() as scratch:
        result = subprocess.run(
            [sys.executable, str(GENTOO / "render.py"), "1.2.3", "--out", scratch, "--no-crates"],
            capture_output=True, text=True,
        )
        check(result.returncode == 0, f"render.py failed: {result.stderr.strip()}")
        rendered = Path(scratch) / "app-i18n" / "msime" / "msime-1.2.3.ebuild"
        if rendered.is_file():
            text = rendered.read_text(encoding="utf-8")
            check(f'RUST_MIN_VER="{channel}"' in text, "rendered ebuild does not carry the pinned Rust version")
            check("voice-runtime/.archive" in text, "rendered ebuild lists no voice runtime")
            syntax = subprocess.run(["bash", "-n", str(rendered)], capture_output=True, text=True)
            check(syntax.returncode == 0, f"rendered ebuild is not valid bash: {syntax.stderr.strip()}")
    for path in [EBUILD_DIR / "msime-9999.ebuild", ARCH / "msime" / "PKGBUILD", ARCH / "msime-bin" / "PKGBUILD", install_scripts[0]]:
        syntax = subprocess.run(["bash", "-n", str(path)], capture_output=True, text=True)
        check(syntax.returncode == 0, f"{path.relative_to(ROOT)} is not valid bash: {syntax.stderr.strip()}")

    # .SRCINFO 是 AUR 读的元数据，必须跟着 PKGBUILD 走。
    for name in ("msime", "msime-bin"):
        srcinfo = ARCH / name / ".SRCINFO"
        if not srcinfo.is_file():
            continue
        pkgbuild = (ARCH / name / "PKGBUILD").read_text(encoding="utf-8")
        info = srcinfo.read_text(encoding="utf-8")
        for key in ("pkgver", "pkgrel"):
            value = re.search(rf"^{key}=(\S+)", pkgbuild, re.MULTILINE).group(1)
            check(f"\t{key} = {value}\n" in info, f"arch/{name}/.SRCINFO {key} does not match PKGBUILD ({value}); rerun render.py")
        for digest in re.findall(r"'([0-9a-f]{64})'", pkgbuild):
            check(digest in info, f"arch/{name}/.SRCINFO does not carry PKGBUILD checksum {digest}; rerun render.py")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        sys.exit(1)
    print("arch and gentoo packaging definitions are consistent")


if __name__ == "__main__":
    main()
