#!/usr/bin/env python3
"""核对各发行版的源码构建定义（platforms/linux/packaging 下的 RPM 规格、debian/rules、AUR 的 msime PKGBUILD 与 Gentoo 的两份 ebuild）没有和发布页安装包的构建脱节。

发布页的 .deb/.rpm 由 platforms/linux/package-container.sh 构建；这些定义照搬它的步骤，但不会跟着它自动变。这里比对三处：

- 传给 CMake 的 -DMSIME_* 选项集合相同（五份定义都比）。package-container.sh 新增一个选项（例如再随包带一类数据）而某个定义没跟上时，它会悄悄打出缺东西的包。
  唯一有意的差别是手写模型：发布页的包以 -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF 不带模型（设置应用按需下载），发行版仓库的包照旧以 -DMSIME_HANDWRITING_MODEL_DIR 随包，并取默认的 ON。这一对差别单独核对。
- 用 cargo build 构建的包与目标（-p 与 --bin）相同（RPM 规格、debian/rules 与 PKGBUILD；ebuild 经 cargo.eclass 的 cargo_src_compile 构建，不在此列）。
- render-sources.py 渲染出的版本、发布号和更新日志能被正确改写。
- 替换另一个包的定义在替换之后恢复用户单元和输入法列表：RPM 与 Debian 的 msime 替换发布页的 msime-linux、AUR 的 msime 与 msime-bin 互换时，被替换的包按卸载处理，它的卸载脚本停用每个用户的单元并运行 msime-linux-setup --unregister，新包必须再替每个用户运行 msime-linux-setup --register。

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
RENDER = ROOT / "platforms/linux/packaging/render-sources.py"
PKGBUILD = ROOT / "platforms/linux/packaging/arch/msime/PKGBUILD"
DEBIAN_REGISTER = ROOT / "platforms/linux/packaging/debian/postinst-register"
DEBIAN_PREINST = ROOT / "platforms/linux/packaging/debian/msime.preinst"
DEBIAN_POSTRM = ROOT / "platforms/linux/packaging/debian/msime.postrm"
CONTROL = ROOT / "platforms/linux/packaging/debian/control"
ARCH_INSTALL = ROOT / "platforms/linux/packaging/arch/msime/msime.install"
SETUP = ROOT / "platforms/linux/scripts/msime-linux-setup"
# 只比 CMake 选项的定义，以及各自允许不传的选项：live ebuild 跟踪 develop，没有发布版本号，不传 MSIME_PACKAGE_VERSION 时 CMake 取 platforms/linux/version.txt。
OPTIONS_ONLY = [
    (ROOT / "platforms/linux/packaging/gentoo/app-i18n/msime/msime-9999.ebuild", {"MSIME_PACKAGE_VERSION"}),
    (ROOT / "platforms/linux/packaging/gentoo/app-i18n/msime/msime.ebuild.in", set()),
]


# 发布页的包不带手写模型，发行版的包随包：前者只传 MSIME_BUNDLE_HANDWRITING_MODEL=OFF，后者只传 MSIME_HANDWRITING_MODEL_DIR（MSIME_BUNDLE_HANDWRITING_MODEL 取默认的 ON）。
CONTAINER_ONLY = {"MSIME_BUNDLE_HANDWRITING_MODEL"}
DISTRIBUTION_ONLY = {"MSIME_HANDWRITING_MODEL_DIR"}


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
    container_options = cmake_options(reference)
    if "-DMSIME_BUNDLE_HANDWRITING_MODEL=OFF" not in reference or "MSIME_HANDWRITING_MODEL_DIR" in reference or "fetch_handwriting_model.py" in reference:
        failures.append("package-container.sh must configure with -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF and neither fetch nor pass the handwriting model")
    expected_options = (container_options - CONTAINER_ONLY) | DISTRIBUTION_ONLY
    expected_targets = cargo_targets(reference)
    for path in (SPEC, RULES, PKGBUILD):
        text = path.read_text(encoding="utf-8")
        name = path.relative_to(ROOT)
        options = cmake_options(text)
        if options != expected_options:
            failures.append(f"{name}: CMake options differ from package-container.sh; missing {sorted(expected_options - options)}, extra {sorted(options - expected_options)}")
        if "scripts/fetch_handwriting_model.py" not in text:
            failures.append(f"{name}: no longer fetches the handwriting model it bundles with -DMSIME_HANDWRITING_MODEL_DIR")
        targets = cargo_targets(text)
        if targets != expected_targets:
            failures.append(f"{name}: cargo build targets differ from package-container.sh; missing {sorted(expected_targets - targets)}, extra {sorted(targets - expected_targets)}")
    for path, optional in OPTIONS_ONLY:
        options = cmake_options(path.read_text(encoding="utf-8"))
        if options != expected_options - (optional - options):
            failures.append(f"{path.relative_to(ROOT)}: CMake options differ from package-container.sh; missing {sorted(expected_options - options)}, extra {sorted(options - expected_options)}")

    # 编译器下限：RPM 与 Debian 用发行版的 rust，下限要一致；Cargo.toml 的 rust-version 只是下界，锁定依赖（tauri 2.12 等）要求得更高，所以这里不拿它比。
    spec_floor = set(re.findall(r"(?m)^BuildRequires:\s+(?:cargo|rust) >= (\S+)$", SPEC.read_text(encoding="utf-8")))
    control_floor = set(re.findall(r"(?m)^ (?:cargo|rustc) \(>= (\S+)\)", CONTROL.read_text(encoding="utf-8")))
    if len(spec_floor) != 1 or spec_floor != control_floor:
        failures.append(f"rpm/msime.spec ({sorted(spec_floor)}) and debian/control ({sorted(control_floor)}) disagree on the Rust compiler floor")

    # 替换之后恢复：先确认这几份定义确实替换别的包，再确认它们在替换之后运行 --register，且 msime-linux-setup 有这个选项。
    register = 'msime-linux-setup --register </dev/null'
    if '"--register"' not in SETUP.read_text(encoding="utf-8"):
        failures.append("platforms/linux/scripts/msime-linux-setup has no --register for the packages to run after a replacement")
    spec_text = SPEC.read_text(encoding="utf-8")
    if "\nObsoletes:      msime-linux " in spec_text:
        trigger = spec_text.split("\n%triggerpostun -- msime-linux\n", 1)
        if len(trigger) != 2 or register not in trigger[1].split("\n%files\n", 1)[0] or 'if [ "$2" = 0 ]' not in trigger[1]:
            failures.append("rpm/msime.spec obsoletes msime-linux but has no %triggerpostun -- msime-linux that runs msime-linux-setup --register once it is gone")
    if "\nReplaces: msime-linux," in CONTROL.read_text(encoding="utf-8"):
        rules = RULES.read_text(encoding="utf-8")
        fragment = DEBIAN_REGISTER.read_text(encoding="utf-8")
        marker = "/var/lib/msime/register-users"
        preinst = DEBIAN_PREINST.read_text(encoding="utf-8") if DEBIAN_PREINST.is_file() else ""
        if "cat debian/postinst-register" not in rules or register not in fragment or f'[ -e {marker} ]' not in fragment:
            failures.append("debian/ replaces msime-linux but its postinst does not run msime-linux-setup --register after an install")
        # 从 config-files 状态重装时 postinst 的 $2 不为空，和升级一样；只有 preinst 的 `install` 分得出来，所以靠它留下的标记。
        if '[ "$1" = install ]' not in preinst or marker not in preinst or '[ -z "$2" ]' in fragment:
            failures.append(f"debian/msime.preinst must leave {marker} on every install (including reinstalls from config-files) and postinst-register must key on it, not on an empty $2")
        if not DEBIAN_POSTRM.is_file() or marker not in DEBIAN_POSTRM.read_text(encoding="utf-8"):
            failures.append(f"debian/msime.postrm does not remove {marker} on purge or abort-install")
    if "\nconflicts=('msime-bin')\n" in PKGBUILD.read_text(encoding="utf-8"):
        install = ARCH_INSTALL.read_text(encoding="utf-8")
        post_install = install.split("\npost_install() {\n", 1)[-1].split("\n}\n", 1)[0]
        if register not in install or "_msime_register_users" not in post_install:
            failures.append("arch/msime/msime.install: msime and msime-bin replace each other but post_install does not run msime-linux-setup --register")

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
