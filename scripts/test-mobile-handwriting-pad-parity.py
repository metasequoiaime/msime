#!/usr/bin/env python3
"""三个手机平台的触屏手写区必须按同一组份额分配行宽，虚线辅助框的内缩也一致（见 .agents/notes/implemented/bug-fix/2026-10-25-mobile-handwriting-pad-size.md）。

手写区是一行三列：左侧标点列、中间书写区、右侧 ⌫ 与「重写」列。iOS 用宽度约束的倍数、Android 用 LinearLayout 权重、HarmonyOS 用 layoutWeight 表达同一组份额；任一平台单独改动，三端的书写区大小就会再次分叉。Android 的书写卡片不画虚线框，只有 iOS 与 HarmonyOS 比较内缩。
"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
IOS_CONTROLLER = ROOT / "platforms/ios/KeyboardExtension/Sources/keyboard/KeyboardViewController.swift"
IOS_CARD = ROOT / "platforms/ios/SharedUI/input/HandwritingCanvasCard.swift"
ANDROID = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardGeometry.java"
HARMONY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"

# 每一列：名称，以及 iOS、Android、HarmonyOS 各自的常量正则。
SHARES = (
    (
        "标点列",
        (IOS_CONTROLLER, r"static let handwritingPunctuationShare: CGFloat = ([0-9.]+)"),
        (ANDROID, r"public static final float HANDWRITING_PUNCTUATION_WEIGHT = ([0-9.]+)f;"),
        (HARMONY, r"const HANDWRITING_PUNCTUATION_WEIGHT: number = ([0-9.]+);"),
    ),
    (
        "书写区",
        (IOS_CONTROLLER, r"static let handwritingCardShare: CGFloat = ([0-9.]+)"),
        (ANDROID, r"public static final float HANDWRITING_AREA_WEIGHT = ([0-9.]+)f;"),
        (HARMONY, r"const HANDWRITING_AREA_WEIGHT: number = ([0-9.]+);"),
    ),
    (
        "工具列",
        (IOS_CONTROLLER, r"static let handwritingToolsShare: CGFloat = ([0-9.]+)"),
        (ANDROID, r"public static final float HANDWRITING_TOOLS_WEIGHT = ([0-9.]+)f;"),
        (HARMONY, r"const HANDWRITING_TOOLS_WEIGHT: number = ([0-9.]+);"),
    ),
)
GUIDE_INSETS = (
    (IOS_CARD, r"static let guideInset: CGFloat = ([0-9.]+)"),
    (HARMONY, r"const HANDWRITING_GUIDE_INSET_VP: number = ([0-9.]+);"),
)
# 书写区至少占一行的这一比例；收回到旧的 .7 / 3 / .8（约 67%）时失败。
MIN_AREA_FRACTION = 0.74


def value(path: Path, pattern: str, errors: list[str]) -> float | None:
    matches = re.findall(pattern, path.read_text(encoding="utf-8"))
    if len(matches) != 1:
        errors.append(f"{path.relative_to(ROOT)}: 找不到唯一的 {pattern}")
        return None
    return float(matches[0])


def main() -> int:
    errors: list[str] = []
    shares: list[float] = []
    for name, *sources in SHARES:
        values = [value(path, pattern, errors) for path, pattern in sources]
        if None in values:
            continue
        if len(set(values)) != 1:
            errors.append(f"手写区{name}的份额在 iOS / Android / HarmonyOS 不一致: {values}")
        shares.append(values[0])
    if not errors:
        fraction = shares[1] / sum(shares)
        if fraction < MIN_AREA_FRACTION:
            errors.append(f"书写区只占一行的 {fraction:.1%}，低于 {MIN_AREA_FRACTION:.0%}")
    insets = [value(path, pattern, errors) for path, pattern in GUIDE_INSETS]
    if None not in insets and len(set(insets)) != 1:
        errors.append(f"虚线辅助框内缩在 iOS / HarmonyOS 不一致: {insets}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Mobile handwriting pad shares {shares[0]:g} / {shares[1]:g} / {shares[2]:g} and guide inset {insets[0]:g} match on iOS, Android and HarmonyOS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
