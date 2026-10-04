#!/usr/bin/env python3
"""各版本的 macOS 输入法 bundle 身份（platforms/macos/scripts/edition_bundle.py 的输出）。

多个版本同时装在一台 Mac 上，靠的是每个版本的 Info.plist 各自声明不同的 bundle id、连接名和输入模式，原生代码在运行时从 Info.plist 读这些值。生成器错了往往照样能编、能装，只在两个版本同时装上之后才暴露：一个版本的输入模式顶掉另一个的，设置应用按子串认错了 bundle，菜单里冒出标识符本身。这里不需要 Xcode，在任何机器上把每个版本的输出逐项核对一遍：

- full 的 Info.plist、两种语言的 InfoPlist.strings 和 Homebrew cask 与生成前逐字节相同，full 不带版本声明文件，Tauri 配置只有版本号；
- 每个版本的 Info.plist 是合法的属性列表，bundle id、可执行文件名、显示名、连接名（`<bundle id>_Connection`）和版本键都对；
- 模式恰好是本版本的方案对应的模式加「英」，主模式 `.Hans` 总在且装好就启用，不含全拼的版本里它用默认方案的图标，模式标识符与 TISInputSourceID、可见顺序一致；
- 主模式和「英」登记在主模式所借模式的语言下：中文的版本是 zh-Hans，日文、越南文、藏文版是 ja、vi、bo，主模式的字符集和脚本也是那个模式的；
- 任何一个版本的 bundle id 都不出现在另一个版本的 Info.plist 和 InfoPlist.strings 里（设置应用按子串认 bundle）；
- platforms/macos/tests/settings/info_plist_names.py 对每个版本的 plist 和 strings 通过：每个模式在每种语言下都有名字且互不相同，源码里的标识字面量都属于某个版本；
- 生成器的模式后缀与 src/input/InputModeIdentifiers.h 一致。
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import plistlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
MACOS = ROOT / "platforms/macos"
TEMPLATE = MACOS / "Info.plist.in"
LOCALES = ["zh-Hans", "en"]
CASK = MACOS / "homebrew/msime.rb.in"
MODE_HEADER = MACOS / "src/input/InputModeIdentifiers.h"
NAMES_CHECK = MACOS / "tests/settings/info_plist_names.py"
# 默认方案是这些语言方案的版本，在系统里登记在这个语言下。
LANGUAGE_EDITION_LANGUAGES = {"japanese": "ja", "vietnamese": "vi", "tibetan": "bo"}


def load_generator():
    spec = importlib.util.spec_from_file_location("edition_bundle", MACOS / "scripts/edition_bundle.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def strings_source(locale: str) -> str:
    return (MACOS / "resources" / f"{locale}.lproj" / "InfoPlist.strings").read_text(encoding="utf-8")


def check_full(errors: list[str], generator, table: dict) -> None:
    template = TEMPLATE.read_text(encoding="utf-8")
    if generator.info_plist(template, "full", table) != template:
        errors.append("the full Info.plist differs from Info.plist.in")
    for locale in LOCALES:
        if generator.info_plist_strings(strings_source(locale), "full", table) != strings_source(locale):
            errors.append(f"the full {locale} InfoPlist.strings differs from the source")
    cask = CASK.read_text(encoding="utf-8")
    expected = cask.replace("@VERSION@", "1.2.3").replace("@SHA256@", "0" * 64)
    if generator.cask(cask, "full", "1.2.3", "0" * 64) != expected:
        errors.append("the full cask differs from the plain @VERSION@/@SHA256@ substitution")
    if generator.marker("full") is not None:
        errors.append("the full settings app must not carry an edition declaration")
    if generator.tauri_config("full", "1.2.3") != {"version": "1.2.3"}:
        errors.append("the full Tauri bundle configuration must only set the version")


def check_edition(errors: list[str], generator, table: dict, entry: dict, others: list[str], directory: pathlib.Path) -> None:
    edition_id = entry["id"]
    macos = entry["platforms"]["macos"]
    bundle = macos["input_method_bundle_id"]
    where = f"edition {edition_id}"
    text = generator.info_plist(TEMPLATE.read_text(encoding="utf-8"), edition_id, table)
    try:
        plist = plistlib.loads(text.encode("utf-8"))
    except Exception as error:  # noqa: BLE001 - plistlib raises several unrelated types for malformed input
        errors.append(f"{where}: the generated Info.plist is not a property list: {error}")
        return
    resources = directory / edition_id / "resources"
    strings: dict[str, str] = {}
    for locale in LOCALES:
        strings[locale] = generator.info_plist_strings(strings_source(locale), edition_id, table)
        target = resources / f"{locale}.lproj" / "InfoPlist.strings"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(strings[locale], encoding="utf-8")
    plist_path = directory / edition_id / "Info.plist.in"
    plist_path.write_text(text, encoding="utf-8")

    expected = {
        "CFBundleIdentifier": bundle,
        "CFBundleExecutable": macos["input_method_name"],
        "InputMethodConnectionName": f"{bundle}_Connection",
    }
    if edition_id == "full":
        expected["CFBundleDisplayName"] = entry["display_name"]["zh-Hans"]
    else:
        expected.update({
            "CFBundleDisplayName": entry["display_name"]["zh-Hans"],
            "CFBundleName": entry["display_name"]["en"],
            "MSIMEEdition": edition_id,
            "MSIMEInputSchemes": entry["input_schemes"],
            "MSIMEDefaultScheme": entry["default_scheme"],
            "MSIMESettingsBundleIdentifier": macos["settings_bundle_id"],
            "MSIMEKeychainService": macos["keychain_service"],
        })
        mixed = entry["preference_defaults"].get("wubi_mixed_pinyin")
        if mixed is not None:
            expected["MSIMEWubiMixedPinyinDefault"] = mixed
        if not entry["features"]["handwriting"]:
            expected["MSIMEHandwriting"] = False
        elif "MSIMEHandwriting" in plist:
            errors.append(f"{where}: an edition with handwriting must not declare MSIMEHandwriting")
    for key, value in expected.items():
        if plist.get(key) != value:
            errors.append(f"{where}: {key} is {plist.get(key)!r}, expected {value!r}")
    if edition_id == "full" and any(key.startswith("MSIME") for key in plist):
        errors.append(f"{where}: the full Info.plist must not declare an edition")

    plan = generator.mode_plan(entry)
    modes = plist["ComponentInputModeDict"]["tsInputModeListKey"]
    declared = {identifier[len(bundle) + 1:] for identifier in modes}
    if declared != set(plan):
        errors.append(f"{where}: declares modes {sorted(declared)}, expected {sorted(plan)}")
    if "Hans" not in declared or "Roman" not in declared:
        errors.append(f"{where}: the Chinese (.Hans) and English (.Roman) modes must both be declared")
    visible = plist["ComponentInputModeDict"]["tsVisibleInputModeOrderedArrayKey"]
    if set(visible) != set(modes):
        errors.append(f"{where}: tsVisibleInputModeOrderedArrayKey {visible} does not list exactly the declared modes")
    template_modes = plistlib.loads(TEMPLATE.read_bytes())["ComponentInputModeDict"]["tsInputModeListKey"]
    full_bundle = generator.full_entry(table)["platforms"]["macos"]["input_method_bundle_id"]
    for suffix, source in plan.items():
        body = modes.get(f"{bundle}.{suffix}", {})
        if body.get("TISInputSourceID") != f"{bundle}.{suffix}":
            errors.append(f"{where}: mode {suffix} carries TISInputSourceID {body.get('TISInputSourceID')!r}")
        icon = template_modes[f"{full_bundle}.{source}"]["tsInputModeMenuIconFileKey"]
        if body.get("tsInputModeMenuIconFileKey") != icon:
            errors.append(f"{where}: mode {suffix} should show the {source} icon {icon}")
    # 系统设置「添加」对话框按 TISIntendedLanguage 分组：主模式和「英」都在主模式所借模式的语言下（日文版 ja、越南文版 vi、藏文版 bo，中文的版本 zh-Hans），主模式装好就启用。
    primary = template_modes[f"{full_bundle}.{plan['Hans']}"]
    language = primary["TISIntendedLanguage"]
    for suffix in ("Hans", "Roman"):
        body = modes.get(f"{bundle}.{suffix}", {})
        if body.get("TISIntendedLanguage") != language:
            errors.append(f"{where}: mode {suffix} is filed under {body.get('TISIntendedLanguage')!r}, expected {language!r}")
    hans = modes.get(f"{bundle}.Hans", {})
    for key in ("tsInputModeCharacterRepertoireKey", "tsInputModeScriptKey"):
        if hans.get(key) != primary[key]:
            errors.append(f"{where}: the primary mode's {key} is {hans.get(key)!r}, expected {primary[key]!r} from the {plan['Hans']} mode")
    if hans.get("tsInputModeDefaultStateKey") is not True:
        errors.append(f"{where}: the primary mode must be enabled on install")
    if edition_id != "full" and entry["default_scheme"] in LANGUAGE_EDITION_LANGUAGES and language != LANGUAGE_EDITION_LANGUAGES[entry["default_scheme"]]:
        errors.append(f"{where}: registers under {language!r}, expected {LANGUAGE_EDITION_LANGUAGES[entry['default_scheme']]!r}")

    for other in others:
        if other == bundle:
            continue
        if other in text:
            errors.append(f"{where}: the Info.plist mentions {other}, another edition's bundle id")
        for locale in LOCALES:
            if other in strings[locale]:
                errors.append(f"{where}: the {locale} InfoPlist.strings mentions {other}, another edition's bundle id")

    result = subprocess.run(
        [sys.executable, str(NAMES_CHECK), str(plist_path), str(resources), str(ROOT)],
        capture_output=True, text=True, check=False,
    )
    if result.returncode != 0:
        for line in result.stderr.strip().splitlines():
            errors.append(f"{where}: {line}")

    if edition_id != "full":
        marker = generator.marker(edition_id)
        if json.loads(marker) != {"edition": edition_id}:
            errors.append(f"{where}: the settings app's edition declaration is {marker!r}")
        config = generator.tauri_config(edition_id, "1.2.3")
        if config != {"version": "1.2.3", "identifier": macos["settings_bundle_id"], "productName": entry["display_name"]["en"]}:
            errors.append(f"{where}: unexpected Tauri bundle configuration {config}")
        cask = generator.cask(CASK.read_text(encoding="utf-8"), edition_id, "1.2.3", "0" * 64)
        if f'cask "{macos["cask"]}"' not in cask or f"/{macos['dmg_prefix']}-#{{version}}-universal.dmg" not in cask:
            errors.append(f"{where}: the cask does not name {macos['cask']} and {macos['dmg_prefix']}")
        if "msime-mcp" in cask:
            errors.append(f"{where}: only the full cask puts msime-mcp on PATH")
        for other in others:
            if other != bundle and other in cask:
                errors.append(f"{where}: the cask mentions {other}, another edition's bundle id")


def check_mode_suffixes(errors: list[str], generator) -> None:
    header = MODE_HEADER.read_text(encoding="utf-8")
    for suffix in set(generator.SCHEME_MODES.values()) | {generator.ENGLISH}:
        if f'MSIMEInputModeIdentifier(@"{suffix}")' not in header:
            errors.append(f"{MODE_HEADER.relative_to(ROOT)} has no mode for the suffix {suffix} the generator uses")


def main() -> int:
    generator = load_generator()
    table = generator.load_table()
    editions = [entry for entry in table["editions"] if entry["platforms"].get("macos") is not None]
    others = [entry["platforms"]["macos"]["input_method_bundle_id"] for entry in editions]
    errors: list[str] = []
    check_full(errors, generator, table)
    check_mode_suffixes(errors, generator)
    with tempfile.TemporaryDirectory() as directory:
        for entry in editions:
            check_edition(errors, generator, table, entry, others, pathlib.Path(directory))
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"macos editions: {len(editions)} editions generate distinct, well-formed bundles and full is unchanged")
    return 0


if __name__ == "__main__":
    sys.exit(main())
