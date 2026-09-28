#!/usr/bin/env python3
"""Every `scripts/apply_engine_*.py` on disk is either registered in engine-lock.json `overlay_scripts` or listed in RETIRED with a reason.

`fetch_engine.py` applies only what the lock lists, so an overlay script that is on disk but missing from the lock silently never reaches the Engine, while the runtime code and tests written against it keep describing behaviour the Engine does not have. That happened to `apply_engine_promoted_english_first.py`: 795a41cf5 registered it, and merge 37af8123c resolved the lock as the union of two other overlay additions and dropped it. Nothing referenced the script by name, so no build or test noticed.

A script that is deliberately no longer applied goes into RETIRED with the reason, so retiring one is a visible decision rather than a merge accident. The check also rejects duplicate lock entries and registered paths that do not exist.
"""

from __future__ import annotations

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOCK = ROOT / "engine-lock.json"

# Overlay scripts kept on disk but intentionally not applied: {repo-relative path: reason}.
RETIRED: dict[str, str] = {
    "scripts/apply_engine_promoted_english_first.py": "Mixed input never seats an English word ahead of Chinese. The shipped english.db weighs words by corpus counts on a different scale from the pinyin tables, so promoting the unique heaviest English word seated shipped or once-pinned words ahead of Chinese across every prefix they matched, and ranking a mixed-input English word against the whole mixed list compared weights on the two scales. Upstream seats English at the priority slot behind the leading Chinese candidate and ranks a pinned English word among English words only.",
}


def duplicates(entries: list[str]) -> list[str]:
    seen: set[str] = set()
    repeated = []
    for entry in entries:
        if entry in seen:
            repeated.append(entry)
        seen.add(entry)
    return repeated


def main() -> int:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    scripts = lock.get("overlay_scripts", [])
    assets = lock.get("overlay_assets", [])
    on_disk = {path.relative_to(ROOT).as_posix() for path in (ROOT / "scripts").glob("apply_engine_*.py")}
    failures = []

    for field, entries in (("overlay_scripts", scripts), ("overlay_assets", assets)):
        for entry in duplicates(entries):
            failures.append(f"{field} lists {entry} more than once")
        for entry in entries:
            if not (ROOT / entry).is_file():
                failures.append(f"{field} lists {entry}, which does not exist")

    for entry in sorted(set(scripts) & RETIRED.keys()):
        failures.append(f"{entry} is both registered in overlay_scripts and listed in RETIRED")
    for entry in sorted(RETIRED.keys() - on_disk):
        failures.append(f"RETIRED lists {entry}, which is not on disk; drop the entry")
    for entry in sorted(on_disk - set(scripts) - RETIRED.keys()):
        failures.append(f"{entry} is on disk but neither in engine-lock.json overlay_scripts nor in RETIRED, so fetch_engine.py never applies it")
    for entry in sorted(set(scripts) - on_disk):
        if (ROOT / entry).is_file():
            failures.append(f"overlay_scripts lists {entry}, which is not a scripts/apply_engine_*.py overlay")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print(f"engine overlay registry: {len(scripts)} registered, {len(RETIRED)} retired, {len(assets)} assets")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
