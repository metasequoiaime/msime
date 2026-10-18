#!/usr/bin/env python3
"""Every source under platforms/macos/src must be compiled by some target, or say out loud that it is not.

#912 reported that the macOS input method hands English keys to the application in Chinese mode, found a
hardcoded engine resource path in src/dictionary/DictionaryRuntime.mm, fixed it there, passed review and
changed nothing: no target compiles that file. The shipping bundle uses src/core/ClientDictionaryRuntime.mm.
A CMake comment already said so, 750 lines away from the edit, and it was not enough.

A file that nothing builds is not automatically wrong - this platform keeps adapters from the pinned Apple
snapshot for reference. What is wrong is finding that out after shipping a fix into one. So each of them is
listed here with the file that replaced it, and has to carry the marker below near the top, where the next
person to edit it is already looking. A new orphan fails until it is either built or listed.

Compiled means named as a source in platforms/macos/CMakeLists.txt with comments stripped, because the
mention that misled #912 was itself a comment.
"""

import re
import sys
from pathlib import Path

MARKER = "not compiled by any target"
SOURCE_SUFFIXES = (".mm", ".m", ".cpp", ".cc", ".c")

# Orphan -> the file that carries this behaviour in the product. Keep the reason in the file itself.
RETAINED: dict[str, str] = {}


def compiled_sources(cmake: Path) -> str:
    """The CMake text with comments removed. `#` cannot appear in these paths, so this needs no parser."""
    return "\n".join(re.sub(r"#.*", "", line) for line in cmake.read_text(encoding="utf-8").splitlines())


def main() -> int:
    root = Path(__file__).resolve().parent.parent / "platforms/macos"
    cmake = root / "CMakeLists.txt"
    if not cmake.is_file():
        print(f"skipped: no {cmake}")
        return 0

    text = compiled_sources(cmake)
    orphans = []
    for path in sorted((root / "src").rglob("*")):
        if path.suffix not in SOURCE_SUFFIXES or not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        if relative not in text:
            orphans.append(relative)

    failures = []
    for relative in orphans:
        if relative not in RETAINED:
            failures.append(
                f"{relative} is compiled by no target in platforms/macos/CMakeLists.txt.\n"
                f"  Add it to a target, or - if it is retained for reference - list it in RETAINED in "
                f"{Path(__file__).name} and say so at the top of the file."
            )
            continue
        head = "\n".join((root / relative).read_text(encoding="utf-8").splitlines()[:40])
        if MARKER not in head:
            failures.append(
                f'{relative} is retained but does not say so: the first 40 lines must contain "{MARKER}" '
                f"and name {RETAINED[relative]}, so an edit here cannot look like an edit to the product."
            )

    for relative, replacement in RETAINED.items():
        if not (root / relative).is_file():
            failures.append(f"{relative} is listed as retained but does not exist; drop it from RETAINED")
        elif relative not in orphans:
            failures.append(
                f"{relative} is listed as retained but is compiled now; drop it from RETAINED "
                f"(and from the note pointing at {replacement})"
            )

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print(f"macOS sources: {len(orphans)} retained and marked, every other source is built")
    return 0


if __name__ == "__main__":
    sys.exit(main())
