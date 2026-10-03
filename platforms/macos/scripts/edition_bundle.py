#!/usr/bin/env python3
"""从版本表 `shared/contracts/editions.json` 生成某个版本的 macOS 输入法 bundle 身份。

多个版本可以同时装在一台 Mac 上，彼此完全隔离：每个版本是一个独立的 InputMethodKit bundle，有自己的 bundle id、可执行文件名、连接名、输入模式、NSUserDefaults 域和设置应用。原生代码在运行时从自己的 Info.plist 读这些值（`src/core/EditionIdentity.h`），所以同一次编译出来的 bundle 换上某个版本的 Info.plist 和 InfoPlist.strings、改掉可执行文件名，就是那个版本的输入法。

full 是现有产品本身：对 full，这里的每个输出都与输入逐字节相同，Info.plist.in、InfoPlist.strings、cask 模板渲染出来的结果与引入版本之前完全一样。

其他版本的 Info.plist 由 full 的模板变换而来：

- `CFBundleIdentifier`、`CFBundleExecutable`、`CFBundleName`、`CFBundleDisplayName` 换成版本表里的值，`InputMethodConnectionName` 是新 bundle id 加 `_Connection`（imklaunchagent 只认这种形式，见 Info.plist.in）；
- 输入模式只留下本版本的方案对应的模式，再加上「英」（Roman）。中文主模式 `.Hans` 总是在：输入控制器把它当作任何方案都能退回的模式。版本不含全拼时，`.Hans` 换上默认方案的图标和名字，默认方案自己的模式不再单列，所以五笔版的菜单里只有「五」和「英」；
- 加上 `MSIMEEdition`、`MSIMEInputSchemes`、`MSIMEDefaultScheme`、`MSIMESettingsBundleIdentifier`、`MSIMEKeychainService`，以及版本表 `preference_defaults` 里的 `MSIMEWubiMixedPinyinDefault`；
- 去掉模板里的 XML 注释：它们描述的是 full 的十个模式。

用法：

    edition_bundle.py plist   --edition ID --template Info.plist.in --output Info.plist.in
    edition_bundle.py strings --edition ID --input InfoPlist.strings --output InfoPlist.strings
    edition_bundle.py settings-strings --edition ID --input InfoPlist.strings --output InfoPlist.strings  # 设置应用的显示名
    edition_bundle.py apply   --edition ID path/to/水杉输入法.app      # 把编好的 full bundle 原地改成该版本（改名由调用方负责）
    edition_bundle.py field   --edition ID KEY                        # 打印 macOS 段的字段、display_name.zh-Hans/en 或 bundle_name
    edition_bundle.py marker  --edition ID --output edition.json      # 设置应用 Resources 里的版本声明；full 不写
    edition_bundle.py tauri-config --edition ID --version VERSION     # 设置应用按版本打包时传给 tauri bundle --config 的配置
    edition_bundle.py cask    --edition ID --version V --sha256 S --template msime.rb.in
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
EDITIONS = ROOT / "shared/contracts/editions.json"
FULL = "full"

# 方案到它的输入模式后缀，与 src/input/InputModeIdentifiers.h 一致。
SCHEME_MODES = {
    "quanpin": "Hans",
    "shuangpin": "Shuangpin",
    "wubi": "Wubi",
    "japanese": "Japanese",
    "korean": "Korean",
    "cantonese": "Cantonese",
    "zhuyin": "Zhuyin",
    "vietnamese": "Vietnamese",
    "tibetan": "Tibetan",
    "stroke": "Stroke",
}
PRIMARY = "Hans"
ENGLISH = "Roman"
# 可以充当中文主模式的模式：它们和 .Hans 一样归在「简体中文」下。
SIMPLIFIED_CHINESE_MODES = {"Hans", "Shuangpin", "Wubi"}
ICON_KEYS = ("tsInputModeMenuIconFileKey", "tsInputModePaletteIconFileKey")


def load_table() -> dict:
    return json.loads(EDITIONS.read_text(encoding="utf-8"))


def edition_entry(edition_id: str, table: dict | None = None) -> dict:
    table = table or load_table()
    for entry in table["editions"]:
        if entry["id"] == edition_id:
            if entry["platforms"].get("macos") is None:
                raise SystemExit(f"edition {edition_id} has no macOS identifiers in {EDITIONS.relative_to(ROOT)}")
            return entry
    raise SystemExit(f"unknown edition {edition_id!r}; see {EDITIONS.relative_to(ROOT)}")


def full_entry(table: dict | None = None) -> dict:
    return edition_entry(FULL, table)


def mode_plan(entry: dict) -> dict[str, str]:
    """本版本声明的模式：模式后缀到它取图标和名字的模板模式后缀，顺序按模板的可见顺序另行决定。"""
    schemes = entry["input_schemes"]
    if "quanpin" in schemes:
        primary_source = PRIMARY
    else:
        primary_source = SCHEME_MODES[entry["default_scheme"]]
        if primary_source not in SIMPLIFIED_CHINESE_MODES:
            raise SystemExit(f"edition {entry['id']}: the default scheme {entry['default_scheme']} cannot stand in for the Chinese mode")
    plan = {PRIMARY: primary_source}
    for scheme in schemes:
        suffix = SCHEME_MODES[scheme]
        if suffix not in (PRIMARY, primary_source):
            plan[suffix] = suffix
    plan[ENGLISH] = ENGLISH
    return plan


def declared_identifiers(entry: dict) -> set[str]:
    """一个版本的 bundle 声明的全部输入源标识符：bundle id 和每个模式。"""
    bundle = entry["platforms"]["macos"]["input_method_bundle_id"]
    return {bundle} | {f"{bundle}.{suffix}" for suffix in mode_plan(entry)}


def plist_string(text: str, key: str) -> str:
    match = re.search(rf"<key>{key}</key><string>([^<]*)</string>", text)
    if not match:
        raise SystemExit(f"the template has no {key}")
    return match.group(1)


def replace_string(text: str, key: str, value: str) -> str:
    pattern = re.compile(rf"(<key>{key}</key><string>)[^<]*(</string>)")
    if len(pattern.findall(text)) != 1:
        raise SystemExit(f"the template must declare {key} exactly once")
    return pattern.sub(lambda match: match.group(1) + value + match.group(2), text)


def escape(value: str) -> str:
    return value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def edition_keys(entry: dict) -> str:
    macos = entry["platforms"]["macos"]
    schemes = "".join(f"<string>{scheme}</string>" for scheme in entry["input_schemes"])
    lines = [
        f"<key>MSIMEEdition</key><string>{entry['id']}</string>",
        f"<key>MSIMEInputSchemes</key><array>{schemes}</array>",
        f"<key>MSIMEDefaultScheme</key><string>{entry['default_scheme']}</string>",
        f"<key>MSIMESettingsBundleIdentifier</key><string>{macos['settings_bundle_id']}</string>",
        f"<key>MSIMEKeychainService</key><string>{escape(macos['keychain_service'])}</string>",
    ]
    mixed = entry["preference_defaults"].get("wubi_mixed_pinyin")
    if mixed is not None:
        lines.append(f"<key>MSIMEWubiMixedPinyinDefault</key><{'true' if mixed else 'false'}/>")
    return "".join(line + "\n" for line in lines)


MODE_BLOCK = re.compile(r"    <key>(?P<id>[^<]+)</key>\n    <dict>\n(?P<body>.*?)\n    </dict>\n", re.S)
VISIBLE = re.compile(r"(<key>tsVisibleInputModeOrderedArrayKey</key>\n  <array>)(.*?)(</array>)", re.S)


def info_plist(template: str, edition_id: str, table: dict | None = None) -> str:
    """把 full 的 Info.plist（模板或编好的 bundle 里的那份）变成某个版本的。full 原样返回。"""
    table = table or load_table()
    entry = edition_entry(edition_id, table)
    if edition_id == FULL:
        return template
    full_bundle = full_entry(table)["platforms"]["macos"]["input_method_bundle_id"]
    if plist_string(template, "CFBundleIdentifier") != full_bundle:
        raise SystemExit(f"the input is not the full Info.plist; its CFBundleIdentifier is not {full_bundle}")
    macos = entry["platforms"]["macos"]
    bundle = macos["input_method_bundle_id"]

    text = re.sub(r"[ \t]*<!--.*?-->[ \t]*\n", "", template, flags=re.S)
    if "<!--" in text:
        raise SystemExit("the template has an XML comment that does not take up whole lines")
    text = replace_string(text, "CFBundleIdentifier", bundle)
    text = replace_string(text, "CFBundleExecutable", escape(macos["input_method_name"]))
    text = replace_string(text, "CFBundleName", escape(entry["display_name"]["en"]))
    text = replace_string(text, "CFBundleDisplayName", escape(entry["display_name"]["zh-Hans"]))
    text = replace_string(text, "InputMethodConnectionName", f"{bundle}_Connection")

    blocks = {match.group("id"): match for match in MODE_BLOCK.finditer(text)}
    prefix = full_bundle + "."
    if not blocks or any(not identifier.startswith(prefix) for identifier in blocks):
        raise SystemExit("cannot find the input modes in the template")
    template_bodies = {identifier[len(prefix):]: match.group("body") for identifier, match in blocks.items()}
    plan = mode_plan(entry)
    missing = [source for source in plan.values() if source not in template_bodies]
    if missing:
        raise SystemExit(f"the template declares no {missing} mode")

    def body_for(suffix: str) -> str:
        body = template_bodies[suffix].replace(f"{prefix}{suffix}<", f"{bundle}.{suffix}<")
        source = plan[suffix]
        if source != suffix:
            # 主模式借用默认方案模式的图标；其余键（语言、脚本、默认启用）仍是主模式自己的。
            for key in ICON_KEYS:
                icon = re.search(rf"<key>{key}</key><string>([^<]*)</string>", template_bodies[source]).group(1)
                body = re.sub(rf"(<key>{key}</key><string>)[^<]*(</string>)", lambda match: match.group(1) + icon + match.group(2), body)
        return body

    first = min(match.start() for match in blocks.values())
    last = max(match.end() for match in blocks.values())
    ordered = [identifier[len(prefix):] for identifier in blocks if identifier[len(prefix):] in plan]
    rendered = "".join(f"    <key>{bundle}.{suffix}</key>\n    <dict>\n{body_for(suffix)}\n    </dict>\n" for suffix in ordered)
    text = text[:first] + rendered + text[last:]

    visible = VISIBLE.search(text)
    if not visible:
        raise SystemExit("the template has no tsVisibleInputModeOrderedArrayKey")
    order = [identifier[len(prefix):] for identifier in re.findall(r"<string>([^<]+)</string>", visible.group(2))]
    kept = "".join(f"<string>{bundle}.{suffix}</string>" for suffix in order if suffix in plan)
    text = text[: visible.start(2)] + kept + text[visible.end(2):]

    anchor = "<key>ComponentInputModeDict</key>"
    if text.count(anchor) != 1:
        raise SystemExit("the template must declare ComponentInputModeDict exactly once")
    text = text.replace(anchor, edition_keys(entry) + anchor)
    # 设置应用按子串找 full 的 bundle id 认 full 的 bundle；别的版本的 plist 里不能出现它。
    if full_bundle in text:
        raise SystemExit(f"the {edition_id} Info.plist still mentions {full_bundle}")
    return text


STRINGS_LINE = re.compile(r'^"(?P<key>[^"]+)" = "(?P<value>[^"]*)";$')


def info_plist_strings(source: str, edition_id: str, table: dict | None = None) -> str:
    """某个语言的 InfoPlist.strings。full 原样返回；其他版本改名、只留本版本的模式。"""
    table = table or load_table()
    entry = edition_entry(edition_id, table)
    if edition_id == FULL:
        return source
    full = full_entry(table)
    full_bundle = full["platforms"]["macos"]["input_method_bundle_id"]
    full_names = (full["display_name"]["zh-Hans"], "Metasequoia")
    bundle = entry["platforms"]["macos"]["input_method_bundle_id"]
    zh = entry["display_name"]["zh-Hans"]
    en = entry["display_name"]["en"]
    plan = mode_plan(entry)
    prefix = full_bundle + "."

    values: dict[str, str] = {}
    lines = source.splitlines()
    for line in lines:
        match = STRINGS_LINE.match(line)
        if match:
            values[match.group("key")] = match.group("value")

    def renamed(value: str) -> str:
        if value == full_names[0]:
            return zh
        for name, replacement in ((full_names[0], zh), (full_names[1], en)):
            if value.startswith(name + " · "):
                return replacement + value[len(name):]
        return value

    output = []
    for line in lines:
        match = STRINGS_LINE.match(line)
        if not match:
            output.append(line)
            continue
        key, value = match.group("key"), match.group("value")
        if key == full_bundle:
            output.append(f'"{bundle}" = "{zh}";')
        elif key.startswith(prefix):
            suffix = key[len(prefix):]
            if suffix not in plan:
                continue
            output.append(f'"{bundle}.{suffix}" = "{renamed(values[prefix + plan[suffix]])}";')
        else:
            output.append(f'"{key}" = "{renamed(value)}";')
    text = "\n".join(output) + ("\n" if source.endswith("\n") else "")
    if full_bundle in text:
        raise SystemExit(f"the {edition_id} InfoPlist.strings still mentions {full_bundle}")
    return text


def apply_to_bundle(bundle: pathlib.Path, edition_id: str) -> None:
    """把编好的 full bundle 原地改成某个版本：Info.plist、各语言的 InfoPlist.strings 和可执行文件名。bundle 目录本身的改名由调用方负责。"""
    if edition_id == FULL:
        return
    table = load_table()
    entry = edition_entry(edition_id, table)
    contents = bundle / "Contents"
    info = contents / "Info.plist"
    template = info.read_text(encoding="utf-8")
    executable = plist_string(template, "CFBundleExecutable")
    info.write_text(info_plist(template, edition_id, table), encoding="utf-8")
    for strings in sorted((contents / "Resources").glob("*.lproj/InfoPlist.strings")):
        strings.write_text(info_plist_strings(strings.read_text(encoding="utf-8"), edition_id, table), encoding="utf-8")
    name = entry["platforms"]["macos"]["input_method_name"]
    (contents / "MacOS" / executable).rename(contents / "MacOS" / name)


def settings_strings(source: str, edition_id: str, table: dict | None = None) -> str:
    """设置应用（Tauri）某个语言的 InfoPlist.strings：Finder、Dock 和菜单栏显示的名字。full 原样返回；其他版本把名字换成该版本的，并去掉只对 full 成立的注释。"""
    table = table or load_table()
    entry = edition_entry(edition_id, table)
    if edition_id == FULL:
        return source
    full_name = full_entry(table)["display_name"]["zh-Hans"]
    text = re.sub(r"/\*.*?\*/\n?", "", source, flags=re.S)
    if f'"{full_name}"' not in text:
        raise SystemExit(f"the settings app's InfoPlist.strings no longer names {full_name}")
    return text.replace(f'"{full_name}"', f'"{entry["display_name"]["zh-Hans"]}"')


def field(entry: dict, key: str) -> str:
    if key == "bundle_name":
        return entry["platforms"]["macos"]["input_method_name"] + ".app"
    if key.startswith("display_name."):
        return entry["display_name"][key.split(".", 1)[1]]
    if key in entry["platforms"]["macos"]:
        return entry["platforms"]["macos"][key]
    raise SystemExit(f"unknown field {key}")


def marker(edition_id: str) -> str | None:
    """设置应用 `Contents/Resources/edition.json` 的内容（`Edition::PACKAGE_MARKER_FILE`）。full 不带这个文件。"""
    edition_entry(edition_id)
    return None if edition_id == FULL else json.dumps({"edition": edition_id}) + "\n"


def tauri_config(edition_id: str, version: str) -> dict:
    """设置应用按版本打包（`tauri bundle --config`）时合并进去的配置，决定包的 Info.plist 和 .app 的文件名。所有版本共用同一个 `msime-desktop` 可执行文件，它在运行时按 `Contents/Resources/edition.json` 把同样的 identifier 和 productName 写进自己的配置（`apply_edition_to_config`）。full 只带版本号，与引入版本之前相同。"""
    config: dict = {"version": version}
    if edition_id == FULL:
        return config
    entry = edition_entry(edition_id)
    config["identifier"] = entry["platforms"]["macos"]["settings_bundle_id"]
    config["productName"] = entry["display_name"]["en"]
    return config


def cask(template: str, edition_id: str, version: str, sha256: str) -> str:
    """Homebrew cask。full 等于原来的 `sed` 替换结果；其他版本换掉名字、标识和路径，并且不把 msime-mcp 放上 PATH：同名的命令只能有一个，留给 full。"""
    text = template.replace("@VERSION@", version).replace("@SHA256@", sha256)
    if edition_id == FULL:
        return text
    table = load_table()
    full = full_entry(table)
    entry = edition_entry(edition_id, table)
    full_macos, macos = full["platforms"]["macos"], entry["platforms"]["macos"]
    lines = text.splitlines(keepends=True)
    output = []
    for line in lines:
        if line.lstrip().startswith("binary ") and "msime-mcp" in line:
            # 连同它上面那行说明一起去掉。
            if output and output[-1].lstrip().startswith("#"):
                output.pop()
            continue
        output.append(line)
    text = "".join(output)
    names = {"quanpin": "pinyin", "shuangpin": "shuangpin", "wubi": "wubi"}
    offered = [names[scheme] for scheme in entry["input_schemes"] if scheme in names]
    described = offered[0] if len(offered) == 1 else ", ".join(offered[:-1]) + " and " + offered[-1]
    replacements = [
        (f"# Casks/{full_macos['cask']}.rb ", f"# Casks/{macos['cask']}.rb "),
        (f'cask "{full_macos["cask"]}"', f'cask "{macos["cask"]}"'),
        ('desc "Chinese input method for pinyin, shuangpin and wubi"', f'desc "Chinese input method for {described}"'),
        (f"/{full_macos['dmg_prefix']}-#{{version}}", f"/{macos['dmg_prefix']}-#{{version}}"),
        (f'name "{full["display_name"]["en"]}"', f'name "{entry["display_name"]["en"]}"'),
        (f'name "{full["display_name"]["zh-Hans"]}"', f'name "{entry["display_name"]["zh-Hans"]}"'),
        (f'app "{full["display_name"]["en"]}.app"', f'app "{entry["display_name"]["en"]}.app"'),
        (full_macos["input_method_bundle_id"], macos["input_method_bundle_id"]),
        (f"{full_macos['input_method_name']}.app", f"{macos['input_method_name']}.app"),
        (f"Open {full['display_name']['en']} once", f"Open {entry['display_name']['en']} once"),
        (f"adds {full['display_name']['zh-Hans']} to", f"adds {entry['display_name']['zh-Hans']} to"),
    ]
    for old, new in replacements:
        if old not in text:
            raise SystemExit(f"the cask template no longer contains {old!r}")
        text = text.replace(old, new)
    # 设置应用的标识出现在 quit 和 zap 的路径里，带引号或后跟 . / 时才是它，免得改到别的版本的标识。
    text = re.sub(rf'(?<=["/]){re.escape(full_macos["settings_bundle_id"])}(?=["./])', macos["settings_bundle_id"], text)
    for identifier in (full_macos["input_method_bundle_id"], f"{full_macos['input_method_name']}.app"):
        if identifier in text:
            raise SystemExit(f"the {edition_id} cask still mentions {identifier}")
    return text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ["plist", "strings", "settings-strings", "apply", "field", "marker", "tauri-config", "cask"]:
        command = commands.add_parser(name)
        command.add_argument("--edition", required=True)
        if name == "plist":
            command.add_argument("--template", type=pathlib.Path, required=True)
            command.add_argument("--output", type=pathlib.Path, required=True)
        elif name in ("strings", "settings-strings"):
            command.add_argument("--input", type=pathlib.Path, required=True)
            command.add_argument("--output", type=pathlib.Path, required=True)
        elif name == "apply":
            command.add_argument("bundle", type=pathlib.Path)
        elif name == "field":
            command.add_argument("key")
        elif name == "marker":
            command.add_argument("--output", type=pathlib.Path, required=True)
        elif name == "tauri-config":
            command.add_argument("--version", required=True)
        elif name == "cask":
            command.add_argument("--version", required=True)
            command.add_argument("--sha256", required=True)
            command.add_argument("--template", type=pathlib.Path, required=True)
    args = parser.parse_args()

    if args.command == "plist":
        write_if_changed(args.output, info_plist(args.template.read_text(encoding="utf-8"), args.edition))
    elif args.command == "strings":
        write_if_changed(args.output, info_plist_strings(args.input.read_text(encoding="utf-8"), args.edition))
    elif args.command == "settings-strings":
        write_if_changed(args.output, settings_strings(args.input.read_text(encoding="utf-8"), args.edition))
    elif args.command == "apply":
        apply_to_bundle(args.bundle, args.edition)
    elif args.command == "field":
        print(field(edition_entry(args.edition), args.key))
    elif args.command == "marker":
        content = marker(args.edition)
        if content is None:
            args.output.unlink(missing_ok=True)
        else:
            write_if_changed(args.output, content)
    elif args.command == "tauri-config":
        print(json.dumps(tauri_config(args.edition, args.version), ensure_ascii=False, separators=(",", ":")))
    elif args.command == "cask":
        sys.stdout.write(cask(args.template.read_text(encoding="utf-8"), args.edition, args.version, args.sha256))
    return 0


def write_if_changed(path: pathlib.Path, text: str) -> None:
    """内容没变时不碰文件，CMake 配置时生成的文件才不会让整个 bundle 重新链接。"""
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_file() and path.read_text(encoding="utf-8") == text:
        return
    path.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())
