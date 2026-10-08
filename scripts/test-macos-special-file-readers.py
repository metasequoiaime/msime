#!/usr/bin/env python3
"""macOS bounded readers must reject special files without blocking."""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]


CHECKS = (
    (
        ROOT / "platforms/macos/src/dictionary/DictionaryInstaller.mm",
        "static BOOL StageDictionary(",
        "BOOL MSIMEInstallDictionary(",
    ),
    (
        ROOT / "platforms/macos/src/voice/LocalVoiceRequest.mm",
        "NSData *BoundedModelManifestData(",
        "bool TrustedModelPathLink(",
    ),
    (
        ROOT / "platforms/macos/src/settings/RuntimeOptions.h",
        "static inline NSData *MSIMEReadRuntimeOptionsData(",
        "// 设置应用的 bundle identifier",
    ),
)


def main() -> int:
    failed = False
    for path, start, end in CHECKS:
        source = path.read_text(encoding="utf-8")
        begin = source.index(start)
        region = source[begin : source.index(end, begin)]
        required = ("O_NOFOLLOW", "O_NONBLOCK", "fstat", "S_ISREG", "open(", "read(")
        missing = [token for token in required if token not in region]
        if missing:
            print(f"{path}: reader missing {', '.join(missing)}", file=sys.stderr)
            failed = True
    if not failed:
        print("macOS bounded readers reject special files without blocking")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
