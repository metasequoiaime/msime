#!/usr/bin/env python3
"""Installer choices must be read and deleted through one trusted Windows handle."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LEASE = ROOT / "platforms/windows/src/system/StateRootLease.h"
FIRST_RUN = ROOT / "platforms/windows/src/system/FirstRun.h"


def main() -> int:
    lease = LEASE.read_text(encoding="utf-8")
    first_run = FIRST_RUN.read_text(encoding="utf-8")
    required = {
        "one handle reader exists": "take_private_file(" in lease,
        "reader opens with delete access": "GENERIC_READ | DELETE" in lease,
        "reader uses a no-follow handle": "FILE_FLAG_OPEN_REPARSE_POINT" in lease,
        "reader validates regular files": "handle_is_trusted_file(handle)" in lease,
        "reader deletes the opened object": "SetFileInformationByHandle" in lease
        and "FileDispositionInfo" in lease,
        "first-run uses the handle reader": "take_private_file(path, 4096)" in first_run,
        "first-run has no path deletion after read": "std::filesystem::remove(path, error)" not in first_run,
    }
    missing = [name for name, present in required.items() if not present]
    if missing:
        print("windows installer choice lifecycle: missing " + ", ".join(missing))
        return 1
    print("windows installer choices are read and removed through one trusted handle")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
