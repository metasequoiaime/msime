#!/usr/bin/env python3
"""Android 跨进程文件读取必须以 NOFOLLOW_LINKS 打开。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
READERS = (
    ROOT / "platforms/android/java/app/msime/android/KeyboardFeedbackFileReader.java",
    ROOT / "platforms/android/java/app/msime/android/core/Bootstrap.java",
    ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java",
    ROOT / "platforms/android/java/app/msime/android/dictionary/CustomSkinLibrary.java",
    ROOT / "platforms/android/java/app/msime/android/dictionary/DictionarySnapshotQueue.java",
    ROOT / "platforms/android/java/app/msime/android/home/CloudSync.java",
    ROOT / "platforms/android/java/app/msime/android/voice/VoiceResultStore.java",
    ROOT / "platforms/android/java/app/msime/android/account/SyncApi.java",
    ROOT / "platforms/android/java/app/msime/android/account/UpdateApi.java",
    ROOT / "platforms/android/java/app/msime/android/account/DiagnosticsApi.java",
    ROOT / "platforms/android/java/app/msime/android/core/NativeClient.java",
)


def main() -> int:
    ok = True
    for path in READERS:
        source = path.read_text(encoding="utf-8")
        for line_number, line in enumerate(source.splitlines(), start=1):
            if (("newInputStream(" in line or "new FileInputStream(" in line)
                    and "NOFOLLOW_LINKS" not in line):
                print(f"{path}:{line_number}: 文件读取没有使用 LinkOption.NOFOLLOW_LINKS", file=sys.stderr)
                ok = False
    if ok:
        print("Android cross-process file readers open files without following symlinks")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
