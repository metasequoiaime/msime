#!/usr/bin/env python3
"""生成 candidate-skins 下各验证皮肤的图片：每个皮肤的 preview.png，`verify-canvas` 的九宫格面板与三个挂件，`verify-canvas-frame` 的中空面板框与四个挂件（含一张 GIF），`verify-v2-motion-system` 的锦鲤 APNG。

只用标准库（zlib、struct、tomllib），不读随机数和时间，同一份脚本每次写出同样的像素。预览图的颜色、圆角、外框、阴影、翻页箭头和展开面板都从各自的 skin.toml 读，改了清单重跑一次即可：

    timeout 60 python3 crates/client-services/tests/fixtures/candidate-skins/generate.py

预览图只是示意：候选文字画成类似汉字的笔画块，拼音和序号用 5×7 点阵字，几何按 1 DIP = 1 像素、候选字号 13 估算，不追求与任何宿主逐像素一致。
"""

import math
import pathlib
import struct
import tomllib
import zlib

ROOT = pathlib.Path(__file__).resolve().parent
FONT_PX = 13

# ---- PNG / APNG 编码 ----


def _chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


def _scanlines(width, height, rgba):
    stride = width * 4
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        rows += rgba[y * stride:(y + 1) * stride]
    return zlib.compress(bytes(rows), 9)


def encode_png(width, height, rgba):
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", header) + _chunk(b"IDAT", _scanlines(width, height, rgba)) + _chunk(b"IEND", b"")


def encode_apng(width, height, frames, delay_ms):
    """每帧整幅画布、无限循环的 APNG；第一帧同时是默认图（`fcTL` 在 `IDAT` 之前），旧解码器看到的就是它。"""
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    out = b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", header) + _chunk(b"acTL", struct.pack(">II", len(frames), 0))
    sequence = 0
    for index, rgba in enumerate(frames):
        control = struct.pack(">IIIIIHHBB", sequence, width, height, 0, 0, delay_ms, 1000, 0, 0)
        out += _chunk(b"fcTL", control)
        sequence += 1
        data = _scanlines(width, height, rgba)
        if index == 0:
            out += _chunk(b"IDAT", data)
        else:
            out += _chunk(b"fdAT", struct.pack(">I", sequence) + data)
            sequence += 1
    return out + _chunk(b"IEND", b"")


def _lzw(indices, minimum):
    """GIF 的变长 LZW 压缩，按 GIF89a 的规则在码表满 4096 项时清表重来。"""
    clear, end = 1 << minimum, (1 << minimum) + 1
    out, buffer, bits = bytearray(), 0, 0
    width = minimum + 1

    def emit(code):
        nonlocal buffer, bits
        buffer |= code << bits
        bits += width
        while bits >= 8:
            out.append(buffer & 0xFF)
            buffer >>= 8
            bits -= 8

    table = {(i,): i for i in range(clear)}
    emit(clear)
    current = ()
    for index in indices:
        candidate = current + (index,)
        if candidate in table:
            current = candidate
            continue
        emit(table[current])
        if len(table) + 2 < 4096:
            table[candidate] = len(table) + 2
            if len(table) + 1 >= 1 << width and width < 12:
                width += 1
        else:
            emit(clear)
            table = {(i,): i for i in range(clear)}
            width = minimum + 1
        current = (index,)
    if current:
        emit(table[current])
    emit(end)
    if bits:
        out.append(buffer & 0xFF)
    return bytes(out)


