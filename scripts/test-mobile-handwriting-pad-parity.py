#!/usr/bin/env python3
"""三个手机平台的触屏手写键盘必须同一个样子（见 .agents/notes/implemented/feature/2026-10-25-mobile-handwriting-full-width-pad.md）。

书写区铺满键区宽度，两侧不再有标点列和工具列；竖屏比其他布局多一行键高，书写区有四行高，横屏保留三行。⌫ 和「重写」在底行，底行是 123 |（地球键）| ， | 空格 | 。 | 中 | 重写 | ⌫ | 回车，三端同一组宽度份额；原来书写区左侧的 ？ ！ 由 ， 和 。 的长按输入。iOS 用约束倍数、Android 用 LinearLayout 权重、HarmonyOS 用 layoutWeight 表达同一组份额，任一平台单独改动，三端的手写键盘就会再次分叉。地球键只在宿主要求时出现（iOS、Android），HarmonyOS 没有这个键，比较顺序时不算它。
"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
IOS_CONTROLLER = ROOT / "platforms/ios/KeyboardExtension/Sources/keyboard/KeyboardViewController.swift"
IOS_FORM_FACTOR = ROOT / "platforms/ios/KeyboardExtension/Sources/keyboard/KeyboardFormFactor.swift"
IOS_CARD = ROOT / "platforms/ios/SharedUI/input/HandwritingCanvasCard.swift"
ANDROID_ROW = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardActionRow.java"
ANDROID_GEOMETRY = ROOT / "platforms/android/java/app/msime/android/keyboard/KeyboardGeometry.java"
ANDROID_PAD = ROOT / "platforms/android/java/app/msime/android/core/ImeLayoutRows.java"
ANDROID_SERVICE = ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java"
ANDROID_BOTTOM = ROOT / "platforms/android/java/app/msime/android/core/ImeBottomRow.java"
HARMONY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardView.ets"
HARMONY_METRICS = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeyboardMetrics.ts"

# 底行每个键：名称，以及 iOS、Android、HarmonyOS 各自的份额常量。
WEIGHTS = (
    ("123", "handwritingLayerWeight", "HANDWRITING_LAYER_WEIGHT", "HANDWRITING_LAYER_SHARE"),
    ("， 。", "handwritingPunctuationWeight", "HANDWRITING_PUNCTUATION_WEIGHT", "HANDWRITING_PUNCTUATION_SHARE"),
    ("空格", "handwritingSpaceWeight", "HANDWRITING_SPACE_WEIGHT", "HANDWRITING_SPACE_SHARE"),
    ("中", "handwritingLanguageWeight", "HANDWRITING_LANGUAGE_WEIGHT", "HANDWRITING_LANGUAGE_SHARE"),
    ("重写", "handwritingRewriteWeight", "HANDWRITING_REWRITE_WEIGHT", "HANDWRITING_REWRITE_SHARE"),
    ("⌫", "handwritingDeleteWeight", "HANDWRITING_DELETE_WEIGHT", "HANDWRITING_DELETE_SHARE"),
    ("回车", "handwritingReturnWeight", "HANDWRITING_RETURN_WEIGHT", "HANDWRITING_RETURN_SHARE"),
)
# 底行的键序（不含只在宿主要求时出现的地球键）。
ROW = ["LAYER", "COMMA", "SPACE", "PERIOD", "LANGUAGE", "REWRITE", "DELETE", "RETURN"]
# iOS 底行的键序由 arrangeActionRow 给手机底行（逗号不兼句号时，即手写和 14 键）排的那份 order 决定；这些变量是手写底行用到的键。
IOS_KEYS = {
    "toggle": "LAYER", "globe": "GLOBE", "comma": "COMMA", "space": "SPACE", "period": "PERIOD",
    "language": "LANGUAGE", "rewrite": "REWRITE", "handwritingDelete": "DELETE", "enter": "RETURN",
}
# HarmonyOS handwritingActionRow 里每个键的标志，按在构建器里出现的先后排列。
HARMONY_KEYS = (
    ("LAYER", "action: () => this.toggleSymbolLayer()"),
    ("COMMA", "action: () => this.typeComma()"),
    ("SPACE", "this.spaceKey(true, HANDWRITING_SPACE_SHARE)"),
    ("PERIOD", "action: () => this.typePeriod()"),
    ("LANGUAGE", "share: HANDWRITING_LANGUAGE_SHARE"),
    ("REWRITE", "share: HANDWRITING_REWRITE_SHARE"),
    ("DELETE", "share: HANDWRITING_DELETE_SHARE"),
    ("RETURN", "this.designReturnKey(HANDWRITING_RETURN_SHARE"),
)
GUIDE_INSETS = (
    (IOS_CARD, r"static let guideInset: CGFloat = ([0-9.]+)"),
    (HARMONY, r"const HANDWRITING_GUIDE_INSET_VP: number = ([0-9.]+);"),
)
# 最窄的手机宽度（iOS 320pt 浮动键盘，带地球键）上，， 和 。 不得窄于这么多 pt。26 键字母键在 320pt 上约 26pt。
NARROW_WIDTH = 320.0
NARROW_PADDING = 6.0
KEY_SPACING = 6.0
MIN_PUNCTUATION_WIDTH = 22.0


def text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def body(source: str, start: str, path: Path, errors: list[str]) -> str:
    """从 `start` 开始到下一个同缩进的右花括号为止的一段源码。"""
    index = source.find(start)
    if index < 0:
        errors.append(f"{path.relative_to(ROOT)}: 找不到 {start}")
        return ""
    indent = len(start) - len(start.lstrip(" "))
    end = source.find("\n" + " " * indent + "}", index)
    return source[index:end if end >= 0 else len(source)]


def value(path: Path, pattern: str, errors: list[str]) -> float | None:
    matches = re.findall(pattern, text(path))
    if len(matches) != 1:
        errors.append(f"{path.relative_to(ROOT)}: 找不到唯一的 {pattern}")
        return None
    return float(matches[0])


def require(path: Path, needle: str, errors: list[str], why: str, source: str | None = None) -> None:
    if needle not in (source if source is not None else text(path)):
        errors.append(f"{path.relative_to(ROOT)}: 缺少 {needle!r}（{why}）")


def weights(errors: list[str]) -> dict[str, float]:
    shares: dict[str, float] = {}
    for name, ios, android, harmony in WEIGHTS:
        values = [
            value(IOS_CONTROLLER, rf"static let {ios}: CGFloat = ([0-9.]+)\n", errors),
            value(ANDROID_ROW, rf"public static final float {android} = ([0-9.]+)f;", errors),
            value(HARMONY, rf"const {harmony}: number = ([0-9.]+);", errors),
        ]
        if None in values:
            continue
        if len(set(values)) != 1:
            errors.append(f"手写底行「{name}」的份额在 iOS / Android / HarmonyOS 不一致: {values}")
        shares[name] = values[0]
    return shares


def rows(errors: list[str]) -> None:
    android = body(text(ANDROID_ROW), "    private static List<DesignEntry> handwritingEntries(", ANDROID_ROW, errors)
    android_row = [slot for slot in re.findall(r"DesignSlot\.([A-Z_]+)", android) if slot != "GLOBE"]
    arrange = body(text(IOS_CONTROLLER), "  private func arrangeActionRow(", IOS_CONTROLLER, errors)
    phone = re.search(r"case \.phone:\n\s*order = commaSecondary\n\s*\? \[[^\]]*\]\n\s*: \[([^\]]*)\]", arrange)
    if phone is None:
        errors.append(f"{IOS_CONTROLLER.relative_to(ROOT)}: arrangeActionRow 找不到手机底行的键序")
        ios_row = []
    else:
        names = [name.strip() for name in phone.group(1).split(",")]
        ios_row = [IOS_KEYS[name] for name in names if name in IOS_KEYS and IOS_KEYS[name] != "GLOBE"]
    harmony = body(text(HARMONY), "  handwritingActionRow() {", HARMONY, errors)
    positions = [(harmony.find(marker), slot) for slot, marker in HARMONY_KEYS]
    missing = [slot for index, slot in positions if index < 0]
    if missing:
        errors.append(f"{HARMONY.relative_to(ROOT)}: handwritingActionRow 缺少 {missing}")
    harmony_row = [slot for _, slot in sorted(positions) if _ >= 0]
    for name, found in (("iOS", ios_row), ("Android", android_row), ("HarmonyOS", harmony_row)):
        if found != ROW:
            errors.append(f"{name} 的手写底行是 {found}，应为 {ROW}")


def full_width_pad(errors: list[str]) -> None:
    ios = text(IOS_CONTROLLER)
    require(IOS_CONTROLLER, "keyColumn.addArrangedSubview(handwriting)\n", errors, "书写区直接占满键区这一行", ios)
    if "static let handwritingPunctuation =" in ios or "handwritingCardShare" in ios:
        errors.append(f"{IOS_CONTROLLER.relative_to(ROOT)}: 书写区两侧又出现了标点列或份额")
    android = body(text(ANDROID_PAD), "    void rebuildHandwritingRows() {", ANDROID_PAD, errors)
    if android.count("row.addView(") != 1 or "NineKeyLayout.punctuation()" in android:
        errors.append(f"{ANDROID_PAD.relative_to(ROOT)}: rebuildHandwritingRows 的一行里只能有书写区")
    harmony = body(text(HARMONY), "  handwritingPad() {", HARMONY, errors)
    if "NineKeyLayout.punctuation()" in harmony or "deleteHandwriting" in harmony or "clearHandwriting" in harmony:
        errors.append(f"{HARMONY.relative_to(ROOT)}: handwritingPad 的一行里只能有书写区")


def heights(errors: list[str]) -> None:
    require(IOS_FORM_FACTOR, "static func handwritingAddsRow(handwriting: Bool, landscape: Bool) -> Bool { handwriting && !landscape }",
            errors, "竖屏手写多一行，横屏不加")
    portrait = value(ANDROID_GEOMETRY, r"HANDWRITING_PORTRAIT_ROWS = ([0-9]+);", errors)
    landscape = value(ANDROID_GEOMETRY, r"HANDWRITING_LANDSCAPE_ROWS = ([0-9]+);", errors)
    if portrait is not None and landscape is not None and (portrait, landscape) != (4, 3):
        errors.append(f"Android 手写区竖屏 / 横屏行数是 {portrait:g} / {landscape:g}，应为 4 / 3")
    require(HARMONY_METRICS, "return handwritingFace && !landscape ? 1 : 0;", errors, "竖屏手写多一行，横屏不加")
    require(ANDROID_PAD, "KeyboardGeometry.handwritingRows(", errors, "手写区按横竖屏取行数")


def marks(errors: list[str]) -> None:
    ios = text(IOS_CONTROLLER)
    for needle in ('self?.handleSymbol("?")', 'self?.handleSymbol("!")', 'Self.chineseSymbolFaces["?"]', 'Self.chineseSymbolFaces["!"]'):
        require(IOS_CONTROLLER, needle, errors, "， 长按 ？、。 长按 ！，带角标", ios)
    require(ANDROID_SERVICE, "type(commaSecondaryPeriod() ? '.' : '?');", errors, "手写底行的逗号长按输入问号")
    require(ANDROID_BOTTOM, "s.type('!');", errors, "手写底行的句号长按输入叹号")
    harmony = body(text(HARMONY), "  handwritingActionRow() {", HARMONY, errors)
    for needle in ("KeyboardSession.shared.punctuation(0x3f)", "KeyboardSession.shared.punctuation(0x21)", "cornerHint:"):
        require(HARMONY, needle, errors, "， 长按 ？、。 长按 ！，带角标", harmony)


def narrow(shares: dict[str, float], errors: list[str]) -> float | None:
    if len(shares) != len(WEIGHTS):
        return None
    total = sum(shares.values()) + shares["， 。"] + 1  # 两个标点键，加一个地球键
    keys = len(WEIGHTS) + 2
    unit = (NARROW_WIDTH - NARROW_PADDING - KEY_SPACING * (keys - 1)) / total
    width = unit * shares["， 。"]
    if width < MIN_PUNCTUATION_WIDTH:
        errors.append(f"{NARROW_WIDTH:g}pt 宽带地球键时 ， 只有 {width:.1f}pt，窄于 {MIN_PUNCTUATION_WIDTH:g}pt")
    return width


def main() -> int:
    errors: list[str] = []
    shares = weights(errors)
    rows(errors)
    full_width_pad(errors)
    heights(errors)
    marks(errors)
    insets = [value(path, pattern, errors) for path, pattern in GUIDE_INSETS]
    if None not in insets and len(set(insets)) != 1:
        errors.append(f"虚线辅助框内缩在 iOS / HarmonyOS 不一致: {insets}")
    width = narrow(shares, errors)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Mobile handwriting keyboard matches on iOS, Android and HarmonyOS: full-width pad, four rows in portrait, "
          f"bottom row {' | '.join(ROW)} with shares {' / '.join(f'{v:g}' for v in shares.values())}, "
          f"guide inset {insets[0]:g}, ， {width:.1f}pt at {NARROW_WIDTH:g}pt with the globe key")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
