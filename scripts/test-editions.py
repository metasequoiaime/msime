#!/usr/bin/env python3
"""版本表 `shared/contracts/editions.json` 的结构和跨字段约束。

版本表是各版本（full、pinyin、wubi 等）的单一事实源，Rust、平台构建脚本和打包脚本都从它取值。单个字段错了往往能编译、能打包，只在某个版本装到机器上才暴露：默认方案不在方案列表里会让回退落到一个不存在的方案，带临时日文却不带日文词库会让开关一直被强制关掉，两个版本的标识撞车会让一个版本覆盖另一个。这些约束跨字段、跨文件，类型系统查不到，所以在这里集中检查。

检查内容：

- 字段集合与 `shared/contracts/editions.schema.json` 声明的一致，`platforms` 的六个键都在；
- id 唯一且第一个是 full，显示名两两不同；
- 方案是引擎全部方案（`host_surface.rs` 的 `ALL_INPUT_SCHEMES`）的子集，保持同样的顺序，默认方案在列表里；
- full 等于现状：全部 8 个方案、默认全拼、没有额外默认值、带全部组件和功能，显示名等于 Info.plist 的 `CFBundleDisplayName` 和 Tauri 的 `productName`；
- 资源组件互不重叠，并集恰好等于 `resources/desktop-dictionary.lock.json` 的条目；
- 生成的资源锁没有漂移：`resources/components/` 和 `resources/editions/` 下的文件与 `scripts/editions.py gen-locks` 的输出逐字节相同，没有多余文件，全部组件的并集逐字节等于 `resources/desktop-dictionary.lock.json`；
- 数据依赖：用到 msime.db 的方案要带 chinese-main，功能开关要带对应组件，粤语和注音要列出对应语言词库；
- macOS 身份标识：每个字段在所有版本间两两不同（不区分大小写），一个版本的输入法 bundle id 不能是另一个版本输入模式标识符的前缀，钥匙串服务名连同 `.refresh` 和语音服务凭据的服务名（`EditionIdentity.h` 从 bundle id 推出）也不能撞，使用统计目录（同样由 `EditionIdentity.h` 从版本 id 推出）互不嵌套；full 的值等于今天的 Info.plist.in、tauri.macos.conf.json、cask 和 DMG 名；
- Windows 身份标识：全部版本的全部 GUID（CLSID、profile、TSF 内部 GUID、Inno AppId）两两不同（不区分大小写），名字类字段两两不同，注册表键互不嵌套，%LOCALAPPDATA% 下的目录名（安装器默认数据目录、状态目录、用户目录）两两不同；不是 full 的版本的名字后缀、host DLL 名和安装包名按版本 id 推出，安装包名与 `update-manifest.ts` 认的形式一致；full 的值等于今天的 Globals.cpp、msime_setup.iss、StateDirectory.h 和 tauri.windows.conf.json；
- Linux 身份标识：每个字段在所有版本间两两不同（不区分大小写），一个版本的安装前缀不能嵌在另一个版本的前缀里，由包名推出的 systemd 用户单元、图标和 /usr/bin 命令名也两两不同；不是 full 的版本按版本 id 推出（`msime-linux-<id>`、`/opt/msime-linux-<id>`、`msime-client-<id>`、`msime-<id>`、`app.msime.linux.<id>`）；full 的值等于今天 packaging.cmake 的包名、IBus 组件、Fcitx5 配置、msime-linux-setup 和 tauri.linux.conf.json 里的值；
- Android 身份标识：applicationId 和 APK 名在所有版本间两两不同（不区分大小写），不是 full 的版本按版本 id 推出（`app.msime.android.<id>`、`msime-client-<id>`），清单里每个 ContentProvider 的 authority 都写成 `${applicationId}.<名字>`，所以各版本的 authority 也两两不同；full 的值等于今天 gradle-app 的 applicationId、tauri.android.conf.json 的 identifier 和 build-apk.sh 产出的 APK 名，主资源的应用名等于 full 的显示名；其他版本的 `platforms/android/editions/<id>/res` 里应用名等于版本的显示名，覆盖的另外几句与主资源只差产品名，method.xml 与主资源只差子类型标签；
- 只追加不改写：`shared/contracts/editions.frozen.json` 里的每个版本都还在，冻结的平台标识一字未改，新写入的平台标识必须同时冻结。

不带参数运行时检查仓库里的文件；`--editions` 和 `--frozen` 可以换成别的文件，用来确认某种错误确实会被拦下。
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import re
import sys
from xml.etree import ElementTree

ROOT = pathlib.Path(__file__).resolve().parent.parent
EDITIONS = ROOT / "shared/contracts/editions.json"
FROZEN = ROOT / "shared/contracts/editions.frozen.json"
SCHEMA = ROOT / "shared/contracts/editions.schema.json"
HOST_SURFACE = ROOT / "crates/client-core/src/host_surface.rs"
DESKTOP_LOCK = ROOT / "resources/desktop-dictionary.lock.json"
LANGUAGE_LOCK = ROOT / "resources/language-dictionaries.lock.json"
INFO_PLIST = ROOT / "platforms/macos/Info.plist.in"
TAURI_CONF = ROOT / "apps/desktop/src-tauri/tauri.conf.json"
TAURI_MACOS_CONF = ROOT / "apps/desktop/src-tauri/tauri.macos.conf.json"
EDITION_IDENTITY = ROOT / "platforms/macos/src/core/EditionIdentity.h"
TAURI_WINDOWS_CONF = ROOT / "apps/desktop/src-tauri/tauri.windows.conf.json"
UPDATE_MANIFEST = ROOT / "packages/ui/src/settings/update-manifest.ts"
ANDROID_ROOT = ROOT / "platforms/android"
ANDROID_GRADLE = ANDROID_ROOT / "gradle-app/app/build.gradle.kts"
ANDROID_MANIFEST = ANDROID_ROOT / "AndroidManifest.xml"
TAURI_ANDROID_CONF = ROOT / "apps/desktop/src-tauri/tauri.android.conf.json"
TAURI_LINUX_CONF = ROOT / "apps/desktop/src-tauri/tauri.linux.conf.json"
LINUX_ROOT = ROOT / "platforms/linux"

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
# full 今天写死在 macOS 各处的标识。改了其中任何一个，已安装的用户就会被当成另一个产品：输入源、偏好域、状态目录、钥匙串条目、cask 和更新资产都对不上。
FULL_MACOS = {
    "input_method_bundle_id": "app.msime.inputmethod.MetasequoiaIME",
    "input_method_name": "水杉输入法",
    "settings_bundle_id": "app.msime.macos",
    "keychain_service": "com.metasequoia.msime.account",
    "cask": "msime",
    "dmg_prefix": "msime-macos",
}
# full 今天写死在 Windows 各处的标识：TSF 的 GUID 在 tsf/Global/Globals.cpp，名字在 common/StateDirectory.h、tsf/IME/MetasequoiaIME.cpp 和 installer/msime_setup.iss。改了其中任何一个，已经装着的 full 就会被当成另一个产品：TIP 注册、卸载项、数据目录和登录任务都对不上。
FULL_WINDOWS = {
    "langid": "0x0804",
    "clsid": "{E3062E9A-D834-4637-8958-ED8CFA427D01}",
    "profile_guid": "{4D59B1B4-D503-44AE-9259-BAD9BB2778AB}",
    "tsf_guids": {
        "preserve_key_ime_mode": "{34764E82-AE6D-4F71-BB3A-96799AECE466}",
        "preserve_key_ime_mode_02": "{748C1D81-246B-4849-921F-143BA2BED3F5}",
        "preserve_key_ime_mode_03": "{B7E4F2A1-9C3D-4E8F-A1B2-C3D4E5F60718}",
        "preserve_key_english_input_mode": "{D625C0B1-5A8F-4CC4-9C65-6C536BFF2D91}",
        "preserve_key_double_single_byte": "{4393748A-89DC-485C-A7F7-5FA232CEC70B}",
        "preserve_key_punctuation": "{628DDA3B-38D8-4521-BDD4-85CA38F475B8}",
        "compartment_double_single_byte": "{851BC7CB-8395-4FA6-9C95-DB6EFC2E648E}",
        "compartment_punctuation": "{58DA9E0F-88B2-426F-91C8-802C9B4D9115}",
        "langbar_ime_mode": "{94B8FD94-E918-4667-93BE-57A49D35B02D}",
        "langbar_double_single_byte": "{3E044725-9617-402E-B113-9865AD9B4F8E}",
        "langbar_punctuation": "{596E7EE3-B629-4895-A5B0-C60A82B47A04}",
        "display_attribute_input": "{688746FF-BAF2-4153-93ED-96943436422F}",
        "display_attribute_converted": "{1E2209EA-13CD-4550-8A8F-B352E9744DF2}",
        "candidate_ui_element": "{9FFF12AA-B5EE-4477-A1AA-A4BF5F7B2447}",
    },
    "inno_app_id": "{A7C3E91F-4B2D-4E8A-9F1C-6D5E8B0A2C4D}",
    "app_name": "Metasequoia IME 水杉输入法",
    "text_service_description": "Metasequoia 水杉输入法",
    "install_dir": "metasequoiaime",
    "registry_key": "Software\\Metasequoia\\MetasequoiaIME",
    "state_directory": "MSIME-Client",
    "user_data_directory": "MSIME",
    "data_dir_environment_variable": "METASEQUOIA_IME_DATA_DIR",
    "name_suffix": "",
    "watchdog_task": "Metasequoia IME Watchdog",
    "host_dll": "msime_host_api.dll",
    "tauri_identifier": "app.msime.windows",
    "installer_base_name": "MetasequoiaIME_Setup",
}
# Windows 段里两两不同的名字类字段（GUID 另查）。
WINDOWS_UNIQUE_NAMES = ["app_name", "text_service_description", "install_dir", "registry_key", "state_directory", "user_data_directory", "data_dir_environment_variable", "name_suffix", "watchdog_task", "host_dll", "tauri_identifier", "installer_base_name"]
# 落在 %LOCALAPPDATA% 下的目录名：安装器的默认数据目录（install_dir）、没有 DataDir 时的状态目录和按用户的账号与统计目录。任意两个版本的任意两个撞名，一个版本就会读写、卸载时删掉另一个版本的数据。
WINDOWS_LOCAL_APP_DATA_NAMES = ["install_dir", "state_directory", "user_data_directory"]
# full 今天的 Linux 标识：包名写在 platforms/linux/cmake/packaging.cmake，IBus 组件和引擎名在 data/msime-linux.xml.in，Fcitx5 插件和输入法条目是 fcitx5/msime.conf 与 fcitx5/msime-inputmethod.conf（装成 inputmethod/msime.conf），状态目录 ~/.config/msime-client 写在 msime-linux-setup 和各宿主里，identifier 在 tauri.linux.conf.json。改了其中任何一个，已经装着的 msime-linux 就会被当成另一个产品：包升级不上来，输入法列表里的条目、用户服务和状态目录都对不上。
FULL_LINUX = {
    "package": "msime-linux",
    "install_prefix": "/usr",
    "client_directory": "msime-client",
    "ibus_engine": "msime-linux",
    "fcitx5_addon": "msime",
    "tauri_identifier": "app.msime.linux",
}
# 由包名推出、不单独写进版本表的 Linux 名字：systemd 用户单元、图标和 /usr/bin 下的命令。与 platforms/linux/scripts/edition_linux.py 的推出规则相同。
LINUX_UNITS = ["online.socket", "online.service", "voice.socket", "voice.service", "clipboard.service"]
LINUX_COMMANDS = ["setup", "settings"]
# full 今天的 Android 标识：applicationId 写在 gradle-app/app/build.gradle.kts 的 defaultConfig 和 tauri.android.conf.json 里，APK 名是 build-apk.sh 产出、release-android.yml 发布的文件名。改了 applicationId，已装的用户就收不到覆盖升级，私有数据也换了一个目录。
FULL_ANDROID = {"application_id": "app.msime.android", "apk_name": "msime-client"}
# macOS 上不写进版本表、由 EditionIdentity.h 从版本身份推出的标识：full 沿用今天的值，其他版本按下面的规则推出。
FULL_VOICE_PROVIDER_SERVICE = "app.msime.client.voice.providers"
FULL_USAGE_REPORTING_DIRECTORY = "MSIME/telemetry"


def voice_provider_service(edition_id: str, section: dict) -> str:
    return FULL_VOICE_PROVIDER_SERVICE if edition_id == FULL else section["input_method_bundle_id"] + ".voice"


def usage_reporting_directory(edition_id: str) -> str:
    return FULL_USAGE_REPORTING_DIRECTORY if edition_id == FULL else f"MSIME/{edition_id}/telemetry"


def load_generator():
    spec = importlib.util.spec_from_file_location("editions", ROOT / "scripts/editions.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_generated_locks(errors: list[str], table: dict) -> None:
    """生成的资源锁与 `scripts/editions.py gen-locks` 的输出一致。Rust 用 `include_str!` 嵌入这些文件，手改其中一个、或改了版本表却没重新生成，都会让某个版本按错误的清单校验资源目录。"""
    generator = load_generator()
    desktop_text = DESKTOP_LOCK.read_text(encoding="utf-8")
    desktop = json.loads(desktop_text)
    for problem in generator.drift(generator.generated_locks(table, desktop)):
        errors.append(f"{problem}; run python3 scripts/editions.py gen-locks")
    # full 的锁就是原文件；拆成组件再合起来必须逐字节还原它，否则拆分丢了条目或改了字段。
    if generator.union_of_components(table, desktop) != desktop_text:
        errors.append(f"the union of resource_components does not reproduce {DESKTOP_LOCK.relative_to(ROOT)} byte for byte")


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
            for platform in ["macos", "windows", "linux", "android"]:
                section = entry["platforms"].get(platform)
                node = edition["properties"]["platforms"]["properties"][platform]
                if isinstance(section, dict):
                    check_strings(errors, f"{where}.platforms.{platform}", section, node)


def check_strings(errors: list[str], where: str, section: dict, node: dict) -> None:
    """平台段的字段集合与 schema 一致，每个字符串非空并符合 schema 的 pattern；嵌套的对象（Windows 的 `tsf_guids`）逐层检查。"""
    if not check_keys(errors, where, section, schema_keys(node), set(node["required"])):
        return
    for key, value in section.items():
        rule = node["properties"][key]
        if rule.get("type") == "object":
            if isinstance(value, dict):
                check_strings(errors, f"{where}.{key}", value, rule)
            else:
                errors.append(f"{where}.{key}: expected an object")
            continue
        # name_suffix 是唯一允许为空串的字段：full 的名字不带后缀。
        minimum = rule.get("minLength", 0 if key == "name_suffix" else 1)
        if not isinstance(value, str) or len(value) < minimum:
            errors.append(f"{where}.{key}: expected a non-empty string")
        elif "pattern" in rule and not re.search(rule["pattern"], value):
            errors.append(f"{where}.{key}: {value!r} does not match {rule['pattern']}")


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
        # No hyphen: the id is spliced into asset names after a hyphen (`msime-linux-<id>-<version>`), and packages/ui/src/settings/update-manifest.ts only accepts `[a-z][a-z0-9]*`.
        if not re.fullmatch(r"[a-z][a-z0-9]*", edition_id):
            errors.append(f"{where}: id must be lowercase letters and digits, starting with a letter")

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

    check_macos(errors, editions)
    check_windows(errors, editions)
    check_linux(errors, editions)
    check_android(errors, editions)

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
    macos = full["platforms"].get("macos")
    if macos != FULL_MACOS:
        errors.append(f"edition full: platforms.macos must be the identifiers the product ships with today: {FULL_MACOS}")
    else:
        bundle = plist_string("CFBundleIdentifier")
        if macos["input_method_bundle_id"] != bundle:
            errors.append(f"edition full: platforms.macos.input_method_bundle_id must equal CFBundleIdentifier {bundle!r} in {INFO_PLIST.relative_to(ROOT)}")
        executable = plist_string("CFBundleExecutable")
        if macos["input_method_name"] != executable:
            errors.append(f"edition full: platforms.macos.input_method_name must equal CFBundleExecutable {executable!r} in {INFO_PLIST.relative_to(ROOT)}")
        identifier = json.loads(TAURI_MACOS_CONF.read_text(encoding="utf-8")).get("identifier")
        if macos["settings_bundle_id"] != identifier:
            errors.append(f"edition full: platforms.macos.settings_bundle_id must equal identifier {identifier!r} in {TAURI_MACOS_CONF.relative_to(ROOT)}")


def check_macos(errors: list[str], editions: list[dict]) -> None:
    """多个版本同时安装在一台 Mac 上：任何一个标识撞了，一个版本就会覆盖、停掉或读到另一个版本的东西。"""
    sections = [(entry["id"], entry["platforms"].get("macos")) for entry in editions]
    sections = [(edition_id, section) for edition_id, section in sections if section is not None]
    for key in FULL_MACOS:
        seen: dict[str, str] = {}
        for edition_id, section in sections:
            folded = section[key].casefold()
            if folded in seen:
                errors.append(f"editions {seen[folded]} and {edition_id}: platforms.macos.{key} {section[key]!r} is not unique (case-insensitive)")
            seen[folded] = edition_id
    # 输入模式标识符是 `<bundle id>.<后缀>`。一个版本的 bundle id 以另一个版本的 bundle id 加点开头时，它就可能等于另一个版本的某个输入模式；设置应用的状态目录、偏好域也一样不能落进另一个版本的命名空间。
    bundles = [(edition_id, section["input_method_bundle_id"].casefold()) for edition_id, section in sections]
    for edition_id, bundle in bundles:
        for other_id, other in bundles:
            if edition_id != other_id and bundle.startswith(other + "."):
                errors.append(f"edition {edition_id}: platforms.macos.input_method_bundle_id starts with edition {other_id}'s bundle id, so it can collide with one of that edition's input modes")
    identifiers: dict[str, str] = {}
    for edition_id, section in sections:
        for key in ["input_method_bundle_id", "settings_bundle_id"]:
            folded = section[key].casefold()
            if folded in identifiers and identifiers[folded] != f"{edition_id}.{key}":
                errors.append(f"edition {edition_id}: platforms.macos.{key} {section[key]!r} is also {identifiers[folded]}")
            identifiers[folded] = f"{edition_id}.{key}"
    services: dict[str, str] = {}
    for edition_id, section in sections:
        for service in [section["keychain_service"], section["keychain_service"] + ".refresh", voice_provider_service(edition_id, section)]:
            folded = service.casefold()
            if folded in services:
                errors.append(f"edition {edition_id}: keychain service {service!r} is also used by edition {services[folded]}")
            services[folded] = edition_id
    # 推出规则写在 EditionIdentity.h 里，这里的副本和它对不上时，上面的检查查的就不是装到机器上的那个名字。
    identity = EDITION_IDENTITY.read_text(encoding="utf-8")
    for fragment in [
        f'MSIMEFullVoiceProviderKeychainService = @"{FULL_VOICE_PROVIDER_SERVICE}"',
        f'MSIMEFullUsageReportingDirectoryName = @"{FULL_USAGE_REPORTING_DIRECTORY}"',
        'stringByAppendingString:@".voice"',
        '@"MSIME/%@/telemetry"',
    ]:
        if fragment not in identity:
            errors.append(f"{EDITION_IDENTITY.relative_to(ROOT)} no longer contains {fragment!r}; update the derived identifiers in this script")
    # 使用统计目录一个嵌在另一个里面时，删掉外层（卸载、清理）会连带删掉另一个版本的 install_id 和队列。
    directories = [(edition_id, usage_reporting_directory(edition_id).casefold()) for edition_id, _ in sections]
    for edition_id, directory in directories:
        for other_id, other in directories:
            if edition_id != other_id and (directory == other or directory.startswith(other + "/")):
                errors.append(f"edition {edition_id}: usage reporting directory {directory!r} is inside edition {other_id}'s {other!r}")


def windows_guids(section: dict) -> list[tuple[str, str]]:
    """一个 Windows 段里的全部 GUID：字段名和值。"""
    guids = [(key, section[key]) for key in ["clsid", "profile_guid", "inno_app_id"]]
    guids += [(f"tsf_guids.{key}", value) for key, value in section["tsf_guids"].items()]
    return guids


def installer_base_name(edition_id: str) -> str:
    """不是 full 的版本的安装包名前缀，与 `update-manifest.ts` 的 `editionInstallerPrefix` 相同。"""
    return f"MetasequoiaIME-{edition_id[:1].upper()}{edition_id[1:]}_Setup"


def check_windows(errors: list[str], editions: list[dict]) -> None:
    """多个版本同时安装在一台 Windows 上，而且两个版本的 TIP 可能被同一个应用同时加载：任何一个 GUID 或名字撞了，一个版本就会覆盖、停掉、读到或卸载掉另一个版本的东西。"""
    sections = [(entry["id"], entry["platforms"].get("windows")) for entry in editions]
    sections = [(edition_id, section) for edition_id, section in sections if section is not None]
    guids: dict[str, str] = {}
    for edition_id, section in sections:
        for key, value in windows_guids(section):
            folded = value.casefold()
            if folded in guids:
                errors.append(f"edition {edition_id}: platforms.windows.{key} {value} is also {guids[folded]}")
            guids[folded] = f"edition {edition_id}'s platforms.windows.{key}"
    for key in WINDOWS_UNIQUE_NAMES:
        seen: dict[str, str] = {}
        for edition_id, section in sections:
            folded = section[key].casefold()
            if folded in seen:
                errors.append(f"editions {seen[folded]} and {edition_id}: platforms.windows.{key} {section[key]!r} is not unique (case-insensitive)")
            seen[folded] = edition_id
    # 卸载用 RegDeleteKeyIncludingSubkeys 一类的递归删除：一个版本的键在另一个版本的键下面，卸载外层就带走了里层。
    keys = [(edition_id, section["registry_key"].casefold()) for edition_id, section in sections]
    for edition_id, key in keys:
        for other_id, other in keys:
            if edition_id != other_id and key.startswith(other + "\\"):
                errors.append(f"edition {edition_id}: platforms.windows.registry_key is inside edition {other_id}'s key")
    local: dict[str, str] = {}
    for edition_id, section in sections:
        for key in WINDOWS_LOCAL_APP_DATA_NAMES:
            folded = section[key].casefold()
            if folded in local and local[folded] != f"{edition_id}.{key}":
                errors.append(f"edition {edition_id}: platforms.windows.{key} {section[key]!r} is also %LOCALAPPDATA%\\{section[key]} of {local[folded]}")
            local[folded] = f"{edition_id}.{key}"
    for edition_id, section in sections:
        if edition_id == FULL:
            continue
        expected = {
            "name_suffix": f".{edition_id}",
            "host_dll": f"msime_host_api_{edition_id}.dll",
            "installer_base_name": installer_base_name(edition_id),
        }
        for key, value in expected.items():
            if section[key] != value:
                errors.append(f"edition {edition_id}: platforms.windows.{key} must be {value!r}, found {section[key]!r}")
    full = next((section for edition_id, section in sections if edition_id == FULL), None)
    if full is not None and full != FULL_WINDOWS:
        errors.append(f"edition full: platforms.windows must be the identifiers the product ships with today: {FULL_WINDOWS}")
    if full is not None:
        identifier = json.loads(TAURI_WINDOWS_CONF.read_text(encoding="utf-8")).get("identifier")
        if full["tauri_identifier"] != identifier:
            errors.append(f"edition full: platforms.windows.tauri_identifier must equal identifier {identifier!r} in {TAURI_WINDOWS_CONF.relative_to(ROOT)}")
    # 推出规则抄自 update-manifest.ts；那边改了而这里没跟上时，上面查的就不是更新检查认的名字。
    manifest = UPDATE_MANIFEST.read_text(encoding="utf-8")
    for fragment in ['if (isFullEdition(edition)) return "MetasequoiaIME_Setup_v";', "return `MetasequoiaIME-${edition.charAt(0).toUpperCase()}${edition.slice(1)}_Setup_v`;"]:
        if fragment not in manifest:
            errors.append(f"{UPDATE_MANIFEST.relative_to(ROOT)} no longer contains {fragment!r}; update installer_base_name in this script")


def linux_derived_names(section: dict) -> list[str]:
    """由包名推出的系统级名字：systemd 用户单元、图标和 /usr/bin 下的命令。"""
    package = section["package"]
    return [f"{package}-{unit}" for unit in LINUX_UNITS] + [f"{package}-{command}" for command in LINUX_COMMANDS]


def check_linux(errors: list[str], editions: list[dict]) -> None:
    """多个版本同时装在一台 Linux 上：包名一样，后装的就会把先装的当成旧版本替换掉；IBus 引擎、Fcitx5 条目、systemd 单元或状态目录一样，一个版本就会顶掉、停掉或读写另一个版本的东西。"""
    sections = [(entry["id"], entry["platforms"].get("linux")) for entry in editions]
    sections = [(edition_id, section) for edition_id, section in sections if section is not None]
    for key in FULL_LINUX:
        seen: dict[str, str] = {}
        for edition_id, section in sections:
            folded = section[key].casefold()
            if folded in seen:
                errors.append(f"editions {seen[folded]} and {edition_id}: platforms.linux.{key} {section[key]!r} is not unique (case-insensitive)")
            seen[folded] = edition_id
    # 卸载和升级按前缀删文件：一个版本的前缀在另一个版本的前缀下面，删外层就带走了里层。
    prefixes = [(edition_id, section["install_prefix"].rstrip("/") + "/") for edition_id, section in sections]
    for edition_id, prefix in prefixes:
        for other_id, other in prefixes:
            if edition_id != other_id and prefix.startswith(other):
                errors.append(f"edition {edition_id}: platforms.linux.install_prefix is inside edition {other_id}'s prefix")
    derived: dict[str, str] = {}
    for edition_id, section in sections:
        for name in linux_derived_names(section):
            folded = name.casefold()
            if folded in derived:
                errors.append(f"edition {edition_id}: derived Linux name {name!r} is also edition {derived[folded]}'s")
            derived[folded] = edition_id
    for edition_id, section in sections:
        if edition_id == FULL:
            if section != FULL_LINUX:
                errors.append(f"edition full: platforms.linux must be the identifiers the product ships with today: {FULL_LINUX}")
            continue
        expected = {
            "package": f"msime-linux-{edition_id}",
            "install_prefix": f"/opt/msime-linux-{edition_id}",
            "client_directory": f"msime-client-{edition_id}",
            "ibus_engine": f"msime-linux-{edition_id}",
            "fcitx5_addon": f"msime-{edition_id}",
            "tauri_identifier": f"app.msime.linux.{edition_id}",
        }
        for key, value in expected.items():
            if section[key] != value:
                errors.append(f"edition {edition_id}: platforms.linux.{key} must be {value!r}, found {section[key]!r}")
    full = next((section for edition_id, section in sections if edition_id == FULL), None)
    if full is None or full != FULL_LINUX:
        return
    # full 的值与今天写死在 Linux 宿主各处的值逐个对照：这些文件就是 full 的包装出来的样子。
    packaging = (LINUX_ROOT / "cmake/packaging.cmake").read_text(encoding="utf-8")
    if f'set(CPACK_PACKAGE_NAME "{full["package"]}")' not in packaging:
        errors.append(f"edition full: platforms.linux.package must be CPACK_PACKAGE_NAME in platforms/linux/cmake/packaging.cmake")
    component = (LINUX_ROOT / "data/msime-linux.xml.in").read_text(encoding="utf-8")
    if component.count(f"<name>{full['ibus_engine']}</name>") != 2:
        errors.append("edition full: platforms.linux.ibus_engine must be the component and engine <name> in platforms/linux/data/msime-linux.xml.in")
    if f"/{full['client_directory']}/runtime-options.json" not in component:
        errors.append("edition full: platforms.linux.client_directory must be the /etc directory platforms/linux/data/msime-linux.xml.in passes to the launcher")
    setup = (LINUX_ROOT / "scripts/msime-linux-setup").read_text(encoding="utf-8")
    for fragment in [f'IBUS_ENGINE = "{full["ibus_engine"]}"', f'FCITX5_INPUT_METHOD = "{full["fcitx5_addon"]}"', f'config_home() / "{full["client_directory"]}"']:
        if fragment not in setup:
            errors.append(f"edition full: platforms/linux/scripts/msime-linux-setup no longer contains {fragment!r}")
    if not (LINUX_ROOT / f"fcitx5/{full['fcitx5_addon']}.conf").is_file():
        errors.append(f"edition full: platforms.linux.fcitx5_addon must name platforms/linux/fcitx5/{full['fcitx5_addon']}.conf")
    identifier = json.loads(TAURI_LINUX_CONF.read_text(encoding="utf-8")).get("identifier")
    if full["tauri_identifier"] != identifier:
        errors.append(f"edition full: platforms.linux.tauri_identifier must equal identifier {identifier!r} in {TAURI_LINUX_CONF.relative_to(ROOT)}")


def android_strings(path: pathlib.Path) -> dict[str, str]:
    """一个 Android values 资源文件里的字符串。"""
    root = ElementTree.parse(path).getroot()
    return {element.get("name"): element.text or "" for element in root.iter("string")}


def without_subtype_label(path: pathlib.Path) -> str:
    """method.xml 去掉注释和子类型标签之后的样子，用来和主资源比较。"""
    root = ElementTree.parse(path).getroot()
    for element in root.iter():
        element.text = None
        element.tail = None
    for subtype in root.iter("subtype"):
        subtype.attrib.pop("{http://schemas.android.com/apk/res/android}label", None)
    return ElementTree.tostring(root, encoding="unicode")


def check_android(errors: list[str], editions: list[dict]) -> None:
    """多个版本同时装在一台 Android 设备上：applicationId 一样，后装的就会被当成先装的那个的升级（或因签名不同装不上）；authority 一样，第二个版本根本装不上。"""
    sections = [(entry["id"], entry, entry["platforms"].get("android")) for entry in editions]
    sections = [(edition_id, entry, section) for edition_id, entry, section in sections if section is not None]
    for key in FULL_ANDROID:
        seen: dict[str, str] = {}
        for edition_id, _, section in sections:
            folded = section[key].casefold()
            if folded in seen:
                errors.append(f"editions {seen[folded]} and {edition_id}: platforms.android.{key} {section[key]!r} is not unique (case-insensitive)")
            seen[folded] = edition_id
    manifest = ANDROID_MANIFEST.read_text(encoding="utf-8")
    authorities = re.findall(r'android:authorities="([^"]*)"', manifest)
    if not authorities:
        errors.append(f"{ANDROID_MANIFEST.relative_to(ROOT)} declares no provider authority; update check_android in this script")
    for authority in authorities:
        if not authority.startswith("${applicationId}."):
            errors.append(f"{ANDROID_MANIFEST.relative_to(ROOT)}: authority {authority!r} must be written as ${{applicationId}}.<name>, or two editions installed together collide on it")
    resolved: dict[str, str] = {}
    for edition_id, _, section in sections:
        for authority in authorities:
            value = authority.replace("${applicationId}", section["application_id"]).casefold()
            if value in resolved:
                errors.append(f"edition {edition_id}: provider authority {value!r} is also edition {resolved[value]}'s")
            resolved[value] = edition_id
    main_strings = android_strings(ANDROID_ROOT / "res/values/strings.xml")
    main_method = without_subtype_label(ANDROID_ROOT / "res/xml/method.xml")
    for edition_id, entry, section in sections:
        name = entry["display_name"]["zh-Hans"]
        if edition_id == FULL:
            if section != FULL_ANDROID:
                errors.append(f"edition full: platforms.android must be the identifiers the product ships with today: {FULL_ANDROID}")
            gradle = re.search(r'^\s*applicationId = "([^"]+)"', ANDROID_GRADLE.read_text(encoding="utf-8"), re.MULTILINE)
            if gradle is None or gradle.group(1) != section["application_id"]:
                errors.append(f"edition full: platforms.android.application_id must equal applicationId in {ANDROID_GRADLE.relative_to(ROOT)}")
            identifier = json.loads(TAURI_ANDROID_CONF.read_text(encoding="utf-8")).get("identifier")
            if section["application_id"] != identifier:
                errors.append(f"edition full: platforms.android.application_id must equal identifier {identifier!r} in {TAURI_ANDROID_CONF.relative_to(ROOT)}")
            if main_strings.get("app_name") != name:
                errors.append(f"edition full: app_name in platforms/android/res/values/strings.xml must be {name!r}")
            continue
        expected = {"application_id": f"{FULL_ANDROID['application_id']}.{edition_id}", "apk_name": f"{FULL_ANDROID['apk_name']}-{edition_id}"}
        for key, value in expected.items():
            if section[key] != value:
                errors.append(f"edition {edition_id}: platforms.android.{key} must be {value!r}, found {section[key]!r}")
        resources = ANDROID_ROOT / "editions" / edition_id / "res"
        strings_path = resources / "values/strings.xml"
        method_path = resources / "xml/method.xml"
        if not strings_path.is_file() or not method_path.is_file():
            errors.append(f"edition {edition_id}: {resources.relative_to(ROOT)} must hold values/strings.xml and xml/method.xml")
            continue
        strings = android_strings(strings_path)
        if strings.get("app_name") != name:
            errors.append(f"edition {edition_id}: app_name in {strings_path.relative_to(ROOT)} must be its display_name.zh-Hans {name!r}")
        for key, value in strings.items():
            if key == "app_name":
                continue
            if key not in main_strings:
                errors.append(f"edition {edition_id}: {strings_path.relative_to(ROOT)} overrides {key!r}, which the main resources do not define")
            elif value != main_strings[key].replace(main_strings["app_name"], name) or value == main_strings[key]:
                errors.append(f"edition {edition_id}: {key} in {strings_path.relative_to(ROOT)} must be the main string with only the product name replaced")
        for key, value in main_strings.items():
            if key != "app_name" and main_strings["app_name"] in value and key not in strings:
                errors.append(f"edition {edition_id}: {key} names {main_strings['app_name']!r} in the main resources; override it in {strings_path.relative_to(ROOT)}")
        if without_subtype_label(method_path) != main_method:
            errors.append(f"edition {edition_id}: {method_path.relative_to(ROOT)} must equal res/xml/method.xml apart from the subtype label")
        method = method_path.read_text(encoding="utf-8")
        if 'android:label="@string/app_name"' not in method:
            errors.append(f"edition {edition_id}: {method_path.relative_to(ROOT)} must label its subtype with @string/app_name")


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
    if not errors:
        check_generated_locks(errors, table)

    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"editions: {len(table['editions'])} editions agree with the schema, the engine schemes, the resource locks and the frozen baseline, and the generated locks are current")
    return 0


if __name__ == "__main__":
    sys.exit(main())
