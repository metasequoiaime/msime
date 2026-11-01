#!/usr/bin/env python3
"""macOS 发布包只带核心词库，日文词典、五笔码表（五笔版除外）、粤拼/注音词库、手写模型等资源包由 App 按需下载。

这里核对几处必须一致的地方：resources.rs 里不进包的日文文件名单、打包脚本不再取回或拷入资源包、tauri.macos.conf.json 不声明资源包文件、三个资源包锁文件只用不可变的地址（固定 tag 的 GitHub release 附件或固定提交的 raw 地址），ci-macos-package.yml 的改动判断覆盖每个打包输入，以及 pkg 安装包的命名、pkgbuild 参数、分发文件、postinstall 和组件表（installer_package.py）。任何一处只改一边，都可能出一个缺数据又下载不到的发布包，而这种包在开发机上照样能跑。

只用标准库；任何一个输入文件缺失时打印 skipped 并以 0 退出。
"""
import json
import plistlib
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
    "platforms/macos/scripts/installer_package.py",
    "platforms/macos/scripts/edition_bundle.py",
    "platforms/macos/resources/installer/",
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


def check_productbuild_retry() -> None:
    bash = shutil.which("bash")
    if not bash:
        print("skipped: productbuild retry needs bash")
        return
    source = PACKAGE.read_text(encoding="utf-8")
    helper = re.search(r"^productbuild_timestamped\(\) \{\n.*?^\}\n", source, re.MULTILINE | re.DOTALL)
    check(helper is not None, "package-release.sh has no bounded productbuild helper")
    if helper is None:
        return
    # 只替换 productbuild 和 sleep：时间戳失败时清掉半成品重试，别的错误不重试，有安装包身份时才签名并带时间戳。
    probe = """set -euo pipefail
installer_identity=$2
installer_keychain=$3
failures=$4
message=$5
sleep() { :; }
# 被测函数在命令替换（子 shell）里调用 productbuild，调用次数和参数记在文件里。
productbuild() {
  local output="${!#}" calls
  calls=$(( $(cat "$output.calls" 2>/dev/null || echo 0) + 1 ))
  echo "$calls" > "$output.calls"
  [ ! -e "$output" ] || return 42
  printf '%s\\n' "$*" >> "$output.args"
  printf synthetic-package > "$output"
  if [ "$calls" -le "$failures" ]; then echo "$message" >&2; return 1; fi
}
""" + helper.group() + """
if productbuild_timestamped "$1" --distribution 'synthetic distribution.xml'; then status=0; else status=$?; fi
printf '%s %s %s\\n' "$(cat "$1.calls")" "$status" "$([ -e "$1" ] && echo present || echo absent)"
"""
    timestamp = "productbuild: error: Could not add a trusted timestamp (The timestamp service is not available.)"
    with tempfile.TemporaryDirectory(prefix="msime-pkg-retry-") as directory:
        cases = (
            ("", "", 0, timestamp, "1 0 present", None),
            ("Developer ID Installer: Synthetic (TEAMID)", "/synthetic/keychain-db", 1, timestamp, "2 0 present",
             "--sign Developer ID Installer: Synthetic (TEAMID) --timestamp --keychain /synthetic/keychain-db"),
            ("Developer ID Installer: Synthetic (TEAMID)", "", 5, timestamp, "5 1 absent", "--timestamp"),
            ("", "", 1, "productbuild: error: no such distribution file", "1 1 absent", None),
        )
        for index, (identity, keychain, count, message, expected, signing) in enumerate(cases):
            output = Path(directory) / f"合成 安装包-{index}.pkg"
            result = subprocess.run([bash, "-c", probe, "probe", str(output), identity, keychain, str(count), message],
                                    capture_output=True, text=True, timeout=10)
            check(result.returncode == 0 and result.stdout.strip() == expected,
                  f"productbuild with {count} failures ({message}): expected {expected}, got {result.stdout.strip()} {result.stderr.strip()}")
            arguments = Path(f"{output}.args").read_text(encoding="utf-8") if Path(f"{output}.args").exists() else ""
            if signing is None:
                check("--sign" not in arguments and "--timestamp" not in arguments, f"productbuild signs without an installer identity: {arguments}")
            else:
                check(signing in arguments, f"productbuild did not sign with {signing}: {arguments}")


