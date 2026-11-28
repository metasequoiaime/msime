#!/usr/bin/env python3
"""macOS 发布包只带核心词库，日文词典、五笔码表（五笔版除外）、粤拼/注音词库、手写模型等资源包由 App 按需下载。

这里核对几处必须一致的地方：resources.rs 里不进包的日文文件名单、打包脚本不再取回或拷入资源包、tauri.macos.conf.json 不声明资源包文件、三个资源包锁文件只用不可变的地址（固定 tag 的 GitHub release 附件或固定提交的 raw 地址），ci-macos-package.yml 的改动判断覆盖每个打包输入，以及 pkg 安装包的命名、pkgbuild 参数、分发文件、postinstall 和组件表（installer_package.py）。任何一处只改一边，都可能出一个缺数据又下载不到的发布包，而这种包在开发机上照样能跑。

只用标准库；任何一个输入文件缺失时打印 skipped 并以 0 退出。
"""
import contextlib
import importlib.util
import io
import json
import plistlib
import re
import shutil
import stat as stat_module
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
ENGINE_BUDGET = ROOT / "platforms/macos/scripts/engine_resources_budget.py"
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
    "platforms/macos/scripts/engine_resources_budget.py",
    "platforms/macos/resources/installer/",
    "platforms/macos/resources/system-updater/",
    "crates/macos-system-updater/",
    "platforms/macos/stage-resources.sh",
    "apps/desktop/src-tauri/tauri.macos.conf.json",
    "resources/desktop-dictionary.lock.json",
    "resources/editions/",
    "resources/helpcodes/",
    "resources/language-dictionaries.lock.json",
    "resources/handwriting-model.lock.json",
    "resources/offline-glosses.lock.json",
    "resources/voice-runtime-macos.lock.json",
    "resources/wordbook.lock.json",
    "scripts/fetch_wordbooks.py",
    "resources/rerank-model.lock.json",
    "scripts/fetch_rerank_model.py",
    "resources/onnxruntime-minimal.lock.json",
    "scripts/fetch_onnxruntime.py",
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
            scripts_postinstall = expanded / ids["component"] / "Scripts/postinstall"
            broken(scripts_postinstall, f"app_name='{ids['app_bundle']}'", "app_name='Other.app'", f"does not open {ids['app_bundle']}")
            broken(scripts_postinstall, f"ime_name='{ids['ime_bundle']}'", "ime_name='Other.app'", f"does not install {ids['ime_bundle']}")
            broken(scripts_postinstall, f"ime_bundle_id='{ids['ime_bundle_id']}'", "ime_bundle_id='app.msime.other'", "does not check the bundle id")
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
            check_postinstall(edition, ids, postinstall)

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


# postinstall 用到、测试里要换成桩的系统命令和路径：每一项都必须原样出现在渲染出的脚本里，否则替换悄悄落空，测试跑的就是真命令。
POSTINSTALL_STUBBED = ("/usr/bin/stat ", "/usr/bin/id ", "/usr/bin/dscl ", "/usr/sbin/chown ", "/usr/bin/codesign ", "/bin/launchctl ")
POSTINSTALL_STUB = """#!/bin/bash
name="${0##*/}"
{ printf '%s' "$name"; for argument in "$@"; do printf ' [%s]' "$argument"; done; echo; } >> "$STUB_LOG"
case "$name" in
  stat) echo "$STUB_CONSOLE_USER" ;;
  id) echo "$STUB_UID" ;;
  dscl) echo "NFSHomeDirectory: $STUB_HOME" ;;
  codesign) [ ! -e "${!#}/Contents/codesign-fails" ] ;;
esac
"""
RUST_INPUT_SOURCE = ROOT / "apps/desktop/src-tauri/src/platform/macos/macos_input_source.rs"


