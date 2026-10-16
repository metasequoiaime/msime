#!/usr/bin/env python3
"""The built-in sound packs are exactly what scripts/generate_sound_packs.py synthesizes, and each one is a pack client-services accepts.

Their CC0-1.0 dedication rests on every sample being computed by that script rather than recorded or downloaded. This regenerates the samples into a temporary directory and compares them with the committed ones, so a sample edited by hand, or one dropped in from elsewhere, fails here instead of shipping under a licence nobody can vouch for. Samples are compared to within one step of 16-bit quantization, since the sines and exponentials come from the platform's libm.

Every built-in folder is also held to the limits client-services enforces on a pack, read from its source rather than copied here: the manifest's kind and id, the built-in id lists, the files a pack may hold, each sample's rate, length and size, and the melody's notes. A pack that breaks one of them is listed as an issue at runtime and never plays, on every platform at once, which only shows on a running host. The loops also have a budget of their own, because every host bundle carries them.
"""

from __future__ import annotations

import importlib.util
import re
import sys
import tempfile
import tomllib
import wave
from array import array
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PACKS = ROOT / "resources/sound-packs"
PLUGINS = ROOT / "crates/client-services/src/plugins.rs"
SOUND_PACK = ROOT / "crates/client-services/src/plugins/sound_pack.rs"
MUSIC_PACK = ROOT / "crates/client-services/src/plugins/music_pack.rs"
# Bytes of every built-in music track together. They ship in every host's bundle, so a new loop has to fit or replace one.
MUSIC_BUDGET = 3_670_016
SAMPLE_RATES = range(8_000, 192_001)
SEMITONE_RANGE = range(-24, 25)