def strings_entries(text: str) -> list[tuple[str, str]]:
    return re.findall(r'^"([^"]+)" = "([^"]*)";$', text, flags=re.M)


def strings_value(text: str, key: str) -> str:
    values = [value for found, value in strings_entries(text) if found == key]
    check(len(values) == 1, f"InfoPlist.strings has {len(values)} entries for {key}")
    return values[0] if values else f"<missing {key}>"


def check_installer_package() -> None:
    """按版本渲染安装包的分发文件、postinstall 和欢迎/结束页，拼一个与 pkgutil --expand-full 相同布局的合成安装包，用 installer_package.py check 核对；再逐项改坏，确认每一种都被拦下。"""
    sys.path.insert(0, str(ROOT / "platforms/macos/scripts"))
    import installer_package  # noqa: E402
    import edition_bundle  # noqa: E402

    table = edition_bundle.load_table()
    editions = [entry for entry in table["editions"] if entry["platforms"].get("macos") is not None]
    for entry in editions:
        edition = entry["id"]
        macos = entry["platforms"]["macos"]
        ids = installer_package.identity(edition)
        check(ids["identifier"] == f"{macos['settings_bundle_id']}.pkg", f"{edition}: the pkg identifier is {ids['identifier']}")
        check(ids["app_bundle"] == f"{entry['display_name']['en']}.app", f"{edition}: the app is {ids['app_bundle']}")
        # 官网和 Homebrew 认的文件名：full 是 msime-macos-X.Y.Z-universal.pkg，其他版本 msime-macos-<版本>-X.Y.Z-universal.pkg，文件名里从不带 -beta 之类的后缀。
        expected_prefix = "msime-macos" if edition == "full" else f"msime-macos-{edition}"
        check(macos["dmg_prefix"] == expected_prefix, f"{edition}: the package name prefix is {macos['dmg_prefix']}, the website expects {expected_prefix}")
        with tempfile.TemporaryDirectory(prefix="msime-installer-") as directory:
            root = Path(directory)
            installer_package.render(edition, "57000000000000123", root / "rendered")
            expanded = root / "expanded"
            (expanded / ids["component"] / "Scripts").mkdir(parents=True)
            (expanded / ids["component"] / "Payload" / ids["app_bundle"]).mkdir(parents=True)
            shutil.copytree(root / "rendered/resources", expanded / "Resources")
            distribution = (root / "rendered/Distribution.xml").read_text(encoding="utf-8")
            (expanded / "Distribution").write_text(distribution, encoding="utf-8")
            postinstall = (root / "rendered/scripts/postinstall").read_text(encoding="utf-8")
            (expanded / ids["component"] / "Scripts/postinstall").write_text(postinstall, encoding="utf-8")
            app = f"./{ids['app_bundle']}"
            info = (f'<pkg-info identifier="{ids["identifier"]}" version="57000000000000123" install-location="/Applications" auth="root">'
                    f'<bundle path="{app}" id="{macos["settings_bundle_id"]}"/><relocate/>'
                    '<scripts><postinstall file="./postinstall"/></scripts></pkg-info>')
            (expanded / ids["component"] / "PackageInfo").write_text(info, encoding="utf-8")
            bom = root / "bom.txt"
            listing = f".\t40775\t0\n{app}\t40755\t0\n./._{ids['app_bundle']}\t40755\t0\n{app}/Contents\t40755\t0\n"
            bom.write_text(listing, encoding="utf-8")

            def problems() -> list:
                return installer_package.check(edition, "57000000000000123", expanded, bom)

            check(problems() == [], f"{edition}: a well-formed package fails the check: {problems()}")
            check(f'<os-version min="13.0"/>' in distribution and 'hostArchitectures="arm64,x86_64"' in distribution
                  and "RequireLogout" not in distribution.replace('onConclusion="RequireLogout"：', ""),
                  f"{edition}: the Distribution does not pin macOS 13, both architectures and no logout")

            def broken(path: Path, old: str, new: str, expected: str) -> None:
                original = path.read_text(encoding="utf-8")
                check(old in original, f"{edition}: cannot break {path.name}: {old!r} is not in it")
                path.write_text(original.replace(old, new, 1), encoding="utf-8")
                found = problems()
                check(any(expected in problem for problem in found), f"{edition}: breaking {path.name} with {new!r} was not caught ({found})")
                path.write_text(original, encoding="utf-8")

            package_info = expanded / ids["component"] / "PackageInfo"
            broken(package_info, "<relocate/>", f'<relocate><bundle id="{macos["settings_bundle_id"]}"/></relocate>', "relocatable bundles")
            broken(package_info, 'install-location="/Applications"', 'install-location="/Users/Shared"', "install-location")
            broken(package_info, f'identifier="{ids["identifier"]}"', 'identifier="app.msime.inputmethod.MetasequoiaIME.pkg"', "identifier")
            broken(package_info, '<scripts><postinstall file="./postinstall"/></scripts>', "", "no postinstall")
            broken(bom, f"{app}/Contents\t", "./Library/Input Methods\t", "outside")
            broken(bom, f"{app}\t40755\t0", f"{app}\t40755\t501", "not root")
            broken(bom, ".\t40775\t0", ".\t40700\t0", "install location")
            dist = expanded / "Distribution"
            broken(dist, 'enable_localSystem="true" enable_currentUserHome="false"', 'enable_localSystem="true" enable_currentUserHome="true"', "localSystem")
            broken(dist, 'customize="never"', 'customize="allow"', "customize")
            broken(dist, 'onConclusion="none"', 'onConclusion="RequireLogout"', "logout")
            broken(dist, f'<app id="{macos["settings_bundle_id"]}"/>', '<app id="app.msime.macos.other"/>', "must-close")
            broken(dist, "</installer-gui-script>", f'<must-close><app id="{macos["settings_bundle_id"]}"/></must-close></installer-gui-script>', "outside a pkg-ref")
            (expanded / ids["component"] / "Scripts/preinstall").write_text("#!/bin/sh\n", encoding="utf-8")
            check(any("only postinstall" in problem for problem in problems()), f"{edition}: an extra install script was not caught")
            (expanded / ids["component"] / "Scripts/preinstall").unlink()

            # 页面要用 macOS 实际显示的名字称呼应用和输入法：Finder 和 Dock 显示设置应用 InfoPlist.strings 的 CFBundleDisplayName，「输入法」列表显示输入法 InfoPlist.strings 里 bundle id 那一项，菜单显示模式名。英文系统上前两者也是中文名，只看 display_name.en 会写出用户在系统里找不到的名字。
            for locale in ("zh-Hans", "en"):
                app_strings = edition_bundle.settings_strings((ROOT / f"apps/desktop/src-tauri/macos/{locale}.lproj/InfoPlist.strings").read_text(encoding="utf-8"), edition)
                app_name = strings_value(app_strings, "CFBundleDisplayName")
                ime_strings = edition_bundle.info_plist_strings((ROOT / f"platforms/macos/resources/{locale}.lproj/InfoPlist.strings").read_text(encoding="utf-8"), edition)
                source_name = strings_value(ime_strings, macos["input_method_bundle_id"])
                modes = [value for key, value in strings_entries(ime_strings) if key.startswith(macos["input_method_bundle_id"] + ".")]
                for page in ("welcome.html", "conclusion.html"):
                    text = (expanded / "Resources" / f"{locale}.lproj" / page).read_text(encoding="utf-8")
                    check(app_name in text and "@" not in text, f"{edition}: {locale} {page} does not name the app as Finder shows it, {app_name}")
                conclusion = (expanded / "Resources" / f"{locale}.lproj/conclusion.html").read_text(encoding="utf-8")
                check(source_name in conclusion, f"{edition}: the {locale} conclusion does not name the input source as Input Sources lists it, {source_name}")
                if locale == "en":
                    check(any(mode in conclusion for mode in modes), f"{edition}: the en conclusion names none of the input menu's modes {modes}")
                check(("退出登录" if locale == "zh-Hans" else "Log Out") in conclusion, f"{edition}: the {locale} conclusion does not tell a first install to log out and back in")
                title = (expanded / "Resources" / f"{locale}.lproj/Localizable.strings").read_text(encoding="utf-8")
                check(title.startswith(f'"SU_TITLE" = "{app_name}') and title.endswith('";\n'), f"{edition}: the {locale} title is {title!r}, it must start with {app_name}")
            check_postinstall(edition, ids["app_bundle"], postinstall)

    # 组件表：顶层只能有设置应用一个；它和每一层嵌套组件都不可重定位，顶层原子替换、不降级。
    with tempfile.TemporaryDirectory(prefix="msime-components-") as directory:
        path = Path(directory) / "components.plist"
        child = {"BundleOverwriteAction": "", "RootRelativeBundlePath": "MSIME.app/Contents/Resources/水杉输入法.app", "BundleIsRelocatable": True,
                 "ChildBundles": [{"BundleOverwriteAction": "", "RootRelativeBundlePath": "MSIME.app/Contents/Resources/水杉输入法.app/Contents/Frameworks/Sparkle.framework/Versions/B/Updater.app"}]}
        path.write_bytes(plistlib.dumps([{"BundleIsRelocatable": True, "BundleIsVersionChecked": False, "BundleOverwriteAction": "", "RootRelativeBundlePath": "MSIME.app", "ChildBundles": [child]}]))
        installer_package.rewrite_components("full", path)
        top = plistlib.loads(path.read_bytes())[0]
        flags = [top["BundleIsRelocatable"], top["ChildBundles"][0]["BundleIsRelocatable"], top["ChildBundles"][0]["ChildBundles"][0]["BundleIsRelocatable"]]
        check(flags == [False, False, False], f"the component plist leaves a relocatable component: {flags}")
        check(top["BundleOverwriteAction"] == "upgrade" and top["BundleIsVersionChecked"] is True, f"the top component is not an atomic, version-checked upgrade: {top}")
        for components in ([{"RootRelativeBundlePath": "MSIME.app"}, {"RootRelativeBundlePath": "Other.app"}], [{"RootRelativeBundlePath": "MSIME Wubi.app"}]):
            path.write_bytes(plistlib.dumps(components))
            try:
                installer_package.rewrite_components("full", path)
                failures.append(f"the component plist {components} was accepted")
            except installer_package.PackageError:
                pass


