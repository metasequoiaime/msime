#!/usr/bin/env python3
"""Check that the licence, upstream NOTICE and corpus attribution of the keystroke rerank models travel with them on every platform that packages them.

The two models of resources/rerank-model.lock.json (rerank-v1 of chinese-ime-lm, 8x448 and 6x256) are Apache-2.0. Section 4(a) requires a copy of the licence with every redistribution and 4(d) the upstream NOTICE; the training corpora (the Chinese portions of C4 and FineWeb-2 under ODC-BY, LCCC under MIT) require their attribution to travel with the weights. resources/licenses/rerank-model-Apache-2.0.txt carries all three and is the one file every platform ships. A platform that drops it still builds and still ships the models, so only reading the channels finds that out.

The trigger is the platform's packaging: once one of its live (non-comment) lines fetches the models (fetch_rerank_model.py) or reads their lock (rerank-model.lock.json), every notice channel of that platform must name rerank-model-Apache-2.0.txt on a live line. A platform whose models PR has not landed is not triggered and not checked. Any other copy of the file in the tree (Linux kept its own, whose opening paragraph names the Linux install paths) must carry the same NOTICE and licence text as the shared one.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = "resources/rerank-model.lock.json"
LICENCE = "resources/licenses/rerank-model-Apache-2.0.txt"
NAME = "rerank-model-Apache-2.0.txt"
# The iOS about page names the resource without its extension.
STEM = NAME.removesuffix(".txt")
RELEASE = "https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/rerank-v1"
# The attribution field of both pinned model.json files, verbatim.
ATTRIBUTION = "Trained on the Chinese portion of C4 (ODC-BY, https://huggingface.co/datasets/allenai/c4), FineWeb-2 (cmn_Hani) (ODC-BY, https://huggingface.co/datasets/HuggingFaceFW/fineweb-2) and LCCC (MIT, https://github.com/thu-coai/CDial-GPT)."
HUGGING_FACE = ("https://huggingface.co/msime/pinyin-ime-keystroke-reranker-25M", "https://huggingface.co/msime/pinyin-ime-keystroke-reranker-8M")
NOTICE_MARKER = "-------------------------------- NOTICE --------------------------------"
LICENSE_MARKER = "-------------------------------- LICENSE -------------------------------"
# chinese-ime-lm's NOTICE and reference/LICENSE (the Apache text; the GPL-3.0 LICENSE at that repository's root covers the training code, not the weights).
NOTICE_PHRASES = ("chinese-ime-lm\nCopyright 2026 chinese-ime-lm contributors", "C4 中文部分（ODC-BY）", "LCCC（MIT）", "FineWeb-2 中文（cmn_Hani）部分（ODC-BY）", "元数据 `attribution` 字段里")
LICENSE_PHRASES = ("Apache License\n                           Version 2.0, January 2004", "4. Redistribution.", "(d) If the Work includes a \"NOTICE\" text file", "END OF TERMS AND CONDITIONS")
# Per platform: the files whose live lines decide that it packages the models, and the notice channels that then have to ship the licence. Paths are globbed.
PLATFORMS = {
    "iOS": (
        ("platforms/ios/stage-resources.sh", "platforms/ios/build-app.sh"),
        ("platforms/ios/project.yml", "platforms/ios/App/Sources/settings/AboutAndDownloadViews.swift", "platforms/ios/tests/settings/ProjectConfigurationTests.py"),
    ),
    "macOS": (
        ("platforms/macos/CMakeLists.txt", "platforms/macos/package-release.sh", "platforms/macos/stage-resources.sh"),
        ("platforms/macos/CMakeLists.txt", "platforms/macos/resources/Licenses/THIRD_PARTY_NOTICES.txt", "platforms/macos/tests/settings/bundle_contents.py"),
    ),
    "Linux": (
        ("platforms/linux/CMakeLists.txt", "platforms/linux/package-container.sh"),
        ("platforms/linux/CMakeLists.txt", "platforms/linux/data/THIRD_PARTY_NOTICES.txt"),
    ),
    "Windows": (
        ("platforms/windows/installer/*.ps1", "platforms/windows/*.ps1"),
        ("platforms/windows/Collect-Notices.ps1",),
    ),
    "Android": (
        ("platforms/android/*.sh", "platforms/android/gradle-app/**/*.kts"),
        ("platforms/android/build-native.sh",),
    ),
    "HarmonyOS": (
        ("platforms/harmony/*.sh",),
        ("platforms/harmony/stage-resources.sh",),
    ),
}
TRIGGERS = ("fetch_rerank_model.py", "rerank-model.lock.json")
COMMENT_PREFIXES = ("#", "//", "///")

failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def live_lines(path: Path) -> list:
    if not path.is_file():
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    # THIRD_PARTY_NOTICES.txt and the iOS test are prose or Python strings: every line counts, a "#" there is not a comment marker that hides a channel.
    if path.suffix == ".txt":
        return lines
    return [line for line in lines if not line.lstrip().startswith(COMMENT_PREFIXES)]


def body(text: str) -> str:
    start = text.find(NOTICE_MARKER)
    return text[start:] if start >= 0 else ""


def main() -> int:
    lock = json.loads((ROOT / LOCK).read_text(encoding="utf-8"))
    check(lock.get("license") == "Apache-2.0", f"{LOCK} no longer records Apache-2.0; {LICENCE} and the channels assume it")
    check(lock.get("source") == RELEASE, f"{LOCK} points at {lock.get('source')}, {LICENCE} and docs/third-party.md name {RELEASE}")

    licence_path = ROOT / LICENCE
    if not licence_path.is_file():
        print(f"FAIL: {LICENCE} is missing", file=sys.stderr)
        return 1
    text = licence_path.read_text(encoding="utf-8")
    opening = text.split(NOTICE_MARKER, 1)[0]
    check(RELEASE in opening, f"{LICENCE} does not open with the release {RELEASE}")
    for url in HUGGING_FACE:
        check(url in opening, f"{LICENCE} does not name the Hugging Face copy {url}")
    check(ATTRIBUTION in opening, f"{LICENCE} does not carry the corpus attribution of the pinned model.json files verbatim")
    check(NOTICE_MARKER in text and LICENSE_MARKER in text, f"{LICENCE} lost its NOTICE or LICENSE section marker")
    notice, _, licence = body(text).partition(LICENSE_MARKER)
    for phrase in NOTICE_PHRASES:
        check(phrase in notice, f"{LICENCE}: the upstream NOTICE section no longer contains {phrase!r}")
    for phrase in LICENSE_PHRASES:
        check(phrase in licence, f"{LICENCE}: the licence section no longer contains {phrase!r}")

    docs = (ROOT / "docs/third-party.md").read_text(encoding="utf-8")
    check(NAME in docs and ATTRIBUTION in docs, f"docs/third-party.md does not name {NAME} or quote the corpus attribution")
    for model in lock["models"]:
        for artifact in model["artifacts"]:
            check(artifact["sha256"] in docs, f"docs/third-party.md does not list {model['directory']}/{artifact['file']} at the lock's SHA-256 {artifact['sha256']}")
            if artifact["file"] == "lm_step.ort":
                check(artifact["sha256"] in opening, f"{LICENCE} does not name {model['directory']}/lm_step.ort at the lock's SHA-256 {artifact['sha256']}")

    # Every other copy of the file must carry the same NOTICE and licence text.
    for copy in sorted(ROOT.glob(f"platforms/**/{NAME}")):
        relative = copy.relative_to(ROOT)
        check(body(copy.read_text(encoding="utf-8")) == body(text), f"{relative} differs from {LICENCE} in its NOTICE or licence section; ship the shared file or keep the copy identical")

    for platform, (packaging, channels) in PLATFORMS.items():
        packaging_files = sorted({path for pattern in packaging for path in ROOT.glob(pattern)})
        if not any(trigger in line for path in packaging_files for line in live_lines(path) for trigger in TRIGGERS):
            continue
        for channel in channels:
            path = ROOT / channel
            check(path.is_file(), f"{platform} packages the rerank models but its notice channel {channel} is missing")
            needle = f'LicenseNotice(file: "{STEM}"' if path.suffix == ".swift" else NAME
            check(any(needle in line for line in live_lines(path)), f"{platform} packages the rerank models but {channel} does not ship {NAME} on a live line; add {LICENCE} the way that channel ships the other licences")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print("rerank model licence, NOTICE and corpus attribution ship on every platform that packages the models")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
