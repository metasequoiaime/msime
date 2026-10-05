#!/usr/bin/env python3
"""Generate `keyboard/KeyboardIconPaths.java` from the keyboard's SVG icon paths.

The IME cannot reach `R`, so the keyboard's icons are shipped as Java `Path` construction code instead of vector drawables. The paths below are the ones the Android design draws (design tokens §6.2 toolbar, §6.3 keys, §6.4 function panel, all viewBox 0 0 24 24); the few function-panel and one-hand entries the design leaves without an icon are Lucide-style strokes in the same grid, marked as such.

Every command is converted to absolute `moveTo` / `lineTo` / `cubicTo` / `close`; elliptical arcs become cubic Béziers here, so the Java side needs nothing beyond `android.graphics.Path`.

Usage: `generate_keyboard_icons.py` rewrites the Java file; `generate_keyboard_icons.py --check` exits non-zero when the committed file differs from what this script would write.
"""
from __future__ import annotations

import argparse
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "java/app/msime/android/keyboard/KeyboardIconPaths.java"

FILL = "FILL"
STROKE = "STROKE"

# (enum name, style, stroke width in viewBox units, source, path data)
ICONS: list[tuple[str, str, float, str, str]] = [
    # 6.2 toolbar: legacy Material Icons, filled.
    ("TOOLBAR_EMOJI", FILL, 0, "Material Icons sentiment_satisfied (outlined)",
     "M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm3.5-9c.83 0 1.5-.67 1.5-1.5S16.33 8 15.5 8 14 8.67 14 9.5s.67 1.5 1.5 1.5zm-7 0c.83 0 1.5-.67 1.5-1.5S9.33 8 8.5 8 7 8.67 7 9.5 7.67 11 8.5 11zm3.5 6.5c2.33 0 4.31-1.46 5.11-3.5H6.89c.8 2.04 2.78 3.5 5.11 3.5z"),
    ("TOOLBAR_PHRASE", FILL, 0, "Material Icons chat (outlined)",
     "M20 2H4c-1.1 0-1.99.9-1.99 2L2 22l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H5.17L4 17.17V4h16v12zM6 9h12v2H6zm0-3h12v2H6zm0 6h8v2H6z"),
    ("TOOLBAR_CLIPBOARD", FILL, 0, "Material Icons content_paste",
     "M19 2h-4.18C14.4.84 13.3 0 12 0c-1.3 0-2.4.84-2.82 2H5c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-7 0c.55 0 1 .45 1 1s-.45 1-1 1-1-.45-1-1 .45-1 1-1zm7 18H5V4h2v3h10V4h2v16z"),
    ("TOOLBAR_SKIN", FILL, 0, "Material Icons palette (outlined)",
     "M12 22C6.49 22 2 17.51 2 12S6.49 2 12 2s10 4.04 10 9c0 3.31-2.69 6-6 6h-1.77c-.28 0-.5.22-.5.5 0 .12.05.23.13.33.41.47.64 1.06.64 1.67 0 1.38-1.12 2.5-2.5 2.5zm0-18c-4.41 0-8 3.59-8 8s3.59 8 8 8c.28 0 .5-.22.5-.5 0-.16-.08-.28-.14-.35-.41-.46-.63-1.05-.63-1.65 0-1.38 1.12-2.5 2.5-2.5H16c2.21 0 4-1.79 4-4 0-3.86-3.59-7-8-7zM6.5 13a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3zm3-4a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3zm5 0a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3zm3 4a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3z"),
    ("TOOLBAR_SCHEME", FILL, 0, "Material Icons keyboard (outlined)",
     "M20 5H4c-1.1 0-1.99.9-1.99 2L2 17c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 12H4V7h16v10zm-9-9h2v2h-2zm0 3h2v2h-2zM8 8h2v2H8zm0 3h2v2H8zm-3 0h2v2H5zm0-3h2v2H5zm3 6h8v2H8zm6-3h2v2h-2zm0-3h2v2h-2zm3 3h2v2h-2zm0-3h2v2h-2z"),
    ("TOOLBAR_DISMISS", STROKE, 1.8, "design chevron-down (21dp)", "m6 9 6 6 6-6"),
    # 6.3 keys: strokes 1.7.
    ("SHIFT", STROKE, 1.7, "design key shift", "M12 4.5 4.5 12.5H8.5V19h7v-6.5h4z"),
    ("CAPS_LOCK", STROKE, 1.7, "design key caps lock", "M12 4.5 4.5 12.5H8.5V16h7v-3.5h4zM8.5 19.5h7"),
    ("BACKSPACE", STROKE, 1.7, "design key backspace",
     "M9 5.5h10.5a1.5 1.5 0 0 1 1.5 1.5v10a1.5 1.5 0 0 1-1.5 1.5H9L3 12zM11.5 9.5l5 5M16.5 9.5l-5 5"),
    ("RETURN", STROKE, 1.7, "design key return", "M19 6v5.5a2 2 0 0 1-2 2H6M9.5 10 6 13.5 9.5 17"),
    ("KEY_EMOJI", STROKE, 1.7, "design key emoji (123 layer)",
     "M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17zM8.5 14.2s1.2 1.8 3.5 1.8 3.5-1.8 3.5-1.8M9 9.8h.01M15 9.8h.01"),
    ("MIC", STROKE, 1.7, "design space mic",
     "M12 5a2 2 0 0 1 2 2v4a2 2 0 0 1-4 0V7a2 2 0 0 1 2-2zM8 10.5a4 4 0 0 0 8 0M12 14.5V17"),
    ("CHEVRON", STROKE, 1.8, "design candidate expand chevron (20dp)", "m6 9.5 6 6 6-6"),
    # 6.4 function panel: strokes 1.7.
    ("HANDWRITING", STROKE, 1.7, "design panel 手写", "M12 20h9M16.4 3.6a2.1 2.1 0 1 1 3 3L7 19l-4 1 1-4Z"),
    ("LEXICON", STROKE, 1.7, "design panel 词库",
     "M2 4h6a4 4 0 0 1 4 4v13a3 3 0 0 0-3-3H2zM22 4h-6a4 4 0 0 0-4 4v13a3 3 0 0 1 3-3h7z"),
    ("KEYBOARD_HEIGHT", STROKE, 1.7, "design panel 键盘高度",
     "M12 22v-6M12 8V2M4 12H2M10 12H8M16 12h-2M22 12h-2M15 19l-3 3-3-3M15 5l-3-3-3 3"),
    ("SETTINGS", STROKE, 1.7, "design panel 设置",
     "M20 7h-9M14 17H5M17 20a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM7 10a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"),
    ("KEY_SOUND", STROKE, 1.7, "design panel 按键音",
     "M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A1.4 1.4 0 0 1 5.4 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.4a1.4 1.4 0 0 1 1 .4l3.4 3.4a.7.7 0 0 0 1.2-.5zM16 9a5 5 0 0 1 0 6M19.4 18.4a9 9 0 0 0 0-12.8"),
    ("VIBRATION", STROKE, 1.7, "design panel 振动",
     "M2 8l2 2-2 2 2 2-2 2M22 8l-2 2 2 2-2 2 2 2M9 5h6a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z"),
    ("ONE_HAND", STROKE, 1.7, "design panel 单手模式",
     "M18 11V6a2 2 0 0 0-4 0M14 10V4a2 2 0 0 0-4 0v2M10 10.5V6a2 2 0 0 0-4 0v8M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-6-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15"),
    ("INCOGNITO", STROKE, 1.7, "design panel 隐私模式",
     "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1zM9 12l2 2 4-4"),
    ("FEEDBACK", STROKE, 1.7, "design panel 反馈",
     "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2zM13 8H7M17 12H7"),
    ("ABOUT", STROKE, 1.7, "design panel 关于",
     "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM12 16v-4M12 8h.01"),
    ("CHECK", STROKE, 4, "design on-state check badge", "m5 12.5 4.5 4.5L19 7.5"),
    # Lucide-style additions for entries the design draws no icon for.
    ("AI_ASSIST", STROKE, 1.7, "Lucide-style sparkles (AI 回复与润色)",
     "M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9zM19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"),
    ("LOCAL_INPUT", STROKE, 1.7, "Lucide-style laptop (本地输入)",
     "M4 6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v9H4zM2 19h20"),
    ("VOICE_RESULT", STROKE, 1.7, "Lucide-style mic (语音结果)",
     "M9 5a3 3 0 0 1 6 0v6a3 3 0 0 1-6 0zM5 10a7 7 0 0 0 14 0M12 17v4M8 21h8"),
    ("VIBRATION_STRENGTH", STROKE, 1.7, "Lucide-style activity (振动强度)", "M22 12h-4l-3 9L9 3l-3 9H2"),
    ("CLIPBOARD_HISTORY", STROKE, 1.7, "Lucide-style clipboard-list (剪贴板历史)",
     "M9 2h6a1 1 0 0 1 1 1v2a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1zM16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2M12 11h4M12 16h4M8 11h.01M8 16h.01"),
    ("SWAP_SIDE", STROKE, 1.8, "Lucide-style chevron-left (单手换边)", "m15 18-6-6 6-6"),
    ("EXIT_ONE_HAND", STROKE, 1.7, "Lucide-style maximize-2 (退出单手)",
     "M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"),
]

