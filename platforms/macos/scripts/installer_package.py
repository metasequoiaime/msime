#!/usr/bin/env python3
"""macOS 发布安装包（pkg）的按版本渲染与结构检查，供 platforms/macos/package-release.sh 调用。

每个版本一个 localSystem 域的分发包，载荷只有该版本的设置应用，装到 /Applications；输入法仍由设置应用装进当前用户目录。为什么这样做见 .agents/notes/implemented/process/2026-11-02-macos-pkg-only.md。

用法：

    installer_package.py field --edition ID KEY                 # identifier（<settings_bundle_id>.pkg）、app_bundle（设置应用的文件名）或 component（组件包文件名）
    installer_package.py render --edition ID --version BUILD --output DIR
        # 在 DIR 下写出 Distribution.xml、scripts/postinstall 和 resources/<语言>.lproj/{Localizable.strings,welcome.html,conclusion.html}
    installer_package.py components --edition ID PLIST          # 改写 pkgbuild --analyze 的组件表：所有组件不可重定位，顶层原子替换并检查版本
    installer_package.py check --edition ID --version BUILD --expanded DIR --bom-listing FILE
        # 核对 pkgutil --expand-full 展开的安装包；FILE 是 `lsbom -p fmu` 对组件 Bom 的输出

模板在 platforms/macos/resources/installer/。只用标准库。
"""

from __future__ import annotations

import argparse
import html
import plistlib
import re
import sys
import xml.etree.ElementTree as ElementTree
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import edition_bundle  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]
TEMPLATES = ROOT / "platforms/macos/resources/installer"
INSTALL_LOCATION = "/Applications"
LOCALES = {"zh-Hans": "zh-Hans", "en": "en"}
PLACEHOLDER = re.compile(r"@[A-Z_]+@")


class PackageError(Exception):
    pass


def identity(edition_id: str) -> dict[str, str]:
    """一个版本在安装包里的全部身份。identifier 一经发布就不能再改：Homebrew cask 的 uninstall pkgutil 按它找回执。"""
    entry = edition_bundle.edition_entry(edition_id)
    macos = entry["platforms"]["macos"]
    # 设置应用的文件名是 Tauri 的 productName，即版本的英文显示名（edition_bundle.tauri_config）。
    app_bundle = f"{entry['display_name']['en']}.app"
    if "'" in app_bundle or "/" in app_bundle:
        raise PackageError(f"edition {edition_id}: the app name {app_bundle!r} cannot be quoted in postinstall")
    return {
        "identifier": f"{macos['settings_bundle_id']}.pkg",
        "settings_bundle_id": macos["settings_bundle_id"],
        "app_bundle": app_bundle,
        "component": f"{macos['dmg_prefix']}.pkg",
        "name.zh-Hans": entry["display_name"]["zh-Hans"],
        "name.en": entry["display_name"]["en"],
        "mode.en": english_mode_label(edition_id),
    }


def english_mode_label(edition_id: str) -> str:
    """英文系统的输入法菜单里本版本第一个模式的名字，例如 full 的 "Metasequoia · ZH"。英文系统上 Finder、Dock 和「输入法」列表显示的仍是中文名（两份 en.lproj/InfoPlist.strings 都这样写），只有菜单里的模式名是英文，英文页面要按用户实际看到的名字称呼。"""
    entry = edition_bundle.edition_entry(edition_id)
    source = (ROOT / "platforms/macos/resources/en.lproj/InfoPlist.strings").read_text(encoding="utf-8")
    prefix = entry["platforms"]["macos"]["input_method_bundle_id"] + "."
    for line in edition_bundle.info_plist_strings(source, edition_id).splitlines():
        match = edition_bundle.STRINGS_LINE.match(line)
        if match and match.group("key").startswith(prefix):
            return match.group("value")
    raise PackageError(f"edition {edition_id}: the English InfoPlist.strings names no input mode")


def substitute(text: str, values: dict[str, str], where: str) -> str:
    for key, value in values.items():
        text = text.replace(f"@{key}@", value)
    left = PLACEHOLDER.findall(text)
    if left:
        raise PackageError(f"{where} still has placeholders {sorted(set(left))}")
    return text


