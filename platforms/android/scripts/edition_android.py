#!/usr/bin/env python3
"""从版本表 `shared/contracts/editions.json` 读出各版本的 Android 身份，给打包脚本和发布 workflow 用。

多个版本可以同时装在一台 Android 设备上，彼此完全隔离：每个版本有自己的 applicationId（也就是包名、私有数据目录和输入法服务的组件包名）、自己的 ContentProvider authority（清单里写成 `${applicationId}.<名字>`）和自己的 APK 文件名。版本在 `platforms/android/gradle-app` 里是同名的 productFlavor，Gradle 直接读版本表；这里只服务不跑 Gradle 的那几步：选资源锁、选语言词库、给产出的 APK 命名。

full 是现有产品本身：applicationId 仍是 `app.msime.android`，APK 叫 `msime-android.apk`，资源锁仍是 `resources/desktop-dictionary.lock.json`（`scripts/test-editions.py` 检查）。

用法：

    edition_android.py editions                    # 有 Android 段的版本 id，逗号分隔，full 在最前
    edition_android.py field --edition ID KEY      # 打印 Android 段的字段，或 id、resource_lock、language_dictionaries（每行一个）、features.<功能>（true/false）
    edition_android.py download-guide --editions ID[,ID...] --download-base URL [--prerelease]  # 发布说明开头的下载指引
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
EDITIONS = ROOT / "shared/contracts/editions.json"
FULL = "full"


def android_editions() -> list[dict]:
    """有 Android 段的版本，顺序与版本表相同（full 在最前）。"""
    table = json.loads(EDITIONS.read_text(encoding="utf-8"))
    return [entry for entry in table["editions"] if entry["platforms"].get("android") is not None]


def edition_entry(edition_id: str) -> dict:
    for entry in android_editions():
        if entry["id"] == edition_id:
            return entry
    raise SystemExit(f"edition {edition_id!r} has no Android identifiers in {EDITIONS.relative_to(ROOT)}")


def field(entry: dict, key: str) -> str:
    if key == "id":
        return entry["id"]
    if key == "resource_lock":
        # full 的锁就是桌面词库锁本身，其他版本的锁由 scripts/editions.py gen-locks 生成。
        lock = "resources/desktop-dictionary.lock.json" if entry["id"] == FULL else f"resources/editions/{entry['id']}.lock.json"
        return str(ROOT / lock)
    if key == "language_dictionaries":
        return "\n".join(entry["language_dictionaries"])
    if key.startswith("features.") and key.split(".", 1)[1] in entry["features"]:
        # 打包脚本据 features.offline_glosses 决定带不带非英文离线释义：它们按中文候选查，不提供中文方案的版本（日文、越南文和藏文版）不带。
        return "true" if entry["features"][key.split(".", 1)[1]] else "false"
    section = entry["platforms"]["android"]
    if key not in section:
        raise SystemExit(f"unknown Android field {key!r}; expected one of id, resource_lock, language_dictionaries, {', '.join(section)}")
    return section[key]


def download_guide(edition_ids: list[str], download_base: str, prerelease: bool) -> str:
    """Android 发布说明开头的下载指引：每个版本一行，写明中文名和 APK，完整版排第一、标为推荐。版本发布页上有六个 APK，不说明的话来下载的人不知道该选哪个。名字和 APK 名都取自版本表，加一个版本不用改文案。"""
    if FULL not in edition_ids:
        raise SystemExit("the download guide recommends the full edition, which is not in --editions")
    ordered = [FULL] + [edition for edition in edition_ids if edition != FULL]
    entries = [edition_entry(edition_id) for edition_id in ordered]
    full = entries[0]
    full_apk = f"{full['platforms']['android']['apk_name']}.apk"
    # 首尾的注释在 GitHub 上不显示，标出这一段是生成的下载指引，读发布说明做更新日志的地方（官网下载页的更新记录）可以按它跳过；与 macOS 的 edition_bundle.py download-guide 用同一对标记。
    lines = [
        "<!-- msime-download-guide -->",
        "## 下载",
        "",
        f"大多数人下载第一行的**{full['display_name']['zh-Hans']}**（`{full_apk}`）就行：它是完整版，包含全部输入方案。"
        "其他版本只带其中一部分方案，包名各不相同，可以和完整版装在同一台手机上。安装包都只支持 64 位 ARM（arm64-v8a）的手机。",
        "",
        "| 版本 | 安装包 |",
        "| --- | --- |",
    ]
    for entry in entries:
        apk = f"{entry['platforms']['android']['apk_name']}.apk"
        name = entry["display_name"]["zh-Hans"]
        label = f"**{name}**（完整版，推荐）" if entry["id"] == FULL else name
        lines.append(f"| {label} | [{apk}]({download_base}{apk}) |")
    lines += [
        "",
        "下载后在手机上打开 APK 安装，再到系统设置的输入法列表里启用它。已经装好的应用会在应用内收到更新，不用再来这里下载。"
        "`SHA256SUMS` 是这些安装包的 SHA-256 校验值；每个安装包旁边的 `.apk.sha256` 是给旧版本的应用内更新用的，不用下载。",
    ]
    if prerelease:
        lines += ["", "这是预览版：应用内更新只在「预览版」通道提供它，「稳定版」通道不会收到。"]
    lines.append("<!-- /msime-download-guide -->")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("editions")
    lookup = commands.add_parser("field")
    lookup.add_argument("--edition", required=True)
    lookup.add_argument("key")
    guide = commands.add_parser("download-guide")
    guide.add_argument("--editions", required=True)
    guide.add_argument("--download-base", required=True)
    guide.add_argument("--prerelease", action="store_true")
    args = parser.parse_args()
    if args.command == "editions":
        print(",".join(entry["id"] for entry in android_editions()))
    elif args.command == "download-guide":
        sys.stdout.write(download_guide(args.editions.split(","), args.download_base, args.prerelease))
    else:
        value = field(edition_entry(args.edition), args.key)
        if value:
            print(value)
    return 0


if __name__ == "__main__":
    sys.exit(main())
