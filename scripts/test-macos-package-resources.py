#!/usr/bin/env python3
"""macOS 发布包只带核心词库，其余三个资源包（日文词典、粤拼/注音词库、手写模型）由 App 按需下载。

这里静态核对几处必须一致的地方：resources.rs 里不进包的日文文件名单、打包脚本不再取回或拷入资源包、tauri.macos.conf.json 不声明资源包文件、三个资源包锁文件只用不可变的地址（固定 tag 的 GitHub release 附件或固定提交的 raw 地址），以及 ci-macos-package.yml 的改动判断覆盖每个打包输入。任何一处只改一边，都可能出一个缺数据又下载不到的发布包，而这种包在开发机上照样能跑。

只用标准库；任何一个输入文件缺失时打印 skipped 并以 0 退出。
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESOURCES_RS = ROOT / "crates/client-core/src/resources.rs"
DESKTOP_LOCK = ROOT / "resources/desktop-dictionary.lock.json"
LANGUAGE_LOCK = ROOT / "resources/language-dictionaries.lock.json"
HANDWRITING_LOCK = ROOT / "resources/handwriting-model.lock.json"
PACKAGE = ROOT / "platforms/macos/package-release.sh"
TAURI_MACOS = ROOT / "apps/desktop/src-tauri/tauri.macos.conf.json"
PR_WORKFLOW = ROOT / ".github/workflows/ci-macos-package.yml"
RELEASE_WORKFLOW = ROOT / ".github/workflows/release-macos.yml"
INPUTS = (RESOURCES_RS, DESKTOP_LOCK, LANGUAGE_LOCK, HANDWRITING_LOCK, PACKAGE, TAURI_MACOS, PR_WORKFLOW, RELEASE_WORKFLOW)

EXPECTED_ON_DEMAND = ["msime-japanese.dat", "msime-mozc_dictionary_oss_README.txt", "msime-mozc_LICENSE.txt"]
# 决定发布包内容和资源包能否下载的文件。任何一个改动都要让 ci-macos-package.yml 打一次包。
PACKAGING_INPUTS = (
    "platforms/macos/package-release.sh",
    "platforms/macos/stage-resources.sh",
    "apps/desktop/src-tauri/tauri.macos.conf.json",
    "resources/desktop-dictionary.lock.json",
    "resources/language-dictionaries.lock.json",
    "resources/handwriting-model.lock.json",
    "resources/offline-glosses.lock.json",
    "crates/client-core/src/resources.rs",
    "crates/client-core/src/resource_packs.rs",
    "crates/client-core/src/voice/local_models.rs",
    "crates/client-core/examples/install_resources.rs",
    "crates/client-core/examples/verify_resources.rs",
    "crates/client-core/examples/install_resource_pack.rs",
    ".github/workflows/release-macos.yml",
    ".github/workflows/ci-macos-package.yml",
)
# 发布之后不再变的地址：固定 tag 的 release 附件（发布流程不删除、不替换附件），或固定到 40 位提交的 raw 地址。
IMMUTABLE_URL = re.compile(
    r"^https://github\.com/[\w.-]+/[\w.-]+/releases/download/[\w.-]+/[\w.-]+$"
    r"|^https://raw\.githubusercontent\.com/[\w.-]+/[\w.-]+/[0-9a-f]{40}/[\w./-]+$"
)
failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def live_lines(path: Path) -> list:
    return [line for line in path.read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]


def main() -> int:
    missing = [str(path.relative_to(ROOT)) for path in INPUTS if not path.is_file()]
    if missing:
        print(f"skipped: macOS package resources, missing {', '.join(missing)}")
        return 0

    # 不进包的日文文件名单，必须正好是锁文件里的那一组。
    match = re.search(r"pub const ON_DEMAND_JAPANESE_ARTIFACTS: \[&str; \d+\] =\s*\[([^\]]*)\]", RESOURCES_RS.read_text(encoding="utf-8"))
    on_demand = re.findall(r'"([^"]+)"', match.group(1)) if match else []
    check(on_demand == EXPECTED_ON_DEMAND, f"ON_DEMAND_JAPANESE_ARTIFACTS in resources.rs is {on_demand}, expected {EXPECTED_ON_DEMAND}")
    desktop = json.loads(DESKTOP_LOCK.read_text(encoding="utf-8"))
    desktop_names = {artifact["name"] for artifact in desktop["artifacts"]}
    for name in EXPECTED_ON_DEMAND:
        check(name in desktop_names, f"{name} is not in resources/desktop-dictionary.lock.json, so the japanese pack has nothing to download")

    # 打包脚本：不再取回或拷入资源包，并在编译前确认资源包可下载。
    package = live_lines(PACKAGE)
    package_text = "\n".join(package)
    for forbidden in ("fetch_handwriting_model.py", "fetch_language_dictionaries.py", "MSIME_REQUIRE_LANGUAGE_DICTIONARIES"):
        check(forbidden not in package_text, f"package-release.sh still uses {forbidden} on a live line")
    for line in package:
        if "language-dictionaries" in line and re.search(r"\b(ditto|cp|mv|rsync)\b", line):
            failures.append(f"package-release.sh copies language dictionaries into the package: {line.strip()}")
    check("install_resource_pack" in package_text, "package-release.sh does not check the on-demand packs with install_resource_pack")
    check("MSIME_MACOS_OMIT_ON_DEMAND=1" in package_text, "package-release.sh does not stage with MSIME_MACOS_OMIT_ON_DEMAND=1")
    # 包内检查要逐个确认按需文件不在 EngineResources 里，名单多了一个文件时这里跟着要求多一行。
    for name in EXPECTED_ON_DEMAND:
        check(f'test ! -e "$resources_dir/EngineResources/{name}"' in package_text, f"package-release.sh does not check that {name} stays out of EngineResources")

    # tauri.macos.conf.json 不能把资源包文件声明成包内资源。
    language = json.loads(LANGUAGE_LOCK.read_text(encoding="utf-8"))
    handwriting = json.loads(HANDWRITING_LOCK.read_text(encoding="utf-8"))
    pack_names = set(EXPECTED_ON_DEMAND) | {artifact["name"] for artifact in language["artifacts"]} | {artifact["name"] for artifact in handwriting["artifacts"]}
    resources = json.loads(TAURI_MACOS.read_text(encoding="utf-8"))["bundle"]["resources"]
    entries = list(resources.items()) if isinstance(resources, dict) else [(entry, entry) for entry in resources]
    for source, destination in entries:
        for name in pack_names | {"language-dictionaries"}:
            check(name not in source and name not in destination, f"tauri.macos.conf.json bundles {source} -> {destination}, which belongs to an on-demand pack ({name})")

    # 三个资源包锁文件里的地址都必须是不可变的。日文资源包是 desktop-dictionary.lock.json 的子集，整个锁一起查。
    for lock_path, lock in ((DESKTOP_LOCK, desktop), (LANGUAGE_LOCK, language), (HANDWRITING_LOCK, handwriting)):
        for artifact in lock["artifacts"]:
            url = artifact["url"]
            check(bool(IMMUTABLE_URL.match(url)), f"{lock_path.relative_to(ROOT)} pins {url}, which is neither a release asset nor a raw URL at a fixed commit")
            check(url.endswith("/" + artifact["name"]), f"{lock_path.relative_to(ROOT)} names {artifact['name']} but downloads {url}")

    # PR 打包检查的改动判断要覆盖每个打包输入，否则改了它的 PR 不会打包。
    workflow = "\n".join(live_lines(PR_WORKFLOW))
    for path in PACKAGING_INPUTS:
        check(path in workflow, f"ci-macos-package.yml does not treat {path} as a packaging input")

    check(any("platforms/macos/package-release.sh" in line for line in live_lines(RELEASE_WORKFLOW)), "release-macos.yml no longer runs platforms/macos/package-release.sh")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"macOS package resources: {len(pack_names)} pack files stay out of the package, {len(PACKAGING_INPUTS)} packaging inputs gate the develop package check")
    return 0


if __name__ == "__main__":
    sys.exit(main())
