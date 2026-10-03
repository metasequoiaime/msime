#!/usr/bin/env python3
"""版本表 `shared/contracts/editions.json` 的结构和跨字段约束。

版本表是各版本（full、pinyin、wubi 等）的单一事实源，Rust、平台构建脚本和打包脚本都从它取值。单个字段错了往往能编译、能打包，只在某个版本装到机器上才暴露：默认方案不在方案列表里会让回退落到一个不存在的方案，带临时日文却不带日文词库会让开关一直被强制关掉，两个版本的标识撞车会让一个版本覆盖另一个。这些约束跨字段、跨文件，类型系统查不到，所以在这里集中检查。

检查内容：

- 字段集合与 `shared/contracts/editions.schema.json` 声明的一致，`platforms` 的六个键都在；
- id 唯一且第一个是 full，显示名两两不同；
- 方案是引擎全部方案（`host_surface.rs` 的 `ALL_INPUT_SCHEMES`）的子集，保持同样的顺序，默认方案在列表里；
- full 等于现状：全部 8 个方案、默认全拼、没有额外默认值、带全部组件和功能，显示名等于 Info.plist 的 `CFBundleDisplayName` 和 Tauri 的 `productName`；
- 资源组件互不重叠，并集恰好等于 `resources/desktop-dictionary.lock.json` 的条目；
- 数据依赖：用到 msime.db 的方案要带 chinese-main，功能开关要带对应组件，粤语和注音要列出对应语言词库；
- 只追加不改写：`shared/contracts/editions.frozen.json` 里的每个版本都还在，冻结的平台标识一字未改，新写入的平台标识必须同时冻结。

不带参数运行时检查仓库里的文件；`--editions` 和 `--frozen` 可以换成别的文件，用来确认某种错误确实会被拦下。
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EDITIONS = ROOT / "shared/contracts/editions.json"
FROZEN = ROOT / "shared/contracts/editions.frozen.json"
SCHEMA = ROOT / "shared/contracts/editions.schema.json"
HOST_SURFACE = ROOT / "crates/client-core/src/host_surface.rs"
DESKTOP_LOCK = ROOT / "resources/desktop-dictionary.lock.json"
LANGUAGE_LOCK = ROOT / "resources/language-dictionaries.lock.json"
INFO_PLIST = ROOT / "platforms/macos/Info.plist.in"
TAURI_CONF = ROOT / "apps/desktop/src-tauri/tauri.conf.json"

FULL = "full"
PLATFORMS = ["macos", "windows", "linux", "android", "ios", "harmony"]
# 这些方案的候选来自 msime.db（拼音表和五笔码表），五笔混拼也走全拼引擎。
CHINESE_MAIN_SCHEMES = {"quanpin", "shuangpin", "wubi"}
# 功能开关和它依赖的资源组件。
FEATURE_COMPONENTS = {"temporary_japanese": "japanese", "neural_keyboard": "sentence-model"}
# 方案和它依赖的语言词库。
SCHEME_LANGUAGE_DICTIONARIES = {"cantonese": "cantonese.db", "zhuyin": "zhuyin.db"}
# 版本默认值只对含某个方案的版本有意义。
PREFERENCE_DEFAULT_SCHEMES = {"wubi_mixed_pinyin": "wubi"}


def camel_to_snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def engine_schemes(errors: list[str]) -> list[str]:
    text = HOST_SURFACE.read_text(encoding="utf-8")
    match = re.search(r"const ALL_INPUT_SCHEMES: \[InputScheme; \d+\] = \[(.*?)\];", text, re.S)
    if not match:
        errors.append(f"{HOST_SURFACE.relative_to(ROOT)}: ALL_INPUT_SCHEMES not found")
        return []
    return [camel_to_snake(name) for name in re.findall(r"InputScheme::(\w+)", match.group(1))]


def lock_names(path: pathlib.Path) -> list[str]:
    return [artifact["name"] for artifact in json.loads(path.read_text(encoding="utf-8"))["artifacts"]]


def plist_string(key: str) -> str | None:
    match = re.search(rf"<key>{key}</key>\s*<string>([^<]*)</string>", INFO_PLIST.read_text(encoding="utf-8"))
    return match.group(1) if match else None


def schema_keys(node: dict) -> set[str]:
    return set(node["properties"])


def check_keys(errors: list[str], where: str, value: object, expected: set[str], required: set[str]) -> bool:
    if not isinstance(value, dict):
        errors.append(f"{where}: expected an object")
        return False
    unknown = set(value) - expected
    missing = required - set(value)
    if unknown:
        errors.append(f"{where}: fields not declared in the schema: {sorted(unknown)}")
    if missing:
        errors.append(f"{where}: missing fields: {sorted(missing)}")
    return not unknown and not missing


def check_schema_shape(errors: list[str], table: dict, schema: dict) -> None:
    """版本表的字段集合与 schema 声明的一致，避免 schema 只是一份没人对照的说明。"""
    check_keys(errors, "editions.json", table, schema_keys(schema), set(schema["required"]))
    edition = schema["$defs"]["edition"]
    for index, entry in enumerate(table.get("editions", [])):
        where = f"editions[{index}]"
        if not check_keys(errors, where, entry, schema_keys(edition), set(edition["required"])):
            continue
        for field in ["display_name", "preference_defaults", "resources", "features", "platforms"]:
            node = edition["properties"][field]
            check_keys(errors, f"{where}.{field}", entry[field], schema_keys(node), set(node.get("required", [])))
        if isinstance(entry["platforms"], dict):
            for platform, section in entry["platforms"].items():
                if section is not None and not isinstance(section, dict):
                    errors.append(f"{where}.platforms.{platform}: expected an object or null")


def check_editions(errors: list[str], table: dict, frozen: dict) -> None:
    engine = engine_schemes(errors)
    desktop = lock_names(DESKTOP_LOCK)
    language = set(lock_names(LANGUAGE_LOCK))

    if table.get("schema_version") != 1:
        errors.append(f"schema_version: expected 1, found {table.get('schema_version')!r}")

    components: dict[str, list[str]] = table.get("resource_components", {})
    seen_files: dict[str, str] = {}
    for component, files in components.items():
        for name in files:
            if name in seen_files:
                errors.append(f"resource_components: {name} is in both {seen_files[name]} and {component}")
            seen_files[name] = component
            if name not in desktop:
                errors.append(f"resource_components.{component}: {name} is not in {DESKTOP_LOCK.relative_to(ROOT)}")
    for name in desktop:
        if name not in seen_files:
            errors.append(f"resource_components: {DESKTOP_LOCK.relative_to(ROOT)} entry {name} belongs to no component")

    editions = table.get("editions", [])
    ids = [entry.get("id") for entry in editions]
    if not ids or ids[0] != FULL:
        errors.append("editions: the first edition must be full")
    for edition_id in sorted({i for i in ids if ids.count(i) > 1}):
        errors.append(f"editions: id {edition_id!r} appears more than once")
    for language_tag in ["zh-Hans", "en"]:
        names = [entry["display_name"][language_tag] for entry in editions]
        for name in sorted({n for n in names if names.count(n) > 1}):
            errors.append(f"editions: display_name.{language_tag} {name!r} is used by more than one edition")

    for entry in editions:
        edition_id = entry["id"]
        where = f"edition {edition_id}"
        if not re.fullmatch(r"[a-z][a-z0-9-]*", edition_id):
            errors.append(f"{where}: id must be lowercase letters, digits and hyphens")

        schemes = entry["input_schemes"]
        if not schemes:
            errors.append(f"{where}: input_schemes is empty")
        if len(set(schemes)) != len(schemes):
            errors.append(f"{where}: input_schemes repeats a scheme")
        unknown = [s for s in schemes if s not in engine]
        if unknown:
            errors.append(f"{where}: input_schemes not offered by the engine: {unknown}")
        elif schemes != [s for s in engine if s in schemes]:
            errors.append(f"{where}: input_schemes must follow the ALL_INPUT_SCHEMES order {engine}")
        if entry["default_scheme"] not in schemes:
            errors.append(f"{where}: default_scheme {entry['default_scheme']!r} is not in input_schemes")

        for key, value in entry["preference_defaults"].items():
            if not isinstance(value, bool):
                errors.append(f"{where}: preference_defaults.{key} must be a boolean")
            scheme = PREFERENCE_DEFAULT_SCHEMES.get(key)
            if scheme is not None and scheme not in schemes:
                errors.append(f"{where}: preference_defaults.{key} has no effect without the {scheme} scheme")

        chosen = entry["resources"]["components"]
        if len(set(chosen)) != len(chosen):
            errors.append(f"{where}: resources.components repeats a component")
        for component in chosen:
            if component not in components:
                errors.append(f"{where}: resources.components names unknown component {component!r}")
        if "core" not in chosen:
            errors.append(f"{where}: resources.components must include core")
        if CHINESE_MAIN_SCHEMES & set(schemes) and "chinese-main" not in chosen:
            errors.append(f"{where}: {sorted(CHINESE_MAIN_SCHEMES & set(schemes))} read msime.db, so resources.components must include chinese-main")
        if "japanese" in schemes and "japanese" not in chosen:
            errors.append(f"{where}: the japanese scheme needs the japanese component")
        for feature, component in FEATURE_COMPONENTS.items():
            enabled = entry["features"][feature]
            if not isinstance(enabled, bool):
                errors.append(f"{where}: features.{feature} must be a boolean")
            elif enabled and component not in chosen:
                errors.append(f"{where}: features.{feature} needs the {component} component")
        if "japanese" in chosen and not entry["features"]["temporary_japanese"] and "japanese" not in schemes:
            errors.append(f"{where}: ships the japanese component but neither the japanese scheme nor temporary_japanese uses it")
        if "sentence-model" in chosen and not entry["features"]["neural_keyboard"]:
            errors.append(f"{where}: ships the sentence-model component but neural_keyboard is off")

        dictionaries = entry["language_dictionaries"]
        for name in dictionaries:
            if name not in language:
                errors.append(f"{where}: language_dictionaries names {name!r}, which is not in {LANGUAGE_LOCK.relative_to(ROOT)}")
        needed = {SCHEME_LANGUAGE_DICTIONARIES[s] for s in schemes if s in SCHEME_LANGUAGE_DICTIONARIES}
        if set(dictionaries) != needed:
            errors.append(f"{where}: language_dictionaries must be exactly {sorted(needed)}, found {sorted(dictionaries)}")

    full = next((entry for entry in editions if entry["id"] == FULL), None)
    if full is not None:
        check_full(errors, full, engine, components)

    check_frozen(errors, editions, frozen)


def check_full(errors: list[str], full: dict, engine: list[str], components: dict) -> None:
    """full 是现有产品本身，它的每个值都必须等于今天写死在代码里的那个。"""
    if full["input_schemes"] != engine:
        errors.append(f"edition full: input_schemes must be every engine scheme {engine}")
    if full["default_scheme"] != "quanpin":
        errors.append("edition full: default_scheme must stay quanpin, the fallback host-api uses today")
    if full["preference_defaults"]:
        errors.append("edition full: preference_defaults must be empty, full uses Preferences::default() unchanged")
    if sorted(full["resources"]["components"]) != sorted(components):
        errors.append("edition full: resources.components must include every component")
    for feature, enabled in full["features"].items():
        if enabled is not True:
            errors.append(f"edition full: features.{feature} must be true")
    display = plist_string("CFBundleDisplayName")
    if full["display_name"]["zh-Hans"] != display:
        errors.append(f"edition full: display_name.zh-Hans must equal CFBundleDisplayName {display!r} in {INFO_PLIST.relative_to(ROOT)}")
    product = json.loads(TAURI_CONF.read_text(encoding="utf-8")).get("productName")
    if full["display_name"]["en"] != product:
        errors.append(f"edition full: display_name.en must equal productName {product!r} in {TAURI_CONF.relative_to(ROOT)}")


def check_frozen(errors: list[str], editions: list[dict], frozen: dict) -> None:
    """id 和平台标识写入后不能再改：它们会成为状态目录、包名、CLSID，改了就等于换了一个产品。"""
    if frozen.get("schema_version") != 1:
        errors.append(f"editions.frozen.json: schema_version: expected 1, found {frozen.get('schema_version')!r}")
    current = {entry["id"]: entry for entry in editions}
    frozen_ids = set()
    for record in frozen.get("editions", []):
        edition_id = record.get("id")
        frozen_ids.add(edition_id)
        entry = current.get(edition_id)
        if entry is None:
            errors.append(f"editions.frozen.json: edition {edition_id!r} was removed or renamed")
            continue
        for platform, section in record.get("platforms", {}).items():
            if entry["platforms"].get(platform) != section:
                errors.append(f"edition {edition_id}: platforms.{platform} differs from its frozen value")
    for edition_id, entry in current.items():
        if edition_id not in frozen_ids:
            errors.append(f"edition {edition_id}: missing from editions.frozen.json; record every new edition there")
            continue
        record = next(r for r in frozen["editions"] if r.get("id") == edition_id)
        for platform, section in entry["platforms"].items():
            if section is not None and platform not in record.get("platforms", {}):
                errors.append(f"edition {edition_id}: platforms.{platform} is defined but not frozen in editions.frozen.json")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--editions", type=pathlib.Path, default=EDITIONS)
    parser.add_argument("--frozen", type=pathlib.Path, default=FROZEN)
    args = parser.parse_args()

    table = json.loads(args.editions.read_text(encoding="utf-8"))
    frozen = json.loads(args.frozen.read_text(encoding="utf-8"))
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))

    errors: list[str] = []
    check_schema_shape(errors, table, schema)
    # 结构不对时后面的跨字段检查会在缺失的字段上抛异常，先把结构错误报出来。
    if not errors:
        check_editions(errors, table, frozen)

    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"editions: {len(table['editions'])} editions agree with the schema, the engine schemes, the resource locks and the frozen baseline")
    return 0


if __name__ == "__main__":
    sys.exit(main())
