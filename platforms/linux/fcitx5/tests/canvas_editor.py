"""在隔离 X11／Wayland 显示上检查真实 GTK 编辑器与原生 canvas 像素。"""
import ctypes
import configparser
from collections import Counter
from contextlib import ExitStack, nullcontext
import io
import json
import os
from pathlib import Path
import re
import sqlite3
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import zlib

import dbus
import dbus.mainloop.glib
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk

WAYLAND = "--wayland" in sys.argv
if not WAYLAND:
    gi.require_version("GdkX11", "3.0")
    from gi.repository import GdkX11
from daemon import daemon_environment, wait

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from host_api import ResponseDecoder


DARK = ((144, 17, 34), (18, 144, 52), (52, 37, 144),
        (20, 92, 114), (113, 90, 35), (139, 79, 49),
        (49, 127, 139), (116, 145, 38), (98, 66, 117))
LIGHT = tuple(tuple(channel + 80 for channel in color) for color in DARK)
FORMAT = os.environ.get("MSIME_TEST_CANVAS_FORMAT", "png")


def png(colors):
    """生成九块纯色的合成 PNG，不依赖外部资源或图片库。"""
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    rows = b"".join(b"\0" + b"".join(bytes(colors[(y // 20) * 3 + x // 20])
                                      for x in range(60)) for y in range(60))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 60, 60, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def panel_image(colors):
    if FORMAT in ("png", "apng"):
        return png(colors)
    try:
        from PIL import Image
    except ImportError:
        print("skipped: 非 PNG 的合成验收图片需要 Pillow")
        sys.exit(77)
    result = io.BytesIO()
    Image.open(io.BytesIO(png(colors))).save(
        result, format={"gif": "GIF", "webp": "WEBP"}[FORMAT],
        lossless=True)
    return result.getvalue()


def animation_image(first, second):
    from PIL import Image
    frames = [Image.open(io.BytesIO(png(colors))).convert("RGBA") for colors in (first, second)]
    result = io.BytesIO()
    frames[0].save(result, format={"apng": "PNG", "gif": "GIF", "webp": "WEBP"}[FORMAT],
                   save_all=True, append_images=frames[1:], duration=[200, 1000],
                   loop=1 if FORMAT == "apng" else 0,
                   lossless=True)
    data = result.getvalue()
    if FORMAT == "gif":
        # 无循环扩展才是一次播放；Pillow 的 loop=0 会生成无限循环扩展。
        data = data.replace(b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00", b"")
    return data


def accept_animation(skin, entry, control, daemon):
    second, replacement = (0, 239, 215), (255, 171, 0)
    # image-webp 的非预乘 alpha 合成对完全不透明通道仍可能向下舍入一级。
    # 此处检验帧变化，WebP 每通道最多允许 1 的误差；APNG/GIF 保持精确颜色断言。
    def count_color(counts, color):
        if FORMAT != "webp":
            return counts[color]
        return sum(counts[(r, g, b)] for r in range(max(0, color[0] - 1), min(255, color[0] + 1) + 1)
                   for g in range(max(0, color[1] - 1), min(255, color[1] + 1) + 1)
                   for b in range(max(0, color[2] - 1), min(255, color[2] + 1) + 1))

    def visible(colors):
        _, counts = screenshot()
        return all(count_color(counts, color) >= 16 for color in colors)

    wait(native_window)
    wait(lambda: visible((second,)))
    if FORMAT != "webp":
        # 有限动画已经进入末帧；连续候选更新不能重新显示首帧。
        for _ in range(6):
            subprocess.run(["xdotool", "key", "--delay", "1", "BackSpace", "i"], check=True, timeout=5)
            wait(lambda: visible((SELECTED,)))
            assert visible((second,)) and not visible((WIDGET,)), "候选刷新重启了动画"
    else:
        # 在反复刷新候选期间，固定 100ms 的 WebP 必须继续循环到两个画面。
        seen = set()
        deadline = time.monotonic() + 1.2
        while time.monotonic() < deadline:
            subprocess.run(["xdotool", "key", "--delay", "1", "BackSpace", "i"], check=True, timeout=5)
            while GLib.MainContext.default().iteration(False):
                pass
            _, counts = screenshot()
            seen.update(color for color in (WIDGET, second) if count_color(counts, color) >= 16)
        assert seen == {WIDGET, second}, "连续候选刷新阻止了 WebP 播放"
    save_image("animation-playing")
    subprocess.run(["xdotool", "key", "Escape"], check=True, timeout=5)
    wait(lambda: native_window() is None)
    subprocess.run(["xdotool", "type", "--delay", "1", "ni"], check=True, timeout=5)
    wait(lambda: visible((WIDGET,)))
    wait(lambda: visible((second,)))
    # 同一路径更换内容后必须失效帧缓存，同时重置播放时间。
    (skin / f"widget.{FORMAT}").write_bytes(animation_image((replacement,) * 9, (second,) * 9))
    wait(lambda: visible((replacement,)))
    wait(lambda: visible((second,)))
    # 九宫格边框也走动画序列，而不是只有自由挂件会动。
    (skin / f"light.{FORMAT}").write_bytes(animation_image(LIGHT, DARK))
    wait(lambda: visible(DARK))
    save_image("animation-panel")
    control.SetCurrentIM("keyboard-us")
    wait(lambda: native_window() is None)
    assert entry.get_text() == "", "动画验收意外提交了预编辑"
    control.Exit()
    assert daemon.wait(timeout=10) == 0
    print(f"真实 GTK 原生 {FORMAT} 动画、连续输入、隐藏重显、文件失效和九宫格播放通过")


def screenshot():
    if WAYLAND:
        loader = GdkPixbuf.PixbufLoader.new_with_type("png")
        loader.write(subprocess.check_output(["grim", "-"], timeout=5))
        loader.close()
        image = loader.get_pixbuf()
    else:
        root = Gdk.get_default_root_window()
        image = Gdk.pixbuf_get_from_window(root, 0, 0, root.get_width(), root.get_height())
    assert image is not None, "无法读取隔离显示的像素"
    channels, stride = image.get_n_channels(), image.get_rowstride()
    data = image.get_pixels()
    packed = b"".join(data[y * stride:y * stride + image.get_width() * channels]
                      for y in range(image.get_height()))
    return image, Counter(zip(packed[0::channels], packed[1::channels], packed[2::channels]))


NATIVE = os.environ.get("MSIME_TEST_NATIVE_CANVAS") == "1"
ANIMATION = os.environ.get("MSIME_TEST_CANVAS_ANIMATION") == "1"
NATIVE_SCALE = int(os.environ.get("GDK_SCALE", "1"))
COMPOSITED = os.environ.get("MSIME_TEST_COMPOSITED") == "1"
SELECTED = (7, 91, 173)
WIDGET = (255, 0, 171)


def synthetic_translation_provider(root, cleanups):
    """只返回合成释义，通过真实本地 socket 进入宿主刷新链。"""
    path = root / "translation.sock"
    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server.bind(str(path))
    server.listen(4)
    server.settimeout(0.2)
    stopped, errors = threading.Event(), []

    def serve():
        while not stopped.is_set():
            try:
                client, _ = server.accept()
            except socket.timeout:
                continue
            except OSError:
                if not stopped.is_set():
                    errors.append("合成释义 socket 意外关闭")
                return
            try:
                with client:
                    client.settimeout(2)
                    request = b""
                    while b"\n" not in request and len(request) < 65536:
                        chunk = client.recv(4096)
                        if not chunk:
                            break
                        request += chunk
                    document = json.loads(request)
                    assert document["kind"] == "translation"
                    texts = document["query"]["candidates"]
                    reply = {"translations": ([{"text": texts[0], "translation": "proper n. MMM；n. MMM"}]
                                               if texts else [])}
                    client.sendall(json.dumps(reply).encode() + b"\n")
            except Exception as error:
                if not stopped.is_set():
                    errors.append(type(error).__name__)

    thread = threading.Thread(target=serve, name="synthetic-canvas-translation", daemon=True)
    thread.start()

    def close():
        stopped.set()
        server.close()
        thread.join(timeout=3)
        assert not thread.is_alive(), "合成释义服务未退出"
        assert not errors, f"合成释义协议失败：{errors}"

    cleanups.callback(close)
    return str(path)


def native_window():
    result = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "^MSIMECandidateCanvas$"],
                            stdout=subprocess.PIPE, text=True, timeout=5)
    return result.stdout.strip().splitlines()[0] if result.returncode == 0 else None


def window_geometry(window):
    output = subprocess.check_output(["xdotool", "getwindowgeometry", "--shell", window], text=True, timeout=5)
    return dict(line.split("=", 1) for line in output.strip().splitlines())


def selected_pixels():
    image, _ = screenshot()
    channels, stride = image.get_n_channels(), image.get_rowstride()
    data = image.get_pixels()
    window = native_window()
    if not window:
        return None
    bounds = window_geometry(window)
    start_x, start_y, width, height = (int(bounds[key]) for key in ("X", "Y", "WIDTH", "HEIGHT"))
    points = []
    expected = bytes(SELECTED)
    for y in range(start_y, start_y + height):
        row = data[y * stride:y * stride + image.get_width() * channels]
        for x in range(start_x, start_x + width):
            if row[x * channels:x * channels + 3] == expected:
                points.append((x, y))
    if not points:
        return None
    left, right = min(x for x, _ in points), max(x for x, _ in points) + 1
    top, bottom = min(y for _, y in points), max(y for _, y in points) + 1
    return right - left, bottom - top, b"".join(
        data[y * stride + left * channels:y * stride + right * channels] for y in range(top, bottom))


def color_position(color):
    image, _ = screenshot()
    channels, stride = image.get_n_channels(), image.get_rowstride()
    data = image.get_pixels()
    for y in range(image.get_height()):
        for x in range(image.get_width()):
            offset = y * stride + x * channels
            if data[offset:offset + 3] == bytes(color):
                return x + 2, y + 2
    raise AssertionError("未找到预期颜色")


def color_bounds(color):
    image, _ = screenshot()
    channels, stride, data = image.get_n_channels(), image.get_rowstride(), image.get_pixels()
    bounds = window_geometry(native_window())
    left, top, width, height = (int(bounds[key]) for key in ("X", "Y", "WIDTH", "HEIGHT"))
    points = [(x, y) for y in range(top, top + height) for x in range(left, left + width)
              if data[y * stride + x * channels:y * stride + x * channels + 3] == bytes(color)]
    assert points, f"原生窗口缺少配色 {color}"
    return min(x for x, _ in points), min(y for _, y in points), max(x for x, _ in points) + 1, max(y for _, y in points) + 1


def accept_appearance(manifest, preferences, publish, entry):
    original = manifest.read_text()
    bar, hover, background, divider, caret = (22, 180, 240), (242, 128, 17), (61, 20, 80), (153, 77, 222), (0, 255, 204)
    style = original.replace("[candidate_window]\n", "[candidate_window]\nitem_corner_radius_dip = 12\ncorner_radius_dip = 20\n")
    for mode in ("light", "dark"):
        style = style.replace(f"[candidate.{mode}]\n", f"[candidate.{mode}]\n"
                              "show_selected_bar = true\nselected_bar = '#16B4F0'\nhover = '#F28011'\n"
                              "preedit_background = '#3D1450'\npreedit_divider = '#994DDE'\n"
                              "preedit_text = '#FDE75A'\npreedit_caret = '#00FFCC'\n")
    style += (f"\n[candidate_window.{preferences['candidate_layout']}]\n"
              "selected_bar_width = 5\nselected_bar_height_em = 1.1\nselected_bar_inside = true\n"
              "item_extra_height = 24\nitem_gap = 12\n")
    for mode in ('light', 'dark'):
        style += (f"\n[candidate.{mode}.menu]\nbackground = '#1F252B'\nborder = '#556677'\n"
                  "text = '#E1E2E3'\nhover = '#515B65'\n")
    manifest.write_text(style)
    wait(lambda: palette_visible((bar, background, divider, caret)))
    save_image("native-appearance")
    left, top, right, bottom = color_bounds(SELECTED)
    _, counts = screenshot()
    assert counts[SELECTED] < (right - left) * (bottom - top), "高亮仍然是无圆角矩形"
    bar_box = color_bounds(bar)
    assert bar_box[2] - bar_box[0] == 5 * NATIVE_SCALE, "选中条宽度未进入实际像素"
    # 指针只改变 hover，不改变键盘选中项；按下后离开并释放不提交。
    if preferences['candidate_layout'] == 'horizontal':
        point = (right + 12 * NATIVE_SCALE + 8 * NATIVE_SCALE, (top + bottom) // 2)
    else:
        point = ((left + right) // 2, bottom + 12 * NATIVE_SCALE + 8 * NATIVE_SCALE)
    subprocess.run(["xdotool", "mousemove", *map(str, point)], check=True, timeout=5)
    wait(lambda: palette_visible((hover,)))
    assert color_bounds(SELECTED) == (left, top, right, bottom), "悬停改变了键盘选中项"
    save_image("native-hover")
    subprocess.run(["xdotool", "mousedown", "1"], check=True, timeout=5)
    wait(lambda: not palette_visible((hover,)))
    save_image("native-pressed")
    subprocess.run(["xdotool", "mousemove", "0", "0", "mouseup", "1"], check=True, timeout=5)
    wait(lambda: not palette_visible((hover,)))
    assert entry.get_text() == "", "拖出候选后的释放错误上屏"
    # 菜单独立配色进入真实原生窗口；返回仍保留同一候选快照。
    subprocess.run(["xdotool", "mousemove", str((left + right) // 2), str((top + bottom) // 2), "click", "3"],
                   check=True, timeout=5)
    wait(lambda: palette_visible(((31, 37, 43), (225, 226, 227))))
    assert not palette_visible((background,)), "候选预编辑背景渗进右键菜单"
    bounds = window_geometry(native_window())
    point = (int(bounds['X']) + 40 * NATIVE_SCALE,
             int(bounds['Y']) + int(bounds['HEIGHT']) - 22 * NATIVE_SCALE)
    subprocess.run(["xdotool", "mousemove", *map(str, point)], check=True, timeout=5)
    wait(lambda: palette_visible(((81, 91, 101),)))
    save_image("native-menu")
    subprocess.run(["xdotool", "click", "1"], check=True, timeout=5)
    wait(lambda: palette_visible((bar, background, divider, caret)))
    assert entry.get_text() == "", "返回菜单意外提交候选"
    # 文字取色器必须覆盖皮肤预编辑文字，再恢复全部外观。
    preferences['custom_theme'].setdefault('candidate_colors', {})['text'] = '#12EF45'
    publish()
    def picked_preedit():
        # 字体抗锯齿不保证存在足够多纯色像素；只查预编辑背景框内向取色器绿色收敛的字形。
        left, top, right, bottom = color_bounds(background)
        image, _ = screenshot()
        data, stride, channels = image.get_pixels(), image.get_rowstride(), image.get_n_channels()
        return sum(data[y * stride + x * channels + 1] > data[y * stride + x * channels] + 100 and
                   data[y * stride + x * channels + 1] > data[y * stride + x * channels + 2] + 100
                   for y in range(top, bottom) for x in range(left, right)) > 10
    wait(picked_preedit)
    del preferences['custom_theme']['candidate_colors']['text']
    publish()
    manifest.write_text(original)
    wait(lambda: all(screenshot()[1][color] < 16 for color in (bar, background, divider, caret)))


def accept_pager(manifest, preferences, publish, entry):
    original = manifest.read_text()
    original_pixels = selected_pixels()
    font_size = preferences['candidate_font_size']
    preferences['candidate_font_size'] = 24
    publish()
    hover, divider = (34, 238, 119), (49, 209, 255)
    horizontal = preferences['candidate_layout'] == 'horizontal'
    for triangle in (False, True):
        style = original.replace('[candidate_window]\n',
                                 '[candidate_window]\npage_arrows = true\n'
                                 f"page_arrow_style = '{'chevron' if triangle else 'triangle'}'\n")
        style += (f"\n[candidate_window.{preferences['candidate_layout']}]\n"
                  f"page_arrow_style = '{'triangle' if triangle else 'chevron'}'\n"
                  'item_extra_height = 20\n')
        for mode in ('light', 'dark'):
            style = style.replace(f'[candidate.{mode}]\n', f'[candidate.{mode}]\n'
                                  "hover = '#22EE77'\npage_arrow_divider = '#31D1FF'\n")
        manifest.write_text(style)
        # 最小候选行高为 24 * 1.35 + 20；热更新后再据真实面板边缘定位。
        wait(lambda: selected_pixels() and selected_pixels()[1] >= 52 * NATIVE_SCALE)
        subprocess.run(['xdotool', 'mousemove', '0', '0'], check=True, timeout=5)

        def buttons():
            left, top, _, bottom = color_bounds(SELECTED)
            panel_right = color_bounds(LIGHT[2])[2]
            panel_bottom = color_bounds(LIGHT[8])[3]
            width = round(24 * (0.96 if horizontal else 0.81)) if triangle else round(24 * 0.72)
            height = round(24 * 1.35 + 20) if triangle and horizontal else round(24 * (1.2 if triangle else 0.9))
            gap = 0 if triangle else 1
            if horizontal:
                lift = 0 if triangle else (24 * 1.35 + 20 - height) / 2
                x = panel_right - (12 + (0 if triangle else 1) + width / 2) * NATIVE_SCALE
                y = bottom - (lift + height / 2) * NATIVE_SCALE
            else:
                glyph = 24 * (0.52 * 0.84 if triangle else 0.5 * 0.5 * 0.55)
                x = left + (8 + glyph / 2 + width + gap) * NATIVE_SCALE
                # 真实分页状态仍在辅助行；从预编辑到首候选的距离实测字体行高。
                panel_top = color_bounds(LIGHT[0])[1]
                line_height = (top - panel_top) / NATIVE_SCALE - 12 - 19
                y = panel_bottom - (12 + line_height + 4 + 1 + height / 2) * NATIVE_SCALE
            return (round(x - (width + gap) * NATIVE_SCALE), round(y)), (round(x), round(y))

        def mouse(point, *actions):
            subprocess.run(['xdotool', 'mousemove', *map(str, point), *actions], check=True, timeout=5)

        previous, next_button = buttons()
        before = selected_pixels()
        candidate_box = color_bounds(SELECTED)

        def candidate_pixels():
            image, _ = screenshot()
            data, stride, channels = image.get_pixels(), image.get_rowstride(), image.get_n_channels()
            left, top, right, bottom = candidate_box
            return b''.join(data[y * stride + left * channels:y * stride + right * channels]
                            for y in range(top, bottom))

        first_page = candidate_pixels()
        # 第一页的上一页箭头不可用，仍接收点击但不穿透、不高亮，也不改变候选。
        mouse(previous, 'click', '1')
        while GLib.MainContext.default().iteration(False):
            pass
        assert selected_pixels() == before and entry.get_text() == ''
        assert not palette_visible((hover,)), '禁用翻页箭头响应了悬停'
        mouse(next_button)
        wait(lambda: palette_visible((hover,)))
        if triangle and horizontal:
            wait(lambda: palette_visible((divider,)))
            box = color_bounds(divider)
            assert box[2] - box[0] == NATIVE_SCALE, '翻页分隔线宽度错误'
        elif not horizontal:
            assert not palette_visible((divider,)), '竖排错误地绘制了翻页分隔线'
        save_image('native-pager-' + ('triangle' if triangle else 'chevron'))
        mouse(next_button, 'mousedown', '1')
        wait(lambda: not palette_visible((hover,)))
        subprocess.run(['xdotool', 'mousemove', '0', '0', 'mouseup', '1'], check=True, timeout=5)
        wait(lambda: selected_pixels() == before)
        assert candidate_pixels() == first_page and entry.get_text() == '', '拖出箭头后仍然翻页或上屏'
        mouse(next_button, 'click', '1')
        wait(lambda: candidate_pixels() != first_page)
        wait(lambda: palette_visible((SELECTED,)))
        assert entry.get_text() == '', '翻页点击错误地选择了候选'
        previous, _ = buttons()
        mouse(previous, 'click', '1')
        wait(lambda: candidate_pixels() == first_page)
        wait(lambda: selected_pixels() == before)
        # 开关热更新必须清掉旧按下状态；关闭后的释放不能触发旧页回调。
        _, next_button = buttons()
        mouse(next_button, 'mousedown', '1')
        old_geometry = window_geometry(native_window())
        manifest.write_text(style.replace('page_arrows = true', 'page_arrows = false'))
        wait(lambda: not palette_visible((hover,)) and window_geometry(native_window()) != old_geometry)
        subprocess.run(['xdotool', 'mouseup', '1', 'mousemove', '0', '0'], check=True, timeout=5)
        assert selected_pixels() == before and entry.get_text() == ''
    preferences['candidate_font_size'] = font_size
    publish()
    manifest.write_text(original)
    wait(lambda: selected_pixels() == original_pixels)


def accept_translation(manifest, preferences, publish, entry):
    original = manifest.read_text()
    normal = ((239, 48, 64), (20, 223, 90), (40, 112, 255))
    selected = ((255, 212, 42), (250, 57, 224), (38, 231, 231))
    keys = ("translation", "translation_pos", "translation_separator", "selected_translation",
            "selected_translation_pos", "selected_translation_separator")
    style = original
    for mode in ("light", "dark"):
        slots = "".join(f"{key} = '#{r:02X}{g:02X}{b:02X}'\n"
                        for key, (r, g, b) in zip(keys, normal + selected))
        style = style.replace(f"[candidate.{mode}]\n", f"[candidate.{mode}]\n{slots}")
    manifest.write_text(style)
    preferences.update(candidate_translations=True, candidate_english_gloss=False, translation_target_language="en")
    publish()

    def visible(colors, background=SELECTED):
        _, counts = screenshot()
        # 小字号分号可能只有一个纯色像素；检查前景与已知底色之间的抗锯齿混合线。
        def ink(color):
            delta = tuple(value - base for value, base in zip(color, background))
            length = sum(value * value for value in delta)
            result = 0
            for actual, count in counts.items():
                alpha = sum((value - base) * change for value, base, change in zip(actual, background, delta)) / length
                if 0.6 <= alpha <= 1.02 and all(abs(value - (base + alpha * change)) <= 4
                                               for value, base, change in zip(actual, background, delta)):
                    result += count
            return result
        return all(ink(color) >= 2 for color in colors)

    wait(lambda: visible(selected))
    save_image("native-translation-selected")
    subprocess.run(["xdotool", "key", "Down"], check=True, timeout=5)
    wait(lambda: visible(normal, LIGHT[4]))
    save_image("native-translation-normal")
    subprocess.run(["xdotool", "key", "Up"], check=True, timeout=5)
    wait(lambda: visible(selected))
    replacement = (198, 103, 241)
    manifest.write_text(style.replace("#FA39E0", "#C667F1"))
    wait(lambda: visible((selected[0], replacement, selected[2])))
    save_image("native-translation-hot-update")
    left, top, right, bottom = color_bounds(SELECTED)
    subprocess.run(["xdotool", "mousemove", str((left + right) // 2), str((top + bottom) // 2), "click", "1"],
                   check=True, timeout=5)
    wait(lambda: entry.get_text() == "你")
    assert "MMM" not in entry.get_text(), "释义进入了上屏文字"
    wait(lambda: native_window() is None)
    preferences["candidate_translations"] = False
    publish()
    manifest.write_text(original)
    entry.set_text("")
    subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "100", "ni"], check=True, timeout=5)
    wait(lambda: palette_visible((SELECTED,)))


def click_color(color, button):
    x, y = color_position(color)
    subprocess.run(["xdotool", "mousemove", str(x), str(y), "click", str(button)], check=True, timeout=5)


def palette_visible(colors):
    _, counts = screenshot()
    return all(counts[color] >= 16 for color in colors)


def palette_top(color):
    image, _ = screenshot()
    channels, stride = image.get_n_channels(), image.get_rowstride()
    pixel = bytes(color) + (b"\xff" if channels == 4 else b"")
    data = image.get_pixels()
    for y in range(image.get_height()):
        if pixel in data[y * stride:y * stride + image.get_width() * channels]:
            return y
    return None


def save_image(name):
    destination = os.environ.get("MSIME_TEST_CANVAS_ARTIFACTS")
    if destination:
        directory = (Path(destination) / ((f"native-x11-{NATIVE_SCALE}x" + ("-composited" if COMPOSITED else ""))
                     if NATIVE else "wayland" if WAYLAND else "x11") / FORMAT
                     / os.environ.get("MSIME_TEST_CANVAS_LAYOUT", "vertical"))
        directory.mkdir(parents=True, exist_ok=True)
        screenshot()[0].savev(str(directory / f"{name}.png"), "png", [], [])


def fallback_visible(theme):
    config = configparser.ConfigParser(interpolation=None)
    config.read(theme)
    if not config["InputPanel/Background"]["Image"].startswith("shape-"):
        return False
    color = config["InputPanel/Background"]["Color"].lstrip("#")
    rgb = tuple(int(color[at:at + 2], 16) for at in (0, 2, 4))
    return screenshot()[1][rgb] >= 100


def main():
    resources, library, addon = map(Path, sys.argv[1:4])
    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    assert not bus.name_has_owner("org.fcitx.Fcitx5"), "必须使用独立 D-Bus 会话"
    bus.request_name("org.freedesktop.portal.Desktop")
    host = ctypes.CDLL(str(library))
    responses = ResponseDecoder(host)
    for name in ("msime_client_prepare_host", "msime_client_load_preferences"):
        function = getattr(host, name)
        function.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        function.restype = ctypes.c_void_p
    host.msime_client_save_preferences.argtypes = [
        ctypes.c_char_p, ctypes.c_size_t, ctypes.c_uint64, ctypes.c_char_p, ctypes.c_size_t]
    host.msime_client_save_preferences.restype = ctypes.c_void_p

    def value(pointer):
        result = responses.decode(pointer)
        assert result["ok"], "共享 Host API 请求失败"
        return result["value"]

    # Wayland 模块与编辑器同进程，GLib 读取前就要固定 XDG 目录；X11 面板在独立 daemon 中。
    profile = os.environ.get("MSIME_TEST_CANVAS_ROOT") if WAYLAND else None
    if WAYLAND:
        assert profile and os.environ.get("XDG_DATA_HOME") == str(Path(profile) / "data"), "需要启动前固定的合成配置目录"
    profile_context = nullcontext(profile) if profile else tempfile.TemporaryDirectory(prefix="msime-canvas-editor-")
    with profile_context as directory, ExitStack() as cleanups:
        root = Path(directory)
        if WAYLAND:
            assert GLib.get_user_data_dir() == str(root / "data"), "GLib 缓存了夹具以外的数据目录"
            assert GLib.get_user_config_dir() == str(root / "config"), "GLib 缓存了夹具以外的配置目录"
        request = json.dumps({"resources": str(resources), "state_root": str(root / "state")}).encode()
        options = value(host.msime_client_prepare_host(request, len(request)))
        skin = root / "skins" / "canvas-fixture"
        skin.mkdir(parents=True)
        panel_file, light_file = f"panel.{FORMAT}", f"light.{FORMAT}"
        (skin / panel_file).write_bytes(panel_image(DARK))
        (skin / light_file).write_bytes(panel_image(LIGHT))
        (skin / "skin.toml").write_text(
            "schema_version = 2\nid = 'canvas-fixture'\nname = 'Canvas fixture'\nversion = '1'\nbase = 'canvas'\n"
            "[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['light', 'dark']\n"
            "[candidate_window]\ncontent_insets_dip = { top = 12, right = 12, bottom = 12, left = 12 }\n"
            "outer_insets_dip = { top = 4, right = 4, bottom = 4, left = 4 }\n"
            f"[candidate_window.border_image]\nsource = '{panel_file}'\nlight_source = '{light_file}'\n"
            "slice_dip = [20]\nwidth_dip = [8]\n[candidate.light]\ntext = '#010203'\n"
            "[candidate.dark]\ntext = '#FDFCFB'\n")
        if NATIVE:
            assert not WAYLAND, "原生 X11 验收不能在 Wayland 冒充通过"
            widget_name = f"widget.{FORMAT}" if ANIMATION else "widget.png"
            (skin / widget_name).write_bytes(animation_image((WIDGET,) * 9, ((0, 239, 215),) * 9)
                                             if ANIMATION else png((WIDGET,) * 9))
            manifest = skin / "skin.toml"
            text = manifest.read_text().replace("[candidate_window]\n", "[candidate_window]\nline_gap_dip = 19\n")
            text = text.replace("[candidate.light]\n", "[candidate.light]\nselected = '#075BAD'\n")
            text = text.replace("[candidate.dark]\n", "[candidate.dark]\nselected = '#075BAD'\n")
            text += (f"[[candidate_window.decorations]]\nimage = '{widget_name}'\nanchor = 'top-right'\n"
                     "offset_x_dip = 24\noffset_y_dip = -24\nwidth_dip = 20\nheight_dip = 20\nz_index = 2\n")
            manifest.write_text(text)
        options["candidate_skin_catalog"] = {
            "directory": str(skin.parent), "packages": [{
                "id": "canvas-fixture", "title": "Canvas fixture", "base": "system",
                "layouts": ["horizontal", "vertical"], "candidate": {"light": {}, "dark": {}}}]}
        preferences = options["preferences"]
        preferences.update(learning=False, cloud_candidates=False, usage_reporting=False,
                           global_theme="custom", candidate_theme="light",
                           candidate_layout=os.environ.get("MSIME_TEST_CANVAS_LAYOUT", "vertical"))
        if NATIVE:
            preferences["navigation"]["mouse_wheel"] = True
            # 固定合成字号，避免默认偏好借用桌面字体后，改字号再恢复变成首次显式设置。
            preferences.update(candidate_translations=False, candidate_english_gloss=False, candidate_font_size=20)
            options["translation_provider_socket"] = synthetic_translation_provider(root, cleanups)
        preferences["custom_theme"].update(base="system", candidate_skin="canvas-fixture",
                                           candidate_skin_dark="canvas-fixture")

        def publish():
            store = options["preferences_directory"].encode()
            snapshot = value(host.msime_client_load_preferences(store, len(store)))
            snapshot["preferences"] = preferences
            document = json.dumps(snapshot).encode()
            value(host.msime_client_save_preferences(store, len(store), snapshot["revision"], document, len(document)))
            (root / "options.json").write_text(json.dumps(options))

        publish()
        env = daemon_environment(root, addon)
        # GTK 模块在创建上下文时才连接 daemon，环境必须在创建编辑器之前生效。
        os.environ.update(env)
        os.environ["GTK_IM_MODULE"] = "fcitx"
        os.environ["GDK_BACKEND"] = "wayland" if WAYLAND else "x11"
        if WAYLAND:
            assert Path(env.get("WAYLAND_DISPLAY", "")).is_absolute(), "需要隔离合成器的绝对 socket 路径"
        window = keyboard = control = compositor = None
        with (root / "daemon.log").open("w") as log:
            daemon = subprocess.Popen(["fcitx5", "-D", "--keep"], env=env, stdout=log, stderr=log)
            try:
                if COMPOSITED:
                    assert NATIVE and not WAYLAND, "合成器用例只验收原生 X11 画布"
                    compositor = subprocess.Popen(["xcompmgr", "-a"], env=env, stdout=log, stderr=log)
                    wait(lambda: Gdk.selection_owner_get(Gdk.Atom.intern("_NET_WM_CM_S0", False)) is not None)
                wait(lambda: bus.name_has_owner("org.fcitx.Fcitx5"))
                control = dbus.Interface(bus.get_object("org.fcitx.Fcitx5", "/controller"),
                                         "org.fcitx.Fcitx.Controller1")
                window = Gtk.Window(title="MSIME synthetic canvas editor")
                entry = Gtk.Entry()
                entry.set_property("im-module", "fcitx")
                # 平铺合成器会放大窗口；输入框保持自然高度，候选锚点不能被拉到屏幕底部。
                entry.set_valign(Gtk.Align.START)
                if WAYLAND:
                    entry.set_margin_top(80)
                window.add(entry)
                window.show_all()
                entry.grab_focus()
                wait(lambda: window.get_window() is not None)
                if WAYLAND:
                    assert "Wayland" in type(Gdk.Display.get_default()).__name__, "GTK 未使用 Wayland"
                    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests/runtime"))
                    from wayland_keyboard import WaylandKeyboard

                    def pump():
                        while GLib.MainContext.default().iteration(False):
                            pass

                    keyboard = WaylandKeyboard(pump, lambda predicate, _description: wait(predicate))
                    wait(window.is_active)
                else:
                    wait(lambda: window.get_window().is_viewable())
                    Gdk.Display.get_default().sync()
                    xid = GdkX11.X11Window.get_xid(window.get_window())
                    subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)
                    # GTK 模块异步建立 D-Bus 上下文；连接前的首次焦点事件可能丢失。
                    wait(lambda: "frontend:dbus" in str(control.DebugInfo()))
                    root_xid = GdkX11.X11Window.get_xid(Gdk.get_default_root_window())
                    subprocess.run(["xdotool", "windowfocus", "--sync", str(root_xid)], check=True, timeout=5)
                    wait(lambda: not entry.has_focus())
                    subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)

                def focused():
                    if WAYLAND:
                        return window.is_active() and entry.has_focus()
                    return int(subprocess.check_output(["xdotool", "getwindowfocus"])) == xid
                wait(lambda: entry.has_focus())
                next_focus_retry = 0.0

                def ime_focused():
                    nonlocal next_focus_retry
                    if re.search(r"frontend:dbus\b.*\bfocus:1\b", str(control.DebugInfo())):
                        return True
                    # GTK 创建远端上下文是异步的；连接之前发出的首次焦点可能丢失。
                    # 只在握手未完成时重发真实窗口焦点，不重启 daemon 或绕过输入框。
                    if not WAYLAND and time.monotonic() >= next_focus_retry:
                        subprocess.run(["xdotool", "windowfocus", "--sync", str(root_xid)], check=True, timeout=5)
                        wait(lambda: not entry.has_focus())
                        subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)
                        next_focus_retry = time.monotonic() + 0.5
                    return False

                wait(ime_focused)
                focused_line = next(line for line in str(control.DebugInfo()).splitlines()
                                    if re.search(r"frontend:dbus\b.*\bfocus:1\b", line))
                focused_context = re.search(r"IC \[[^\]]+\]", focused_line).group()
                control.SetCurrentIM("msime")
                control.Activate()
                wait(lambda: str(control.CurrentInputMethod()) == "msime")
                if WAYLAND:
                    keyboard.keys(*"nihao")
                else:
                    subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "100", "ni" if NATIVE else "nihao"],
                                   check=True, timeout=5)
                wait(lambda: palette_visible(LIGHT))
                assert entry.get_text() == "", "预编辑文字提前进入编辑器"
                assert focused(), "候选窗口抢走焦点"
                if ANIMATION:
                    assert NATIVE and not WAYLAND
                    accept_animation(skin, entry, control, daemon)
                    return
                if NATIVE:
                    wait(native_window)
                    if COMPOSITED:
                        popup = GdkX11.X11Window.foreign_new_for_display(Gdk.Display.get_default(), int(native_window()))
                        assert popup.get_visual().get_depth() == 32, "合成器存在时没有创建 ARGB 候选窗"
                    wait(lambda: palette_visible((WIDGET, SELECTED)))
                    selected_before = palette_top(SELECTED)
                    widget_before = palette_top(WIDGET)
                    assert widget_before < selected_before, "上方挂件未扩展原生窗口留白"
                    # 修改的只是拼音下方行距；真实窗口中的首行候选也必须相应移动。
                    manifest = skin / "skin.toml"
                    manifest.write_text(manifest.read_text().replace("line_gap_dip = 19", "line_gap_dip = 39"))
                    wait(lambda: palette_top(SELECTED) == selected_before + 20 * NATIVE_SCALE)
                    assert palette_top(WIDGET) == widget_before, "行距改变错误地移动了挂件"
                    manifest.write_text(manifest.read_text().replace("line_gap_dip = 39", "line_gap_dip = 19"))
                    wait(lambda: palette_top(SELECTED) == selected_before)
                    save_image("native-layout")
                    # 同一原生窗口热更新布局，测量、高亮和点击区域必须同步改变。
                    original_manifest = manifest.read_text()
                    original_selected = selected_pixels()
                    layout = preferences["candidate_layout"]
                    styled_manifest = original_manifest + (
                        f"\n[candidate_window.{layout}]\nitem_extra_height = 34\n"
                        "number_scale = 0.45\nnumber_inset = 16\nnumber_gap = 24\n"
                        "item_padding_right = 20\nitem_gap = 12\npreedit_margin_dip = [7, 11, 9, 13]\n")
                    manifest.write_text(styled_manifest)
                    wait(lambda: selected_pixels() is not None and
                         selected_pixels()[1] >= int(preferences['candidate_font_size'] * 1.35 + 34) * NATIVE_SCALE)
                    assert palette_top(SELECTED) == selected_before + 7 * NATIVE_SCALE, "拼音上边距未进入真实候选布局"
                    before_font = selected_pixels()
                    manifest.write_text(styled_manifest.replace("[candidate_window]\n", "[candidate_window]\nfont_family = 'monospace'\n"))
                    wait(lambda: selected_pixels() is not None and selected_pixels() != before_font)
                    save_image("native-skin-layout")
                    assert focused(), "布局或字体热更新抢走编辑器焦点"
                    # 点击新增行高底部，若命中仍用旧矩形就会错选下一项。
                    x, y = color_position(SELECTED)
                    y += selected_pixels()[1] - 4
                    subprocess.run(["xdotool", "mousemove", str(x), str(y), "click", "1"], check=True, timeout=5)
                    wait(lambda: entry.get_text() == "你")
                    wait(lambda: native_window() is None)
                    entry.set_text("")
                    subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "100", "ni"], check=True, timeout=5)
                    wait(lambda: palette_visible((SELECTED,)))
                    manifest.write_text(original_manifest)
                    wait(lambda: selected_pixels() == original_selected)
                    accept_appearance(manifest, preferences, publish, entry)
                    wait(lambda: selected_pixels() == original_selected)
                    accept_pager(manifest, preferences, publish, entry)
                    wait(lambda: selected_pixels() == original_selected)
                    accept_translation(manifest, preferences, publish, entry)
                    wait(lambda: selected_pixels() == original_selected)
                    before_move = window_geometry(native_window())
                    window.move(100, 80)
                    # GTK 3 在下一次光标更新时上报根坐标，候选窗按该坐标重新定位。
                    subprocess.run(["xdotool", "key", "BackSpace", "i"], check=True, timeout=5)
                    wait(lambda: window_geometry(native_window())["X"] != before_move["X"])
                    assert focused(), "移动输入框后候选窗口抢走焦点"

                save_image("canvas-light")
                if WAYLAND:
                    # 窗口留在同一输出；GTK 下一次输入上报新光标矩形后，真实 popup 必须跟随。
                    original_top = palette_top(LIGHT[0])
                    assert original_top is not None
                    entry.set_margin_top(200)
                    wait(lambda: entry.get_allocation().y >= 200)
                    keyboard.keys("BackSpace", "o")
                    wait(lambda: (palette_top(LIGHT[0]) or 0) >= original_top + 100)
                    assert focused(), "移动候选锚点后编辑器失焦"
                    save_image("canvas-moved")
                    entry.set_margin_top(80)
                    wait(lambda: entry.get_allocation().y < 200)
                    keyboard.keys("BackSpace", "o")
                    wait(lambda: palette_top(LIGHT[0]) == original_top)
                preferences["candidate_theme"] = "dark"
                publish()
                # 设置页由偏好轮询热更新；ReloadAddonConfig 表示重启服务，会主动清空组字。
                wait(lambda: palette_visible(DARK) and not palette_visible(LIGHT))
                save_image("canvas-dark")
                # 损坏图片必须让真实窗口回退；修复后不得一直沿用缓存的纯色卡片。
                (skin / panel_file).write_bytes(b"broken")
                wait(lambda: not palette_visible(DARK) and fallback_visible(root / "data/fcitx5/themes/msime/theme.conf"))
                save_image("canvas-broken")
                (skin / panel_file).write_bytes(panel_image(DARK))
                wait(lambda: palette_visible(DARK))
                save_image("canvas-repaired")
                assert focused(), "换肤后编辑器失焦"
                if NATIVE:
                    wait(native_window)
                    # 按下后更新候选，再释放旧鼠标事件，不能误选新页中相同的格位。
                    x, y = color_position(SELECTED)
                    subprocess.run(["xdotool", "mousemove", str(x), str(y), "mousedown", "1"], check=True, timeout=5)
                    old_pixels = selected_pixels()
                    subprocess.run(["xdotool", "key", "BackSpace"], check=True, timeout=5)
                    wait(lambda: selected_pixels() != old_pixels)
                    subprocess.run(["xdotool", "mouseup", "1", "key", "i"], check=True, timeout=5)
                    wait(lambda: selected_pixels() == old_pixels)
                    assert entry.get_text() == "", "旧鼠标释放错误地提交了新代次候选"
                    x, y = color_position(SELECTED)
                    bounds = window_geometry(native_window())
                    menu_offset = x - int(bounds["X"]), y - int(bounds["Y"])
                    click_color(SELECTED, 3)
                    wait(lambda: not palette_visible((SELECTED,)))
                    save_image("native-context-menu")
                    # 实际触发首行“置顶”，确认写入本次合成词库且没有提交文字。
                    bounds = window_geometry(native_window())
                    subprocess.run(["xdotool", "mousemove", str(int(bounds["X"]) + menu_offset[0]),
                                    str(int(bounds["Y"]) + menu_offset[1]), "click", "1"], check=True, timeout=5)

                    def pinned():
                        journal = root / "state/user/msime_user.db"
                        if not journal.exists():
                            return False
                        with sqlite3.connect(journal.as_uri() + "?mode=ro", uri=True) as connection:
                            return connection.execute(
                                "SELECT count(*) FROM pinned_candidates WHERE context_key=? AND value=?", ("ni", "你")
                            ).fetchone()[0] == 1

                    wait(pinned)
                    wait(lambda: palette_visible((SELECTED,)))
                    assert entry.get_text() == "", "置顶操作错误地提交文字"
                    click_color(SELECTED, 3)
                    wait(lambda: not palette_visible((SELECTED,)))
                    # 按键仍进编辑器并由运行时重建候选，不能被候选窗口抢焦点。
                    subprocess.run(["xdotool", "key", "BackSpace", "i"], check=True, timeout=5)
                    wait(lambda: palette_visible((SELECTED,)))
                    assert focused(), "右键菜单抢走编辑器焦点"
                    before_page = selected_pixels()
                    click_color(SELECTED, 5)
                    wait(lambda: selected_pixels() != before_page)
                    wait(lambda: palette_visible((SELECTED,)))
                    save_image("native-next-page")
                    click_color(SELECTED, 4)
                    wait(lambda: selected_pixels() == before_page)
                    # 左键选择使用当前 CandidateWord 身份，必须真实上屏。
                    click_color(SELECTED, 1)
                elif WAYLAND:
                    keyboard.keys("space")
                else:
                    subprocess.run(["xdotool", "key", "space"], check=True, timeout=5)
                wait(lambda: entry.get_text() == ("你" if NATIVE else "你好"))
                wait(lambda: not palette_visible(DARK))
                if NATIVE:
                    wait(lambda: native_window() is None)
                if NATIVE:
                    subprocess.run(["xdotool", "type", "--clearmodifiers", "ni"], check=True, timeout=5)
                    wait(native_window)
                    other = Gtk.Window(title="MSIME synthetic second editor")
                    other.add(Gtk.Entry())
                    other.show_all()
                    wait(lambda: other.get_window() is not None and other.get_window().is_viewable())
                    Gdk.Display.get_default().sync()
                    other_xid = GdkX11.X11Window.get_xid(other.get_window())
                    subprocess.run(["xdotool", "windowfocus", "--sync", str(other_xid)], check=True, timeout=5)
                    wait(lambda: native_window() is None)
                    save_image("native-unfocused")
                    subprocess.run(["xdotool", "windowfocus", "--sync", str(xid)], check=True, timeout=5)
                    wait(focused)
                    wait(lambda: re.search(re.escape(focused_context) + r".*\bfocus:1\b", str(control.DebugInfo())))
                    control.SetCurrentIM("msime")
                    control.Activate()
                    wait(lambda: str(control.CurrentInputMethod()) == "msime")
                    subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "100", "ni"],
                                   check=True, timeout=5)
                    wait(native_window)
                    control.SetCurrentIM("keyboard-us")
                    wait(lambda: native_window() is None)
                    other.destroy()
                control.Exit()
                assert daemon.wait(timeout=10) == 0, "Fcitx5 未正常退出"
                backend = "Wayland 客户端面板" if WAYLAND else "X11 classicui"
                print(f"真实 GTK／{backend} {FORMAT} canvas 像素、明暗、坏图回退、修复、焦点与选词通过")
            finally:
                if sys.exc_info()[0] is not None:
                    save_image("canvas-failed")
                    if control is not None:
                        print(control.DebugInfo(), file=sys.stderr)
                        print("失败时输入法", control.CurrentInputMethod(), "状态", control.State(),
                              "编辑器", repr(entry.get_text()), file=sys.stderr)
                    for status in (root / "run").rglob("input-status.json"):
                        print(status.read_text(), file=sys.stderr)
                    theme = root / "data/fcitx5/themes/msime/theme.conf"
                    if theme.exists():
                        print(theme.read_text(), file=sys.stderr)
                    classicui = root / "config/fcitx5/conf/classicui.conf"
                    if classicui.exists():
                        print(classicui.read_text(), file=sys.stderr)
                if WAYLAND and keyboard is not None:
                    keyboard.close()
                if window is not None:
                    window.destroy()
                if daemon.poll() is None:
                    daemon.terminate()
                    daemon.wait(timeout=10)
                if compositor is not None:
                    compositor.terminate()
                    compositor.wait(timeout=10)
                if sys.exc_info()[0] is not None:
                    # 此日志仅含隔离的合成状态，不读取用户输入或用户配置。
                    print((root / "daemon.log").read_text(), file=sys.stderr)


if __name__ == "__main__":
    main()