def check_updater_install_matches_app(script: str) -> None:
    """postinstall 装更新助手的路径、权限和载入方式必须与设置应用的 SYSTEM_INSTALL_SCRIPT 一致：两边各装一次，装出来的东西不能有两种样子。"""
    rust = RUST_INPUT_SOURCE.read_text(encoding="utf-8")
    match = re.search(r'const SYSTEM_INSTALL_SCRIPT: &str = r#"(.*?)"#;', rust, re.DOTALL)
    check(match is not None, "macos_input_source.rs has no SYSTEM_INSTALL_SCRIPT")
    applescript = match.group(1).replace('\\"', '"') if match else ""
    tool = "/Library/PrivilegedHelperTools/app.msime.system-updater"
    job = "/Library/LaunchDaemons/app.msime.system-updater.plist"
    inbox = "/Library/Application Support/app.msime.system-updater/inbox"
    for name, value in (("SYSTEM_UPDATER_TOOL", tool), ("SYSTEM_UPDATER_JOB", job), ("SYSTEM_UPDATER_INBOX", inbox), ("SYSTEM_INPUT_METHODS", "/Library/Input Methods")):
        check(f'const {name}: &str = "{value}";' in rust, f"macos_input_source.rs no longer has {name} = {value}; update pkg-postinstall.sh with it")
    for variable, value in (("updater_tool", tool), ("updater_job", job), ("updater_inbox", inbox), ("updater_label", "app.msime.system-updater"), ("system_input_methods", "/Library/Input Methods")):
        check(f"{variable}='{value}'" in script, f"postinstall does not set {variable}='{value}' like the settings app")
    # 每一行：设置应用的授权脚本里的写法，postinstall 里对应的写法。
    pairs = (
        (f'/bin/mkdir -p /Library/PrivilegedHelperTools "{inbox}"', '/bin/mkdir -p "$(/usr/bin/dirname "$updater_tool")" "$updater_inbox"'),
        (f'/bin/chmod 1777 "{inbox}"', '/bin/chmod 1777 "$updater_inbox"'),
        ("/usr/bin/install -o root -g wheel -m 755 \" & quoted form of (item 2 of argv) & \" " + tool, '/usr/bin/install -o root -g wheel -m 755 "$updater_source" "$updater_tool"'),
        ("/usr/bin/install -o root -g wheel -m 644 \" & quoted form of (item 3 of argv) & \" " + job, '/usr/bin/install -o root -g wheel -m 644 "$updater_job_source" "$updater_job"'),
        ("/bin/launchctl bootout system/app.msime.system-updater;", '/bin/launchctl bootout "system/$updater_label"'),
        (f"/bin/launchctl bootstrap system {job} || {{ /bin/sleep 2; /bin/launchctl bootstrap system {job}; }}",
         '/bin/launchctl bootstrap system "$updater_job" >/dev/null 2>&1 || { /bin/sleep 2; /bin/launchctl bootstrap system "$updater_job" >/dev/null 2>&1; }'),
    )
    for app_form, script_form in pairs:
        check(app_form in applescript, f"SYSTEM_INSTALL_SCRIPT no longer has {app_form!r}; update pkg-postinstall.sh to match")
        check(script_form in script, f"postinstall does not install the updater like SYSTEM_INSTALL_SCRIPT: {script_form!r}")
    # 随设置应用分发的那两个文件，与 packaged_system_updater 读的是同一处。
    check('.join("MacOS/msime-system-updater")' in rust and '.join("app.msime.system-updater.plist")' in rust, "packaged_system_updater no longer reads Contents/MacOS/msime-system-updater and Contents/Resources/app.msime.system-updater.plist")
    check('updater_source="$app/Contents/MacOS/msime-system-updater"' in script and 'updater_job_source="$app/Contents/Resources/app.msime.system-updater.plist"' in script,
          "postinstall does not take the updater from the same files as packaged_system_updater")
    label = plistlib.loads((ROOT / "platforms/macos/resources/system-updater/app.msime.system-updater.plist").read_bytes())
    check(label.get("Label") == "app.msime.system-updater" and label.get("ProgramArguments") == [tool], "the updater's launchd job no longer runs app.msime.system-updater from PrivilegedHelperTools")


