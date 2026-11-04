#!/usr/bin/env python3
"""Check that the licence, upstream NOTICE and corpus attribution of every redistributed chinese-ime-lm model travel with it on every platform that packages it.

The models are Apache-2.0. Section 4(a) requires a copy of the licence with every redistribution and 4(d) the upstream NOTICE; the training corpora require their attribution to travel with the weights. Each model has one shared file in resources/licenses that carries all three: an opening paragraph with the source, the pinned SHA-256 of the weights and the attribution exactly as the weights carry it, then the upstream NOTICE and the Apache text (chinese-ime-lm's reference/LICENSE, not the GPL-3.0 LICENSE at that repository's root). Every platform ships that file the way it ships its other licences. A platform that drops it still builds and still ships the model, so only reading the channels finds that out.

A model in MODELS is either shipped by a fixed set of platforms (`always`), or by whichever platform's packaging fetches it (`triggers`: a live, non-comment line of one of that platform's packaging files names one of them). Every notice channel of such a platform must name the file on a live line. Any other copy of a shared file elsewhere in the tree must carry the same NOTICE and licence text. To add a model, add an entry to MODELS.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NOTICE_MARKER = "-------------------------------- NOTICE --------------------------------"
LICENSE_MARKER = "-------------------------------- LICENSE -------------------------------"
# chinese-ime-lm's reference/LICENSE.
LICENSE_PHRASES = ("Apache License\n                           Version 2.0, January 2004", "4. Redistribution.", "(d) If the Work includes a \"NOTICE\" text file", "END OF TERMS AND CONDITIONS")
NOTICE_HEAD = "chinese-ime-lm\nCopyright 2026 chinese-ime-lm contributors"

MODELS = {
    # model-v1: sentence-model.safetensors is in the dictionary resources of all six platforms and of the web engine; sentence-model-desktop.safetensors in the settled-model resource pack and in distribution packages.
    "model-v1": {
        "licence": "resources/licenses/sentence-model-Apache-2.0.txt",
        "lock": "resources/neural-model.lock.json",
        # Other locks that pin the same files: their digests must agree with the main lock's.
        "also_pinned_by": ("resources/desktop-dictionary.lock.json", "resources/settled-model.lock.json"),
        "release": "https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/model-v1",
        "hugging_face": ("https://huggingface.co/msime/pinyin-ime-reranker-4M", "https://huggingface.co/msime/pinyin-ime-reranker-25M"),
        # The attribution field of the safetensors __metadata__ header of both pinned files, verbatim.
        "attribution": "Trained on the Chinese portion of C4 (ODC-BY, https://huggingface.co/datasets/allenai/c4) and LCCC (MIT, https://github.com/thu-coai/CDial-GPT).",
        "notice_phrases": (NOTICE_HEAD, "C4 中文部分（ODC-BY）", "LCCC（MIT）", "`__metadata__` 头的 `attribution` 字段"),
        "weights": (".safetensors",),
        "always": ("iOS", "macOS", "Linux", "Windows", "Android", "HarmonyOS", "Web"),
        "triggers": (),
    },
    # rerank-v2: the keystroke rerank models (8x448 in rerank-model/, 6x256 in rerank-model-small/) ship where a platform's packaging fetches them. Linux keeps its own copy of the file, whose opening paragraph names the Linux install paths.
    "rerank-v2": {
        "licence": "resources/licenses/rerank-model-Apache-2.0.txt",
        "lock": "resources/rerank-model.lock.json",
        "also_pinned_by": (),
        "release": "https://github.com/metasequoiaime/chinese-ime-lm/releases/tag/rerank-v2",
        # The Hugging Face repositories still hold rerank-v1, so the licence does not name them for these weights.
        "hugging_face": (),
        # The attribution field of both pinned model.json files (and of the ONNX graph metadata of each lm_step.ort), verbatim.
        "attribution": "Trained on the Chinese portion of C4 (ODC-BY, https://huggingface.co/datasets/allenai/c4), FineWeb-2 (cmn_Hani) (ODC-BY, https://huggingface.co/datasets/HuggingFaceFW/fineweb-2), LCCC (MIT, https://github.com/thu-coai/CDial-GPT) and synthetic one-sided chat messages written by DeepSeek deepseek-v4-pro for this project (neural/teacher/gen_chat.py) (DeepSeek Terms of Use, outputs assigned to the user and usable to train other models, https://cdn.deepseek.com/policies/en-US/deepseek-terms-of-use.html).",
        "notice_phrases": (NOTICE_HEAD, "C4 中文部分（ODC-BY）", "LCCC（MIT）", "FineWeb-2 中文（cmn_Hani）部分（ODC-BY）", "元数据 `attribution` 字段里"),
        "weights": (".ort",),
        "always": (),
        "triggers": ("fetch_rerank_model.py", "rerank-model.lock.json"),
    },
}

# Per platform: the packaging files whose live lines decide that a triggered model ships there, and the notice channels that then have to ship its licence. Paths are globbed.
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
        ("platforms/windows/Collect-Notices.ps1", "platforms/windows/tests/tools/collect_notices.ps1"),
    ),
    "Android": (
        ("platforms/android/*.sh", "platforms/android/gradle-app/**/*.kts"),
        ("platforms/android/build-native.sh",),
    ),
    "HarmonyOS": (
        ("platforms/harmony/*.sh",),
        ("platforms/harmony/stage-resources.sh",),
    ),
    "Web": (
        ("scripts/build-web-engine.sh",),
        ("scripts/web-engine-notice.sh", "resources/licenses/web-engine-NOTICE.md"),
    ),
}
COMMENT_PREFIXES = ("#", "//")

failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def live_lines(path: Path) -> list:
    if not path.is_file():
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    # Notice overviews and generated notices are prose: a leading "#" there is a heading, not a comment that hides a channel.
    if path.suffix in (".txt", ".md"):
        return lines
    return [line for line in lines if not line.lstrip().startswith(COMMENT_PREFIXES)]


def body(text: str) -> str:
    start = text.find(NOTICE_MARKER)
    return text[start:] if start >= 0 else ""


def pinned(node, found: list) -> list:
    """Every (file name, sha256) a lock pins, wherever it nests them."""
    if isinstance(node, dict):
        if "sha256" in node and ("file" in node or "name" in node):
            found.append((node.get("file") or node["name"], node["sha256"]))
        for value in node.values():
            pinned(value, found)
    elif isinstance(node, list):
        for value in node:
            pinned(value, found)
    return found


def check_model(name: str, model: dict, docs: str) -> None:
    licence = model["licence"]
    filename = Path(licence).name
    stem = filename.removesuffix(".txt")
    lock = json.loads((ROOT / model["lock"]).read_text(encoding="utf-8"))
    check(lock.get("license") == "Apache-2.0", f"{model['lock']} no longer records Apache-2.0; {licence} and the channels assume it")
    check(lock.get("source") == model["release"], f"{model['lock']} points at {lock.get('source')}, {licence} names {model['release']}")
    artifacts = pinned(lock, [])
    check(bool(artifacts), f"{model['lock']} pins nothing")

    path = ROOT / licence
    if not path.is_file():
        failures.append(f"{licence} ({name}) is missing")
        return
    text = path.read_text(encoding="utf-8")
    opening = text.split(NOTICE_MARKER, 1)[0]
    check(model["release"] in opening, f"{licence} does not open with the release {model['release']}")
    for url in model["hugging_face"]:
        check(url in opening, f"{licence} does not name the Hugging Face copy {url}")
    check(model["attribution"] in opening, f"{licence} does not carry the corpus attribution of the pinned weights verbatim")
    check(NOTICE_MARKER in text and LICENSE_MARKER in text, f"{licence} lost its NOTICE or LICENSE section marker")
    notice, _, licence_text = body(text).partition(LICENSE_MARKER)
    for phrase in model["notice_phrases"]:
        check(phrase in notice, f"{licence}: the upstream NOTICE section no longer contains {phrase!r}")
    for phrase in LICENSE_PHRASES:
        check(phrase in licence_text, f"{licence}: the licence section no longer contains {phrase!r}")

    check(filename in docs and model["attribution"] in docs, f"docs/third-party.md does not name {filename} or quote the {name} corpus attribution")
    for artifact, digest in artifacts:
        check(digest in docs, f"docs/third-party.md does not list {name} {artifact} at the lock's SHA-256 {digest}")
        if artifact.endswith(model["weights"]):
            check(digest in opening, f"{licence} does not name {artifact} at the lock's SHA-256 {digest}")
    digests = dict(artifacts)
    for other in model["also_pinned_by"]:
        for artifact, digest in pinned(json.loads((ROOT / other).read_text(encoding="utf-8")), []):
            if artifact in digests:
                check(digest == digests[artifact], f"{other} pins {artifact} at {digest}, {model['lock']} at {digests[artifact]}; update {licence} and docs/third-party.md with the pin")

    # Every other copy of the file must carry the same NOTICE and licence text.
    for copy in sorted(ROOT.glob(f"platforms/**/{filename}")):
        check(body(copy.read_text(encoding="utf-8")) == body(text), f"{copy.relative_to(ROOT)} differs from {licence} in its NOTICE or licence section; ship the shared file or keep the copy identical")

    for platform, (packaging, channels) in PLATFORMS.items():
        shipped = platform in model["always"]
        if not shipped and model["triggers"]:
            files = sorted({file for pattern in packaging for file in ROOT.glob(pattern)})
            shipped = any(trigger in line for file in files for line in live_lines(file) for trigger in model["triggers"])
        if not shipped:
            continue
        for channel in channels:
            channel_path = ROOT / channel
            # The iOS about page names the resource without its extension.
            needle = f'LicenseNotice(file: "{stem}"' if channel_path.suffix == ".swift" else filename
            check(channel_path.is_file(), f"{platform} ships {name} but its notice channel {channel} is missing")
            check(any(needle in line for line in live_lines(channel_path)), f"{platform} ships {name} but {channel} does not ship {filename} on a live line; add {licence} the way that channel ships the other licences")


def main() -> int:
    docs = (ROOT / "docs/third-party.md").read_text(encoding="utf-8")
    for name, model in MODELS.items():
        check_model(name, model, docs)
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print(f"model notices: {', '.join(MODELS)} ship their licence, upstream NOTICE and corpus attribution on every platform that packages them")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