def check_postinstall(edition: str, app_bundle: str, script: str) -> None:
    """postinstall 只在图形安装、有正常登录用户时打开设置应用；什么都不写、不注册、不结束进程，任何情况都以 0 退出。"""
    live = [line.strip() for line in script.splitlines() if line.strip() and not line.strip().startswith("#")]
    text = "\n".join(live)
    check(f"app_name='{app_bundle}'" in text, f"{edition}: postinstall does not open {app_bundle}")
    check("set -u" in live, f"{edition}: postinstall does not set -u")
    check('if [ -n "${COMMAND_LINE_INSTALL:-}" ]; then' in text, f"{edition}: postinstall does not stop for command-line installs")
    for user in ("root", "loginwindow", "_mbsetupuser"):
        check(user in text, f"{edition}: postinstall does not skip the {user} console user")
    check('-lt 501' in text, f"{edition}: postinstall does not skip system accounts")
    check('/bin/launchctl asuser "$uid" /usr/bin/sudo -u "$user" -H /usr/bin/open -n "$app"' in text, f"{edition}: postinstall does not open the app as the console user with -H and -n")
    for forbidden in ("chown", "Input Methods", "--register-input-source", "kill", "rm ", "TISEnable"):
        check(forbidden not in text, f"{edition}: postinstall does {forbidden!r}; it may only open the app")
    exits = re.findall(r"\bexit\b\s*(\S*)", text)
    check(exits and all(code == "0" for code in exits), f"{edition}: postinstall can exit with {exits}")
    bash = shutil.which("bash")
    if not bash:
        return
    with tempfile.TemporaryDirectory(prefix="msime-postinstall-") as directory:
        path = Path(directory) / "postinstall"
        path.write_text(script, encoding="utf-8")
        check(subprocess.run([bash, "-n", str(path)], capture_output=True, timeout=10).returncode == 0, f"{edition}: postinstall is not valid bash")
        # 不会走到 open 的几条路：命令行安装、没有参数、安装位置里没有应用、应用是符号链接。
        empty = Path(directory) / "Applications"
        empty.mkdir()
        (empty / "elsewhere").mkdir()
        linked = Path(directory) / "Linked"
        linked.mkdir()
        (linked / app_bundle).symlink_to(empty / "elsewhere")
        environment = {"PATH": "/usr/bin:/bin"}
        for arguments, extra, expected in (
            ([str(path), "synthetic.pkg", str(empty), "/"], {"COMMAND_LINE_INSTALL": "1"}, "command-line install"),
            ([str(path)], {}, "not opening"),
            ([str(path), "synthetic.pkg", str(empty), "/"], {}, "not opening"),
            ([str(path), "synthetic.pkg", str(linked), "/"], {}, "not opening"),
        ):
            result = subprocess.run([bash, *arguments], env={**environment, **extra}, capture_output=True, text=True, timeout=10)
            check(result.returncode == 0 and expected in result.stdout and "opened" not in result.stdout,
                  f"{edition}: postinstall {arguments[1:]} {extra}: {result.returncode} {result.stdout!r} {result.stderr!r}")


