#!/usr/bin/env python3
"""Check that the licences of the Cantonese, Zhuyin and Stroke data travel through every platform's notice channel.

The Cantonese (Jyutping) and Zhuyin (Dachen) schemes take their syllables and words from rime-cantonese (CC BY 4.0) and libchewing-data (LGPL-2.1-or-later), and the Stroke scheme takes its stroke orders from rime-stroke (LGPL-3.0, whose main table also requires the CNS11643 attribution). CC BY 4.0 requires the attribution and a note of the changes to travel with the adapted data, and the LGPL requires the licence text, the copyright notice and a pointer to the source. Every host offers these schemes and ships their dictionaries, each beside the resources with its licence text in the same directory, and the scheme code is in the engine every platform ships, so every platform's notice channel carries every text, the way the libhangul Hanja table's does (scripts/test-korean-hanja-table.py), and one channel list keeps this check simple.

许可证文件写明它覆盖的上游提交。粤拼与注音的数据由 msime-dictionary 原样收在 `yue/`、`tw/` 下，resources/dictionary-sources.lock.json 从它的 `sources-v*` release 附件固定这些文件，并用 `rime-cantonese`、`libchewing-data` 两个引用记下上游提交；笔画的 `stroke/` 等 msime-dictionary 发布后以同样方式固定，引用名 `rime-stroke`。引用必须是许可证文件覆盖的那个提交，所以换了上游提交却忘了改许可证会在这里失败。锁文件还没有固定的来源打印一行 skip；这时 dict-builder 的 `stroke.rs` 记下的提交必须就是许可证覆盖的提交。
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/dictionary-sources.lock.json"
# 粤拼、注音与笔画的源文件只从 msime-dictionary 的 sources release 附件取用。
DICTIONARY_RELEASES = "https://github.com/metasequoiaime/msime-dictionary/releases/download/sources-v"
# 每个许可证文件对应的上游仓库、它覆盖的提交、锁文件里这份数据所在的目录，以及它必须保留的段落：署名或版权行、许可证正文、改动说明或源码地址、不使用的文件。
LICENCES = {
    "resources/licenses/rime-cantonese-CC-BY-4.0.txt": (
        "rime/rime-cantonese",
        "259f0e48bba840c3a2e0d117539e96937f3d89bc",
        "yue/",
        ("CanCLID", "Linguistic Society of Hong Kong", "Attribution 4.0 International", "tone digits are removed", "jyut6ping3.maps.dict.yaml (released under the Open Data Commons Open Database License 1.0)", "jyut6ping3.phrase.dict.yaml"),
    ),
    "resources/licenses/libchewing-data-LGPL-2.1.txt": (
        "chewing/libchewing-data",
        "c44e81aef24b06f1509f19e1be54c99812d0c43f",
        "tw/",
        ("Copyright (c) 2025 libchewing Core Team", "GNU LESSER GENERAL PUBLIC LICENSE", "Version 2.1, February 1999", "https://github.com/chewing/libchewing-data/tree/c44e81aef24b06f1509f19e1be54c99812d0c43f/dict/chewing", "END OF TERMS AND CONDITIONS"),
    ),
    "resources/licenses/rime-stroke-LGPL-3.0.txt": (
        "rime/rime-stroke",
        "1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48",
        "stroke/",
        ("四季的風", "Kunki Chou", "宋天", "數位發展部，CNS11643中文標準交換碼全字庫網站，https://www.cns11643.gov.tw", "北大中文論壇", "cn/SingleCharsAllV1.txt", "GNU LESSER GENERAL PUBLIC LICENSE", "Version 3, 29 June 2007", "https://github.com/rime/rime-stroke/tree/1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48", "https://www.gnu.org/licenses/gpl-3.0.txt"),
    ),
}
# 锁文件固定之前，dict-builder 按自己记下的上游提交校验缓存里的源文件；那个提交也必须是许可证覆盖的提交。
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

        if repository in BUILDER_COMMITS:
            source, declaration = BUILDER_COMMITS[repository]
            check(declaration.format(commit=commit) in (ROOT / source).read_text(encoding="utf-8"), f"{source} does not build from {repository} at {commit}, the commit {relative} covers")

        references = [entry["commit"] for entry in lock["references"].values() if repository in entry["repository"]]
        files = [entry["url"] for entry in lock["files"] if entry["path"].startswith(directory)]
        if not references and not files:
            print(f"skipped: lock pin, resources/dictionary-sources.lock.json does not pin {repository} yet")
            continue
        check(references == [commit], f"the sources lock references {repository} at {references}, the notices cover {commit}; update {relative} and the channels together with the pin")
        check(bool(files), f"the sources lock references {repository} but pins no file under {directory}")
        for url in files:
            check(url.startswith(DICTIONARY_RELEASES), f"the sources lock pins {url} under {directory}, which is not a msime-dictionary sources release asset")
        check(not any(f"/{repository}/" in entry["url"] for entry in lock["files"]), f"the sources lock still downloads from {repository} directly; pin msime-dictionary's {directory} assets instead")

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
