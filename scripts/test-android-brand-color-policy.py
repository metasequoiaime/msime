#!/usr/bin/env python3
"""Android 品牌标记和品牌键共用一份标志绘制：配色固定、只定义一次，都不读皮肤的强调色。

标志在各平台都是同一块深色圆角方形加绿边和白折线（packages/ui/src/assets/msime.svg），不随皮肤变色。KeyboardBrandMark 持有这份配色和唯一的绘制函数，KeyboardBrandButton 只调用它；任何一方重新读强调色、或另抄一份配色，品牌键和候选栏的标志就会分叉。
"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
MARK = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardBrandMark.java"
BUTTON = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardBrandButton.java"
PALETTE = ("0xFF252525", "0xFFA8DF8E")


def main() -> int:
    mark = MARK.read_text(encoding="utf-8")
    button = BUTTON.read_text(encoding="utf-8")
    errors = []
    if mark.count("static void draw(Canvas canvas,") != 1:
        errors.append(f"{MARK}: 缺少唯一的共享绘制函数 draw(Canvas canvas, …)")
    if "KeyboardBrandMark.draw(" not in button:
        errors.append(f"{BUTTON}: 没有调用 KeyboardBrandMark.draw 画标志")
    for colour in PALETTE:
        if mark.count(colour) != 1:
            errors.append(f"{MARK}: 标志配色 {colour} 应在这里定义且只定义一次")
        if colour in button:
            errors.append(f"{BUTTON}: 另抄了一份标志配色 {colour}")
    for path, source in ((MARK, mark), (BUTTON, button)):
        if "getAsInt()" in source or "IntSupplier" in source:
            errors.append(f"{path}: 标志配色固定，不应再读皮肤的强调色")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android brand components share one fixed-colour mark drawing")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