def check_pkg_naming_and_order(package_text: str) -> None:
    """安装包的文件名、pkgbuild 的参数，以及 pkgbuild 必须在放开读权限和 check_app 之后（它按磁盘上的权限位归档）。"""
    check('pkg="$out_dir/$dmg_prefix-$version-$arch.pkg"' in package_text, "package-release.sh no longer names the package <dmg_prefix>-VERSION-universal.pkg, which msime-web and the casks expect")
    check("arch=universal" in package_text, "package-release.sh no longer builds a universal package")
    check("[[ \"$version\" =~ ^[0-9]+\\.[0-9]+\\.[0-9]+$ ]]" in package_text, "package-release.sh accepts a version with a suffix, which would end up in the package name")
    for argument in ("--install-location /Applications", "--ownership recommended", "--compression latest", "--min-os-version 13.0", '--scripts "$installer_dir/scripts"', '--component-plist "$installer_dir/components.plist"'):
        check(argument in package_text, f"package-release.sh does not run pkgbuild with {argument}")
    check('productbuild_timestamped "$pkg"' in package_text, "package-release.sh bypasses the productbuild retry")
    check('notary_submit "$pkg"' in package_text, "package-release.sh does not notarize the package")
    check("create_dmg" not in package_text and "hdiutil" not in package_text, "package-release.sh still builds a DMG")
    chmod = package_text.find('chmod -R u+rwX,go+rX,go-w "$app"')
    checked = package_text.find('check_app "$app"')
    analyze = package_text.find("pkgbuild --analyze")
    check(0 <= chmod < checked < analyze, "pkgbuild must run after the settings app is made readable and checked")
    # 发布配置缺了安装包身份要在编译之前失败，不能出应用签了名、安装包没签名的包。
    guard = package_text.find("MACOS_INSTALLER_IDENTITY is not; refusing")
    check(0 <= guard < package_text.find("install_resources"), "package-release.sh does not refuse a release build without an installer identity before compiling")


