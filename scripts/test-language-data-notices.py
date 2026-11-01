#!/usr/bin/env python3
"""Check that the licences of the Cantonese, Zhuyin and Stroke data travel through every platform's notice channel.

The Cantonese (Jyutping) and Zhuyin (Dachen) schemes take their syllables and words from rime-cantonese (CC BY 4.0), libchewing-data (LGPL-2.1-or-later) and the BSD-derived McBopomofo supplement, and their ranking weights partly from the HKCanCor corpus (CC BY 4.0, attribution and citation required) and McBopomofo's phrase.occ (MIT, copyright and permission notice required), and the Stroke scheme takes its stroke orders from rime-stroke (LGPL-3.0, whose main table also requires the CNS11643 attribution). CC BY 4.0 requires the attribution and a note of the changes to travel with the adapted data, and the LGPL requires the licence text, the copyright notice and a pointer to the source. Every host offers these schemes and ships their dictionaries, each beside the resources with its licence text in the same directory, and the scheme code is in the engine every platform ships, so every platform's notice channel carries every text, the way the libhangul Hanja table's does (scripts/test-korean-hanja-table.py), and one channel list keeps this check simple.

许可证文件写明它覆盖的上游提交；resources/dictionary-sources.lock.json 的 `rime-cantonese`、`libchewing-data`、`rime-stroke` 引用必须等于这些提交，所以换了上游提交却忘了改许可证会在这里失败。数据本身由 msime-dictionary 原样收在 `sources/cantonese/`、`sources/zhuyin/`、`sources/stroke/` 下，并在它的 `upstream.lock.json` 里按字节固定，构建器读取 checkout 时核对记录里的提交等于这些引用；锁文件不再固定任何 msime-dictionary 文件。
"""
import json
import re
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
        ("CanCLID", "Linguistic Society of Hong Kong", "Attribution 4.0 International", "tone digits are removed", "jyut6ping3.maps.dict.yaml (released under the Open Data Commons Open Database License 1.0)", "jyut6ping3.phrase.dict.yaml", "Hong Kong Cantonese Corpus", "Luke Kang Kwong", "K. K. Luke and May L. Y. Wong (2015) The Hong Kong Cantonese Corpus: Design and Uses", "custom/cantonese.txt"),
    ),
    "resources/licenses/libchewing-data-LGPL-2.1.txt": (
        "chewing/libchewing-data",
        "c44e81aef24b06f1509f19e1be54c99812d0c43f",
        "sources/zhuyin/",
        ("Copyright (c) 2025 libchewing Core Team", "GNU LESSER GENERAL PUBLIC LICENSE", "Version 2.1, February 1999", "https://github.com/chewing/libchewing-data/tree/c44e81aef24b06f1509f19e1be54c99812d0c43f/dict/chewing", "END OF TERMS AND CONDITIONS", "Source/Data/phrase.occ", "Copyright (c) 2011-2026 Mengjuei Hsieh et al.", "Permission is hereby granted, free of charge", "The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.", "THE SOFTWARE IS PROVIDED \"AS IS\"", "Copyrighy (c) 1999 TaBE Project.", "Copyright (c) 1999 Computer Systems and Communication Lab,", "Redistributions in binary form must reproduce the above copyright", "Copyright 1996 Chih-Hao Tsai @ Beckman Institute, University of Illinois"),
    ),
    "resources/licenses/rime-stroke-LGPL-3.0.txt": (
        "rime/rime-stroke",
        "1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48",
        "sources/stroke/",
        ("四季的風", "Kunki Chou", "宋天", "數位發展部，CNS11643中文標準交換碼全字庫網站，https://www.cns11643.gov.tw", "北大中文論壇", "sources/pinyin/single-chars.txt", "GNU LESSER GENERAL PUBLIC LICENSE", "Version 3, 29 June 2007", "https://github.com/rime/rime-stroke/tree/1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48", "https://www.gnu.org/licenses/gpl-3.0.txt", "Creator: amzxyz", "https://github.com/amzxyz/rime-wanxiang), release v18.0.16, commit 7ab73ee0c7f80190004233e70f954833df64c80e", "https://creativecommons.org/licenses/by/4.0/", "round(rime-wanxiang weight × 9.42)", "disclaimer of warranties", "rime-wanxiang-CC-BY-4.0.txt"),
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
# 随包词库里的 MIT 与 Apache-2.0 数据（docs/third-party.md「词库里要随附全文的 MIT 与 Apache-2.0 数据」）：msime-pinyin.db 的单字读音按 pinyin-data 校正，msime-english.db 的中英释义取自 ECDICT，msime-others.db 的颜文字取自 rime_kaomoji_dict、关键词整词读音取自 python-pinyin，msime-wubi.db 的 86 码表取自 rime-wubi86-jidian；以及 CC BY 4.0 的 rime-wanxiang 权重（「词库里改编自 rime-wanxiang 的权重」）：msime-pinyin.db 与 msime-stroke.db 里与万象共有的词条和单字。这些许可都要求随副本附上全文或署名，词库在每个平台上分发，所以每个渠道都要带上；每份文本要保留的版权行或条款段落写在这里，换了文件内容会在这里失败。
DATA_LICENCES = {
    "resources/licenses/pinyin-data-MIT.txt": ("Copyright (c) 2016 mozillazg", "The above copyright notice and this permission notice shall be included in all"),
    "resources/licenses/ECDICT-MIT.txt": ("Copyright (c) 2025 Linwei", "The above copyright notice and this permission notice shall be included in all"),
    "resources/licenses/rime_kaomoji_dict-MIT.txt": ("Copyright (c) 2023 aoguai", "The above copyright notice and this permission notice shall be included in all"),
    "resources/licenses/python-pinyin-MIT.txt": ("Copyright (c) 2016 mozillazg, 闲耘 <hotoo.cn@gmail.com>", "The above copyright notice and this permission notice shall be included in all"),
    "resources/licenses/rime-wubi86-jidian-Apache-2.0.txt": ("Version 2.0, January 2004", "4. Redistribution.", "END OF TERMS AND CONDITIONS"),
    # CC BY 4.0 第 3(a) 条要的署名、改动说明、许可证地址与全文（含第 5 节免责声明），以及权重进入的两个产物。
    "resources/licenses/rime-wanxiang-CC-BY-4.0.txt": ("https://github.com/amzxyz/rime-wanxiang), release v18.0.16, commit {WANXIANG_COMMIT}", "Creator: amzxyz", "round(rime-wanxiang weight × factor)", "msime-pinyin.db", "msime-stroke.db", "https://creativecommons.org/licenses/by/4.0/", "Attribution 4.0 International", "Section 5 -- Disclaimer of Warranties and Limitation of Liability."),
}
DATA_CHANNELS = tuple(NOTICE_CHANNELS) + ("platforms/windows/Notices.md", "platforms/ios/App/Sources/settings/AboutAndDownloadViews.swift")
# 网页引擎只分发拼音库和五笔 86 库（不分发 msime-english.db、msime-others.db），引擎 C 库的数据包只带拼音库和英文词库。
# rime-wanxiang 的提交写在署名文件、三份总览和 docs/third-party.md 里，换了提交却忘了改其中一处会失败。
WANXIANG_COMMIT = "7ab73ee0c7f80190004233e70f954833df64c80e"
WANXIANG_LICENCE = "resources/licenses/rime-wanxiang-CC-BY-4.0.txt"
# msime-engine-dictionary-NOTICE.md 的前半部分逐字取自 msime-dictionary 某个提交的 NOTICE.md，开头写明是哪个 dict-v 发布的源数据提交；词库锁换了发布却没有重新同步，会在这里失败。
DESKTOP_LOCK = ROOT / "resources/desktop-dictionary.lock.json"
# 词库来源说明本身也要随每个分发桌面词库的渠道走：它是 msime-bigram.bin、msime-trigram.bin 要求的中文维基百科（CC BY-SA 4.0）署名和 msime-wubi.db 各份码表出处唯一写着的地方。六个平台都随包或经资源包分发这些词库（Android 的 assets/dictionary 与五笔、日文资源包，iOS 键盘扩展的 EngineResources，HarmonyOS 的 resfile/engine），C 库数据包带拼音库和英文词库。iOS 关于页按 md 扩展名列出它。
ENGINE_NOTICE_CHANNELS = ("platforms/windows/Collect-Notices.ps1", "platforms/macos/CMakeLists.txt", "platforms/linux/CMakeLists.txt", "platforms/android/build-native.sh", "platforms/ios/project.yml", "platforms/ios/tests/settings/ProjectConfigurationTests.py", "platforms/ios/App/Sources/settings/AboutAndDownloadViews.swift", "platforms/harmony/stage-resources.sh", "crates/engine-capi/packaging/data-pack.py")
ENGINE_NOTICE_MARKERS = ("### 拼音权重与万象对齐", "## msime 构建时另加的输入", "### 整句词格的上下文表", "Copyright 2000-2026 by Kevin Atkinson", "中文维基百科")
# 水杉输入法本身以 GPL-3.0-only 发布（仓库根目录的 LICENSE），msime-pinyin.db 含 rime-ice 的 GPL-3.0 数据，输入引擎（crates/engine、crates/path-trust）是 MPL-2.0。三个移动端都把引擎编进安装包、随包分发拼音库，GPL-3.0 第 4、6 条要求随目标代码附上许可证全文，引擎的 MPL-2.0 全文也随每份引擎构建分发，所以每个移动端渠道都要带上两份全文，关于页列出它们。各渠道给 GPL-3.0 全文起的名字不同（Android 的 client-LICENSE.txt，iOS bundle 里原样的 LICENSE，HarmonyOS 的 GPL-3.0.txt），这里逐个写出要在非注释行出现的片段。MPL-2.0 全文在所有渠道里的固定由 scripts/test-engine-license.py 负责，这里只把移动端的一并写出，让三端的两份全文在同一处可查。
PROJECT_LICENCE = "LICENSE"
PROJECT_LICENCE_MARKERS = ("GNU GENERAL PUBLIC LICENSE", "Version 3, 29 June 2007", "END OF TERMS AND CONDITIONS")
ENGINE_LICENCE = "resources/licenses/MPL-2.0.txt"
ENGINE_LICENCE_MARKERS = ("Mozilla Public License Version 2.0", "Exhibit A - Source Code Form License Notice")
MOBILE_LICENCE_CHANNELS = {
    "platforms/android/build-apk.sh": ('cp LICENSE "$assets/client-LICENSE.txt"',),
    "platforms/android/build-client-apk.sh": ('cp LICENSE "$assets/client-LICENSE.txt"',),
    "platforms/android/build-native.sh": ('cp resources/licenses/MPL-2.0.txt "$notices/MPL-2.0.txt"',),
    "platforms/android/java/app/msime/android/home/AboutPage.java": ('"client-LICENSE.txt"', 'assets.list("native-notices")'),
    "platforms/ios/project.yml": ("- path: ../../LICENSE", "- path: ../../resources/licenses/MPL-2.0.txt"),
    "platforms/ios/tests/settings/ProjectConfigurationTests.py": ('"../../LICENSE"', '"MPL-2.0.txt"'),
    "platforms/ios/App/Sources/settings/AboutAndDownloadViews.swift": ('LicenseNotice(file: "LICENSE"', 'LicenseNotice(file: "MPL-2.0"'),
    "platforms/harmony/stage-resources.sh": ('cp LICENSE "$licenses_staged/GPL-3.0.txt"', 'cp resources/licenses/MPL-2.0.txt "$licenses_staged/"'),
}
WEB_DATA_CHANNELS = ("scripts/web-engine-notice.sh", "resources/licenses/web-engine-NOTICE.md")
DATA_PACK_CHANNEL = "crates/engine-capi/packaging/data-pack.py"
PARTIAL_DATA_CHANNELS = {
    "resources/licenses/pinyin-data-MIT.txt": WEB_DATA_CHANNELS + (DATA_PACK_CHANNEL,),
    "resources/licenses/ECDICT-MIT.txt": (DATA_PACK_CHANNEL,),
    "resources/licenses/rime-wubi86-jidian-Apache-2.0.txt": WEB_DATA_CHANNELS,
    "resources/licenses/rime-wanxiang-CC-BY-4.0.txt": WEB_DATA_CHANNELS + (DATA_PACK_CHANNEL, "crates/engine-capi/packaging/data-pack-README.md"),
}
failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


# 法日西俄德韩释义行的读音表（scripts/build_target_pronunciations.py）改编自 CC BY-SA 4.0 的维基词典，NOTICE 必须随数据库一起走。它不进上面那些按平台的许可证渠道，而是和英文音标表、离线释义一样放在数据库旁边：资源包的文件清单就是锁的 `built`，随包分发的三个打包脚本各自只连同 NOTICE 一起暂存数据库。这里核对锁里有 NOTICE、生成器写的 NOTICE 带着署名、许可证和改动说明，以及每个打包脚本暂存数据库的地方都带上了 NOTICE。
TARGET_PRONUNCIATIONS_LOCK = "resources/target-pronunciations.lock.json"
TARGET_PRONUNCIATIONS_NOTICE = "target-pronunciations-NOTICE.txt"
TARGET_PRONUNCIATIONS_NOTICE_MARKERS = (
    "English Wiktionary (https://en.wiktionary.org/), by Wiktionary contributors",
    "Wiktextract (https://github.com/tatuylonen/wiktextract)",
    "kaikki.org",
    "Wiktextract dump date: {dump_date}",
    "Wiktextract revision: {source_revision}",
    "Changes:",
    "Creative Commons Attribution-ShareAlike 4.0 International License (https://creativecommons.org/licenses/by-sa/4.0/)",
)
# 随包带着外语读音表的打包脚本：每个都要在暂存数据库的同一处带上 NOTICE。
TARGET_PRONUNCIATIONS_CHANNELS = (
    "platforms/macos/stage-resources.sh",
    "platforms/harmony/stage-resources.sh",
    "platforms/windows/installer/Prepare-PackageFiles.ps1",
)


def check_target_pronunciation_notice() -> None:
    lock = json.loads((ROOT / TARGET_PRONUNCIATIONS_LOCK).read_text(encoding="utf-8"))
    names = [artifact["name"] for artifact in lock.get("built") or []]
    check(TARGET_PRONUNCIATIONS_NOTICE in names, f"{TARGET_PRONUNCIATIONS_LOCK} does not pin {TARGET_PRONUNCIATIONS_NOTICE}, so the resource pack would install the databases without it")
    check(any(name.endswith("-phonetic.db") for name in names), f"{TARGET_PRONUNCIATIONS_LOCK} pins no *-phonetic.db")
    check(lock.get("license") == "CC-BY-SA-4.0", f"{TARGET_PRONUNCIATIONS_LOCK} no longer records CC-BY-SA-4.0")
    generator = (ROOT / "scripts/build_target_pronunciations.py").read_text(encoding="utf-8")
    for marker in TARGET_PRONUNCIATIONS_NOTICE_MARKERS:
        check(marker in generator, f"scripts/build_target_pronunciations.py writes a NOTICE without {marker!r}")
    for channel in TARGET_PRONUNCIATIONS_CHANNELS:
        live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
        staging = [line for line in live if "phonetic.db" in line and ("cp " in line or "Database" in line)]
        check(bool(staging), f"{channel} no longer stages the target pronunciation tables")
        for line in staging:
            check(TARGET_PRONUNCIATIONS_NOTICE in line or "en-phonetic.db" in line, f"{channel} stages the target pronunciation tables without {TARGET_PRONUNCIATIONS_NOTICE}: {line.strip()[:120]}")


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

    for relative, phrases in DATA_LICENCES.items():
        licence = ROOT / relative
        if not licence.is_file():
            failures.append(f"{relative} is missing")
            continue
        text = licence.read_text(encoding="utf-8")
        for phrase in phrases:
            phrase = phrase.format(WANXIANG_COMMIT=WANXIANG_COMMIT)
            check(phrase in text, f"{relative} no longer contains {phrase!r}")
        stem = licence.name.removesuffix(".txt")
        for channel in DATA_CHANNELS + PARTIAL_DATA_CHANNELS.get(relative, ()):
            live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
            check(any(licence.name in line or f'"{stem}"' in line for line in live), f"{channel} does not ship {licence.name}")
    # ECDICT 的文件由锁文件固定，两份总览写的是它的提交，换了提交却忘了改声明会在这里失败。
    ecdict = lock["references"]["ECDICT"]["commit"]
    check(any(entry["path"] == "ecdict/ecdict.csv" and f"/{ecdict}/" in entry["url"] for entry in lock["files"]), f"the sources lock does not download ecdict.csv at the ECDICT reference {ecdict}")
    for overview in OVERVIEWS:
        check(ecdict in (ROOT / overview).read_text(encoding="utf-8"), f"{overview} does not name ECDICT commit {ecdict}")
        check(WANXIANG_COMMIT in (ROOT / overview).read_text(encoding="utf-8"), f"{overview} does not name rime-wanxiang commit {WANXIANG_COMMIT}, the commit {WANXIANG_LICENCE} covers")
    # 副本开头写的 dict-v 发布必须是词库锁下载的那一个，且副本保留万象权重的记录和 msime 一侧另加的来源。
    releases = {re.search(r"/releases/download/(dict-v[0-9.]+)/", artifact["url"]).group(1) for artifact in json.loads(DESKTOP_LOCK.read_text(encoding="utf-8"))["artifacts"] if "/msime-dictionary/releases/" in artifact["url"]}
    header = engine_notice.split("\n", 1)[0]
    check(len(releases) == 1 and f"`{next(iter(releases))}`" in header, f"{ENGINE_NOTICE} opens with {header[:80]!r}, which does not name the release the desktop dictionary lock pins ({sorted(releases)}); re-sync it from that release's source data commit")
    for marker in ENGINE_NOTICE_MARKERS:
        check(marker in engine_notice, f"{ENGINE_NOTICE} no longer contains {marker!r}")
    notice_name = Path(ENGINE_NOTICE).name
    notice_stem = notice_name.removesuffix(".md")
    for channel in ENGINE_NOTICE_CHANNELS:
        live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
        check(any(notice_name in line or f'"{notice_stem}"' in line for line in live), f"{channel} does not ship {notice_name}")
    for relative, markers in ((PROJECT_LICENCE, PROJECT_LICENCE_MARKERS), (ENGINE_LICENCE, ENGINE_LICENCE_MARKERS)):
        text = (ROOT / relative).read_text(encoding="utf-8")
        for marker in markers:
            check(marker in text, f"{relative} no longer contains {marker!r}")
    for channel, fragments in MOBILE_LICENCE_CHANNELS.items():
        live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith(("#", "//"))]
        for fragment in fragments:
            # project.yml 的 `- path:` 条目要整行相等，`../../LICENSE` 才不会被 `../../LICENSE.md` 之类的路径冒充。
            exact = fragment.startswith("- path: ")
            check(any(fragment == line.strip() if exact else fragment in line for line in live), f"{channel} does not ship the GPL-3.0 or MPL-2.0 text: no live line contains {fragment!r}")

    check_target_pronunciation_notice()

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"language data notices: {len(LICENCES)} licences in {len(NOTICE_CHANNELS)} channels, {len(DATA_LICENCES)} dictionary data licences in {len(DATA_CHANNELS)} channels, the dictionary sources notice in {len(ENGINE_NOTICE_CHANNELS)} channels, the GPL-3.0 and MPL-2.0 texts in {len(MOBILE_LICENCE_CHANNELS)} mobile channels, vi in {len(VI_CHANNELS)} channels, ewts in {len(EWTS_CHANNELS)} channels")
    return 0


if __name__ == "__main__":
    sys.exit(main())