def encode_gif(width, height, palette, frames, delay_ms):
    """调色板 GIF89a 动图：每帧整幅画布、无限循环，下标 0 是透明色，`palette` 是 (r, g, b) 的列表（不超过 255 项，从下标 1 起），`frames` 是每帧的下标数组。"""
    colors = [(0, 0, 0)] + list(palette)
    size = 2
    while (1 << size) < len(colors):
        size += 1
    colors += [(0, 0, 0)] * ((1 << size) - len(colors))
    out = bytearray(b"GIF89a" + struct.pack("<HHBBB", width, height, 0x80 | (size - 1), 0, 0))
    for r, g, b in colors:
        out += bytes((r, g, b))
    out += b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\x00\x00\x00"
    for indices in frames:
        # 透明色、换帧前恢复成背景（disposal 2），延时以 1/100 秒计。
        out += b"\x21\xF9\x04" + bytes((0x09,)) + struct.pack("<H", delay_ms // 10) + b"\x00\x00"
        out += b"\x2C" + struct.pack("<HHHHB", 0, 0, width, height, 0)
        data = _lzw(indices, size)
        out.append(size)
        for start in range(0, len(data), 255):
            block = data[start:start + 255]
            out.append(len(block))
            out += block
        out.append(0)
    out.append(0x3B)
    return bytes(out)


# ---- 颜色 ----


def normalized_color(value):
    """与 `theme::normalized_color` 相同的规则：读得懂的写法一律换成 `#RRGGBB` 或 `#RRGGBBAA`，读不懂的返回 None。"""
    value = value.strip()
    if value.lower() == "transparent":
        return "#00000000"
    if value.startswith("#"):
        digits = value[1:]
        if not all(c in "0123456789abcdefABCDEF" for c in digits):
            return None
        if len(digits) in (3, 4):
            return "#" + "".join(c * 2 for c in digits).upper()
        if len(digits) in (6, 8):
            return "#" + digits.upper()
        return None
    lower = value.lower()
    if lower.startswith("rgba(") and lower.endswith(")"):
        parts, alpha = [p.strip() for p in lower[5:-1].split(",")], True
    elif lower.startswith("rgb(") and lower.endswith(")"):
        parts, alpha = [p.strip() for p in lower[4:-1].split(",")], False
    else:
        return None
    if len(parts) != (4 if alpha else 3):
        return None
    red, green, blue = (int(p) for p in parts[:3])
    if not alpha:
        return f"#{red:02X}{green:02X}{blue:02X}"
    a = float(parts[3])
    return f"#{red:02X}{green:02X}{blue:02X}{math.floor(a * 255 + 0.5):02X}"


def rgba(value):
    """颜色写法 → 0–1 的非预乘 (r, g, b, a)。"""
    text = normalized_color(value)
    if text is None:
        raise ValueError(f"unreadable colour {value!r}")
    digits = text[1:]
    channels = [int(digits[i:i + 2], 16) / 255 for i in range(0, len(digits), 2)]
    if len(channels) == 3:
        channels.append(1.0)
    return tuple(channels)


def with_alpha(color, alpha):
    return (color[0], color[1], color[2], color[3] * alpha)


def mix(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(4))


# ---- 光栅 ----


def clamp(value, low=0.0, high=1.0):
    return low if value < low else high if value > high else value


class Canvas:
    """预乘 alpha 的浮点 RGBA 画布。形状都用有向距离场给出，边缘按距离做抗锯齿。"""

    def __init__(self, width, height, color=(0, 0, 0, 0)):
        self.w, self.h = width, height
        premultiplied = (color[0] * color[3], color[1] * color[3], color[2] * color[3], color[3])
        self.px = [list(premultiplied) for _ in range(width * height)]

    def blend(self, index, color, coverage):
        alpha = color[3] * coverage
        if alpha <= 0:
            return
        pixel = self.px[index]
        keep = 1 - alpha
        pixel[0] = color[0] * alpha + pixel[0] * keep
        pixel[1] = color[1] * alpha + pixel[1] * keep
        pixel[2] = color[2] * alpha + pixel[2] * keep
        pixel[3] = alpha + pixel[3] * keep

    def paint(self, shape, color):
        """`shape` 是 (距离函数, 包围盒)；`color` 是颜色或 (x, y) → 颜色的函数。"""
        distance, (x0, y0, x1, y1) = shape
        for y in range(max(0, math.floor(y0)), min(self.h, math.ceil(y1))):
            py = y + 0.5
            for x in range(max(0, math.floor(x0)), min(self.w, math.ceil(x1))):
                px = x + 0.5
                coverage = 0.5 - distance(px, py)
                if coverage <= 0:
                    continue
                self.blend(y * self.w + x, color(px, py) if callable(color) else color, min(coverage, 1.0))

    def paint_coverage(self, bounds, coverage_at, color):
        x0, y0, x1, y1 = bounds
        for y in range(max(0, math.floor(y0)), min(self.h, math.ceil(y1))):
            for x in range(max(0, math.floor(x0)), min(self.w, math.ceil(x1))):
                coverage = coverage_at(x + 0.5, y + 0.5)
                if coverage > 0:
                    self.blend(y * self.w + x, color, min(coverage, 1.0))

    def sample(self, u, v):
        """双线性取样（源像素坐标，越界夹到边上），返回预乘的 RGBA。"""
        u = clamp(u - 0.5, 0, self.w - 1)
        v = clamp(v - 0.5, 0, self.h - 1)
        x0, y0 = int(u), int(v)
        x1, y1 = min(x0 + 1, self.w - 1), min(y0 + 1, self.h - 1)
        fx, fy = u - x0, v - y0
        a, b = self.px[y0 * self.w + x0], self.px[y0 * self.w + x1]
        c, d = self.px[y1 * self.w + x0], self.px[y1 * self.w + x1]
        return [
            (a[i] * (1 - fx) + b[i] * fx) * (1 - fy) + (c[i] * (1 - fx) + d[i] * fx) * fy
            for i in range(4)
        ]

    def composite(self, index, premultiplied, opacity=1.0):
        keep = 1 - premultiplied[3] * opacity
        pixel = self.px[index]
        for i in range(4):
            pixel[i] = premultiplied[i] * opacity + pixel[i] * keep

    def draw_image(self, image, x, y, width, height, opacity=1.0):
        """把 `image` 等比缩放后居中放进 (x, y, width, height) 的框里，与挂件的绘制方式相同。"""
        scale = min(width / image.w, height / image.h)
        dw, dh = image.w * scale, image.h * scale
        left, top = x + (width - dw) / 2, y + (height - dh) / 2
        for py in range(max(0, math.floor(top)), min(self.h, math.ceil(top + dh))):
            for px in range(max(0, math.floor(left)), min(self.w, math.ceil(left + dw))):
                u, v = (px + 0.5 - left) / scale, (py + 0.5 - top) / scale
                if 0 <= u <= image.w and 0 <= v <= image.h:
                    self.composite(py * self.w + px, image.sample(u, v), opacity)

    def nine_slice(self, image, x0, y0, x1, y1, slice_px, border, fill=True):
        """CSS `border-image` 的 stretch：四角按 `border` 宽画切片，四边与中心拉伸，`fill` 为假时中心不画。`slice_px` 与 `border` 是一个数（四边相同）或 (上下, 左右)。"""
        slice_y, slice_x = slice_px if isinstance(slice_px, tuple) else (slice_px, slice_px)
        border_y, border_x = border if isinstance(border, tuple) else (border, border)

        def source(position, start, end, size, slice_px, border):
            if position < start + border:
                return (position - start) / border * slice_px, False
            if position >= end - border:
                return size - slice_px + (position - (end - border)) / border * slice_px, False
            middle = (end - start) - 2 * border
            return slice_px + (position - start - border) / middle * (size - 2 * slice_px), True

        for py in range(max(0, math.floor(y0)), min(self.h, math.ceil(y1))):
            v, inner_y = source(py + 0.5, y0, y1, image.h, slice_y, border_y)
            for px in range(max(0, math.floor(x0)), min(self.w, math.ceil(x1))):
                u, inner_x = source(px + 0.5, x0, x1, image.w, slice_x, border_x)
                if inner_x and inner_y and not fill:
                    continue
                self.composite(py * self.w + px, image.sample(u, v))

    def paste(self, other, x, y):
        for py in range(other.h):
            for px in range(other.w):
                self.px[(y + py) * self.w + x + px] = list(other.px[py * other.w + px])

    def to_bytes(self):
        out = bytearray()
        for r, g, b, a in self.px:
            if a <= 0:
                out += b"\x00\x00\x00\x00"
                continue
            out += bytes(int(clamp(c / a) * 255 + 0.5) for c in (r, g, b))
            out.append(int(clamp(a) * 255 + 0.5))
        return bytes(out)

    def save(self, path):
        path.write_bytes(encode_png(self.w, self.h, self.to_bytes()))


def rounded_box(x0, y0, x1, y1, radius):
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    hw, hh = (x1 - x0) / 2, (y1 - y0) / 2
    radius = max(0.0, min(radius, hw, hh))

    def distance(x, y):
        qx, qy = abs(x - cx) - (hw - radius), abs(y - cy) - (hh - radius)
        return math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - radius

    return distance, (x0 - 1, y0 - 1, x1 + 1, y1 + 1)


def ring(outer, inner):
    outer_distance, bounds = outer
    inner_distance = inner[0]
    return (lambda x, y: max(outer_distance(x, y), -inner_distance(x, y))), bounds


def circle(cx, cy, radius):
    return (lambda x, y: math.hypot(x - cx, y - cy) - radius), (cx - radius - 1, cy - radius - 1, cx + radius + 1, cy + radius + 1)


def ellipse(cx, cy, rx, ry):
    small = min(rx, ry)
    return (lambda x, y: (math.hypot((x - cx) / rx, (y - cy) / ry) - 1) * small), (cx - rx - 1, cy - ry - 1, cx + rx + 1, cy + ry + 1)


def _segment_distance(x, y, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    t = clamp(((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy or 1))
    return math.hypot(x - ax - dx * t, y - ay - dy * t)


def strokes(segments, width):
    """若干条圆头线段的并集，`segments` 是 [(ax, ay, bx, by), ...]。"""
    half = width / 2
    xs = [v for s in segments for v in (s[0], s[2])]
    ys = [v for s in segments for v in (s[1], s[3])]

    def distance(x, y):
        return min(_segment_distance(x, y, *s) for s in segments) - half

    return distance, (min(xs) - half - 1, min(ys) - half - 1, max(xs) + half + 1, max(ys) + half + 1)


def polyline(points, width):
    return strokes([points[i] + points[i + 1] for i in range(len(points) - 1)], width)


def polygon(points, rounding=0.0):
    """凸多边形；`rounding` 把角磨圆（形状向外扩出同样的距离）。"""
    area = sum(points[i][0] * points[i - 1][1] - points[i - 1][0] * points[i][1] for i in range(len(points)))
    sign = 1 if area > 0 else -1
    edges = []
    for i in range(len(points)):
        (ax, ay), (bx, by) = points[i - 1], points[i]
        length = math.hypot(bx - ax, by - ay)
        edges.append((ax, ay, -(by - ay) / length * sign, (bx - ax) / length * sign))

    def distance(x, y):
        return max((x - ax) * nx + (y - ay) * ny for ax, ay, nx, ny in edges) - rounding

    xs, ys = [p[0] for p in points], [p[1] for p in points]
    return distance, (min(xs) - rounding - 1, min(ys) - rounding - 1, max(xs) + rounding + 1, max(ys) + rounding + 1)


def intersect(a, b):
    return (lambda x, y: max(a[0](x, y), b[0](x, y))), a[1]


def union(*shapes):
    xs0, ys0, xs1, ys1 = zip(*(s[1] for s in shapes))
    return (lambda x, y: min(s[0](x, y) for s in shapes)), (min(xs0), min(ys0), max(xs1), max(ys1))


def vertical_gradient(top_color, bottom_color, y0, y1):
    return lambda x, y: mix(top_color, bottom_color, clamp((y - y0) / max(y1 - y0, 1)))


def shadow(canvas, x0, y0, x1, y1, radius, blur, alpha, offset_y):
    if alpha <= 0:
        return
    distance = rounded_box(x0, y0 + offset_y, x1, y1 + offset_y, radius)[0]

    def coverage(x, y):
        t = clamp((distance(x, y) + blur * 0.3) / blur)
        return alpha * (1 - t) ** 2

    canvas.paint_coverage((x0 - blur, y0 - blur + offset_y, x1 + blur, y1 + blur + offset_y), coverage, (0, 0, 0, 1))


# ---- 文字 ----

# 5×7 点阵：拼音用到的小写字母、撇号和序号。
GLYPHS = {
    "1": ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
    "2": [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
    "3": ["####.", "....#", "....#", ".###.", "....#", "....#", "####."],
    "4": ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
    "5": ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
    "6": [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."],
    "7": ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."],
    "8": [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
    "9": [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."],
    "s": [".....", ".....", ".####", "#....", ".###.", "....#", "####."],
    "h": ["#....", "#....", "#.##.", "##..#", "#...#", "#...#", "#...#"],
    "u": [".....", ".....", "#...#", "#...#", "#...#", "#..##", ".##.#"],
    "i": ["..#..", ".....", ".##..", "..#..", "..#..", "..#..", ".###."],
    "a": [".....", ".....", ".###.", "....#", ".####", "#...#", ".####"],
    "n": [".....", ".....", "#.##.", "##..#", "#...#", "#...#", "#...#"],
    "'": ["..#..", "..#..", ".#...", ".....", ".....", ".....", "....."],
}


def text_width(text, height):
    return len(text) * 6 * height / 7 - height / 7


def draw_text(canvas, x, y, text, height, color):
    """点阵字，`y` 是字格顶边，`height` 是 7 行字格的高度；每个像素 4×4 超采样。"""
    cell = height / 7
    for index, char in enumerate(text):
        rows = GLYPHS[char]
        left = x + index * 6 * cell

        def coverage(px, py, rows=rows, left=left):
            hits = 0
            for sy in range(4):
                for sx in range(4):
                    column = math.floor((px - 0.5 + (sx + 0.5) / 4 - left) / cell)
                    row = math.floor((py - 0.5 + (sy + 0.5) / 4 - y) / cell)
                    if 0 <= row < 7 and 0 <= column < 5 and rows[row][column] == "#":
                        hits += 1
            return hits / 16

        canvas.paint_coverage((left - 1, y - 1, left + 5 * cell + 1, y + height + 1), coverage, color)


# 候选文字画成类似汉字的笔画块：单位方格里的几组线段，按字的序号轮换。
HANZI = [
    [(0.5, 0.06, 0.5, 0.94), (0.12, 0.34, 0.4, 0.3), (0.4, 0.3, 0.18, 0.78), (0.6, 0.28, 0.88, 0.2), (0.58, 0.4, 0.9, 0.9)],
    [(0.06, 0.3, 0.46, 0.3), (0.26, 0.06, 0.26, 0.94), (0.26, 0.36, 0.06, 0.7), (0.26, 0.36, 0.44, 0.58), (0.86, 0.12, 0.58, 0.34), (0.88, 0.4, 0.56, 0.64), (0.9, 0.66, 0.54, 0.94)],
    [(0.18, 0.12, 0.82, 0.12), (0.5, 0.12, 0.5, 0.9), (0.12, 0.5, 0.88, 0.5), (0.3, 0.5, 0.12, 0.88), (0.7, 0.5, 0.9, 0.88)],
    [(0.14, 0.14, 0.14, 0.88), (0.14, 0.14, 0.86, 0.14), (0.86, 0.14, 0.86, 0.88), (0.14, 0.5, 0.86, 0.5), (0.14, 0.88, 0.86, 0.88)],
    [(0.1, 0.2, 0.3, 0.32), (0.08, 0.5, 0.28, 0.6), (0.12, 0.92, 0.3, 0.7), (0.42, 0.18, 0.92, 0.18), (0.66, 0.18, 0.66, 0.92), (0.42, 0.55, 0.92, 0.55)],
    [(0.5, 0.06, 0.5, 0.3), (0.1, 0.3, 0.9, 0.3), (0.24, 0.3, 0.24, 0.9), (0.24, 0.6, 0.76, 0.6), (0.76, 0.3, 0.76, 0.9), (0.24, 0.9, 0.76, 0.9)],
    [(0.12, 0.22, 0.88, 0.22), (0.5, 0.06, 0.5, 0.46), (0.2, 0.46, 0.8, 0.46), (0.5, 0.46, 0.16, 0.92), (0.46, 0.66, 0.86, 0.92)],
    [(0.2, 0.08, 0.36, 0.26), (0.08, 0.38, 0.92, 0.38), (0.5, 0.38, 0.5, 0.94), (0.24, 0.62, 0.76, 0.62), (0.14, 0.94, 0.86, 0.94)],
]


def draw_hanzi(canvas, x, y, size, color, seed):
    segments = [(x + ax * size, y + ay * size, x + bx * size, y + by * size) for ax, ay, bx, by in HANZI[seed % len(HANZI)]]
    canvas.paint(strokes(segments, max(1.1, size * 0.1)), color)


# ---- 皮肤 ----

# 皮肤没写的颜色：宿主各有各的默认值，预览图只取一套近似值；`microsoft`、`wechat` 外观按共享 catalog 的 `windows_looks` 补。
DEFAULTS = {
    "light": {"surface": "#FFFFFF", "border": "#0000001F", "text": "#1A1A1A", "number": "#5F5F5F", "accent": "#0078D4", "selected": "#0078D41F", "hover": "#0000000F", "translation": "#6B6B6B"},
    "dark": {"surface": "#2B2B2B", "border": "#FFFFFF1F", "text": "#F2F2F2", "number": "#BDBDBD", "accent": "#60CDFF", "selected": "#60CDFF33", "hover": "#FFFFFF14", "translation": "#A0A0A0"},
}
LOOKS = {
    "microsoft": {
        "radius": 8,
        "light": {"surface": "#F9F9F9", "border": "#0000001A", "text": "#1A1A1A", "number": "#5F5F5F", "accent": "#E183D9", "selected": "#EAEAEA", "hover": "#F0F0F0", "show_selected_bar": True},
        "dark": {"surface": "#2C2C2C", "border": "#1C1C1C", "text": "#FFFFFF", "number": "#CFCFCF", "accent": "#E183D9", "selected": "#383838", "hover": "#353535", "show_selected_bar": True},
    },
    "wechat": {
        "radius": 5,
        "light": {"surface": "#F7F7F7", "border": "#DEDEDE", "text": "#333333", "number": "#757575", "accent": "#07C160", "selected": "#07C160", "hover": "#07C16024", "show_selected_bar": False, "selected_text": "#FFFFFF"},
        "dark": {"surface": "#151515", "border": "#292929", "text": "#B7B7B7", "number": "#858585", "accent": "#07C160", "selected": "#07C160", "hover": "#07C16052", "show_selected_bar": False, "selected_text": "#FFFFFF"},
    },
}
DESKTOP = {"light": ((0xE9, 0xED, 0xF2), (0xDC, 0xE2, 0xEA)), "dark": ((0x2A, 0x2E, 0x36), (0x1E, 0x21, 0x27))}
WORDS = [(2, 0), (2, 3), (1, 5), (1, 2), (2, 6), (1, 7), (2, 4), (1, 1)]


class Skin:
    def __init__(self, folder):
        self.folder = folder
        self.manifest = tomllib.loads((folder / "skin.toml").read_text(encoding="utf-8"))
        self.window = self.manifest["candidate_window"]
        self.look = LOOKS.get(self.manifest["base"], {})
        self.canvas = self.manifest["base"] == "canvas"

    def palette(self, mode):
        merged = dict(DEFAULTS[mode])
        merged.update(self.look.get(mode, {}))
        merged.update(self.manifest.get("candidate", {}).get(mode, {}))
        return merged

    def color(self, mode, key, fallback=None):
        palette = self.palette(mode)
        value = palette.get(key)
        if value is None:
            return rgba(palette[fallback]) if fallback else None
        return rgba(value)

    def length(self, key, default):
        value = self.window.get("horizontal", {}).get(key, self.window.get(key))
        if value is None:
            return default
        if isinstance(value, str):
            return float(value[:-2]) * FONT_PX
        return float(value)

    def number(self, key, default):
        return float(self.window.get("horizontal", {}).get(key, self.window.get(key, default)))


def background(width, height, mode):
    top, bottom = DESKTOP[mode]
    canvas = Canvas(width, height)
    for y in range(height):
        t = y / max(height - 1, 1)
        color = tuple((top[i] + (bottom[i] - top[i]) * t) / 255 for i in range(3)) + (1.0,)
        for x in range(width):
            canvas.px[y * width + x] = list(color)
    return canvas


# 本脚本这一次生成的、不是 PNG 的图（GIF），预览直接用生成时的第一帧。
GENERATED = {}


def load_png(path):
    """读本脚本写出的 PNG 或 APNG 的默认图（8 位 RGBA、不隔行、滤波 0）；GIF 取 GENERATED 里的第一帧。"""
    if path in GENERATED:
        return GENERATED[path]
    data = path.read_bytes()
    position, width, height, idat = 8, 0, 0, b""
    while position < len(data):
        length, kind = struct.unpack(">I4s", data[position:position + 8])
        body = data[position + 8:position + 8 + length]
        if kind == b"IHDR":
            width, height = struct.unpack(">II", body[:8])
        elif kind == b"IDAT":
            idat += body
        position += 12 + length
    raw = zlib.decompress(idat)
    image = Canvas(width, height)
    stride = width * 4 + 1
    for y in range(height):
        row = raw[y * stride + 1:(y + 1) * stride]
        for x in range(width):
            r, g, b, a = (row[x * 4 + i] / 255 for i in range(4))
            image.px[y * width + x] = [r * a, g * a, b * a, a]
    return image


def anchored(anchor, x0, y0, x1, y1, decoration):
    width, height = decoration["width_dip"], decoration["height_dip"]
    ox, oy = decoration.get("offset_x_dip", 0), decoration.get("offset_y_dip", 0)
    horizontal = "left" if "left" in anchor else "right" if "right" in anchor else "center"
    vertical = "top" if "top" in anchor else "bottom" if "bottom" in anchor else "center"
    x = {"left": x0, "right": x1 - width, "center": (x0 + x1 - width) / 2}[horizontal] + ox
    y = {"top": y0, "bottom": y1 - height, "center": (y0 + y1 - height) / 2}[vertical] + oy
    return x, y, width, height


def draw_candidates(canvas, skin, mode, x, y, rows, per_row, row_width=None):
    """画候选行，返回 (内容宽, 内容高)。第一行第一个是选中项，第三个是悬停项。"""
    palette = skin.palette(mode)
    text = skin.color(mode, "candidate_text", "text")
    number = skin.color(mode, "number")
    selected = skin.color(mode, "selected")
    hover = skin.color(mode, "hover")
    selected_text = skin.color(mode, "selected_text", "text")
    selected_number = skin.color(mode, "selected_number") or (selected_text if "selected_text" in palette else number)
    translation = skin.color(mode, "translation")
    item_radius = skin.window.get("item_corner_radius_dip", 4)
    gap = skin.length("item_gap", 8)
    number_height = 9 * skin.number("number_scale", 1.0)
    number_gap = skin.length("number_gap", 4)
    pad_right = skin.length("item_padding_right", 6)
    item_height = FONT_PX * 1.35 + skin.length("item_extra_height", 4)
    glyph = FONT_PX
    show_bar = palette.get("show_selected_bar", False)
    inside = skin.window.get("horizontal", {}).get("selected_bar_inside", skin.window.get("selected_bar_inside", True))
    bar_width = skin.length("selected_bar_width", 3)
    # 序号从高亮左缘量起，不小于 5；竖条画在高亮之内时序号让出竖条的宽度。
    inset = max(5, skin.length("number_inset", 5)) + (bar_width + 1 if show_bar and inside else 0)
    widest = 0
    for row in range(rows):
        cx = x
        top = y + row * (item_height + 3)
        for column in range(per_row):
            index = row * per_row + column
            chars, seed = WORDS[index % len(WORDS)]
            label = str(column + 1)
            width = inset + text_width(label, number_height) + number_gap + chars * (glyph + 1) + pad_right
            if row == 0 and column == 0:
                canvas.paint(rounded_box(cx, top, cx + width, top + item_height, item_radius), selected)
                if show_bar:
                    bar_height = skin.number("selected_bar_height_em", 0.6) * FONT_PX
                    bar_x = cx + 2 if inside else cx - bar_width - 1
                    middle = top + item_height / 2
                    canvas.paint(rounded_box(bar_x, middle - bar_height / 2, bar_x + bar_width, middle + bar_height / 2, bar_width / 2), skin.color(mode, "selected_bar", "accent"))
            elif row == 0 and column == 2:
                canvas.paint(rounded_box(cx, top, cx + width, top + item_height, item_radius), hover)
            first = row == 0 and column == 0
            draw_text(canvas, cx + inset, top + (item_height - number_height) / 2, label, number_height, selected_number if first else number)
            gx = cx + inset + text_width(label, number_height) + number_gap
            for c in range(chars):
                draw_hanzi(canvas, gx + c * (glyph + 1), top + (item_height - glyph) / 2, glyph, selected_text if first else text, seed + c)
            if row == 0 and column == 1 and translation:
                # 第二个候选带一小段释义，画成两个小一号的字。
                tx = gx + chars * (glyph + 1) + 1
                for c in range(2):
                    draw_hanzi(canvas, tx + c * 9, top + (item_height - 8) / 2, 8, translation, seed + 3 + c)
                width += 19
            cx += width + gap
        widest = max(widest, cx - gap - x)
    return widest, rows * item_height + (rows - 1) * 3


def draw_arrows(canvas, skin, mode, x, y, height):
    """列表里的翻页箭头，返回占用的宽度。"""
    style = skin.window.get("page_arrow_style", "chevron")
    number = skin.color(mode, "number")
    text = skin.color(mode, "text")
    middle = y + height / 2
    if style == "triangle":
        divider = skin.color(mode, "page_arrow_divider", "border")
        canvas.paint(rounded_box(x + 2, y + 3, x + 3, y + height - 3, 0), divider)
        for index, sign in enumerate((-1, 1)):
            cx = x + 15 + index * 16
            color = with_alpha(text, 0.35) if index == 0 else text
            points = [(cx + 4 * sign, middle), (cx - 3 * sign, middle - 5), (cx - 3 * sign, middle + 5)]
            canvas.paint(polygon(points, 0.8), color)
        return 40
    for index, sign in enumerate((-1, 1)):
        cx = x + 10 + index * 15
        color = with_alpha(number, 0.4) if index == 0 else number
        canvas.paint(polyline([(cx - 2.5 * sign, middle - 5), (cx + 2.5 * sign, middle), (cx - 2.5 * sign, middle + 5)], 1.6), color)
    return 32


def draw_card(canvas, skin, mode, x, y, per_row=5, rows=1, extra_right=0):
    """在 (x, y) 画一张横排候选卡片：拼音行加 `rows` 行候选。返回卡片的 (x0, y0, x1, y1)。"""
    palette = skin.palette(mode)
    probe = Canvas(1, 1)
    content_width, content_height = draw_candidates(probe, skin, mode, 0, 0, rows, per_row)
    if skin.canvas:
        insets = skin.window.get("content_insets_dip", {})
        pad_left, pad_right = insets.get("left", 8), insets.get("right", 8)
        pad_top, pad_bottom = insets.get("top", 8), insets.get("bottom", 8)
        line_gap = skin.window.get("line_gap_dip", 2)
    else:
        padding = skin.length("padding", 6)
        pad_left = pad_right = pad_top = pad_bottom = padding
        line_gap = 2
    margins = skin.window.get("horizontal", {}).get("preedit_margin_dip", skin.window.get("preedit_margin_dip", [0]))
    margins = (margins * 4)[:4] if len(margins) == 1 else (margins * 2)[:4] if len(margins) == 2 else margins
    preedit_height = 12 + margins[0] + margins[2]
    arrows = 40 if skin.window.get("page_arrows") and skin.window.get("page_arrow_style") == "triangle" else 32 if skin.window.get("page_arrows") else 0
    expand = 22 if skin.window.get("expand_panel") else 0
    width = pad_left + content_width + arrows + expand + extra_right + pad_right
    if not skin.window.get("fit_content"):
        width = max(width, skin.window.get("min_width_dip", 0), 160)
    height = pad_top + preedit_height + line_gap + content_height + pad_bottom
    x1, y1 = x + width, y + height
    radius = skin.window.get("corner_radius_dip", skin.look.get("radius", 8))
    border_width = skin.window.get("border_width_dip", 1)

    decorations = skin.window.get("decorations", []) if skin.canvas else []
    images = {d["image"]: load_png(skin.folder / d["image"]) for d in decorations}
    for decoration in decorations:
        if decoration.get("z_index", 2) < 1:
            canvas.draw_image(images[decoration["image"]], *anchored(decoration.get("anchor", "right"), x, y, x1, y1, decoration))

    if skin.canvas:
        border = skin.window["border_image"]
        source = border.get("light_source", border["source"]) if mode == "light" else border["source"]
        # stretch_dip 盖过 slice_dip，没写 width_dip 时边宽取切片；只画一个值或「上下 左右」两值。
        slices = border.get("stretch_dip", border["slice_dip"])
        widths = border.get("width_dip", slices)
        pair = lambda values: (values[0], values[1] if len(values) > 1 else values[0])
        canvas.nine_slice(load_png(skin.folder / source), x, y, x1, y1, pair(slices), pair(widths), border.get("fill", True))
    else:
        level = skin.window.get("shadow", "soft")
        blur, alpha, offset = {"none": (1, 0, 0), "soft": (8, 0.16, 2), "strong": (14, 0.42, 5)}[level]
        shadow(canvas, x, y, x1, y1, radius, blur, alpha * (1.6 if mode == "dark" else 1), offset)
        surface = skin.color(mode, "surface")
        if skin.window.get("backdrop"):
            # 画不出材质的宿主把底色补成不透明，预览图照此画。
            surface = (surface[0], surface[1], surface[2], 1.0)
        outer = rounded_box(x, y, x1, y1, radius)
        canvas.paint(outer, surface)
        if border_width > 0:
            inner = rounded_box(x + border_width, y + border_width, x1 - border_width, y1 - border_width, max(radius - border_width, 0))
            canvas.paint(ring(outer, inner), skin.color(mode, "border"))

    # 拼音行：底色、文字、光标和下方的分隔线。
    px, py = x + pad_left + margins[3], y + pad_top + margins[0]
    preedit = "shui'shan"
    if "preedit_background" in palette:
        canvas.paint(rounded_box(px - 3, py - 2, px + text_width(preedit, 9) + 6, py + 11, skin.window.get("item_corner_radius_dip", 4)), skin.color(mode, "preedit_background"))
    draw_text(canvas, px, py + 1, preedit, 9, skin.color(mode, "preedit_text", "number"))
    caret_x = px + text_width(preedit, 9) + 2
    canvas.paint(rounded_box(caret_x, py, caret_x + 1.2, py + 11, 0), skin.color(mode, "preedit_caret", "accent"))
    row_y = y + pad_top + preedit_height + line_gap
    if "preedit_divider" in palette:
        canvas.paint(rounded_box(x + pad_left, row_y - line_gap / 2 - 0.5, x1 - pad_right, row_y - line_gap / 2 + 0.5, 0), skin.color(mode, "preedit_divider"))

    draw_candidates(canvas, skin, mode, x + pad_left, row_y, rows, per_row)
    item_height = FONT_PX * 1.35 + skin.length("item_extra_height", 4)
    if arrows:
        draw_arrows(canvas, skin, mode, x1 - pad_right - arrows - extra_right, row_y, item_height)
    if expand:
        # 展开按钮：第一行右端一个向上的折线（面板已展开），按钮左边一条分隔线。
        ex = x1 - pad_right - expand - extra_right
        canvas.paint(rounded_box(ex + 2, row_y + 3, ex + 3, row_y + item_height - 3, 0), skin.color(mode, "border"))
        middle = row_y + item_height / 2
        canvas.paint(polyline([(ex + 8, middle + 2.5), (ex + 13, middle - 2.5), (ex + 18, middle + 2.5)], 1.6), skin.color(mode, "accent"))

    for decoration in decorations:
        if decoration.get("z_index", 2) >= 1:
            canvas.draw_image(images[decoration["image"]], *anchored(decoration.get("anchor", "right"), x, y, x1, y1, decoration))
    return x, y, x1, y1


def draw_toolbar_and_menu(canvas, skin, mode, x, y):
    """`verify-color-syntax` 用：在卡片右边画悬浮工具栏和右键菜单，好让 `[toolbar.*]` 与 `[candidate.*.menu]` 的颜色也出现在预览里。"""
    toolbar = skin.manifest.get("toolbar", {}).get(mode, {})
    radius = skin.manifest.get("toolbar", {}).get("corner_radius_dip", 8)
    icon_radius = skin.manifest.get("toolbar", {}).get("icon_corner_radius_dip", 4)
    outer = rounded_box(x, y, x + 112, y + 26, radius)
    canvas.paint(outer, rgba(toolbar["background"]))
    canvas.paint(ring(outer, rounded_box(x + 1, y + 1, x + 111, y + 25, radius - 1)), rgba(toolbar["border"]))
    canvas.paint(rounded_box(x + 7, y + 7, x + 10, y + 19, 1.5), rgba(toolbar["handle"]))
    canvas.paint(rounded_box(x + 15, y + 6, x + 16, y + 20, 0), rgba(toolbar["divider"]))
    canvas.paint(rounded_box(x + 21, y + 4, x + 39, y + 22, icon_radius), rgba(toolbar["hover"]))
    for index in range(4):
        ix = x + 24 + index * 22
        canvas.paint(rounded_box(ix, y + 7, ix + 12, y + 19, icon_radius / 2), with_alpha(rgba(toolbar["icon"]), 0.9 if index == 0 else 0.75))
    menu = skin.palette(mode)["menu"]
    my = y + 34
    outer = rounded_box(x, my, x + 112, my + 34, 6)
    canvas.paint(outer, rgba(menu["background"]))
    canvas.paint(ring(outer, rounded_box(x + 1, my + 1, x + 111, my + 33, 5)), rgba(menu["border"]))
    canvas.paint(rounded_box(x + 3, my + 3, x + 109, my + 17, 4), rgba(menu["hover"]))
    for index in range(2):
        for c in range(4):
            draw_hanzi(canvas, x + 9 + c * 11, my + 5 + index * 14, 10, rgba(menu["text"]), index * 4 + c)


def preview(skin):
    """480×160 的预览：上半浅色、下半深色（只支持横排、要展开的皮肤左浅右深）。"""
    out = Canvas(480, 160)
    if skin.window.get("expand_panel"):
        for index, mode in enumerate(("light", "dark")):
            half = background(240, 160, mode)
            draw_card(half, skin, mode, 10, 12, per_row=3, rows=1 + skin.window.get("expand_rows", 3))
            out.paste(half, index * 240, 0)
        return out
    for index, mode in enumerate(("light", "dark")):
        half = background(480, 80, mode)
        decoration = skin.window.get("decoration")
        top = 10 + (decoration["top_inset_dip"] - 6 if decoration else 0) + (6 if skin.canvas else 0)
        extras = "menu" in skin.palette(mode) and skin.manifest.get("toolbar", {}).get(mode)
        x0, y0, x1, _ = draw_card(half, skin, mode, 18, top, per_row=4 if extras else 5)
        if decoration:
            koi = load_png(skin.folder / decoration["image"])
            height = decoration["top_inset_dip"]
            width = decoration["width_dip"]
            left = {"left": x0 + 6, "center": (x0 + x1 - width) / 2, "right": x1 - width - 6}[decoration.get("align", "right")]
            half.draw_image(koi, left, y0 - height + 4, width, height)
        if extras:
            draw_toolbar_and_menu(half, skin, mode, 350, 6)
        out.paste(half, 0, index * 80)
    return out


# ---- verify-canvas 与 verify-v2-motion-system 的贴图 ----


def panel(top, bottom, edge, dot):
    """64×64 的九宫格面板：圆角渐变底、半透明描边，四角各一个小星点（都在 16 像素的切片之内）。"""
    image = Canvas(64, 64)
    outer = rounded_box(1, 1, 63, 63, 14)
    image.paint(outer, vertical_gradient(top, bottom, 1, 63))
    image.paint(ring(outer, rounded_box(3, 3, 61, 61, 12)), edge)
    for cx, cy in ((9, 9), (55, 9), (9, 55), (55, 55)):
        image.paint(polygon([(cx, cy - 3.2), (cx + 1, cy), (cx, cy + 3.2), (cx - 1, cy)], 0.2), dot)
        image.paint(polygon([(cx - 3.2, cy), (cx, cy - 1), (cx + 3.2, cy), (cx, cy + 1)], 0.2), dot)
    return image


def moon():
    image = Canvas(80, 80)
    glow = circle(40, 40, 38)
    image.paint(glow, lambda x, y: (1.0, 0.93, 0.66, 0.28 * clamp(1 - math.hypot(x - 40, y - 40) / 38) ** 1.6))
    crescent = intersect(circle(40, 40, 22), ((lambda x, y: -(math.hypot(x - 51, y - 31) - 19)), (0, 0, 80, 80)))
    image.paint(crescent, vertical_gradient(rgba("#FFF4C7"), rgba("#F8D774"), 18, 62))
    return image


def twinkle_frames():
    frames = []
    for scale in (0.5, 0.72, 1.0, 0.82, 0.62, 0.42):
        image = Canvas(48, 48)
        image.paint(circle(24, 24, 20), lambda x, y, s=scale: (1.0, 0.95, 0.75, 0.32 * s * clamp(1 - math.hypot(x - 24, y - 24) / 20) ** 2))
        long, short = 20 * scale, 3.2 * scale
        color = rgba("#FFF7CC")
        image.paint(polygon([(24, 24 - long), (24 + short, 24), (24, 24 + long), (24 - short, 24)], 0.3), color)
        image.paint(polygon([(24 - long, 24), (24, 24 - short), (24 + long, 24), (24, 24 + short)], 0.3), color)
        image.paint(circle(24, 24, 2.4 * scale + 0.6), rgba("#FFFFFF"))
        frames.append(image)
    return frames


def cloud():
    image = Canvas(96, 40)
    body = union(circle(26, 25, 12), circle(45, 18, 15), circle(66, 21, 13), circle(80, 27, 9), rounded_box(14, 22, 88, 37, 7))
    image.paint(body, vertical_gradient(rgba("#FFFFFFF2"), rgba("#DCD3FAF2"), 4, 37))
    return image


def frame():
    """48×40 的竹色面板框：上下 10、左右 14 像素的框和四角的小方块都在切片之内，中心涂成品红，fill = false 时它不该被画出来。"""
    image = Canvas(48, 40)
    image.paint(rounded_box(14, 10, 34, 30, 0), rgba("#FF00FF"))
    outer = rounded_box(1, 1, 47, 39, 6)
    image.paint(ring(outer, rounded_box(9, 7, 39, 33, 2)), vertical_gradient(rgba("#C9A86A"), rgba("#8C6A33"), 1, 39))
    image.paint(ring(rounded_box(4, 3, 44, 37, 4), rounded_box(5, 4, 43, 36, 3)), rgba("#F3E2B5CC"))
    for cx, cy in ((7, 5), (41, 5), (7, 35), (41, 35)):
        image.paint(rounded_box(cx - 2, cy - 2, cx + 2, cy + 2, 0.6), rgba("#4D7C0F"))
    return image


def seal():
    image = Canvas(64, 64)
    image.paint(ring(circle(32, 32, 30), circle(32, 32, 25)), rgba("rgba(22, 163, 74, 0.35)"))
    image.paint(circle(32, 32, 18), rgba("rgba(22, 163, 74, 0.18)"))
    return image


def ribbon():
    image = Canvas(120, 32)
    image.paint(polygon([(2, 4), (22, 4), (14, 16), (22, 28), (2, 28)]), rgba("#B91C1C"))
    image.paint(polygon([(118, 4), (98, 4), (106, 16), (98, 28), (118, 28)]), rgba("#B91C1C"))
    image.paint(rounded_box(16, 2, 104, 30, 5), vertical_gradient(rgba("#F87171"), rgba("#DC2626"), 2, 30))
    image.paint(rounded_box(22, 7, 98, 9, 1), rgba("#FECACA"))
    return image


def leaf():
    image = Canvas(56, 56)
    image.paint(polygon([(8, 50), (20, 18), (46, 6), (38, 34)], 2.0), vertical_gradient(rgba("#4ADE80"), rgba("#15803D"), 6, 50))
    image.paint(polyline([(10, 48), (26, 28), (44, 8)], 1.4), rgba("#DCFCE7"))
    return image


# 星星 GIF 的调色板：下标 0 透明，1 淡金，2 亮白。
STAR_PALETTE = [(0xFD, 0xE0, 0x47), (0xFF, 0xFF, 0xFF)]


def star_frames():
    """48×48、4 帧的星星：一个四角星从小到大再变小，每帧整幅画布、硬边（GIF 只有一位透明）。返回 (下标数组列表, 第一帧的 Canvas)。"""
    frames, first = [], None
    for scale in (0.55, 0.85, 1.0, 0.7):
        image = Canvas(48, 48)
        long, short = 20 * scale, 4.5 * scale
        image.paint(polygon([(24, 24 - long), (24 + short, 24), (24, 24 + long), (24 - short, 24)], 0.3), rgba("#FDE047"))
        image.paint(polygon([(24 - long, 24), (24, 24 - short), (24 + long, 24), (24, 24 + short)], 0.3), rgba("#FDE047"))
        image.paint(circle(24, 24, 3.2 * scale + 0.8), rgba("#FFFFFF"))
        indices = []
        for r, g, b, a in image.px:
            if a < 0.5:
                indices.append(0)
            else:
                indices.append(2 if min(r, g, b) / a > 0.9 else 1)
        # 预览里的第一帧照 GIF 的硬边重画一遍，和宿主解码出来的一样。
        if first is None:
            first = Canvas(48, 48)
            for i, index in enumerate(indices):
                if index:
                    color = STAR_PALETTE[index - 1]
                    first.px[i] = [color[0] / 255, color[1] / 255, color[2] / 255, 1.0]
        frames.append(indices)
    return frames, first


def koi_frames():
    frames = []
    for frame in range(8):
        phase = frame / 8 * 2 * math.pi
        image = Canvas(112, 56)
        bob = math.sin(phase) * 1.5
        image.paint(ring(ellipse(56, 44, 40, 7), ellipse(56, 44, 37, 5.2)), (0.55, 0.78, 0.93, 0.55))
        image.paint(ring(ellipse(56, 44, 24, 4), ellipse(56, 44, 22, 2.8)), (0.55, 0.78, 0.93, 0.4))
        wag = math.sin(phase) * 7
        tail = polygon([(30, 27 + bob), (12, 15 + bob + wag), (16, 28 + bob + wag * 0.4), (12, 40 + bob + wag)], 1.2)
        image.paint(tail, rgba("#F7934C"))
        fin = ellipse(52, 37 + bob, 7, 3.2)
        image.paint(fin, rgba("#FAB98A"))
        body = ellipse(58, 27 + bob, 30, 11)
        image.paint(body, vertical_gradient(rgba("#FFFDF8"), rgba("#F4E6D8"), 16, 38))
        for cx, cy, r in ((66, 22, 8), (48, 30, 6.5), (78, 28, 5)):
            image.paint(intersect(circle(cx, cy + bob, r), body), rgba("#F2711C"))
        image.paint(circle(80, 24 + bob, 1.8), rgba("#2A2A2A"))
        image.paint(polyline([(64, 16 + bob), (56, 11 + bob), (48, 16 + bob)], 2.2), rgba("#F7934C"))
        frames.append(image)
    return frames


def main():
    canvas_assets = ROOT / "verify-canvas" / "assets"
    canvas_assets.mkdir(parents=True, exist_ok=True)
    panel(rgba("#24215A"), rgba("#151335"), rgba("rgba(167, 139, 250, 0.75)"), rgba("#FDE68A")).save(canvas_assets / "panel-dark.png")
    panel(rgba("#FDFCFF"), rgba("#EEE8FD"), rgba("rgba(124, 58, 237, 0.45)"), rgba("#F59E0B")).save(canvas_assets / "panel-light.png")
    moon().save(canvas_assets / "moon.png")
    cloud().save(canvas_assets / "cloud.png")
    frames = twinkle_frames()
    (canvas_assets / "twinkle.png").write_bytes(encode_apng(48, 48, [f.to_bytes() for f in frames], 120))

    frame_assets = ROOT / "verify-canvas-frame" / "assets"
    frame_assets.mkdir(parents=True, exist_ok=True)
    frame().save(frame_assets / "frame.png")
    seal().save(frame_assets / "seal.png")
    ribbon().save(frame_assets / "ribbon.png")
    leaf().save(frame_assets / "leaf.png")
    stars, first = star_frames()
    (frame_assets / "stars.gif").write_bytes(encode_gif(48, 48, STAR_PALETTE, stars, 150))
    GENERATED[frame_assets / "stars.gif"] = first

    koi_assets = ROOT / "verify-v2-motion-system" / "assets"
    koi_assets.mkdir(parents=True, exist_ok=True)
    frames = koi_frames()
    (koi_assets / "koi.png").write_bytes(encode_apng(112, 56, [f.to_bytes() for f in frames], 100))

    for folder in sorted(p for p in ROOT.iterdir() if (p / "skin.toml").is_file()):
        preview(Skin(folder)).save(folder / "preview.png")
        print(folder.name)


if __name__ == "__main__":
    main()
