#!/usr/bin/env python3
"""Keep cloud dictionary file-provider reads off the iOS settings view's main actor."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "platforms/ios/App/Sources/dictionary/CloudDictionaryFilesView.swift").read_text()


def main() -> None:
    start = SOURCE.index("    .fileImporter(isPresented: $choosing")
    end = SOURCE.index('    .alert("上传词库文件？', start)
    importer = SOURCE[start:end]
    if "Task.detached" not in importer:
        raise SystemExit("cloud dictionary file import must run in a detached worker")
    before_worker = importer.split("Task.detached", 1)[0]
    if "FileHandle" in before_worker or "startAccessingSecurityScopedResource" in before_worker:
        raise SystemExit("cloud dictionary file data is read synchronously on the main actor")
    print("iOS cloud dictionary file import reads off the main actor")


if __name__ == "__main__":
    main()