TOKEN = re.compile(r"[MmLlHhVvCcSsQqTtAaZz]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")
ARITY = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "S": 4, "Q": 4, "T": 2, "A": 7, "Z": 0}


def tokenize(data: str) -> list[str]:
    """Split path data into commands and numbers; arc flags may be written without separators."""
    tokens: list[str] = []
    index = 0
    command = ""
    arg_index = 0
    while index < len(data):
        char = data[index]
        if char in " ,\t\n":
            index += 1
            continue
        if char.isalpha():
            tokens.append(char)
            command = char.upper()
            arg_index = 0
            index += 1
            continue
        if command == "A" and arg_index % 7 in (3, 4) and char in "01":
            tokens.append(char)
            arg_index += 1
            index += 1
            continue
        match = TOKEN.match(data, index)
        if not match or match.group(0).isalpha():
            raise ValueError(f"bad path data at {index}: {data[index:index + 12]!r}")
        tokens.append(match.group(0))
        arg_index += 1
        index = match.end()
    return tokens


def arc_to_cubics(x1, y1, rx, ry, phi_deg, large, sweep, x2, y2):
    """SVG elliptical arc (endpoint parameterisation) to a list of cubic segments."""
    if (x1, y1) == (x2, y2):
        return []
    if rx == 0 or ry == 0:
        return [(x1, y1, x2, y2, x2, y2)]
    rx, ry = abs(rx), abs(ry)
    phi = math.radians(phi_deg % 360)
    cos_phi, sin_phi = math.cos(phi), math.sin(phi)
    dx, dy = (x1 - x2) / 2, (y1 - y2) / 2
    x1p = cos_phi * dx + sin_phi * dy
    y1p = -sin_phi * dx + cos_phi * dy
    lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry)
    if lam > 1:
        scale = math.sqrt(lam)
        rx *= scale
        ry *= scale
    num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
    den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
    coef = math.sqrt(max(0.0, num / den)) if den else 0.0
    if large == sweep:
        coef = -coef
    cxp = coef * rx * y1p / ry
    cyp = -coef * ry * x1p / rx
    cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) / 2
    cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) / 2

    def angle(ux, uy, vx, vy):
        dot = ux * vx + uy * vy
        length = math.hypot(ux, uy) * math.hypot(vx, vy)
        value = math.acos(max(-1.0, min(1.0, dot / length)))
        return -value if ux * vy - uy * vx < 0 else value

    theta1 = angle(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    delta = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
    if not sweep and delta > 0:
        delta -= 2 * math.pi
    elif sweep and delta < 0:
        delta += 2 * math.pi
    segments = max(1, math.ceil(abs(delta) / (math.pi / 2) - 1e-9))
    step = delta / segments
    alpha = 4 / 3 * math.tan(step / 4)
    result = []
    theta = theta1

    def point(t):
        ex, ey = rx * math.cos(t), ry * math.sin(t)
        return cos_phi * ex - sin_phi * ey + cx, sin_phi * ex + cos_phi * ey + cy

    def derivative(t):
        ex, ey = -rx * math.sin(t), ry * math.cos(t)
        return cos_phi * ex - sin_phi * ey, sin_phi * ex + cos_phi * ey

    for _ in range(segments):
        t2 = theta + step
        p1x, p1y = point(theta)
        p2x, p2y = point(t2)
        d1x, d1y = derivative(theta)
        d2x, d2y = derivative(t2)
        result.append((p1x + alpha * d1x, p1y + alpha * d1y,
                       p2x - alpha * d2x, p2y - alpha * d2y, p2x, p2y))
        theta = t2
    # Land exactly on the declared endpoint.
    last = result[-1]
    result[-1] = (last[0], last[1], last[2], last[3], x2, y2)
    return result


def to_operations(data: str) -> list[tuple]:
    """Absolute operations: ("M", x, y) / ("L", x, y) / ("C", x1, y1, x2, y2, x, y) / ("Z",)."""
    tokens = tokenize(data)
    ops: list[tuple] = []
    index = 0
    cx = cy = 0.0
    sx = sy = 0.0
    last_control: tuple[float, float] | None = None
    last_quad: tuple[float, float] | None = None
    command = ""
    while index < len(tokens):
        token = tokens[index]
        if token.isalpha():
            command = token
            index += 1
            if command in "Zz":
                ops.append(("Z",))
                cx, cy = sx, sy
                last_control = last_quad = None
                continue
        elif not command:
            raise ValueError("path data must start with a command")
        upper = command.upper()
        relative = command.islower()
        count = ARITY[upper]
        args = [float(value) for value in tokens[index:index + count]]
        if len(args) != count:
            raise ValueError(f"command {command} needs {count} numbers")
        index += count
        ox, oy = (cx, cy) if relative else (0.0, 0.0)
        if upper == "M":
            cx, cy = args[0] + ox, args[1] + oy
            sx, sy = cx, cy
            ops.append(("M", cx, cy))
            # Coordinates after the first pair of a moveto are implicit linetos.
            command = "l" if relative else "L"
            last_control = last_quad = None
        elif upper in "LHV":
            if upper == "L":
                cx, cy = args[0] + ox, args[1] + oy
            elif upper == "H":
                cx = args[0] + (cx if relative else 0.0)
            else:
                cy = args[0] + (cy if relative else 0.0)
            ops.append(("L", cx, cy))
            last_control = last_quad = None
        elif upper == "C":
            x1, y1, x2, y2, x, y = (args[0] + ox, args[1] + oy, args[2] + ox, args[3] + oy,
                                    args[4] + ox, args[5] + oy)
            ops.append(("C", x1, y1, x2, y2, x, y))
            last_control = (x2, y2)
            last_quad = None
            cx, cy = x, y
        elif upper == "S":
            if last_control is None:
                x1, y1 = cx, cy
            else:
                x1, y1 = 2 * cx - last_control[0], 2 * cy - last_control[1]
            x2, y2, x, y = args[0] + ox, args[1] + oy, args[2] + ox, args[3] + oy
            ops.append(("C", x1, y1, x2, y2, x, y))
            last_control = (x2, y2)
            last_quad = None
            cx, cy = x, y
        elif upper in "QT":
            if upper == "Q":
                qx, qy, x, y = args[0] + ox, args[1] + oy, args[2] + ox, args[3] + oy
            else:
                qx, qy = (cx, cy) if last_quad is None else (2 * cx - last_quad[0],
                                                             2 * cy - last_quad[1])
                x, y = args[0] + ox, args[1] + oy
            ops.append(("C", cx + 2 / 3 * (qx - cx), cy + 2 / 3 * (qy - cy),
                        x + 2 / 3 * (qx - x), y + 2 / 3 * (qy - y), x, y))
            last_quad = (qx, qy)
            last_control = None
            cx, cy = x, y
        elif upper == "A":
            x, y = args[5] + ox, args[6] + oy
            for segment in arc_to_cubics(cx, cy, args[0], args[1], args[2],
                                         args[3] != 0, args[4] != 0, x, y):
                ops.append(("C",) + segment)
            cx, cy = x, y
            last_control = last_quad = None
    return ops


def number(value: float) -> str:
    rounded = round(value, 4)
    if rounded == 0:
        rounded = 0.0
    text = f"{rounded:.4f}".rstrip("0").rstrip(".")
    return f"{text}f"


def java_body(ops: list[tuple]) -> list[str]:
    lines = []
    for op in ops:
        kind = op[0]
        if kind == "M":
            lines.append(f"p.moveTo({number(op[1])}, {number(op[2])});")
        elif kind == "L":
            lines.append(f"p.lineTo({number(op[1])}, {number(op[2])});")
        elif kind == "C":
            lines.append("p.cubicTo(" + ", ".join(number(value) for value in op[1:]) + ");")
        else:
            lines.append("p.close();")
    return lines


def render() -> str:
    out: list[str] = []
    out.append("// 由 platforms/android/scripts/generate_keyboard_icons.py 生成，不要手工编辑；改图标请改脚本里的 ICONS 表再重新运行。")
    out.append("// 来源：设计令牌 §6.2（工具栏 Material Icons）、§6.3（按键描边图标）、§6.4（功能面板描边图标），viewBox 0 0 24 24；设计未给图标的几项是同网格的 Lucide 风格补充。弧线已在脚本里转成三次贝塞尔。")
    out.append("package app.msime.android;")
    out.append("")
    out.append("import android.graphics.Canvas;")
    out.append("import android.graphics.Paint;")
    out.append("import android.graphics.Path;")
    out.append("")
    out.append("/** 键盘里用到的全部矢量图标：IME 进程不能用 `R`，图标以 `Path` 构造代码的形式随代码发布。 */")
    out.append("public final class KeyboardIconPaths {")
    out.append("    /** 图标坐标系的边长（SVG viewBox 0 0 24 24）。 */")
    out.append("    public static final float VIEWBOX = 24f;")
    out.append("")
    out.append("    /** 一个图标：实心填充，或按给定线宽描边（线宽以 viewBox 单位计，圆头圆角）。 */")
    out.append("    public enum Icon {")
    for position, (name, style, width, source, _data) in enumerate(ICONS):
        sep = ";" if position == len(ICONS) - 1 else ","
        out.append(f"        /** {source} */")
        out.append(f"        {name}({'true' if style == STROKE else 'false'}, {number(width)}){sep}")
    out.append("")
    out.append("        private final boolean stroked;")
    out.append("        private final float strokeWidth;")
    out.append("")
    out.append("        Icon(boolean stroked, float strokeWidth) {")
    out.append("            this.stroked = stroked;")
    out.append("            this.strokeWidth = strokeWidth;")
    out.append("        }")
    out.append("")
    out.append("        /** 为真时按 {@link #strokeWidth()} 描边，否则实心填充。 */")
    out.append("        public boolean stroked() { return stroked; }")
    out.append("")
    out.append("        /** 描边线宽，viewBox 单位；实心图标为 0。 */")
    out.append("        public float strokeWidth() { return strokeWidth; }")
    out.append("    }")
    out.append("")
    out.append("    private static final Path[] CACHE = new Path[Icon.values().length];")
    out.append("")
    out.append("    private KeyboardIconPaths() {}")
    out.append("")
    out.append("    /** 图标在 viewBox 坐标里的路径；结果被缓存，调用方不得修改它。 */")
    out.append("    public static Path path(Icon icon) {")
    out.append("        Path cached = CACHE[icon.ordinal()];")
    out.append("        if (cached != null) return cached;")
    out.append("        Path built = new Path();")
    out.append("        append(icon, built);")
    out.append("        CACHE[icon.ordinal()] = built;")
    out.append("        return built;")
    out.append("    }")
    out.append("")
    out.append("    /**")
    out.append("     * 在 {@code (left, top)} 起、边长 {@code size} 的方框里用 {@code color} 画出图标。")
    out.append("     *")
    out.append("     * <p>{@code paint} 的样式、线宽、端点与颜色会被改写；调用方为每个视图保留一支专用的 Paint。")
    out.append("     */")
    out.append("    public static void draw(Canvas canvas, Paint paint, Icon icon, float left, float top,")
    out.append("            float size, int color) {")
    out.append("        if (size <= 0) return;")
    out.append("        paint.setColor(color);")
    out.append("        if (icon.stroked()) {")
    out.append("            paint.setStyle(Paint.Style.STROKE);")
    out.append("            paint.setStrokeWidth(icon.strokeWidth());")
    out.append("            paint.setStrokeCap(Paint.Cap.ROUND);")
    out.append("            paint.setStrokeJoin(Paint.Join.ROUND);")
    out.append("        } else {")
    out.append("            paint.setStyle(Paint.Style.FILL);")
    out.append("        }")
    out.append("        int saved = canvas.save();")
    out.append("        canvas.translate(left, top);")
    out.append("        float scale = size / VIEWBOX;")
    out.append("        canvas.scale(scale, scale);")
    out.append("        canvas.drawPath(path(icon), paint);")
    out.append("        canvas.restoreToCount(saved);")
    out.append("    }")
    out.append("")
    out.append("    /** 把图标的路径追加到 {@code p}。 */")
    out.append("    public static void append(Icon icon, Path p) {")
    out.append("        switch (icon) {")
    for name, _style, _width, _source, _data in ICONS:
        out.append(f"            case {name} -> {method_name(name)}(p);")
    out.append("        }")
    out.append("    }")
    for name, _style, _width, _source, data in ICONS:
        out.append("")
        out.append(f"    private static void {method_name(name)}(Path p) {{")
        for line in java_body(to_operations(data)):
            out.append(f"        {line}")
        out.append("    }")
    out.append("}")
    return "\n".join(out) + "\n"


def method_name(enum_name: str) -> str:
    # A prefix keeps names such as RETURN from turning into Java keywords.
    return "append" + "".join(part.capitalize() for part in enum_name.lower().split("_"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="fail when the committed Java file differs from the generated one")
    arguments = parser.parse_args()
    generated = render()
    if arguments.check:
        current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
        if current != generated:
            print(f"{OUTPUT.relative_to(ROOT.parents[1])} is stale; run "
                  "platforms/android/scripts/generate_keyboard_icons.py", file=sys.stderr)
            return 1
        return 0
    OUTPUT.write_text(generated, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
