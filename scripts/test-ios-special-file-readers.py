#!/usr/bin/env python3
"""iOS/macOS bounded readers must reject special files without blocking."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]


def region(path: Path, start: str, end: str) -> str:
    text = path.read_text(encoding="utf-8")
    begin = text.index(start)
    return text[begin : text.index(end, begin)]


CHECKS = (
    (
        ROOT / "platforms/ios/App/Services/BoundedFileReader.swift",
        "static func read(from url: URL",
        "  }\n}",
        ("O_NONBLOCK", "fstat", "S_IFREG"),
    ),
    (
        ROOT / "platforms/ios/SharedUI/voice/VoiceTextHandoffStore.swift",
        "static func readBounded(_ file: URL)",
        "  }\n\n  func read(now:",
        ("BoundedFileReader.read(from: file, maximumBytes: Self.maximumBytes)",),
    ),
    (
        ROOT / "platforms/ios/SharedUI/dictionary/DictionarySnapshotQueue.swift",
        "private func read(_ root: URL)",
        "  }\n  func read() throws",
        ("BoundedFileReader.read(from: file, maximumBytes: 65536)",),
    ),
    (
        ROOT / "platforms/ios/SharedUI/core/CloudSyncState.swift",
        "func importLegacy() -> Bool",
        "  }\n\n  /// ",
        ("BoundedFileReader.read(from: file, maximumBytes: 256 * 1024)",),
    ),
    (
        ROOT / "platforms/ios/SharedUI/dictionary/DictionarySnapshotQueue.swift",
        "private func openSnapshotSource(_ file: URL)",
        "  }\n\n  // A killed worker",
        ("O_NONBLOCK",),
    ),
    (
        ROOT / "platforms/macos/src/core/BoundedFileReader.mm",
        "NSData *MSIMEReadFileUpTo",
        "    return data;\n}",
        ("O_NONBLOCK",),
    ),
    (
        ROOT / "platforms/macos/src/backend/dictionary/BackendSecureFileReader.swift",
        "static func readData(from url: URL",
        "    #else",
        ("O_NONBLOCK",),
    ),
    (
        ROOT / "platforms/ios/SharedUI/vocabulary/VocabularyReviewStore.swift",
        "static func readWordbookData(from url: URL)",
        "  }\n\n  /// The device",
        ("BoundedFileReader.read(from: url, maximumBytes: maximumImportBytes)",),
    ),
    (
        ROOT / "platforms/ios/SharedUI/dictionary/PersonalDictionaryImport.swift",
        "  private static func readData(from url: URL)",
        "\n  static let example",
        ("BoundedFileReader.read(from: readableURL, maximumBytes: maximumBytes)",),
    ),
    (
        ROOT / "platforms/ios/SharedUI/core/DiagnosticLog.swift",
        "static func readTail(from url: URL",
        "    let handle = FileHandle",
        ("O_NONBLOCK",),
    ),
    (
        ROOT / "platforms/ios/App/Services/LocalSpeechModel.swift",
        "    let data: Data",
        "    guard let object",
        ("BoundedFileReader.read(from: url, maximumBytes: Self.maximumManifestBytes)",),
    ),
    (
        ROOT / "shared/backend/storage/BackendBoundedFileReader.swift",
        "  static func read(from url: URL",
        "    #else",
        ("O_NOFOLLOW", "O_NONBLOCK"),
    ),
    (
        ROOT / "shared/backend/account/BackendAccountSession.swift",
        "  static func readBounded(_ url: URL",
        "  func save(_ session: BackendSavedSession)",
        ("BackendBoundedFileReader.read(from: url, maximumBytes: maximumBytes)",),
    ),
    (
        ROOT / "shared/backend/storage/BackendLocalStore.swift",
        "  private static func readBounded(_ url: URL",
        "  @discardableResult static func write(",
        ("BackendBoundedFileReader.read(from: url, maximumBytes: maximumBytes)",),
    ),
    (
        ROOT / "shared/backend/clients/BackendSnapshotClient.swift",
        "private func openBackendSnapshotSource(_ source: URL)",
        "\n\n\n/// A synchronous pull stream",
        ("O_NONBLOCK",),
    ),
    (
        ROOT / "shared/snapshot/Sources/MSIMESnapshot/Snapshot.swift",
        "private func openSnapshotSource(_ source: URL)",
        "\n}",
        ("O_NONBLOCK",),
    ),
)


def main() -> int:
    failed = False
    for path, start, end, required in CHECKS:
        body = region(path, start, end)
        missing = [token for token in required if token not in body]
        if missing:
            print(f"{path}: missing {', '.join(missing)}", file=sys.stderr)
            failed = True
    if not failed:
        print("iOS and macOS bounded readers reject special files without blocking")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