def write_ime_bundle(path: Path, ids: dict, build: str, marker: str, revision: str = "", broken: bool = False) -> None:
    """一个合成的输入法 bundle：Info.plist、可执行文件、权限收紧的资源、框架里的相对链接；broken 时 codesign 桩校验不过它。"""
    (path / "Contents/MacOS").mkdir(parents=True)
    info = {"CFBundleIdentifier": ids["ime_bundle_id"], "CFBundleVersion": build, "CFBundleShortVersionString": "0.60.1"}
    if revision:
        info["MSIMEInputSourceRevision"] = revision
    (path / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    executable = path / "Contents/MacOS" / ids["ime_executable"]
    executable.write_text("#!/bin/sh\n", encoding="utf-8")
    executable.chmod(0o700)
    private = path / "Contents/Resources/private"
    private.mkdir(parents=True)
    (private / "table.txt").write_text("synthetic", encoding="utf-8")
    (private / "table.txt").chmod(0o600)
    private.chmod(0o700)
    (path / "Contents/Resources/marker").write_text(marker, encoding="utf-8")
    framework = path / "Contents/Frameworks/Sparkle.framework"
    (framework / "Versions/B").mkdir(parents=True)
    (framework / "Versions/B/Sparkle").write_text("synthetic", encoding="utf-8")
    (framework / "Versions/Current").symlink_to("B")
    (framework / "Sparkle").symlink_to("Versions/Current/Sparkle")
    if broken:
        (path / "Contents/codesign-fails").write_text("", encoding="utf-8")


def check_postinstall(edition: str, ids: dict, script: str) -> None:
    """postinstall 以 root 把内嵌的输入法原子地装进系统目录、装更新助手；图形安装时以登录用户身份登记、停旧进程、打开设置应用。任何情况都以 0 退出。"""
    live = [line.strip() for line in script.splitlines() if line.strip() and not line.strip().startswith("#")]
    text = "\n".join(live)
    for key, value in (("app_name", ids["app_bundle"]), ("ime_name", ids["ime_bundle"]), ("ime_bundle_id", ids["ime_bundle_id"]), ("ime_executable", ids["ime_executable"])):
        check(f"{key}='{value}'" in text, f"{edition}: postinstall does not set {key}='{value}'")
    check("set -u" in live, f"{edition}: postinstall does not set -u")
    check('if [ -n "${COMMAND_LINE_INSTALL:-}" ]; then' in text, f"{edition}: postinstall does not stop before the user's side for command-line installs")
    for user in ("root", "loginwindow", "_mbsetupuser"):
        check(user in text, f"{edition}: postinstall does not skip the {user} console user")
    check('-lt 501' in text, f"{edition}: postinstall does not skip system accounts")
    check('/bin/launchctl asuser "$uid" /usr/bin/sudo -u "$user" -H "$@"' in text, f"{edition}: postinstall does not run the user's side as the console user with -H")
    check('as_user /usr/bin/open -n "$app"' in text, f"{edition}: postinstall does not open the settings app with -n")
    for forbidden in ("TISEnable", "osascript", "with administrator privileges"):
        check(forbidden not in text, f"{edition}: postinstall uses {forbidden!r}")
    exits = re.findall(r"\bexit\b\s*(\S*)", text)
    check(exits and all(code == "0" for code in exits), f"{edition}: postinstall can exit with {exits}")
    check_updater_install_matches_app(script)
    bash = shutil.which("bash")
    if not bash:
        return
    with tempfile.TemporaryDirectory(prefix="msime-postinstall-") as directory:
        path = Path(directory) / "postinstall"
        path.write_text(script, encoding="utf-8")
        check(subprocess.run([bash, "-n", str(path)], capture_output=True, timeout=10).returncode == 0, f"{edition}: postinstall is not valid bash")
        shellcheck = shutil.which("shellcheck")
        if shellcheck:
            result = subprocess.run([shellcheck, "-s", "bash", str(path)], capture_output=True, text=True, timeout=60)
            check(result.returncode == 0, f"{edition}: shellcheck postinstall: {result.stdout.strip()}")
    if sys.platform != "darwin" or not all(Path(tool).exists() for tool in ("/usr/bin/ditto", "/usr/bin/plutil")):
        print("skipped: postinstall behaviour needs macOS ditto and plutil")
        return
    # 各版本的脚本只差四个名字，上面的静态检查逐个版本都做了；真跑一遍要十几秒，只跑完整版和一个名字不同的版本。
    if edition in ("full", "wubi"):
        run_postinstall_cases(edition, ids, script, bash)


def run_postinstall_cases(edition: str, ids: dict, script: str, bash: str) -> None:
    """把 postinstall 里以 root 改系统的命令换成记账的桩、把 /Library 下的路径换到临时目录，按场景真跑一遍：ditto、chmod、find、mv、plutil 都是真的。"""
    def run(name: str, *, system_build: str = "", system_revision: str = "", system_broken: bool = False, source_build: str = "60001000000200",
            source_revision: str = "", source_broken: bool = False, source: str = "bundle", user_copy: bool = False, updater: bool = True,
            console: str = "synthetic", command_line: bool = False, volume: str = "/", location: bool = True) -> tuple:
        directory = Path(tempfile.mkdtemp(prefix="msime-postinstall-case-"))
        try:
            stubs = directory / "bin"
            stubs.mkdir()
            for tool in ("stat", "id", "dscl", "chown", "codesign", "launchctl", "lsregister"):
                (stubs / tool).write_text(POSTINSTALL_STUB, encoding="utf-8")
                (stubs / tool).chmod(0o755)
            library = directory / "Library"
            (library / "LaunchDaemons").mkdir(parents=True)
            system = library / "Input Methods"
            rewritten = script
            replacements = [(f"{tool}", f"{stubs}/{tool.rsplit('/', 1)[1]}") for tool in POSTINSTALL_STUBBED]
            replacements += [
                ("/usr/bin/install -o root -g wheel ", "/usr/bin/install "),
                ("lsregister='/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister'", f"lsregister='{stubs}/lsregister'"),
                ("system_input_methods='/Library/Input Methods'", f"system_input_methods='{system}'"),
                ("updater_tool='/Library/PrivilegedHelperTools/", f"updater_tool='{library}/PrivilegedHelperTools/"),
                ("updater_job='/Library/LaunchDaemons/", f"updater_job='{library}/LaunchDaemons/"),
                ("updater_inbox='/Library/Application Support/", f"updater_inbox='{library}/Application Support/"),
            ]
            for old, new in replacements:
                check(old in rewritten, f"{edition}: postinstall no longer contains {old!r}, the behaviour test would run the real command")
                rewritten = rewritten.replace(old, new)
            (directory / "postinstall").write_text(rewritten, encoding="utf-8")
            applications = directory / "Applications"
            app = applications / ids["app_bundle"]
            (app / "Contents/MacOS").mkdir(parents=True)
            (app / "Contents/Resources").mkdir(parents=True)
            embedded = app / "Contents/Resources" / ids["ime_bundle"]
            if source == "bundle":
                write_ime_bundle(embedded, ids, source_build, "new", source_revision, source_broken)
            elif source == "symlink":
                write_ime_bundle(directory / "elsewhere.app", ids, source_build, "new")
                embedded.symlink_to(directory / "elsewhere.app")
            if updater:
                (app / "Contents/MacOS/msime-system-updater").write_text("#!/bin/sh\n", encoding="utf-8")
                (app / "Contents/Resources/app.msime.system-updater.plist").write_text("<plist/>\n", encoding="utf-8")
            if system_build:
                write_ime_bundle(system / ids["ime_bundle"], ids, system_build, "old", system_revision, system_broken)
            home = directory / "home"
            if user_copy:
                write_ime_bundle(home / "Library/Input Methods" / ids["ime_bundle"], ids, "1", "user")
            log = directory / "stub.log"
            log.write_text("", encoding="utf-8")
            environment = {"PATH": "/usr/bin:/bin", "STUB_LOG": str(log), "STUB_CONSOLE_USER": console, "STUB_UID": "501", "STUB_HOME": str(home)}
            if command_line:
                environment["COMMAND_LINE_INSTALL"] = "1"
            arguments = [str(directory / "postinstall")] + (["synthetic.pkg", str(applications), volume] if location else [])
            result = subprocess.run([bash, *arguments], env=environment, capture_output=True, text=True, timeout=60)
            target = system / ids["ime_bundle"]
            state = {
                "marker": (target / "Contents/Resources/marker").read_text(encoding="utf-8") if (target / "Contents/Resources/marker").is_file() else None,
                "entries": sorted(entry.name for entry in system.iterdir()) if system.is_dir() else [],
                "calls": log.read_text(encoding="utf-8").splitlines(),
                "updater": (library / "PrivilegedHelperTools/app.msime.system-updater").is_file(),
                "target": target,
                "library": library,
                "app": app,
            }
            check(result.returncode == 0, f"{edition}: postinstall ({name}) exited {result.returncode}: {result.stdout!r} {result.stderr!r}")
            state["stdout"] = result.stdout
            state["modes"] = {}
            if target.is_dir():
                for relative in ("", "Contents/Resources/private", "Contents/Resources/private/table.txt", f"Contents/MacOS/{ids['ime_executable']}"):
                    state["modes"][relative] = stat_module.S_IMODE((target / relative).stat().st_mode) if relative else stat_module.S_IMODE(target.stat().st_mode)
                state["framework_link"] = (target / "Contents/Frameworks/Sparkle.framework/Versions/Current").is_symlink()
                inbox = library / "Application Support/app.msime.system-updater/inbox"
                state["inbox_mode"] = stat_module.S_IMODE(inbox.stat().st_mode) if inbox.is_dir() else None
                tool = library / "PrivilegedHelperTools/app.msime.system-updater"
                job = library / "LaunchDaemons/app.msime.system-updater.plist"
                state["tool_mode"] = stat_module.S_IMODE(tool.stat().st_mode) if tool.is_file() else None
                state["job_mode"] = stat_module.S_IMODE(job.stat().st_mode) if job.is_file() else None
            return state
        finally:
            shutil.rmtree(directory, ignore_errors=True)

    ime = ids["ime_bundle"]

    def calls(state: dict, prefix: str) -> list:
        return [call for call in state["calls"] if call.startswith(prefix)]

    def user_side(state: dict) -> list:
        return calls(state, "launchctl [asuser]")

    def where(name: str, state: dict) -> str:
        return f"{edition}: postinstall ({name}): {state['stdout']!r} {state['calls']}"

    # 全新安装：原子装进系统目录、属主改成 root、权限放开给所有用户、框架链接保留，没有暂存残留；装上助手；以登录用户身份登记、停旧进程、打开设置应用。
    state = run("fresh")
    target = str(state["target"])
    check(state["marker"] == "new" and state["entries"] == [ime], where("fresh", state))
    check(state["modes"] == {"": 0o755, "Contents/Resources/private": 0o755, "Contents/Resources/private/table.txt": 0o644, f"Contents/MacOS/{ids['ime_executable']}": 0o755}, f"{where('fresh', state)} modes {state['modes']}")
    check(state.get("framework_link") is True, f"{where('fresh', state)}: the framework's relative link was not kept")
    check(any(call.startswith("chown [-R] [root:wheel] [") and ".pkg-staging-" in call for call in state["calls"]), f"{where('fresh', state)}: the staged copy is not chowned to root:wheel before it is moved in")
    check(any(call.startswith("codesign [--verify] [--deep] [--strict] [") and ".pkg-staging-" in call for call in state["calls"]), f"{where('fresh', state)}: the staged copy is not verified")
    check(state["tool_mode"] == 0o755 and state["job_mode"] == 0o644 and state["inbox_mode"] == 0o1777, f"{where('fresh', state)}: updater modes {state['tool_mode']} {state['job_mode']} {state['inbox_mode']}")
    check(calls(state, "launchctl [bootout] [system/app.msime.system-updater]") and calls(state, "launchctl [bootstrap] [system] ["), f"{where('fresh', state)}: the updater is not reloaded")
    side = user_side(state)
    order = [index for marker in (f"[{state['library'].parent}/bin/lsregister] [-f] [{target}]", f"[{target}/Contents/MacOS/{ids['ime_executable']}] [--register-input-source]",
                                  "[/usr/bin/pkill] [-u] [501] [-f]", "[/usr/bin/open] [-n]") for index, call in enumerate(side) if marker in call]
    check(len(order) == 4 and order == sorted(order), f"{where('fresh', state)}: the user's side does not lsregister, register, stop the old process and open the app in that order")
    check(all(call.startswith("launchctl [asuser] [501] [/usr/bin/sudo] [-u] [synthetic] [-H]") for call in side), f"{where('fresh', state)}: a user step does not run as the console user")
    check(any("[/usr/bin/pkill] [-x] [System Settings]" in call for call in side), f"{where('fresh', state)}: the input source lists are not refreshed")

    # 升级系统目录里的旧版本：换成新的，旧的和暂存都不留。
    state = run("upgrade", system_build="60001000000100")
    check(state["marker"] == "new" and state["entries"] == [ime] and state["updater"], where("upgrade", state))
    # 已装的构建号相同（无修订）且完好：不动它，仍然登记、打开设置应用，但不停进程。
    state = run("same build", system_build="60001000000200")
    check(state["marker"] == "old" and "left as it is" in state["stdout"] and state["updater"], where("same build", state))
    check(any("--register-input-source" in call for call in user_side(state)) and not any("pkill" in call for call in user_side(state)), where("same build", state))
    # 构建号相同而随附的修订不同：换。
    state = run("same build, new revision", system_build="60001000000200", system_revision="aaaa", source_revision="bbbb")
    check(state["marker"] == "new", where("same build, new revision", state))
    # 已装的更新：从不降级。
    state = run("newer installed", system_build="60001000000300")
    check(state["marker"] == "old" and "left as it is" in state["stdout"], where("newer installed", state))
    # 已装的更新却校验不过：换成能用的。
    state = run("newer but broken", system_build="60001000000300", system_broken=True)
    check(state["marker"] == "new" and state["entries"] == [ime], where("newer but broken", state))
    # 暂存的副本校验不过：已装的原样留着，不留暂存，不装助手不登记，打开设置应用让它兜底。
    state = run("staged copy fails", system_build="60001000000100", source_broken=True)
    check(state["marker"] == "old" and state["entries"] == [ime] and not state["updater"], where("staged copy fails", state))
    check(not any("--register-input-source" in call for call in user_side(state)) and any("[/usr/bin/open] [-n]" in call for call in user_side(state)), where("staged copy fails", state))
    # 登录用户的目录里已经有一份：系统目录不动、不装助手、不登记，打开设置应用去更新那份。
    state = run("user copy", user_copy=True)
    check(state["entries"] == [] and not state["updater"] and "takes priority" in state["stdout"], where("user copy", state))
    check(not any("--register-input-source" in call for call in user_side(state)) and any("[/usr/bin/open] [-n]" in call for call in user_side(state)), where("user copy", state))
    # 命令行安装：照样装好输入法和助手，用户那一侧一步都不做。
    state = run("command line", command_line=True)
    check(state["marker"] == "new" and state["updater"] and not user_side(state) and "command-line install" in state["stdout"], where("command line", state))
    # 登录窗口在前台：装好输入法，用户那一侧不做。
    state = run("login window", console="loginwindow")
    check(state["marker"] == "new" and not user_side(state), where("login window", state))
    # 随附的设置应用没带助手：输入法照装，助手不装。
    state = run("no updater", updater=False)
    check(state["marker"] == "new" and not state["updater"] and "carries no system updater" in state["stdout"], where("no updater", state))
    # 设置应用里没有输入法或者它是链接：什么都不装，设置应用照样打开（首次安装窗口兜底）。
    for name, kind in (("no embedded input method", "missing"), ("embedded input method is a link", "symlink")):
        state = run(name, source=kind)
        check(state["entries"] == [] and not state["updater"] and "not installed" in state["stdout"], where(name, state))
    # 没有参数、装到别的卷：什么都不做。
    for name, options in (("no arguments", {"location": False}), ("another volume", {"volume": "/Volumes/Synthetic"})):
        state = run(name, **options)
        check(state["entries"] == [] and not state["calls"], where(name, state))


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
    check('python3 -I "$repo_root/platforms/macos/scripts/engine_resources_budget.py" --edition "$edition" "$resources_dir/EngineResources"' in source,
          "check_app 没有用 engine_resources_budget.py 核对锁驱动的核心词库预算")
    # du 量的是已分配块数，同一份内容展开安装包后曾多出 18 MiB，让 0.57.0 正式版的五笔包被误拒；体积检查只能按文件长度计。
    check_body = re.search(r"^check_app\(\) \{\n.*?^\}\n", source, re.MULTILINE | re.DOTALL)
    check(check_body is not None and "du -sk" not in check_body.group(), "check_app 又用 du 的已分配块数去比按长度计的核心词库预算")
    spec = importlib.util.spec_from_file_location("engine_resources_budget", ENGINE_BUDGET)
    budget = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(budget)
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
        for edition, omit_wubi in (("wubi", False), ("full", True)):
            if omit_wubi:
                (staged / "msime-wubi.db").unlink()
            expected = sum((a["size"] + 4095) // 4096 * 4 for a in artifacts
                           if not omit_wubi or a["name"] != "msime-wubi.db") + 4096
            actual = budget.budget_kib(root, edition, staged)
            check(actual == expected, f"{edition} 核心词库预算应为 {expected} KiB，实际为 {actual}")

        # 被量的一侧按长度计：一个没分配块的稀疏文件照样按全长算，du 会把它量成 0；预算内的内容通过，多出一个文件就超限。
        small = root / "Small"
        (small / "helpcodes").mkdir(parents=True)
        with open(small / "msime-pinyin.db", "wb") as handle:
            handle.truncate(10 * 1024 * 1024 + 1)
        (small / "helpcodes/helpcode.txt").write_bytes(b"a")
        check(budget.content_kib(small) == 10 * 1024 + 4 + 4, f"EngineResources 应按长度计为 {10 * 1024 + 8} KiB，实际为 {budget.content_kib(small)}")
        (root / "resources/editions/probe.lock.json").write_text(json.dumps({"artifacts": [{"name": "msime-pinyin.db", "size": 10 * 1024 * 1024 + 1}]}))
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            within = budget.check(root, "probe", small)
        check(within == 0, "按长度计在预算内的 EngineResources 被拒绝")
        with open(small / "unexpected.bin", "wb") as handle:
            handle.truncate(4096 * 1024 + 1)
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            over = budget.check(root, "probe", small)
        check(over == 1, "多出一个不在锁里的 4 MiB 文件仍在预算内")


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
    # 整句联想「增强」的逐键重排：host 库开着 onnx-rerank、按锁链接两种架构的精简 ONNX Runtime，CMake 按锁取两档模型放进输入法 bundle，打包时按版本去留并在包里核对。
    check("scripts/fetch_onnxruntime.py --target" in package_text, "package-release.sh does not fetch the locked ONNX Runtime for each target")
    check("--features msime-host-api/onnx-rerank" in package_text, "package-release.sh builds the host library without onnx-rerank")
    check('ORT_LIB_LOCATION="${ort_locations[$index]}"' in package_text, "package-release.sh does not point each target's build at its own ONNX Runtime")
    check('test -s "$nested/Contents/$rerank_directory/$rerank_file"' in package_text, "package-release.sh does not check the rerank models in the input method bundle")
    check('features.neural_keyboard' in package_text, "package-release.sh does not decide the rerank models by the edition's neural_keyboard feature")
    cmake = "\n".join(live_lines(ROOT / "platforms/macos/CMakeLists.txt"))
    check("scripts/fetch_rerank_model.py" in cmake, "CMakeLists.txt does not fetch the locked rerank models into the input method bundle")
    # 输入法 bundle 和设置 App 用同一个图标文件。按 macOS 图标网格留边、带 1024 尺寸的只有设置 App 那一份；满画布的图标在 macOS 26 起的 Finder 和系统设置里会被套上灰底板。
    check((ROOT / "platforms/macos/resources/MSIMEClientInputMethod.icns").read_bytes() == (ROOT / "apps/desktop/src-tauri/icons/icon.icns").read_bytes(), "platforms/macos/resources/MSIMEClientInputMethod.icns differs from apps/desktop/src-tauri/icons/icon.icns; copy the settings app icon so the input method keeps the macOS icon grid")
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
    # 背单词的内置词书曾经从来没进过发布包：生成脚本存在，却没有任何工作流运行它，设置页的词书列表在每个安装上都是空的。发布和 main 上的打包检查都必须生成词书并要求它们在包里。
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
    print(f"macOS package resources: {len(pack_names)} pack files stay out of the package, {len(PACKAGING_INPUTS)} packaging inputs gate the main package check")
    return 0


if __name__ == "__main__":
    sys.exit(main())
