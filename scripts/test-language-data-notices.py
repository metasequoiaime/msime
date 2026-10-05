#!/usr/bin/env python3
"""Check that the licences of the Cantonese, Zhuyin and Stroke data travel through every platform's notice channel.

The Cantonese (Jyutping) and Zhuyin (Dachen) schemes take their syllables and words from rime-cantonese (CC BY 4.0), libchewing-data (LGPL-2.1-or-later) and the BSD-derived McBopomofo supplement, and their ranking weights partly from the HKCanCor corpus (CC BY 4.0, attribution and citation required) and McBopomofo's phrase.occ (MIT, copyright and permission notice required), and the Stroke scheme takes its stroke orders from rime-stroke (LGPL-3.0, whose main table also requires the CNS11643 attribution). CC BY 4.0 requires the attribution and a note of the changes to travel with the adapted data, and the LGPL requires the licence text, the copyright notice and a pointer to the source. Every host offers these schemes and ships their dictionaries, each beside the resources with its licence text in the same directory, and the scheme code is in the engine every platform ships, so every platform's notice channel carries every text, the way the libhangul Hanja table's does (scripts/test-korean-hanja-table.py), and one channel list keeps this check simple.

许可证文件写明它覆盖的上游提交；resources/dictionary-sources.lock.json 的 `rime-cantonese`、`libchewing-data`、`rime-stroke` 引用必须等于这些提交，所以换了上游提交却忘了改许可证会在这里失败。数据本身由 msime-dictionary 原样收在 `sources/cantonese/`、`sources/zhuyin/`、`sources/stroke/` 下，并在它的 `upstream.lock.json` 里按字节固定，构建器读取 checkout 时核对记录里的提交等于这些引用；锁文件不再固定任何 msime-dictionary 文件。
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/dictionary-sources.lock.json"
# 锁文件曾用这个前缀固定 msime-dictionary 的文件；现在检查它不再出现。
DICTIONARY_RAW = "https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/"
# 每个许可证文件对应的上游仓库、它覆盖的提交、这份数据在 msime-dictionary 里的目录，以及它必须保留的段落：署名或版权行、许可证正文、改动说明或源码地址、不使用的文件。
LICENCES = {
    "resources/licenses/rime-cantonese-CC-BY-4.0.txt": (
        "rime/rime-cantonese",
        "259f0e48bba840c3a2e0d117539e96937f3d89bc",
        "sources/cantonese/",
        ("CanCLID", "Linguistic Society of Hong Kong", "Attribution 4.0 International", "tone digits are removed", "jyut6ping3.maps.dict.yaml (released under the Open Data Commons Open Database License 1.0)", "jyut6ping3.phrase.dict.yaml", "Hong Kong Cantonese Corpus", "Luke Kang Kwong", "K. K. Luke and May L. Y. Wong (2015) The Hong Kong Cantonese Corpus: Design and Uses"),
    ),
    "resources/licenses/libchewing-data-LGPL-2.1.txt": (
        "chewing/libchewing-data",
        "c44e81aef24b06f1509f19e1be54c99812d0c43f",
        "sources/zhuyin/",
        ("Copyright (c) 2025 libchewing Core Team", "GNU LESSER GENERAL PUBLIC LICENSE", "Version 2.1, February 1999", "https://github.com/chewing/libchewing-data/tree/c44e81aef24b06f1509f19e1be54c99812d0c43f/dict/chewing", "END OF TERMS AND CONDITIONS", "Source/Data/phrase.occ", "Copyright (c) 2011-2026 Mengjuei Hsieh et al.", "Permission is hereby granted, free of charge", "The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.", "THE SOFTWARE IS PROVIDED \"AS IS\""),
    ),
    "resources/licenses/rime-stroke-LGPL-3.0.txt": (
        "rime/rime-stroke",
        "1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48",
        "sources/stroke/",
        ("四季的風", "Kunki Chou", "宋天", "數位發展部，CNS11643中文標準交換碼全字庫網站，https://www.cns11643.gov.tw", "北大中文論壇", "sources/pinyin/single-chars.txt", "GNU LESSER GENERAL PUBLIC LICENSE", "Version 3, 29 June 2007", "https://github.com/rime/rime-stroke/tree/1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48", "https://www.gnu.org/licenses/gpl-3.0.txt"),
    ),
}
# 许可证文件还覆盖的其他上游：锁文件里的引用名，以及许可证文件写出那个引用的提交时用的文字。HKCanCor 的词频随粤拼词库分发，McBopomofo 的补充表与 phrase.occ 随注音词库分发，换了它们的提交却忘了改许可证会在这里失败。
SECONDARY_REFERENCES = {
    "resources/licenses/rime-cantonese-CC-BY-4.0.txt": (("hkcancor", "https://github.com/fcbond/hkcancor, commit {commit}"),),
    "resources/licenses/libchewing-data-LGPL-2.1.txt": (("McBopomofo", "openvanilla/McBopomofo, commit {commit}"), ("McBopomofo", "https://github.com/openvanilla/McBopomofo/tree/{commit}/Source/Data")),
}
# 锁文件没有 rime-stroke 引用时，stroke.rs 用 COMMIT 兜底，它也必须是许可证覆盖的提交。
BUILDER_COMMITS = {"rime/rime-stroke": ("crates/dict-builder/src/stroke.rs", 'pub const COMMIT: &str = "{commit}";')}
# Every channel has to name each licence file on a live (non-comment) line.
NOTICE_CHANNELS = {
    "platforms/windows/Collect-Notices.ps1": "Windows: the notice collection the installer ships",
    "platforms/windows/tests/tools/collect_notices.ps1": "Windows: the collection test's synthetic notice list",
    "platforms/macos/CMakeLists.txt": "macOS: the input method bundle's Resources/Licenses",
    "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt": "macOS: the notice overview",
    "platforms/macos/tests/settings/bundle_contents.py": "macOS: the built bundle check",
    "platforms/linux/CMakeLists.txt": "Linux: the installed notices",
    "platforms/linux/data/THIRD_PARTY_NOTICES.txt": "Linux: the notice overview",
    "platforms/android/build-native.sh": "Android: the native notices both APKs package as assets/native-notices",
    "platforms/ios/project.yml": "iOS: the app's bundled resources",
    "platforms/ios/tests/settings/ProjectConfigurationTests.py": "iOS: the project configuration test",
    "platforms/harmony/stage-resources.sh": "HarmonyOS: the licences staged into the HAP",
    "docs/third-party.md": "the repository's third-party index",
}
# The overviews say what each text covers, so they also have to name the pinned commit.
OVERVIEWS = ("platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt", "platforms/linux/data/THIRD_PARTY_NOTICES.txt", "platforms/windows/Collect-Notices.ps1", "docs/third-party.md")
# 主词库的上游说明：这四个 reference 的提交必须写在里面，换了提交却忘了改说明会在这里失败。
ENGINE_NOTICE = "resources/licenses/msime-engine-dictionary-NOTICE.md"
ENGINE_NOTICE_REFERENCES = ("rime-ice-supplement", "SCOWL", "98wubi-tables", "fcitx5-table-extra")
# The vi crate behind Vietnamese mode is MIT. The macOS bundle and the Windows package, where Vietnamese ships and notices are listed by hand, carry its text explicitly.
VI_LICENCE = "resources/licenses/vi-MIT.txt"
VI_CHANNELS = ("platforms/macos/CMakeLists.txt", "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt", "platforms/macos/tests/settings/bundle_contents.py", "platforms/windows/Collect-Notices.ps1", "platforms/windows/tests/tools/collect_notices.ps1")
# 移动端（Android、iOS、HarmonyOS）的 Engine 同样编入 vi 和 ewts，它们的打包脚本也要带上两份许可证全文。
MOBILE_CRATE_CHANNELS = ("platforms/android/build-native.sh", "platforms/ios/project.yml", "platforms/ios/tests/settings/ProjectConfigurationTests.py", "platforms/harmony/stage-resources.sh")
VI_CHANNELS = VI_CHANNELS + MOBILE_CRATE_CHANNELS
# 藏文方案背后的 ewts crate 以 MIT OR Apache-2.0 发布，这里按 MIT 使用；它的转换表取自 Apache-2.0 的 ewts-js，所以同一个文件里还附着 ewts-js 的版权行和 Apache-2.0 全文。和 vi 一样，由手工列出声明的 macOS 包、Windows 安装包和移动端打包脚本显式带上它的许可证全文。
EWTS_LICENCE = "resources/licenses/ewts-MIT.txt"
EWTS_CHANNELS = VI_CHANNELS + ("platforms/windows/Notices.md", "docs/third-party.md")
failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def main() -> int:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    for relative, (repository, commit, directory, phrases) in LICENCES.items():
        licence = ROOT / relative
        if not licence.is_file():
            failures.append(f"{relative} is missing")
            continue
        text = licence.read_text(encoding="utf-8")
        check(f"https://github.com/{repository}), commit {commit}" in text.splitlines()[0], f"{relative} does not open with {repository} at {commit}")
        for phrase in phrases:
            check(phrase in text, f"{relative} no longer contains {phrase!r}")
        for channel, description in NOTICE_CHANNELS.items():
            live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
            check(any(licence.name in line for line in live), f"{channel} ({description}) does not ship {licence.name}")
        for overview in OVERVIEWS:
            check(commit in (ROOT / overview).read_text(encoding="utf-8"), f"{overview} does not name {repository} commit {commit}")

        for reference, wording in SECONDARY_REFERENCES.get(relative, ()):
            pinned = lock["references"].get(reference, {}).get("commit")
            check(pinned is not None and wording.format(commit=pinned) in text, f"{relative} does not cover {reference} at the sources lock's commit {pinned}")

        if repository in BUILDER_COMMITS:
            source, declaration = BUILDER_COMMITS[repository]
            check(declaration.format(commit=commit) in (ROOT / source).read_text(encoding="utf-8"), f"{source} does not build from {repository} at {commit}, the commit {relative} covers")

        references = [entry["commit"] for entry in lock["references"].values() if repository in entry["repository"]]
        check(references == [commit], f"the sources lock references {repository} at {references}, the notices cover {commit}; update {relative} and the channels together with the pin")
        check(not any(entry["path"].startswith(directory) for entry in lock["files"]), f"the sources lock pins files under {directory}; msime-dictionary data reaches msime only through dict-v release assets, and the builder reads it from --dictionary")
        check(not any(f"/{repository}/" in entry["url"] for entry in lock["files"]), f"the sources lock still downloads from {repository} directly; the data is msime-dictionary's {directory}")

    check("msime-dictionary" not in lock["references"] and not any(entry["url"].startswith(DICTIONARY_RAW) for entry in lock["files"]), "resources/dictionary-sources.lock.json pins msime-dictionary; msime takes that repository's data only from its dict-v release assets")
    engine_notice = (ROOT / ENGINE_NOTICE).read_text(encoding="utf-8")
    for reference in ENGINE_NOTICE_REFERENCES:
        pinned = lock["references"].get(reference, {}).get("commit")
        check(pinned is not None and pinned in engine_notice, f"{ENGINE_NOTICE} does not name {reference} at the sources lock's commit {pinned}")

    vi_licence = ROOT / VI_LICENCE
    check(vi_licence.is_file() and "Copyright 2020, Hung Nguyen" in vi_licence.read_text(encoding="utf-8"), f"{VI_LICENCE} is missing or lost the vi copyright line")
    for channel in VI_CHANNELS:
        live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
        check(any("vi-MIT.txt" in line for line in live), f"{channel} does not ship vi-MIT.txt")
    ewts_licence = ROOT / EWTS_LICENCE
    check(ewts_licence.is_file() and "Copyright (c) Maxim Zommer" in ewts_licence.read_text(encoding="utf-8"), f"{EWTS_LICENCE} is missing or lost the ewts copyright line")
    # ewts 的转换表取自 Apache-2.0 的 ewts-js，Apache-2.0 要求随分发保留版权行并附许可证全文；只有 macOS 和 Linux 的声明另外写了出处，其余渠道只带这一个文件，所以两者都必须在文件里。
    ewts_text = ewts_licence.read_text(encoding="utf-8") if ewts_licence.is_file() else ""
    check("Copyright (C) 2010-2025 Roger Espel Llima" in ewts_text and "END OF TERMS AND CONDITIONS" in ewts_text, f"{EWTS_LICENCE} must carry the ewts-js copyright line and the full Apache License 2.0 text")
    for channel in EWTS_CHANNELS:
        live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
        check(any("ewts-MIT.txt" in line for line in live), f"{channel} does not ship ewts-MIT.txt")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"language data notices: {len(LICENCES)} licences in {len(NOTICE_CHANNELS)} channels, vi in {len(VI_CHANNELS)} channels, ewts in {len(EWTS_CHANNELS)} channels")
    return 0


if __name__ == "__main__":
    sys.exit(main())
