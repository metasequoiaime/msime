#!/usr/bin/env python3
"""Keep iOS vocabulary file-provider reads off the settings view's main actor."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "platforms/ios/App/Sources/settings/VocabularyReviewSettingsView.swift").read_text()


def main() -> None:
    start = SOURCE.index("  private func importWordbook(from url: URL)")
    end = SOURCE.index("  /// One in-flight operation", start)
    method = SOURCE[start:end]
    if "Task.detached" not in method:
        raise SystemExit("vocabulary import must run in a detached worker")
    before_worker = method.split("Task.detached", 1)[0]
    if "readWordbookData" in before_worker:
        raise SystemExit("vocabulary file data is read synchronously on the main actor")
    print("iOS vocabulary import reads and commits off the main actor")


if __name__ == "__main__":
    main()
