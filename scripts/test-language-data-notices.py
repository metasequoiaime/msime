#!/usr/bin/env python3
"""Check that the licences of the Cantonese and Zhuyin data travel through every platform's notice channel.

The Cantonese (Jyutping) and Zhuyin (Dachen) schemes take their syllables and words from rime-cantonese (CC BY 4.0) and libchewing-data (LGPL-2.1-or-later). CC BY 4.0 requires the attribution and a note of the changes to travel with the adapted data, and the LGPL requires the licence text, the copyright notice and a pointer to the source. Every host offers the two schemes and ships their dictionaries, each beside the resources with its licence text in the same directory, and the scheme code is in the engine every platform ships, so every platform's notice channel carries both texts, the way the libhangul Hanja table's does (scripts/test-korean-hanja-table.py), and one channel list keeps this check simple.

The licence files name the upstream commit they cover. When resources/dictionary-sources.lock.json pins a source, every pin of that repository has to be at the same commit, so a re-pin that forgets the notice fails here; a source the lock does not pin yet prints a skip line.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "resources/dictionary-sources.lock.json"
# Each licence file with the repository it covers, the commit it was written for, and phrases it must keep: the attribution or copyright line, the licence body, the changes or source pointer, and the exclusions.
LICENCES = {
    "resources/licenses/rime-cantonese-CC-BY-4.0.txt": (
        "rime/rime-cantonese",
        "259f0e48bba840c3a2e0d117539e96937f3d89bc",
        ("CanCLID", "Linguistic Society of Hong Kong", "Attribution 4.0 International", "tone digits are removed", "jyut6ping3.maps.dict.yaml (released under the Open Data Commons Open Database License 1.0)", "jyut6ping3.phrase.dict.yaml"),
    ),
    "resources/licenses/libchewing-data-LGPL-2.1.txt": (
        "chewing/libchewing-data",
        "c44e81aef24b06f1509f19e1be54c99812d0c43f",
        ("Copyright (c) 2025 libchewing Core Team", "GNU LESSER GENERAL PUBLIC LICENSE", "Version 2.1, February 1999", "https://github.com/chewing/libchewing-data/tree/c44e81aef24b06f1509f19e1be54c99812d0c43f/dict/chewing", "END OF TERMS AND CONDITIONS"),
    ),
}
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
failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def main() -> int:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    for relative, (repository, commit, phrases) in LICENCES.items():
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

        references = [entry["commit"] for entry in lock["references"].values() if repository in entry["repository"]]
        files = [entry["url"] for entry in lock["files"] if f"/{repository}/" in entry["url"]]
        if not references and not files:
            print(f"skipped: lock pin, resources/dictionary-sources.lock.json does not pin {repository} yet")
            continue
        check(references == [commit], f"the sources lock references {repository} at {references}, the notices cover {commit}; update {relative} and the channels together with the pin")
        check(bool(files), f"the sources lock references {repository} but pins no file from it")
        for url in files:
            check(f"/{commit}/" in url, f"the sources lock pins {url}, which is not at the commit {relative} covers ({commit})")

    vi_licence = ROOT / VI_LICENCE
    check(vi_licence.is_file() and "Copyright 2020, Hung Nguyen" in vi_licence.read_text(encoding="utf-8"), f"{VI_LICENCE} is missing or lost the vi copyright line")
    for channel in VI_CHANNELS:
        live = [line for line in (ROOT / channel).read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]
        check(any("vi-MIT.txt" in line for line in live), f"{channel} does not ship vi-MIT.txt")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print(f"language data notices: {len(LICENCES)} licences in {len(NOTICE_CHANNELS)} channels, vi in {len(VI_CHANNELS)} macOS and Windows channels")
    return 0


if __name__ == "__main__":
    sys.exit(main())
