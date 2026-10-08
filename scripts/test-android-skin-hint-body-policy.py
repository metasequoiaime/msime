#!/usr/bin/env python3
"""键盘皮肤提示文件读取复用共享有界 body 策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    region = source.split("    private JSONObject readSkinHint()", 1)[1].split(
        "    /**", 1
    )[0]
    if not any(
        f"HttpBodyPolicy.{method}" in region
        for method in ("readBounded", "readRequired")
    ):
        print(f"{SOURCE}: 没有使用共享 HttpBodyPolicy 有界读取", file=sys.stderr)
        return 1
    if "Files.readAllBytes" in region:
        print(f"{SOURCE}: 仍使用无界 Files.readAllBytes", file=sys.stderr)
        return 1
    write_region = source.split("    private void rememberSkinHint", 1)[1].split(
        "    /**", 1
    )[0]
    if 'SKIN_HINT_FILE + ".pending"' in write_region:
        print(f"{SOURCE}: 仍复用固定的皮肤提示临时文件", file=sys.stderr)
        return 1
    if "Files.createTempFile" not in write_region or "LinkOption.NOFOLLOW_LINKS" not in write_region:
        print(f"{SOURCE}: 皮肤提示写入没有使用唯一临时文件和 NOFOLLOW_LINKS", file=sys.stderr)
        return 1
    print("Android skin hint reads use the shared bounded body policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
