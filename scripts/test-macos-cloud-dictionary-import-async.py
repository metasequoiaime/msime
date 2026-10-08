#!/usr/bin/env python3
"""Keep macOS cloud dictionary file-provider reads off the main actor."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "platforms/macos/src/backend/dictionary/BackendDictionaryView.swift").read_text()


def main() -> None:
    start = SOURCE.index("  func chooseImport()")
    end = SOURCE.index("  func uploadImport()", start)
    method = SOURCE[start:end]
    if "Task.detached" not in method:
        raise SystemExit("macOS cloud dictionary file import must run in a detached worker")
    before_worker = method.split("Task.detached", 1)[0]
    if "FileHandle" in before_worker or "startAccessingSecurityScopedResource" in before_worker:
        raise SystemExit("macOS cloud dictionary file data is read synchronously on the main actor")
    print("macOS cloud dictionary file import reads off the main actor")


if __name__ == "__main__":
    main()
