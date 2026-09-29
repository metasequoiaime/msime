#!/usr/bin/env python3
"""Add Engine sentence-association neural reranking from upstream commits 9856905, af0b2aa and 78ca062.

The locked Engine predates this feature.  The patch is kept as a repository asset so a cold
checkout can apply it offline after the ordinary compatibility overlays.  A small unified-diff
applier is used instead of invoking a platform-specific ``patch`` executable.
"""
from __future__ import annotations

import re
from pathlib import Path

PATCH = Path(__file__).parent / "engine-overlays/neural-association.patch"
_HUNK = re.compile(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")


def _apply_file(root: Path, old_name: str, new_name: str, hunks: list[tuple[int, list[str]]]) -> None:
    old = root / old_name
    new = root / new_name
    text = old.read_text(encoding="utf-8") if old.exists() else ""
    lines = text.splitlines(keepends=True)
    delta = 0
    for start, changes in hunks:
        context = [line[1:] for line in changes if line[:1] in (" ", "-")]
        replacement = [line[1:] for line in changes if line[:1] in (" ", "+")]
        if not context:
            pos = 0 if start <= 0 else min(start - 1 + delta, len(lines))
        else:
            hint = 0 if start <= 0 else max(0, min(start - 1 + delta, len(lines)))
            matches = [
                i for i in range(len(lines) - len(context) + 1)
                if lines[i : i + len(context)] == context
            ]
            if len(matches) != 1:
                raise RuntimeError(
                    f"neural overlay context mismatch in {new_name} near line {start} "
                    f"(found {len(matches)} matches; hint {hint})"
                )
            pos = matches[0]
        lines[pos : pos + len(context)] = replacement
        delta += len(replacement) - len(context)
    new.parent.mkdir(parents=True, exist_ok=True)
    new.write_text("".join(lines), encoding="utf-8")


def apply(root: Path) -> None:
    patch = PATCH.read_text(encoding="utf-8").splitlines(keepends=True)
    files: list[tuple[str, str, list[tuple[int, list[str]]]]] = []
    old = new = None
    hunks: list[tuple[int, list[str]]] = []
    current_start = None
    current_lines: list[str] = []

    def finish_hunk() -> None:
        nonlocal current_start, current_lines
        if current_start is not None:
            hunks.append((current_start, current_lines))
            current_start, current_lines = None, []

    def finish_file() -> None:
        nonlocal old, new, hunks
        finish_hunk()
        if old is not None and new is not None:
            files.append((old, new, hunks))
        old = new = None
        hunks = []

    for line in patch:
        if line.startswith("diff --git "):
            finish_file()
            parts = line.split()
            old, new = parts[2][2:], parts[3][2:]
        elif line.startswith("--- "):
            # Keep the diff paths from diff --git; /dev/null is handled below.
            if line.rstrip().endswith("/dev/null"):
                old = ""
        elif line.startswith("+++ "):
            if line.rstrip().endswith("/dev/null"):
                new = ""
        elif line.startswith("@@ "):
            finish_hunk()
            match = _HUNK.match(line)
            if not match:
                raise RuntimeError(f"invalid neural overlay hunk header: {line.rstrip()}")
            current_start = int(match.group(1))
        elif current_start is not None and (line.startswith((" ", "+", "-")) or line.startswith("\\")):
            if line.startswith("\\"):
                continue
            current_lines.append(line)
    finish_file()

    for old_name, new_name, hunks in files:
        if old_name == "" and new_name:
            # New files have a zero based insertion point in the patch.
            _apply_file(root, new_name, new_name, hunks)
        elif new_name == "":
            path = root / old_name
            if path.exists():
                path.unlink()
        else:
            _apply_file(root, old_name, new_name, hunks)


if __name__ == "__main__":
    import sys

    apply(Path(sys.argv[1]))
