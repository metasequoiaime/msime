#!/usr/bin/env python3
"""File locking must go through `msime_client_core::file_lock`, never `std::fs::File` directly.

std has no implementation of `File::lock` on Android: it fails outright rather than blocking, so
anything that locks a file with it works on every desktop and simulator and then refuses on a
handset. `crates/client-base/src/file_lock.rs` is the one place that knows this and reaches for
rustix's flock on that target; every other caller is supposed to go through it.

`host-api`'s clipboard migration lock did not. Every shared clipboard operation on Android failed on
its first line, and because the Android keyboard clears the history from `onCreateInputView` while
the preference is off, the input method threw out of the framework's `showWindow` and died before
drawing a key - with Android then falling back to another keyboard. No gate saw it: the code
compiles for the Android target, its unit tests pass on the host, and the failure needs a running
IME on a device to appear.

The discriminator here is deliberate. `Mutex::lock()` reads identically and is everywhere, so a
lock call is only reported when the same function opened a file just above it - which is exactly the
shape of a lock sidecar, and was the shape of the defect.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OWNER = ROOT / "crates/client-base/src/file_lock.rs"
SEARCHED = ["crates", "apps", "platforms"]

# The file-locking half of the std API. `lock`/`try_lock` are shared with `Mutex`; the two
# `_shared` spellings and `TryLockError` belong to files alone.
CALL = re.compile(r"\.(lock|try_lock|lock_shared|try_lock_shared)\(\s*\)")
FILE_ONLY = re.compile(r"\.(lock_shared|try_lock_shared)\(\s*\)|std::fs::TryLockError")
# A file handle came into being just above: an open, or a parameter typed as a File.
OPENED = re.compile(r"OpenOptions|fs::File::|File::(open|create)|:\s*&?(std::fs::)?File\b")
WINDOW = 15
SIGNATURE = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?(const\s+|async\s+|unsafe\s+|extern\s+\S+\s+)*fn\s")
ATTRIBUTE = re.compile(r"^\s*(#\[|#!\[|//|/\*|\*)")
EXEMPT = 'target_os = "android"'


def enclosing_signature(lines: list[str], index: int) -> int:
    """The line of the `fn` that contains `index`, or -1 when none is found above it."""
    start = index
    while start >= 0 and not SIGNATURE.match(lines[start]):
        start -= 1
    return start


def excused(lines: list[str], index: int) -> bool:
    """A body that Android never compiles may use the std API, and some tests must.

    The exemption is read off the enclosing function's own attributes rather than a window, so
    that a `cfg` belonging to some unrelated item further up cannot launder a real call.
    """
    start = enclosing_signature(lines, index)
    if start < 0:
        return False
    above = start - 1
    while above >= 0 and ATTRIBUTE.match(lines[above]):
        if EXEMPT in lines[above]:
            return True
        above -= 1
    return False

findings = []
for directory in SEARCHED:
    for source in sorted((ROOT / directory).rglob("*.rs")):
        if source == OWNER:
            continue
        lines = source.read_text().split("\n")
        for index, line in enumerate(lines):
            if not CALL.search(line) and not FILE_ONLY.search(line):
                continue
            # The window stops at the enclosing function's signature: a file opened by the function above is not this function's lock sidecar.
            window = "\n".join(lines[max(0, index - WINDOW, enclosing_signature(lines, index)) : index + 1])
            if not (FILE_ONLY.search(line) or OPENED.search(window)):
                continue
            if excused(lines, index):
                continue
            findings.append((source.relative_to(ROOT), index + 1, line.strip()))

if findings:
    for path, number, text in findings:
        print(f"{path}:{number}: file lock taken outside client-core: {text}")
    sys.exit(
        "std::fs::File locking is unsupported on Android; call "
        "msime_client_core::file_lock instead"
    )

# The helper has to still be the thing everyone calls, or this guard is watching an empty room.
callers = sum(
    1
    for directory in SEARCHED
    for source in (ROOT / directory).rglob("*.rs")
    if source != OWNER and "file_lock::" in source.read_text()
)
if callers < 3:
    sys.exit(f"expected client-core's file_lock helper to be in general use, found {callers} callers")
print(f"file locking: no direct std::fs::File locks, {callers} files go through client-core")
