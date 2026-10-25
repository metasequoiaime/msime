#!/usr/bin/env python3
"""macOS 发布包只带核心词库，日文词典、五笔码表（五笔版除外）、粤拼/注音词库、手写模型等资源包由 App 按需下载。

这里静态核对几处必须一致的地方：resources.rs 里不进包的日文文件名单、打包脚本不再取回或拷入资源包、tauri.macos.conf.json 不声明资源包文件、三个资源包锁文件只用不可变的地址（固定 tag 的 GitHub release 附件或固定提交的 raw 地址），以及 ci-macos-package.yml 的改动判断覆盖每个打包输入。任何一处只改一边，都可能出一个缺数据又下载不到的发布包，而这种包在开发机上照样能跑。

只用标准库；任何一个输入文件缺失时打印 skipped 并以 0 退出。
"""
import json
import re
import shutil
import subprocess
import sys
import tempfile
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
    "resources/voice-runtime-macos.lock.json",
    "resources/wordbook.lock.json",
    "scripts/fetch_wordbooks.py",
    "crates/client-core/src/resources.rs",
    "crates/client-providers/src/resource_packs.rs",
    "crates/client-providers/src/voice/local_models.rs",
    "crates/client-core/examples/install_resources.rs",
    "crates/client-core/examples/verify_resources.rs",
    "crates/client-providers/examples/install_resource_pack.rs",
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


def check_dmg_creation() -> None:
    bash = shutil.which("bash")
    if not bash:
        print("skipped: DMG creation retry needs bash")
        return
    source = PACKAGE.read_text(encoding="utf-8")
    helper = re.search(r"^create_dmg\(\) \{\n.*?^\}\n", source, re.MULTILINE | re.DOTALL)
    check(helper is not None, "package-release.sh has no bounded DMG creation helper")
    if helper is None:
        return
    # 仅替换系统磁盘映像服务：使用真实半成品文件，检查重试前与持续失败后的清理。
    probe = """set -euo pipefail
edition=tibetan
calls=0
failures=$2
sleep() { :; }
hdiutil() {
  calls=$((calls + 1))
  local output="${!#}"
  [ "$1" = create ] && [ ! -e "$output" ] || return 42
  printf synthetic-image > "$output"
  if [ "$calls" -le "$failures" ]; then return 1; fi
}
""" + helper.group() + """
if create_dmg "$1" -srcfolder 'synthetic source' -format ULMO -fs HFS+; then status=0; else status=$?; fi
printf '%s %s %s\\n' "$calls" "$status" "$([ -e "$1" ] && echo present || echo absent)"
"""
    with tempfile.TemporaryDirectory(prefix="msime-dmg-retry-") as directory:
        for count, expected in ((0, "1 0 present"), (1, "2 0 present"), (3, "3 1 absent")):
            output = Path(directory) / f"合成 安装包-{count}.dmg"
            result = subprocess.run([bash, "-c", probe, "probe", str(output), str(count)],
                                    capture_output=True, text=True, timeout=10)
            check(result.returncode == 0 and result.stdout.strip() == expected,
                  f"DMG creation with {count} failures: expected {expected}, got {result.stdout.strip()}")


def check_input_method_supplementary_tables() -> None:
    bash = shutil.which("bash")
    if not bash:
        print("skipped: input method supplementary tables need bash")
        return
    source = PACKAGE.read_text(encoding="utf-8")
    loop = re.search(r"^for table in [^\n]+; do\n.*?^done$", source, re.MULTILINE | re.DOTALL)
    check(loop is not None, "package-release.sh has no input method supplementary table staging")
    if loop is None:
        return
    # 执行真实打包循环，只用文件复制代替 macOS 的 ditto；三张表必须跟随独立输入法安装。
    probe = """set -euo pipefail
repo_root=$1
staged_bundle=$2
ditto() { cp -R "$1" "$2"; }
""" + loop.group()
    tables = {
        "pronunciations": ("en-phonetic.db", "pronunciations-NOTICE.txt"),
        "character-glosses": ("zh-en.db", "character-glosses-NOTICE.txt"),
        "word-glosses": ("zh-en.db", "word-glosses-NOTICE.txt"),
    }
    with tempfile.TemporaryDirectory(prefix="msime-input-tables-") as directory:
        root = Path(directory)
        bundle = root / "合成 输入法.app"
        (bundle / "Contents/Resources").mkdir(parents=True)
        for table, files in tables.items():
            origin = root / "target/macos" / table
            origin.mkdir(parents=True)
            for name in files:
                (origin / name).write_text(f"synthetic {table}/{name}", encoding="utf-8")
        result = subprocess.run([bash, "-c", probe, "probe", str(root), str(bundle)],
                                capture_output=True, text=True, timeout=10)
        check(result.returncode == 0, f"input method supplementary table staging failed: {result.stderr}")
        for table, files in tables.items():
            for name in files:
                expected = root / "target/macos" / table / name
                installed = bundle / "Contents/Resources" / table / name
                check(installed.is_file() and installed.read_bytes() == expected.read_bytes(),
                      f"input method bundle is missing supplementary {table}/{name}")


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
    # 五笔码表按 macOS 的按需规则校验（--platform macos），只随五笔版进包，包内检查按版本二选一。
    check("--omit-on-demand --platform macos" in package_text, "package-release.sh does not verify EngineResources by the macOS on-demand groups")
    check('test ! -e "$resources_dir/EngineResources/msime-wubi.db"' in package_text, "package-release.sh does not check that msime-wubi.db stays out of EngineResources outside the wubi edition")
    match = re.search(r"pub const ON_DEMAND_WUBI_ARTIFACTS: \[&str; \d+\] =\s*\[([^\]]*)\]", RESOURCES_RS.read_text(encoding="utf-8"))
    check(bool(match) and re.findall(r'"([^"]+)"', match.group(1)) == ["msime-wubi.db"], "ON_DEMAND_WUBI_ARTIFACTS in resources.rs is not exactly msime-wubi.db")
    check("msime-wubi.db" in desktop_names, "msime-wubi.db is not in resources/desktop-dictionary.lock.json, so the wubi pack has nothing to download")

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
    # 背单词的内置词书曾经从来没进过发布包：生成脚本存在，却没有任何工作流运行它，设置页的词书列表在每个安装上都是空的。发布和 develop 打包检查都必须生成词书并要求它们在包里。
    for workflow_path in (RELEASE_WORKFLOW, PR_WORKFLOW):
        lines = live_lines(workflow_path)
        name = workflow_path.relative_to(ROOT)
        check(any("python3 scripts/fetch_wordbooks.py" in line for line in lines), f"{name} does not build the bundled wordbooks with scripts/fetch_wordbooks.py")
        check(any("MSIME_REQUIRE_WORDBOOKS: '1'" in line for line in lines), f"{name} does not set MSIME_REQUIRE_WORDBOOKS, so a package without the bundled wordbooks would still ship")

    check('create_dmg "$dmg" -volname' in package_text, "package-release.sh bypasses the DMG creation retry")
    check_dmg_creation()
    check_input_method_supplementary_tables()

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"macOS package resources: {len(pack_names)} pack files stay out of the package, {len(PACKAGING_INPUTS)} packaging inputs gate the develop package check")
    return 0


if __name__ == "__main__":
    sys.exit(main())