def render(edition_id: str, version: str, output: Path) -> None:
    if not re.fullmatch(r"[0-9]+(\.[0-9]+)*", version):
        raise PackageError(f"package version must be dotted digits, not {version!r}")
    ids = identity(edition_id)
    distribution = substitute((TEMPLATES / "InstallerDistribution.xml.in").read_text(encoding="utf-8"), {
        "IDENTIFIER": html.escape(ids["identifier"]),
        "VERSION": version,
        "COMPONENT": html.escape(ids["component"]),
        "SETTINGS_BUNDLE_ID": html.escape(ids["settings_bundle_id"]),
    }, "InstallerDistribution.xml.in")
    output.mkdir(parents=True, exist_ok=True)
    (output / "Distribution.xml").write_text(distribution, encoding="utf-8")

    scripts = output / "scripts"
    scripts.mkdir(exist_ok=True)
    postinstall = scripts / "postinstall"
    postinstall.write_text(substitute((TEMPLATES / "pkg-postinstall.sh").read_text(encoding="utf-8"),
                                      {"APP_BUNDLE": ids["app_bundle"]}, "pkg-postinstall.sh"), encoding="utf-8")
    postinstall.chmod(0o755)

    for locale, directory in LOCALES.items():
        target = output / "resources" / f"{directory}.lproj"
        target.mkdir(parents=True, exist_ok=True)
        # 两种语言的系统上，Finder、Dock 和「输入法」列表都显示中文名，所以英文页面也用中文名称呼应用和输入法，括号里附英文名；菜单里的模式名另给。
        name = ids["name.zh-Hans"]
        values = {"NAME": html.escape(name), "EN_NAME": html.escape(ids["name.en"]), "MODE": html.escape(ids["mode.en"])}
        for page in ("welcome", "conclusion"):
            source = TEMPLATES / f"{locale}.lproj/{page}.html.in"
            (target / f"{page}.html").write_text(substitute(source.read_text(encoding="utf-8"), values, str(source.relative_to(ROOT))), encoding="utf-8")
        shown = name if locale == "zh-Hans" else f"{name} ({ids['name.en']})"
        title = shown.replace("\\", "\\\\").replace('"', '\\"')
        (target / "Localizable.strings").write_text(f'"SU_TITLE" = "{title}";\n', encoding="utf-8")


def rewrite_components(edition_id: str, path: Path) -> None:
    """pkgbuild --analyze 的组件表：只允许设置应用一个顶层组件；它和每个嵌套组件（内嵌的输入法、Sparkle 的 Updater.app 和 XPC）都不可重定位，否则 Installer 会去升级别处同 bundle id 的副本（用户已装的输入法、别家应用里的 Sparkle），/Applications 里反而没有。顶层原子替换、不降级。"""
    ids = identity(edition_id)
    components = plistlib.loads(path.read_bytes())
    if not isinstance(components, list) or len(components) != 1 or components[0].get("RootRelativeBundlePath") != ids["app_bundle"]:
        found = [component.get("RootRelativeBundlePath") for component in components] if isinstance(components, list) else components
        raise PackageError(f"the package root must hold exactly {ids['app_bundle']}, found {found}")

    def pin(component: dict) -> None:
        component["BundleIsRelocatable"] = False
        for child in component.get("ChildBundles", []):
            pin(child)

    top = components[0]
    pin(top)
    top["BundleOverwriteAction"] = "upgrade"
    top["BundleIsVersionChecked"] = True
    path.write_bytes(plistlib.dumps(components))


