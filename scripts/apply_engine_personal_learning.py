#!/usr/bin/env python3
"""Port the Engine's personal learning series to the locked Engine.

The series (online reading validation, personal word-sequence context, pick pair words, learning undo and learnable typo correction) was written against the final MSIME-Engine main, which was archived before it merged. The lock is much older and this repository already overlays other work on the same files, so the series is stored as one strict contextual patch against the tree the earlier overlays produce, and this overlay has to run after all of them.

Hunks are matched with the contextual applier of apply_engine_quanpin_autocorrect_parity.py, including its ``~`` spelling of an empty context line. This patch also creates files, which that applier does not: a ``--- /dev/null`` section writes its added lines as a new file, and refuses to overwrite one that already exists.
"""

from pathlib import Path
import runpy


HERE = Path(__file__).resolve().parent
PATCH = HERE / "engine-overlays" / "personal-learning.patch"
CONTEXTUAL = runpy.run_path(
    str(HERE / "apply_engine_quanpin_autocorrect_parity.py"), run_name="__engine_contextual_patch__"
)
_apply_file = CONTEXTUAL["_apply_file"]
_patch_line = CONTEXTUAL["_patch_line"]
HUNK = CONTEXTUAL["HUNK"]


def _read_patch() -> list[tuple[str, bool, list[list[str]]]]:
    lines = PATCH.read_text(encoding="utf-8").splitlines(keepends=True)
    files: list[tuple[str, bool, list[list[str]]]] = []
    index = 0
    while index < len(lines):
        header = lines[index]
        if header == "--- /dev/null\n":
            old_path = None
        elif header.startswith("--- a/"):
            old_path = header[6:].rstrip("\n")
        else:
            raise RuntimeError(f"unexpected Engine overlay line: {header.rstrip()}")
        index += 1
        if index >= len(lines) or not lines[index].startswith("+++ b/"):
            raise RuntimeError(f"missing new-file header after {header.rstrip()}")
        new_path = lines[index][6:].rstrip("\n")
        if old_path is not None and new_path != old_path:
            raise RuntimeError(f"Engine overlay cannot rename {old_path} to {new_path}")
        index += 1
        hunks: list[list[str]] = []
        while index < len(lines) and not lines[index].startswith("--- "):
            if not lines[index].startswith("@@ "):
                raise RuntimeError(f"missing hunk header for {new_path}: {lines[index].rstrip()}")
            start = index
            index += 1
            while index < len(lines) and not lines[index].startswith(("@@ ", "--- ")):
                index += 1
            hunks.append(lines[start:index])
        files.append((new_path, old_path is None, hunks))
    return files


def _create_file(root: Path, relative: str, hunks: list[list[str]]) -> None:
    path = root / relative
    if path.exists():
        raise RuntimeError(f"Engine overlay would overwrite an existing file: {relative}")
    if len(hunks) != 1 or HUNK.match(hunks[0][0]) is None:
        raise RuntimeError(f"Engine overlay new file {relative} must be a single hunk")
    body: list[str] = []
    for line in hunks[0][1:]:
        if line.startswith("\\ No newline at end of file"):
            if body:
                body[-1] = body[-1].rstrip("\n")
            continue
        marker, text = _patch_line(line)
        if marker != "+":
            raise RuntimeError(f"Engine overlay new file {relative} has a non-added line: {line.rstrip()}")
        body.append(text)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(body), encoding="utf-8", newline="\n")


def apply(root: Path) -> None:
    for relative, created, hunks in _read_patch():
        if created:
            _create_file(root, relative, hunks)
        else:
            _apply_file(root, relative, hunks)


if __name__ == "__main__":
    import sys

    apply(Path(sys.argv[1]))
