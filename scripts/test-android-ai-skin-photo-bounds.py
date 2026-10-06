#!/usr/bin/env python3
"""检查 Android AI 皮肤图片先探测尺寸再有界解码。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/home/AiSkinPage.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    region = source.split("    private static JSONObject withPhoto", 1)[1].split(
        "    private void useResult", 1
    )[0]
    if "inJustDecodeBounds = true" not in region:
        print(f"{SOURCE}: AI 皮肤图片解码前没有探测尺寸", file=sys.stderr)
        return 1
    if "PhotoDecodePolicy.sampleSize" not in region:
        print(f"{SOURCE}: AI 皮肤图片解码没有使用有界采样策略", file=sys.stderr)
        return 1
    if "BitmapFactory.decodeByteArray(bytes, 0, bytes.length);" in region:
        print(f"{SOURCE}: 仍然存在全尺寸 AI 皮肤图片解码", file=sys.stderr)
        return 1
    print("Android AI skin photos use bounded decode dimensions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