def check_core_dictionary_budget() -> None:
    source = PACKAGE.read_text(encoding="utf-8")
    helper = re.search(r"^core_dictionary_budget_kib\(\) \{\n.*?^\}\n", source, re.MULTILINE | re.DOTALL)
    check(helper is not None, "核心词库预算没有随版本锁计算，五笔新锁仍会被旧的固定预算拒绝")
    if helper is None:
        return
    check('engine_budget_kib="$(core_dictionary_budget_kib "$resources_dir/EngineResources")"' in source,
          "check_app 没有使用锁驱动的核心词库预算")
    with tempfile.TemporaryDirectory(prefix="msime-core-budget-") as directory:
        root = Path(directory)
        (root / "resources/editions").mkdir(parents=True)
        (root / "resources/helpcodes").mkdir()
        staged = root / "EngineResources"
        staged.mkdir()
        # 大小取真实新锁，空的合成文件只表示它在本版本内；预算必须读锁，不能跟着文件实际长度放宽。
        artifacts = json.loads((ROOT / "resources/editions/wubi.lock.json").read_text())["artifacts"]
        lock = {"artifacts": artifacts}
        (root / "resources/editions/wubi.lock.json").write_text(json.dumps(lock))
        (root / "resources/desktop-dictionary.lock.json").write_text(json.dumps(lock))
        for artifact in artifacts:
            (staged / artifact["name"]).touch()
        probe = 'set -euo pipefail\nrepo_root=$1\nedition=$2\n' + helper.group() + '\ncore_dictionary_budget_kib "$3"'
        for edition, omit_wubi in (("wubi", False), ("full", True)):
            if omit_wubi:
                (staged / "msime-wubi.db").unlink()
            result = subprocess.run(["bash", "-c", probe, "probe", str(root), edition, str(staged)],
                                    capture_output=True, text=True, timeout=10)
            expected = sum((a["size"] + 4095) // 4096 * 4 for a in artifacts
                           if not omit_wubi or a["name"] != "msime-wubi.db") + 4096
            check(result.returncode == 0 and result.stdout.strip() == str(expected),
                  f"{edition} 核心词库预算应为 {expected} KiB，实际为 {result.stdout.strip()}；{result.stderr.strip()}")


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
    # 执行真实打包循环，只用文件复制代替 macOS 的 ditto；四张表必须跟随独立输入法安装。
    probe = """set -euo pipefail
repo_root=$1
staged_bundle=$2
ditto() { cp -R "$1" "$2"; }
""" + loop.group()
    tables = {
        "pronunciations": ("en-phonetic.db", "pronunciations-NOTICE.txt"),
        "target-pronunciations": ("fr-phonetic.db", "ko-phonetic.db", "target-pronunciations-NOTICE.txt"),
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

    check_pkg_naming_and_order(package_text)
    # 发布工作流：发布页只放 pkg 和 SHA256SUMS，文件名按 X.Y.Z（tag 可以带 -beta，文件名不带），Installer 身份从 secret 接进来，下载指引保留官网认的标记。
    release = "\n".join(live_lines(RELEASE_WORKFLOW))
    for expected in ('dist/*.pkg dist/SHA256SUMS', '"dist/${prefix}-${VERSION}-universal.pkg"', "MACOS_INSTALLER_IDENTITY: \"${{ secrets.MACOS_DEVELOPER_ID_INSTALLER }}\"",
                     "-T /usr/bin/productbuild -T /usr/bin/productsign", "[[ \"$version\" =~ ^[0-9]+\\.[0-9]+\\.[0-9]+$ ]]"):
        check(expected in release, f"release-macos.yml no longer has {expected}")
    check(".dmg" not in release, "release-macos.yml still handles DMGs")
    guide = (ROOT / "platforms/macos/scripts/edition_bundle.py").read_text(encoding="utf-8")
    check('"<!-- msime-download-guide -->"' in guide, "the download guide lost the marker msime-web skips it by")
    check_productbuild_retry()
    check_installer_package()
    check_core_dictionary_budget()
    check_input_method_supplementary_tables()

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"macOS package resources: {len(pack_names)} pack files stay out of the package, {len(PACKAGING_INPUTS)} packaging inputs gate the develop package check")
    return 0


if __name__ == "__main__":
    sys.exit(main())