def load_generator():
    spec = importlib.util.spec_from_file_location("generate_sound_packs", ROOT / "scripts/generate_sound_packs.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def samples(path: Path) -> tuple[tuple[int, int, int], array]:
    with wave.open(str(path), "rb") as source:
        shape = (source.getnchannels(), source.getsampwidth(), source.getframerate())
        frames = array("h", source.readframes(source.getnframes()))
    return shape, frames


def constant(source: Path, name: str) -> int:
    """An integer `pub const` of client-services, written as a product of literals such as `512 * 1024`."""
    for line in source.read_text(encoding="utf-8").splitlines():
        if line.startswith(f"pub const {name}:"):
            expression = line.split("=", 1)[1].rstrip(";").strip()
            value = 1
            for factor in expression.split("*"):
                value *= int(factor.strip().replace("_", ""))
            return value
    raise SystemExit(f"FAIL {source.relative_to(ROOT)} no longer defines {name}")


def id_list(name: str) -> set[str]:
    """The string array `pub const <name>` of plugins.rs, which may span several lines."""
    text = PLUGINS.read_text(encoding="utf-8")
    start = text.find(f"pub const {name}:")
    if start < 0:
        raise SystemExit(f"FAIL {PLUGINS.relative_to(ROOT)} no longer defines {name}")
    equals = text.index("=", start)
    body = text[equals : text.index("];", equals)]
    return set(re.findall(r'"([^"]+)"', body))


def check_sound(name: str, manifest: dict, files: list[str], failures: list[str]) -> None:
    sample_bytes = constant(SOUND_PACK, "MAX_SAMPLE_BYTES")
    sample_millis = constant(SOUND_PACK, "MAX_SAMPLE_MILLIS")
    if len(files) > constant(SOUND_PACK, "MAX_SAMPLES"):
        failures.append(f"{name} names more samples than a sound pack may hold")
    if sum((PACKS / name / file).stat().st_size for file in files) > constant(SOUND_PACK, "MAX_PACK_BYTES"):
        failures.append(f"{name} is larger than a sound pack may be")
    for file in files:
        path = PACKS / name / file
        if path.stat().st_size > sample_bytes:
            failures.append(f"{name}/{file} is larger than a sample may be")
        with wave.open(str(path), "rb") as source:
            rate, frames = source.getframerate(), source.getnframes()
        if rate not in SAMPLE_RATES or frames == 0 or frames * 1_000 > sample_millis * rate:
            failures.append(f"{name}/{file} has a sample rate or length client-services refuses")
    semitones = manifest.get("sequence", {}).get("semitones", [])
    if len(semitones) > constant(SOUND_PACK, "MAX_SEMITONES") or any(note not in SEMITONE_RANGE for note in semitones):
        failures.append(f"{name} has more notes, or a note further from the sample's pitch, than a melody may")


def check_music(name: str, files: list[str], failures: list[str]) -> None:
    track_bytes = constant(MUSIC_PACK, "MAX_TRACK_BYTES")
    track_seconds = constant(MUSIC_PACK, "MAX_TRACK_SECONDS")
    if len(files) > constant(MUSIC_PACK, "MAX_TRACKS"):
        failures.append(f"{name} names more tracks than a music pack may hold")
    if sum((PACKS / name / file).stat().st_size for file in files) > constant(MUSIC_PACK, "MAX_PACK_BYTES"):
        failures.append(f"{name} is larger than a music pack may be")
    for file in files:
        path = PACKS / name / file
        if path.stat().st_size > track_bytes:
            failures.append(f"{name}/{file} is larger than a track may be")
        with wave.open(str(path), "rb") as source:
            rate, frames = source.getframerate(), source.getnframes()
        if rate not in SAMPLE_RATES or frames == 0 or frames > track_seconds * rate:
            failures.append(f"{name}/{file} has a sample rate or length client-services refuses")


def check_manifests(failures: list[str]) -> None:
    builtin = {"sound": id_list("BUILTIN_SOUND_PACKS"), "music": id_list("BUILTIN_MUSIC_PACKS")}
    folders = sorted(path for path in PACKS.iterdir() if path.is_dir())
    music_bytes = 0
    for directory in folders:
        name = directory.name
        manifest = tomllib.loads((directory / "plugin.toml").read_text(encoding="utf-8"))
        if manifest.get("license") != "CC0-1.0" or manifest.get("id") != name:
            failures.append(f"{name}/plugin.toml must name its folder and CC0-1.0")
        kind = manifest.get("kind")
        if kind not in builtin:
            failures.append(f"{name}/plugin.toml must be a sound or music pack")
            continue
        if name not in builtin[kind]:
            failures.append(f"{name} is a {kind} pack missing from client-services' built-in {kind} ids")
        if kind == "sound":
            named = list(manifest.get("sounds", {}).values())
            if "sequence" in manifest:
                named.append(manifest["sequence"]["sample"])
        else:
            named = list(manifest.get("music", {}).get("tracks", []))
        files = sorted(set(named))
        present = sorted(path.name for path in directory.iterdir() if not path.name.startswith("."))
        if present != sorted(files + ["plugin.toml"]):
            failures.append(f"{name} holds {present}; a built-in pack holds its manifest and the files it names, nothing else")
            continue
        if len(present) > constant(PLUGINS, "MAX_PACK_FILES"):
            failures.append(f"{name} holds more files than a pack may")
        if kind == "sound":
            check_sound(name, manifest, files, failures)
        else:
            check_music(name, files, failures)
            music_bytes += sum((directory / file).stat().st_size for file in files)
    for kind, ids in builtin.items():
        for missing in sorted(ids - {directory.name for directory in folders}):
            failures.append(f"client-services lists {missing} as a built-in {kind} pack, but resources/sound-packs has no such folder")
    if music_bytes > MUSIC_BUDGET:
        failures.append(f"the built-in music tracks take {music_bytes} bytes, past the {MUSIC_BUDGET}-byte budget every bundle pays")


def main() -> int:
    generator = load_generator()
    failures: list[str] = []
    with tempfile.TemporaryDirectory() as scratch:
        expected = {path.relative_to(scratch).as_posix(): path for path in generator.generate(Path(scratch))}
        committed = {path.relative_to(PACKS).as_posix(): path for path in PACKS.rglob("*.wav")}
        for name in sorted(set(committed) - set(expected)):
            failures.append(f"{name} is not produced by generate_sound_packs.py")
        for name in sorted(set(expected) - set(committed)):
            failures.append(f"{name} is missing; run scripts/generate_sound_packs.py")
        for name in sorted(set(expected) & set(committed)):
            want_shape, want = samples(expected[name])
            have_shape, have = samples(committed[name])
            if want_shape != have_shape or len(want) != len(have):
                failures.append(f"{name} differs in format or length from the generator's output")
            elif any(abs(a - b) > 1 for a, b in zip(want, have)):
                failures.append(f"{name} differs from the generator's output")
    check_manifests(failures)
    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    if failures:
        return 1
    print(f"sound packs: {len(committed)} samples match the generator")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