def check(edition_id: str, version: str, expanded: Path, bom_listing: Path) -> list[str]:
    """核对展开后的安装包，返回发现的问题。"""
    ids = identity(edition_id)
    problems: list[str] = []
    distribution = ElementTree.parse(expanded / "Distribution").getroot()
    if distribution.get("minSpecVersion") != "2":
        problems.append("Distribution: minSpecVersion is not 2")
    options = distribution.find("options")
    if options is None or options.get("customize") != "never" or options.get("hostArchitectures") != "arm64,x86_64":
        problems.append("Distribution: options must be customize=never, hostArchitectures=arm64,x86_64")
    domains = distribution.find("domains")
    if domains is None or (domains.get("enable_localSystem"), domains.get("enable_currentUserHome"), domains.get("enable_anywhere")) != ("true", "false", "false"):
        problems.append("Distribution: only the localSystem domain may be enabled")
    versions = distribution.findall("volume-check/allowed-os-versions/os-version")
    if [element.get("min") for element in versions] != ["13.0"]:
        problems.append("Distribution: the minimum macOS must be 13.0")
    references = distribution.findall("pkg-ref")
    if any(reference.get("onConclusion") not in (None, "none") for reference in references):
        problems.append("Distribution: a pkg-ref asks for a logout or restart")
    closing = [app.get("id") for app in distribution.findall("pkg-ref/must-close/app")]
    if closing != [ids["settings_bundle_id"]]:
        problems.append(f"Distribution: must-close lists {closing}, expected {ids['settings_bundle_id']}")
    if distribution.find("must-close") is not None:
        problems.append("Distribution: must-close outside a pkg-ref is ignored by Installer")
    for locale in LOCALES.values():
        for page in ("welcome.html", "conclusion.html", "Localizable.strings"):
            if not (expanded / "Resources" / f"{locale}.lproj" / page).is_file():
                problems.append(f"Resources/{locale}.lproj/{page} is missing")

    component = expanded / ids["component"]
    if not (component / "PackageInfo").is_file():
        problems.append(f"the package holds no component {ids['component']}")
        return problems
    info = ElementTree.parse(component / "PackageInfo").getroot()
    expected = {"identifier": ids["identifier"], "version": version, "install-location": INSTALL_LOCATION}
    for key, value in expected.items():
        if info.get(key) != value:
            problems.append(f"PackageInfo: {key} is {info.get(key)!r}, expected {value!r}")
    # pkgbuild 总会写一个 <relocate> 元素，不可重定位时它是空的；里面每个 <bundle> 都是 Installer 会去别处找的副本。
    relocatable = [bundle.get("id") for relocate in info.iter("relocate") for bundle in relocate]
    if relocatable:
        problems.append(f"PackageInfo: relocatable bundles {relocatable}")
    bundles = {bundle.get("path"): bundle for bundle in info.iter("bundle")}
    if f"./{ids['app_bundle']}" not in bundles:
        problems.append(f"PackageInfo: no bundle entry for ./{ids['app_bundle']}")
    postinstall = info.find("scripts/postinstall")
    if postinstall is None or postinstall.get("file") != "./postinstall":
        problems.append("PackageInfo: no postinstall script")
    scripts = sorted(path.name for path in (component / "Scripts").iterdir()) if (component / "Scripts").is_dir() else []
    if scripts != ["postinstall"]:
        problems.append(f"Scripts holds {scripts}, expected only postinstall")
    elif f"app_name='{ids['app_bundle']}'" not in (component / "Scripts/postinstall").read_text(encoding="utf-8"):
        problems.append(f"Scripts/postinstall does not open {ids['app_bundle']}")

    # Bom 里的每一项都必须在 ./<应用名>.app 下面（"." 是安装位置 /Applications 本身，权限位与它相同的 0775），归 root 所有。文件带扩展属性时 pkgbuild 另记一项 AppleDouble 的 ._<名字>，装的时候还原成同级那个文件的扩展属性，所以按同级文件判断位置。
    prefix = f"./{ids['app_bundle']}"
    entries = 0
    for line in bom_listing.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        fields = line.split("\t")
        path, mode, owner = fields[0], fields[1] if len(fields) > 1 else "", fields[2] if len(fields) > 2 else ""
        entries += 1
        if path == ".":
            if mode != "40775":
                problems.append(f"Bom: the install location is recorded with mode {mode or '?'}, expected 40775 like /Applications")
            continue
        parent, _, name = path.rpartition("/")
        owner_path = f"{parent}/{name[2:]}" if name.startswith("._") else path
        if owner_path != prefix and not owner_path.startswith(prefix + "/"):
            problems.append(f"Bom: {path} is outside {prefix}")
        if owner != "0":
            problems.append(f"Bom: {path} is owned by uid {owner or '?'}, not root")
    if entries < 2:
        problems.append("Bom: the listing is empty")
    if not (component / "Payload" / ids["app_bundle"]).is_dir():
        problems.append(f"Payload has no {ids['app_bundle']}")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    field = commands.add_parser("field")
    field.add_argument("--edition", required=True)
    field.add_argument("key", choices=["identifier", "app_bundle", "component"])
    render_command = commands.add_parser("render")
    render_command.add_argument("--edition", required=True)
    render_command.add_argument("--version", required=True)
    render_command.add_argument("--output", type=Path, required=True)
    components = commands.add_parser("components")
    components.add_argument("--edition", required=True)
    components.add_argument("plist", type=Path)
    check_command = commands.add_parser("check")
    check_command.add_argument("--edition", required=True)
    check_command.add_argument("--version", required=True)
    check_command.add_argument("--expanded", type=Path, required=True)
    check_command.add_argument("--bom-listing", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "field":
            print(identity(args.edition)[args.key])
        elif args.command == "render":
            render(args.edition, args.version, args.output)
        elif args.command == "components":
            rewrite_components(args.edition, args.plist)
        else:
            problems = check(args.edition, args.version, args.expanded, args.bom_listing)
            for problem in problems:
                print(f"installer package ({args.edition}): {problem}", file=sys.stderr)
            return 1 if problems else 0
    except PackageError as error:
        print(f"installer package ({args.edition}): {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
